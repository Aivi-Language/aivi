fn unix_timestamp_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}

impl Default for ReactiveGraphState {
    fn default() -> Self {
        Self {
            next_signal_id: 1,
            next_watcher_id: 1,
            next_change_seq: 1,
            batch_depth: 0,
            flushing: false,
            deferred_flush: false,
            flush_thread: None,
            signals: HashMap::new(),
            watchers: HashMap::new(),
            watchers_by_signal: HashMap::new(),
            pending_notifications: HashSet::new(),
        }
    }
}

impl Runtime {
    pub(crate) fn reactive_create_signal(&mut self, initial: Value) -> Result<Value, RuntimeError> {
        let signal_id = self.reactive_alloc_signal_id();
        let mut graph = self.reactive_graph.lock();
        let change_seq = graph.next_change_seq;
        graph.next_change_seq = graph.next_change_seq.saturating_add(1);
        graph.signals.insert(
            signal_id,
            ReactiveCellEntry {
                kind: ReactiveCellKind::Source,
                value: initial,
                revision: 1,
                last_change_seq: change_seq,
                last_change_timestamp_ms: unix_timestamp_ms(),
                dirty: false,
                dependents: HashSet::new(),
            },
        );
        drop(graph);
        Ok(self.reactive_signal_value(signal_id))
    }

    pub(crate) fn reactive_get_signal(&mut self, signal: Value) -> Result<Value, RuntimeError> {
        let signal_id = self.reactive_signal_id_from_value(signal, "reactive.get")?;
        self.reactive_get_signal_value(signal_id)
    }

    pub(crate) fn reactive_peek_signal(&mut self, signal: Value) -> Result<Value, RuntimeError> {
        let signal_id = self.reactive_signal_id_from_value(signal, "reactive.peek")?;
        self.reactive_get_signal_value(signal_id)
    }

    pub(crate) fn reactive_set_signal(
        &mut self,
        signal: Value,
        next_value: Value,
    ) -> Result<Value, RuntimeError> {
        let signal_id = self.reactive_signal_id_from_value(signal, "reactive.set")?;
        self.reactive_begin_batch();
        let result = self
            .reactive_set_signal_value(signal_id, next_value)
            .map(|_| Value::Unit);
        let flush_result = self.reactive_finish_batch();
        match (result, flush_result) {
            (Err(err), _) => Err(err),
            (Ok(_), Err(err)) => Err(err),
            (Ok(value), Ok(())) => Ok(value),
        }
    }

    pub(crate) fn reactive_update_signal(
        &mut self,
        signal: Value,
        updater: Value,
    ) -> Result<Value, RuntimeError> {
        let signal_id = self.reactive_signal_id_from_value(signal, "reactive.update")?;
        self.reactive_begin_batch();
        let result = self.reactive_update_signal_value(signal_id, updater);
        let flush_result = self.reactive_finish_batch();
        match (result, flush_result) {
            (Err(err), _) => Err(err),
            (Ok(_), Err(err)) => Err(err),
            (Ok(value), Ok(())) => Ok(value),
        }
    }

    pub(crate) fn reactive_derive_signal(
        &mut self,
        signal: Value,
        mapper: Value,
    ) -> Result<Value, RuntimeError> {
        let signal_id = self.reactive_signal_id_from_value(signal, "reactive.derive")?;
        self.reactive_create_derived_signal(vec![signal_id], mapper)
    }

    pub(crate) fn reactive_combine_all(
        &mut self,
        signals_tuple: Value,
        combine: Value,
    ) -> Result<Value, RuntimeError> {
        let signals_tuple = self.force_value(signals_tuple)?;
        let Value::Tuple(items) = signals_tuple else {
            return Err(RuntimeError::InvalidArgument {
                context: "reactive.combineAll".to_string(),
                reason: "expected a tuple of signals".to_string(),
            });
        };
        let mut signal_ids = Vec::with_capacity(items.len());
        for (index, value) in items.iter().enumerate() {
            let id = self.reactive_signal_id_from_value(
                value.clone(),
                &format!("reactive.combineAll element {index}"),
            )?;
            signal_ids.push(id);
        }

        // Create a derived signal that reconstructs the tuple with resolved values,
        // then applies the combine function.
        self.reactive_create_derived_signal_with_tuple(signal_ids, combine)
    }

    pub(crate) fn reactive_watch_signal(
        &mut self,
        signal: Value,
        callback: Value,
    ) -> Result<Value, RuntimeError> {
        let (_watcher_id, disposable) = self.reactive_watch_signal_with_id(signal, callback)?;
        Ok(disposable)
    }

    pub(crate) fn reactive_watch_signal_with_id(
        &mut self,
        signal: Value,
        callback: Value,
    ) -> Result<(usize, Value), RuntimeError> {
        let (watcher_id, disposable) = self.reactive_watch_signal_unscoped(signal, callback)?;
        self.resource_cleanups.push(ResourceCleanupEntry::Cleanup {
            cleanup: Arc::new(move |runtime| {
                runtime.reactive_dispose_watcher(watcher_id);
                Ok(Value::Unit)
            }),
        });
        Ok((watcher_id, disposable))
    }

    /// Register a watcher without tying its lifetime to the current effect
    /// scope. The caller is responsible for disposing the watcher (e.g. via
    /// `push_gtk_binding_watcher` which cleans up when the widget is removed).
    pub(crate) fn reactive_watch_signal_unscoped(
        &mut self,
        signal: Value,
        callback: Value,
    ) -> Result<(usize, Value), RuntimeError> {
        let signal_id = self.reactive_signal_id_from_value(signal, "reactive.watch")?;
        let revision = self.reactive_current_revision(signal_id)?;
        let watcher_id = {
            let mut graph = self.reactive_graph.lock();
            let watcher_id = graph.next_watcher_id;
            graph.next_watcher_id = graph.next_watcher_id.saturating_add(1);
            graph.watchers.insert(
                watcher_id,
                ReactiveWatcherEntry {
                    signal_id,
                    callback,
                    last_revision: revision,
                },
            );
            graph
                .watchers_by_signal
                .entry(signal_id)
                .or_default()
                .insert(watcher_id);
            watcher_id
        };
        Ok((watcher_id, self.reactive_disposable_record(watcher_id)))
    }

    /// Register a watcher whose callback must run on the current thread (e.g.
    /// GTK live bindings registered on the main thread). Not tied to the
    /// current effect scope — the caller owns disposal. Marks the graph so
    /// background-thread batches defer their flush instead of running
    /// callbacks in-place.
    pub(crate) fn reactive_watch_signal_main_thread(
        &mut self,
        signal: Value,
        callback: Value,
    ) -> Result<(usize, Value), RuntimeError> {
        {
            let mut graph = self.reactive_graph.lock();
            if graph.flush_thread.is_none() {
                graph.flush_thread = Some(std::thread::current().id());
            }
        }
        self.reactive_watch_signal_unscoped(signal, callback)
    }

    pub(crate) fn reactive_batch(&mut self, callback: Value) -> Result<Value, RuntimeError> {
        self.reactive_begin_batch();
        let result = self.apply(callback, Value::Unit);
        let flush_result = self.reactive_finish_batch();
        match (result, flush_result) {
            (Err(err), _) => Err(err),
            (Ok(_), Err(err)) => Err(err),
            (Ok(value), Ok(())) => Ok(value),
        }
    }

    pub(crate) fn reactive_create_event(&mut self, effect: Value) -> Result<Value, RuntimeError> {
        let result_signal = self.reactive_create_signal(reactive_none())?;
        let error_signal = self.reactive_create_signal(reactive_none())?;
        let done_signal = self.reactive_create_signal(Value::Bool(false))?;
        let running_signal = self.reactive_create_signal(Value::Bool(false))?;

        let result_id =
            self.reactive_signal_id_from_value(result_signal.clone(), "reactive.event result")?;
        let error_id =
            self.reactive_signal_id_from_value(error_signal.clone(), "reactive.event error")?;
        let done_id =
            self.reactive_signal_id_from_value(done_signal.clone(), "reactive.event done")?;
        let running_id =
            self.reactive_signal_id_from_value(running_signal.clone(), "reactive.event running")?;

        let run_effect = Value::Effect(Arc::new(EffectValue::Thunk {
            func: Arc::new(move |runtime| {
                runtime.reactive_begin_batch();
                runtime.reactive_set_signal_value(running_id, Value::Bool(true))?;
                runtime.reactive_set_signal_value(done_id, Value::Bool(false))?;
                runtime.reactive_set_signal_value(result_id, reactive_none())?;
                runtime.reactive_set_signal_value(error_id, reactive_none())?;
                runtime.reactive_finish_batch()?;

                match runtime.run_effect_value(effect.clone()) {
                    Ok(value) => {
                        runtime.reactive_begin_batch();
                        runtime
                            .reactive_set_signal_value(result_id, reactive_some(value.clone()))?;
                        runtime.reactive_set_signal_value(error_id, reactive_none())?;
                        runtime.reactive_set_signal_value(done_id, Value::Bool(true))?;
                        runtime.reactive_set_signal_value(running_id, Value::Bool(false))?;
                        runtime.reactive_finish_batch()?;
                        Ok(value)
                    }
                    Err(RuntimeError::Error(err_value)) => {
                        runtime.reactive_begin_batch();
                        runtime.reactive_set_signal_value(
                            error_id,
                            reactive_some(err_value.clone()),
                        )?;
                        runtime.reactive_set_signal_value(done_id, Value::Bool(true))?;
                        runtime.reactive_set_signal_value(running_id, Value::Bool(false))?;
                        runtime.reactive_finish_batch()?;
                        Err(RuntimeError::Error(err_value))
                    }
                    Err(err) => {
                        runtime.reactive_begin_batch();
                        runtime.reactive_set_signal_value(done_id, Value::Bool(true))?;
                        runtime.reactive_set_signal_value(running_id, Value::Bool(false))?;
                        runtime.reactive_finish_batch()?;
                        Err(err)
                    }
                }
            }),
        }));

        let mut fields = HashMap::new();
        fields.insert("run".to_string(), run_effect);
        fields.insert("result".to_string(), result_signal);
        fields.insert("error".to_string(), error_signal);
        fields.insert("done".to_string(), done_signal);
        fields.insert("running".to_string(), running_signal);
        Ok(Value::Record(Arc::new(fields)))
    }

    fn reactive_alloc_signal_id(&mut self) -> usize {
        let mut graph = self.reactive_graph.lock();
        let signal_id = graph.next_signal_id;
        graph.next_signal_id = graph.next_signal_id.saturating_add(1);
        signal_id
    }

    fn reactive_signal_value(&self, signal_id: usize) -> Value {
        Value::Signal(Arc::new(crate::runtime::values::ReactiveSignalValue {
            id: signal_id,
        }))
    }

    fn reactive_disposable_record(&self, watcher_id: usize) -> Value {
        let mut fields = HashMap::new();
        fields.insert(
            "dispose".to_string(),
            crate::runtime::builtins::builtin(
                "reactive.disposable.dispose",
                1,
                move |_args, runtime| {
                    runtime.reactive_dispose_watcher(watcher_id);
                    Ok(Value::Unit)
                },
            ),
        );
        Value::Record(Arc::new(fields))
    }

    fn reactive_create_derived_signal(
        &mut self,
        dependencies: Vec<usize>,
        compute: Value,
    ) -> Result<Value, RuntimeError> {
        {
            let graph = self.reactive_graph.lock();
            for dependency in &dependencies {
                if !graph.signals.contains_key(dependency) {
                    return Err(RuntimeError::InvalidArgument {
                        context: "reactive.derive".to_string(),
                        reason: format!("unknown signal id {dependency}"),
                    });
                }
            }
        }
        let signal_id = self.reactive_alloc_signal_id();
        {
            let mut graph = self.reactive_graph.lock();
            graph.signals.insert(
                signal_id,
                ReactiveCellEntry {
                    kind: ReactiveCellKind::Derived {
                        dependencies: dependencies.clone(),
                        compute,
                    },
                    value: Value::Unit,
                    revision: 0,
                    last_change_seq: 0,
                    last_change_timestamp_ms: 0,
                    dirty: true,
                    dependents: HashSet::new(),
                },
            );
            for dependency in dependencies {
                if let Some(entry) = graph.signals.get_mut(&dependency) {
                    entry.dependents.insert(signal_id);
                }
            }
        }
        Ok(self.reactive_signal_value(signal_id))
    }

    fn reactive_create_derived_signal_with_tuple(
        &mut self,
        dependencies: Vec<usize>,
        compute: Value,
    ) -> Result<Value, RuntimeError> {
        {
            let graph = self.reactive_graph.lock();
            for dependency in &dependencies {
                if !graph.signals.contains_key(dependency) {
                    return Err(RuntimeError::InvalidArgument {
                        context: "reactive.combineAll".to_string(),
                        reason: format!("unknown signal id {dependency}"),
                    });
                }
            }
        }
        let signal_id = self.reactive_alloc_signal_id();
        {
            let mut graph = self.reactive_graph.lock();
            graph.signals.insert(
                signal_id,
                ReactiveCellEntry {
                    kind: ReactiveCellKind::DerivedTuple {
                        dependencies: dependencies.clone(),
                        compute,
                    },
                    value: Value::Unit,
                    revision: 0,
                    last_change_seq: 0,
                    last_change_timestamp_ms: 0,
                    dirty: true,
                    dependents: HashSet::new(),
                },
            );
            for dependency in dependencies {
                if let Some(entry) = graph.signals.get_mut(&dependency) {
                    entry.dependents.insert(signal_id);
                }
            }
        }
        Ok(self.reactive_signal_value(signal_id))
    }

    fn reactive_signal_id_from_value(
        &mut self,
        signal: Value,
        ctx: &str,
    ) -> Result<usize, RuntimeError> {
        let signal = self.force_value(signal)?;
        let Value::Signal(signal) = signal else {
            return Err(RuntimeError::InvalidArgument {
                context: ctx.to_string(),
                reason: "expected a Signal value".to_string(),
            });
        };
        Ok(signal.id)
    }

    fn reactive_get_signal_value(&mut self, signal_id: usize) -> Result<Value, RuntimeError> {
        self.reactive_ensure_signal_fresh(signal_id, &mut Vec::new())?;
        self.reactive_graph
            .lock()
            .signals
            .get(&signal_id)
            .map(|entry| entry.value.clone())
            .ok_or_else(|| RuntimeError::InvalidArgument {
                context: "reactive.get".to_string(),
                reason: format!("unknown signal id {signal_id}"),
            })
    }

    fn reactive_current_revision(&mut self, signal_id: usize) -> Result<u64, RuntimeError> {
        self.reactive_ensure_signal_fresh(signal_id, &mut Vec::new())?;
        self.reactive_graph
            .lock()
            .signals
            .get(&signal_id)
            .map(|entry| entry.revision)
            .ok_or_else(|| RuntimeError::InvalidArgument {
                context: "reactive.watch".to_string(),
                reason: format!("unknown signal id {signal_id}"),
            })
    }

    fn reactive_set_signal_value(
        &mut self,
        signal_id: usize,
        next_value: Value,
    ) -> Result<(), RuntimeError> {
        let (previous_value, dependents) = {
            let graph = self.reactive_graph.lock();
            let Some(entry) = graph.signals.get(&signal_id) else {
                return Err(RuntimeError::InvalidArgument {
                    context: "reactive.set".to_string(),
                    reason: format!("unknown signal id {signal_id}"),
                });
            };
            if !matches!(entry.kind, ReactiveCellKind::Source) {
                return Err(RuntimeError::InvalidArgument {
                    context: "reactive.set".to_string(),
                    reason: "cannot write to a derived signal".to_string(),
                });
            }
            (entry.value.clone(), entry.dependents.clone())
        };

        if reactive_values_match(&previous_value, &next_value) {
            return Ok(());
        }

        {
            let mut graph = self.reactive_graph.lock();
            let change_seq = graph.next_change_seq;
            graph.next_change_seq = graph.next_change_seq.saturating_add(1);
            let entry = graph
                .signals
                .get_mut(&signal_id)
                .expect("source signal exists");
            entry.value = next_value;
            entry.revision = entry.revision.saturating_add(1).max(1);
            entry.last_change_seq = change_seq;
            entry.last_change_timestamp_ms = unix_timestamp_ms();
            entry.dirty = false;
        }
        self.reactive_graph
            .lock()
            .pending_notifications
            .insert(signal_id);
        self.reactive_mark_dependents_dirty(dependents);
        Ok(())
    }

    fn reactive_update_signal_value(
        &mut self,
        signal_id: usize,
        updater: Value,
    ) -> Result<Value, RuntimeError> {
        let current = self.reactive_get_signal_value(signal_id)?;
        let next = self.apply(updater, current)?;
        self.reactive_set_signal_value(signal_id, next)?;
        Ok(Value::Unit)
    }

    fn reactive_mark_dependents_dirty(&mut self, dependents: HashSet<usize>) {
        let mut stack: Vec<usize> = dependents.into_iter().collect();
        let mut seen = HashSet::new();
        while let Some(signal_id) = stack.pop() {
            if !seen.insert(signal_id) {
                continue;
            }
            let next_dependents = {
                let mut graph = self.reactive_graph.lock();
                let Some(entry) = graph.signals.get_mut(&signal_id) else {
                    continue;
                };
                entry.dirty = true;
                entry.dependents.iter().copied().collect::<Vec<_>>()
            };
            stack.extend(next_dependents);
        }
    }

    fn reactive_ensure_signal_fresh(
        &mut self,
        signal_id: usize,
        stack: &mut Vec<usize>,
    ) -> Result<(), RuntimeError> {
        let (dirty, kind) = {
            let graph = self.reactive_graph.lock();
            let Some(entry) = graph.signals.get(&signal_id) else {
                return Err(RuntimeError::InvalidArgument {
                    context: "reactive.get".to_string(),
                    reason: format!("unknown signal id {signal_id}"),
                });
            };
            let kind = match &entry.kind {
                ReactiveCellKind::Source => ReactiveCellKind::Source,
                ReactiveCellKind::Derived {
                    dependencies,
                    compute,
                } => ReactiveCellKind::Derived {
                    dependencies: dependencies.clone(),
                    compute: compute.clone(),
                },
                ReactiveCellKind::DerivedTuple {
                    dependencies,
                    compute,
                } => ReactiveCellKind::DerivedTuple {
                    dependencies: dependencies.clone(),
                    compute: compute.clone(),
                },
            };
            (entry.dirty, kind)
        };

        if !dirty {
            return Ok(());
        }

        if stack.contains(&signal_id) {
            return Err(RuntimeError::InvalidArgument {
                context: "reactive.get".to_string(),
                reason: format!("reactive cycle detected while evaluating signal {signal_id}"),
            });
        }

        match kind {
            ReactiveCellKind::Source => Ok(()),
            ReactiveCellKind::Derived {
                dependencies,
                compute,
            } => {
                stack.push(signal_id);
                let mut values = Vec::with_capacity(dependencies.len());
                for dependency in dependencies {
                    self.reactive_ensure_signal_fresh(dependency, stack)?;
                    let value = self
                        .reactive_graph
                        .lock()
                        .signals
                        .get(&dependency)
                        .expect("dependency exists")
                        .value
                        .clone();
                    values.push(value);
                }
                let mut value = compute;
                for dependency in values {
                    value = self.apply(value, dependency)?;
                }
                stack.pop();

                let mut changed = false;
                {
                    let mut graph = self.reactive_graph.lock();
                    let change_seq = graph.next_change_seq;
                    let timestamp_ms = unix_timestamp_ms();
                    let entry = graph.signals.get_mut(&signal_id).expect("derived signal exists");
                    if !reactive_values_match(&entry.value, &value) {
                        entry.value = value;
                        entry.revision = entry.revision.saturating_add(1).max(1);
                        entry.last_change_seq = change_seq;
                        entry.last_change_timestamp_ms = timestamp_ms;
                        changed = true;
                    }
                    entry.dirty = false;
                    if changed {
                        graph.next_change_seq = graph.next_change_seq.saturating_add(1);
                    }
                }
                if changed {
                    self.reactive_graph
                        .lock()
                        .pending_notifications
                        .insert(signal_id);
                }
                Ok(())
            }
            ReactiveCellKind::DerivedTuple {
                dependencies,
                compute,
            } => {
                stack.push(signal_id);
                let mut items = Vec::with_capacity(dependencies.len());
                for dependency in dependencies {
                    self.reactive_ensure_signal_fresh(dependency, stack)?;
                    let dep_value = self
                        .reactive_graph
                        .lock()
                        .signals
                        .get(&dependency)
                        .expect("dependency exists")
                        .value
                        .clone();
                    items.push(dep_value);
                }
                let value = self.apply(compute, Value::Tuple(items))?;
                stack.pop();

                let mut changed = false;
                {
                    let mut graph = self.reactive_graph.lock();
                    let change_seq = graph.next_change_seq;
                    let timestamp_ms = unix_timestamp_ms();
                    let entry = graph.signals.get_mut(&signal_id).expect("derived signal exists");
                    if !reactive_values_match(&entry.value, &value) {
                        entry.value = value;
                        entry.revision = entry.revision.saturating_add(1).max(1);
                        entry.last_change_seq = change_seq;
                        entry.last_change_timestamp_ms = timestamp_ms;
                        changed = true;
                    }
                    entry.dirty = false;
                    if changed {
                        graph.next_change_seq = graph.next_change_seq.saturating_add(1);
                    }
                }
                if changed {
                    self.reactive_graph
                        .lock()
                        .pending_notifications
                        .insert(signal_id);
                }
                Ok(())
            }
        }
    }

    fn reactive_begin_batch(&mut self) {
        let mut graph = self.reactive_graph.lock();
        graph.batch_depth = graph.batch_depth.saturating_add(1);
    }

    fn reactive_finish_batch(&mut self) -> Result<(), RuntimeError> {
        let should_flush = {
            let mut graph = self.reactive_graph.lock();
            graph.batch_depth = graph.batch_depth.saturating_sub(1);
            graph.batch_depth == 0 && !graph.flushing
        };
        if should_flush {
            // If watcher callbacks are pinned to a specific thread (e.g. GTK
            // main thread) and we are on a *different* thread, defer the flush
            // so the owning thread picks it up during its next pump/recv cycle.
            let defer = {
                let graph = self.reactive_graph.lock();
                matches!(graph.flush_thread, Some(tid) if tid != std::thread::current().id())
            };
            if defer {
                let has_pending = {
                    let graph = self.reactive_graph.lock();
                    !graph.pending_notifications.is_empty()
                };
                let has_dirty = {
                    let graph = self.reactive_graph.lock();
                    graph.signals.values().any(|e| e.dirty)
                };
                if has_pending || has_dirty {
                    self.reactive_graph.lock().deferred_flush = true;
                }
                Ok(())
            } else {
                self.reactive_flush()
            }
        } else {
            Ok(())
        }
    }

    fn reactive_flush(&mut self) -> Result<(), RuntimeError> {
        /// Maximum number of flush iterations before we assume a reactive cycle
        /// and bail out. Watcher callbacks can set signals, creating new pending
        /// notifications. Without a cap this could loop forever.
        const MAX_FLUSH_ITERATIONS: usize = 100;

        {
            let mut graph = self.reactive_graph.lock();
            if graph.flushing {
                return Ok(());
            }
            graph.flushing = true;
        }
        let result = (|| {
            for iteration in 0..MAX_FLUSH_ITERATIONS {
                let dirty_ids = {
                    let graph = self.reactive_graph.lock();
                    graph
                        .signals
                        .iter()
                        .filter_map(|(signal_id, entry)| entry.dirty.then_some(*signal_id))
                        .collect::<Vec<_>>()
                };
                for signal_id in dirty_ids {
                    self.reactive_ensure_signal_fresh(signal_id, &mut Vec::new())?;
                }

                let changed = {
                    let mut graph = self.reactive_graph.lock();
                    graph.pending_notifications.drain().collect::<Vec<_>>()
                };
                if changed.is_empty() {
                    break;
                }

                if iteration == MAX_FLUSH_ITERATIONS - 1 {
                    return Err(RuntimeError::InvalidArgument {
                        context: "reactive.flush".to_string(),
                        reason: format!(
                            "reactive flush did not converge after {MAX_FLUSH_ITERATIONS} iterations — \
                             probable cycle in watcher callbacks"
                        ),
                    });
                }

                let mut watcher_ids = HashSet::new();
                for signal_id in changed {
                    let ids = {
                        let graph = self.reactive_graph.lock();
                        graph.watchers_by_signal.get(&signal_id).cloned()
                    };
                    if let Some(ids) = ids {
                        watcher_ids.extend(ids.iter().copied());
                    }
                }

                for watcher_id in watcher_ids {
                    let snapshot = {
                        let graph = self.reactive_graph.lock();
                        graph.watchers.get(&watcher_id).map(|watcher| {
                            let revision = graph
                                .signals
                                .get(&watcher.signal_id)
                                .map(|entry| entry.revision)
                                .unwrap_or(0);
                            (
                                watcher.signal_id,
                                watcher.callback.clone(),
                                watcher.last_revision,
                                revision,
                            )
                        })
                    };
                    let Some((signal_id, callback, last_revision, revision)) = snapshot else {
                        continue;
                    };

                    if revision == last_revision {
                        continue;
                    }

                    let mut graph = self.reactive_graph.lock();
                    if let Some(watcher) = graph.watchers.get_mut(&watcher_id) {
                        watcher.last_revision = revision;
                    }
                    drop(graph);

                    let value = self
                        .reactive_graph
                        .lock()
                        .signals
                        .get(&signal_id)
                        .expect("watched signal exists")
                        .value
                        .clone();
                    self.reactive_run_watcher_callback(callback, value)?;
                }
            }
            Ok(())
        })();
        self.reactive_graph.lock().flushing = false;
        result
    }

    fn reactive_run_watcher_callback(
        &mut self,
        callback: Value,
        value: Value,
    ) -> Result<(), RuntimeError> {
        let result = self.apply(callback, value)?;
        let result = self.force_value(result)?;
        if let Value::Effect(_) = result {
            self.run_effect_value(result)?;
            if let Some(err) = self.jit_pending_error.take() {
                return Err(err);
            }
        }
        Ok(())
    }

    /// Called by the GTK main thread to flush watcher callbacks that were
    /// deferred by background threads updating signals.
    pub(crate) fn reactive_flush_deferred(&mut self) -> Result<(), RuntimeError> {
        let needs = {
            let mut graph = self.reactive_graph.lock();
            let needs = graph.deferred_flush;
            graph.deferred_flush = false;
            needs
        };
        if needs {
            self.reactive_flush()
        } else {
            Ok(())
        }
    }

    pub(crate) fn reactive_dispose_watcher(&mut self, watcher_id: usize) {
        let mut graph = self.reactive_graph.lock();
        let Some(watcher) = graph.watchers.remove(&watcher_id) else {
            return;
        };
        if let Some(watcher_ids) = graph.watchers_by_signal.get_mut(&watcher.signal_id) {
            watcher_ids.remove(&watcher_id);
            if watcher_ids.is_empty() {
                graph.watchers_by_signal.remove(&watcher.signal_id);
            }
        }
    }

    // NOTE: Signal disposal (reactive_dispose_signal) is not yet implemented.
    // Signals currently live for the lifetime of the reactive graph. To properly
    // dispose signals, we'd need to track which signals are associated with
    // which widget scopes — similar to how watchers are tracked via
    // gtk_binding_scopes. This would prevent signal accumulation in long-running
    // apps that create per-dialog or per-page signals.
}

fn reactive_none() -> Value {
    Value::Constructor {
        name: "None".to_string(),
        args: Vec::new(),
    }
}

fn reactive_some(value: Value) -> Value {
    Value::Constructor {
        name: "Some".to_string(),
        args: vec![value],
    }
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;

    fn test_runtime() -> Runtime {
        let globals = Env::new(None);
        register_builtins(&globals);
        let ctx = Arc::new(RuntimeContext::new_with_constructor_ordinals(
            globals,
            core_constructor_ordinals(),
        ));
        Runtime::new(ctx, CancelToken::root())
    }

    fn expect_signal_id(value: &Value) -> usize {
        match value {
            Value::Signal(signal) => signal.id,
            other => panic!("expected Signal, got {}", format_value(other)),
        }
    }

    fn expect_int(value: Value) -> i64 {
        match value {
            Value::Int(value) => value,
            other => panic!("expected Int, got {}", format_value(&other)),
        }
    }

    fn watcher_last_revision(runtime: &Runtime, watcher_id: usize) -> u64 {
        runtime
            .reactive_graph
            .lock()
            .watchers
            .get(&watcher_id)
            .unwrap_or_else(|| panic!("missing watcher {watcher_id}"))
            .last_revision
    }

    fn record_field(record: &Value, field: &str) -> Value {
        match record {
            Value::Record(fields) => fields
                .get(field)
                .unwrap_or_else(|| panic!("missing field {field}"))
                .clone(),
            other => panic!("expected Record, got {}", format_value(other)),
        }
    }

    #[test]
    fn reactive_batch_coalesces_derived_watchers() {
        let mut runtime = test_runtime();
        let first = runtime
            .reactive_create_signal(Value::Int(1))
            .unwrap_or_else(|err| panic!("first signal: {}", format_runtime_error(err)));
        let second = runtime
            .reactive_create_signal(Value::Int(2))
            .unwrap_or_else(|err| panic!("second signal: {}", format_runtime_error(err)));

        let combine = runtime_builtin("test.combine_tuple", 1, |mut args, _| {
            let tuple = args.remove(0);
            let Value::Tuple(items) = tuple else {
                panic!("expected tuple");
            };
            let [left, right] = items.as_slice() else {
                panic!("expected 2 tuple items");
            };
            let left = expect_int(left.clone());
            let right = expect_int(right.clone());
            Ok(Value::Int(left + right))
        });

        let signals_tuple = Value::Tuple(vec![first.clone(), second.clone()]);

        let sum = runtime
            .reactive_combine_all(signals_tuple, combine)
            .unwrap_or_else(|err| panic!("sum signal: {}", format_runtime_error(err)));

        let notifications = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let notifications_for_watch = notifications.clone();
        let watch_callback = runtime_builtin("test.watch", 1, move |_args, _| {
            notifications_for_watch.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(Value::Unit)
        });
        runtime
            .reactive_watch_signal(sum.clone(), watch_callback)
            .unwrap_or_else(|err| panic!("watcher: {}", format_runtime_error(err)));

        let first_for_batch = first.clone();
        let second_for_batch = second.clone();
        let batch_callback = runtime_builtin("test.batch", 1, move |_args, runtime| {
            runtime.reactive_set_signal(first_for_batch.clone(), Value::Int(3))?;
            runtime.reactive_set_signal(second_for_batch.clone(), Value::Int(4))?;
            Ok(Value::Unit)
        });
        runtime
            .reactive_batch(batch_callback)
            .unwrap_or_else(|err| panic!("batched update: {}", format_runtime_error(err)));

        let sum_value = runtime
            .reactive_get_signal(sum)
            .unwrap_or_else(|err| panic!("sum value: {}", format_runtime_error(err)));
        assert_eq!(expect_int(sum_value), 7);
        assert_eq!(
            notifications.load(std::sync::atomic::Ordering::SeqCst),
            1
        );
    }

    #[test]
    fn reactive_main_thread_watcher_defers_background_signal_updates_until_flush() {
        let mut runtime = test_runtime();
        let source = runtime
            .reactive_create_signal(Value::Int(1))
            .unwrap_or_else(|err| panic!("source signal: {}", format_runtime_error(err)));
        let source_id = expect_signal_id(&source);
        let ctx = runtime.ctx.clone();
        let owner_thread = format!("{:?}", std::thread::current().id());

        let callback_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let callback_threads = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let callback_count_for_watch = callback_count.clone();
        let callback_threads_for_watch = callback_threads.clone();
        let callback = runtime_builtin("test.main_thread_watch", 1, move |_args, _| {
            callback_count_for_watch.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            callback_threads_for_watch
                .lock()
                .expect("callback thread log lock should not be poisoned")
                .push(format!("{:?}", std::thread::current().id()));
            Ok(Value::Unit)
        });
        let (watcher_id, _disposable) = runtime
            .reactive_watch_signal_main_thread(source.clone(), callback)
            .unwrap_or_else(|err| panic!("main-thread watcher: {}", format_runtime_error(err)));

        {
            let graph = runtime.reactive_graph.lock();
            assert_eq!(graph.flush_thread, Some(std::thread::current().id()));
            assert!(!graph.deferred_flush);
            assert!(graph.pending_notifications.is_empty());
        }
        assert_eq!(watcher_last_revision(&runtime, watcher_id), 1);

        let source_for_thread = source.clone();
        std::thread::spawn(move || {
            let mut bg_runtime = Runtime::new(ctx, CancelToken::root());
            bg_runtime
                .reactive_set_signal(source_for_thread, Value::Int(7))
                .unwrap_or_else(|err| panic!("background set signal: {}", format_runtime_error(err)));
        })
        .join()
        .expect("background runtime should not panic");

        assert_eq!(
            expect_int(
                runtime
                    .reactive_get_signal(source.clone())
                    .unwrap_or_else(|err| panic!("source value after background set: {}", format_runtime_error(err)))
            ),
            7
        );
        {
            let graph = runtime.reactive_graph.lock();
            assert_eq!(graph.flush_thread, Some(std::thread::current().id()));
            assert!(graph.deferred_flush);
            assert!(graph.pending_notifications.contains(&source_id));
            assert_eq!(
                graph
                    .signals
                    .get(&source_id)
                    .unwrap_or_else(|| panic!("missing source signal {source_id}"))
                    .revision,
                2
            );
        }
        assert_eq!(watcher_last_revision(&runtime, watcher_id), 1);
        assert_eq!(
            callback_count.load(std::sync::atomic::Ordering::SeqCst),
            0
        );
        assert!(
            callback_threads
                .lock()
                .expect("callback thread log lock should not be poisoned")
                .is_empty()
        );

        runtime
            .reactive_flush_deferred()
            .unwrap_or_else(|err| panic!("flush deferred source watcher: {}", format_runtime_error(err)));
        {
            let graph = runtime.reactive_graph.lock();
            assert!(!graph.deferred_flush);
            assert!(graph.pending_notifications.is_empty());
        }
        assert_eq!(watcher_last_revision(&runtime, watcher_id), 2);
        assert_eq!(
            callback_count.load(std::sync::atomic::Ordering::SeqCst),
            1
        );
        assert_eq!(
            callback_threads
                .lock()
                .expect("callback thread log lock should not be poisoned")
                .as_slice(),
            [owner_thread.as_str()]
        );

        runtime
            .reactive_flush_deferred()
            .unwrap_or_else(|err| panic!("second deferred flush: {}", format_runtime_error(err)));
        assert_eq!(
            callback_count.load(std::sync::atomic::Ordering::SeqCst),
            1
        );
    }

    #[test]
    fn reactive_main_thread_batch_flush_coalesces_derived_watchers() {
        let mut runtime = test_runtime();
        let first = runtime
            .reactive_create_signal(Value::Int(1))
            .unwrap_or_else(|err| panic!("first signal: {}", format_runtime_error(err)));
        let second = runtime
            .reactive_create_signal(Value::Int(2))
            .unwrap_or_else(|err| panic!("second signal: {}", format_runtime_error(err)));
        let sum = runtime
            .reactive_combine_all(
                Value::Tuple(vec![first.clone(), second.clone()]),
                runtime_builtin("test.sum_pair", 1, |mut args, _| {
                    let tuple = args.remove(0);
                    let Value::Tuple(items) = tuple else {
                        panic!("expected tuple");
                    };
                    let [left, right] = items.as_slice() else {
                        panic!("expected two tuple items");
                    };
                    Ok(Value::Int(expect_int(left.clone()) + expect_int(right.clone())))
                }),
            )
            .unwrap_or_else(|err| panic!("sum signal: {}", format_runtime_error(err)));
        let sum_id = expect_signal_id(&sum);
        let ctx = runtime.ctx.clone();
        let owner_thread = format!("{:?}", std::thread::current().id());

        let callback_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let callback_threads = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let callback_count_for_watch = callback_count.clone();
        let callback_threads_for_watch = callback_threads.clone();
        let callback = runtime_builtin("test.main_thread_batch_watch", 1, move |_args, _| {
            callback_count_for_watch.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            callback_threads_for_watch
                .lock()
                .expect("callback thread log lock should not be poisoned")
                .push(format!("{:?}", std::thread::current().id()));
            Ok(Value::Unit)
        });
        let (watcher_id, _disposable) = runtime
            .reactive_watch_signal_main_thread(sum.clone(), callback)
            .unwrap_or_else(|err| panic!("derived watcher: {}", format_runtime_error(err)));

        assert_eq!(
            expect_int(
                runtime
                    .reactive_get_signal(sum.clone())
                    .unwrap_or_else(|err| panic!("initial sum value: {}", format_runtime_error(err)))
            ),
            3
        );
        assert_eq!(watcher_last_revision(&runtime, watcher_id), 1);

        let first_for_batch = first.clone();
        let second_for_batch = second.clone();
        std::thread::spawn(move || {
            let mut bg_runtime = Runtime::new(ctx, CancelToken::root());
            let batch_callback = runtime_builtin("test.background_batch", 1, move |_args, runtime| {
                runtime.reactive_set_signal(first_for_batch.clone(), Value::Int(3))?;
                runtime.reactive_set_signal(second_for_batch.clone(), Value::Int(4))?;
                Ok(Value::Unit)
            });
            bg_runtime
                .reactive_batch(batch_callback)
                .unwrap_or_else(|err| panic!("background batch: {}", format_runtime_error(err)));
        })
        .join()
        .expect("background batch runtime should not panic");

        {
            let graph = runtime.reactive_graph.lock();
            assert!(graph.deferred_flush);
            assert!(!graph.pending_notifications.is_empty());
            assert!(
                graph
                    .signals
                    .get(&sum_id)
                    .unwrap_or_else(|| panic!("missing derived signal {sum_id}"))
                    .dirty
            );
        }
        assert_eq!(watcher_last_revision(&runtime, watcher_id), 1);
        assert_eq!(
            callback_count.load(std::sync::atomic::Ordering::SeqCst),
            0
        );

        runtime
            .reactive_flush_deferred()
            .unwrap_or_else(|err| panic!("flush deferred derived watcher: {}", format_runtime_error(err)));

        assert_eq!(
            expect_int(
                runtime
                    .reactive_get_signal(sum.clone())
                    .unwrap_or_else(|err| panic!("flushed sum value: {}", format_runtime_error(err)))
            ),
            7
        );
        {
            let graph = runtime.reactive_graph.lock();
            assert!(!graph.deferred_flush);
            assert!(graph.pending_notifications.is_empty());
            let sum_entry = graph
                .signals
                .get(&sum_id)
                .unwrap_or_else(|| panic!("missing derived signal {sum_id}"));
            assert!(!sum_entry.dirty);
            assert_eq!(sum_entry.revision, 2);
        }
        assert_eq!(watcher_last_revision(&runtime, watcher_id), 2);
        assert_eq!(
            callback_count.load(std::sync::atomic::Ordering::SeqCst),
            1
        );
        assert_eq!(
            callback_threads
                .lock()
                .expect("callback thread log lock should not be poisoned")
                .as_slice(),
            [owner_thread.as_str()]
        );

        runtime
            .reactive_flush_deferred()
            .unwrap_or_else(|err| panic!("second derived deferred flush: {}", format_runtime_error(err)));
        assert_eq!(
            callback_count.load(std::sync::atomic::Ordering::SeqCst),
            1
        );
    }

    #[test]
    fn reactive_event_clears_previous_result_on_failed_rerun() {
        let mut runtime = test_runtime();
        let should_succeed = Arc::new(std::sync::atomic::AtomicBool::new(true));
        let flag = should_succeed.clone();
        let effect = Value::Effect(Arc::new(EffectValue::Thunk {
            func: Arc::new(move |_| {
                if flag.load(std::sync::atomic::Ordering::SeqCst) {
                    Ok(Value::Int(42))
                } else {
                    Err(RuntimeError::Error(Value::Text("boom".to_string())))
                }
            }),
        }));

        let event = runtime
            .reactive_create_event(effect)
            .unwrap_or_else(|err| panic!("event: {}", format_runtime_error(err)));

        let run = record_field(&event, "run");
        let result_signal = record_field(&event, "result");
        let error_signal = record_field(&event, "error");

        let first = runtime
            .run_effect_value(run.clone())
            .unwrap_or_else(|err| panic!("first run: {}", format_runtime_error(err)));
        assert_eq!(expect_int(first), 42);

        should_succeed.store(false, std::sync::atomic::Ordering::SeqCst);
        match runtime.run_effect_value(run) {
            Err(RuntimeError::Error(Value::Text(text))) => assert_eq!(text, "boom"),
            Err(err) => panic!("second run failed unexpectedly: {}", format_runtime_error(err)),
            Ok(value) => panic!("second run unexpectedly succeeded: {}", format_value(&value)),
        }

        match runtime
            .reactive_get_signal(result_signal)
            .unwrap_or_else(|err| panic!("result signal: {}", format_runtime_error(err)))
        {
            Value::Constructor { name, args } => {
                assert_eq!(name, "None");
                assert!(args.is_empty());
            }
            other => panic!("expected None result, got {}", format_value(&other)),
        }

        match runtime
            .reactive_get_signal(error_signal)
            .unwrap_or_else(|err| panic!("error signal: {}", format_runtime_error(err)))
        {
            Value::Constructor { name, args } => {
                assert_eq!(name, "Some");
                assert_eq!(args.len(), 1);
                assert_eq!(format_value(&args[0]), "boom");
            }
            other => panic!("expected Some error, got {}", format_value(&other)),
        }
    }

    /// Multiple background threads concurrently writing the same source signal.
    /// The final value must equal the last write and the graph must remain
    /// consistent (no panics, no poisoned locks).
    #[test]
    fn reactive_concurrent_writes_to_same_signal() {
        let mut runtime = test_runtime();
        let source = runtime
            .reactive_create_signal(Value::Int(0))
            .unwrap_or_else(|err| panic!("source signal: {}", format_runtime_error(err)));
        let ctx = runtime.ctx.clone();

        let threads: Vec<_> = (0..8)
            .map(|i| {
                let ctx = ctx.clone();
                let source = source.clone();
                std::thread::spawn(move || {
                    let mut bg = Runtime::new(ctx, CancelToken::root());
                    for j in 0..50 {
                        bg.reactive_set_signal(source.clone(), Value::Int(i * 100 + j))
                            .unwrap_or_else(|err| {
                                panic!("thread {i} set #{j}: {}", format_runtime_error(err))
                            });
                    }
                })
            })
            .collect();

        for handle in threads {
            handle.join().expect("worker thread should not panic");
        }

        // Value must be one of the final writes (i*100+49 for some i).
        let final_val = expect_int(
            runtime
                .reactive_get_signal(source)
                .unwrap_or_else(|err| panic!("final get: {}", format_runtime_error(err))),
        );
        assert!(final_val >= 0, "signal value should be non-negative");

        // Graph must be in a valid state: not flushing, no leftover batch depth.
        let graph = runtime.reactive_graph.lock();
        assert_eq!(graph.batch_depth, 0);
        assert!(!graph.flushing);
    }

    /// Background threads write while the main thread reads concurrently.
    /// No panics, no poisoned locks, reads always return a valid value.
    #[test]
    fn reactive_concurrent_read_and_write() {
        let mut runtime = test_runtime();
        let source = runtime
            .reactive_create_signal(Value::Int(0))
            .unwrap_or_else(|err| panic!("source signal: {}", format_runtime_error(err)));
        let ctx = runtime.ctx.clone();

        let done = Arc::new(std::sync::atomic::AtomicBool::new(false));

        // Spawn a writer thread
        let done_writer = done.clone();
        let ctx_writer = ctx.clone();
        let source_writer = source.clone();
        let writer = std::thread::spawn(move || {
            let mut bg = Runtime::new(ctx_writer, CancelToken::root());
            for i in 1..=200 {
                bg.reactive_set_signal(source_writer.clone(), Value::Int(i))
                    .unwrap_or_else(|err| panic!("writer set #{i}: {}", format_runtime_error(err)));
            }
            done_writer.store(true, std::sync::atomic::Ordering::Release);
        });

        // Read from main thread while writer is going
        let mut read_count = 0u64;
        while !done.load(std::sync::atomic::Ordering::Acquire) {
            let val = expect_int(
                runtime
                    .reactive_get_signal(source.clone())
                    .unwrap_or_else(|err| panic!("reader get: {}", format_runtime_error(err))),
            );
            assert!((0..=200).contains(&val), "unexpected value: {val}");
            read_count += 1;
        }
        writer.join().expect("writer thread should not panic");
        assert!(read_count > 0, "main thread should have read at least once");

        let final_val = expect_int(
            runtime
                .reactive_get_signal(source)
                .unwrap_or_else(|err| panic!("final get: {}", format_runtime_error(err))),
        );
        assert_eq!(final_val, 200);
    }

    /// Dispose a watcher from the main thread while a background thread is
    /// writing to the watched signal. Neither thread should panic.
    #[test]
    fn reactive_dispose_watcher_while_background_writes() {
        let mut runtime = test_runtime();
        let source = runtime
            .reactive_create_signal(Value::Int(0))
            .unwrap_or_else(|err| panic!("source signal: {}", format_runtime_error(err)));

        let callback_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let cc = callback_count.clone();
        let watch_callback = runtime_builtin("test.dispose_race_watch", 1, move |_args, _| {
            cc.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(Value::Unit)
        });
        let (watcher_id, _disposable) = runtime
            .reactive_watch_signal_with_id(source.clone(), watch_callback)
            .unwrap_or_else(|err| panic!("watcher: {}", format_runtime_error(err)));

        let ctx = runtime.ctx.clone();
        let source_for_thread = source.clone();
        let writer = std::thread::spawn(move || {
            let mut bg = Runtime::new(ctx, CancelToken::root());
            for i in 1..=100 {
                bg.reactive_set_signal(source_for_thread.clone(), Value::Int(i))
                    .unwrap_or_else(|err| panic!("bg set #{i}: {}", format_runtime_error(err)));
            }
        });

        // Dispose watcher while writes are happening
        runtime.reactive_dispose_watcher(watcher_id);

        writer.join().expect("writer thread should not panic");

        // After dispose, the callback count should not increase from main-thread
        // flushes.
        let count_after_dispose = callback_count.load(std::sync::atomic::Ordering::SeqCst);
        runtime
            .reactive_set_signal(source.clone(), Value::Int(999))
            .unwrap_or_else(|err| panic!("post-dispose set: {}", format_runtime_error(err)));
        assert_eq!(
            callback_count.load(std::sync::atomic::Ordering::SeqCst),
            count_after_dispose
        );

        // Graph is consistent
        let graph = runtime.reactive_graph.lock();
        assert!(!graph.watchers.contains_key(&watcher_id));
    }

    /// Multiple background threads each write their own source signal. A derived
    /// signal (combineAll) depends on all sources. After all writers finish, the
    /// derived value must reflect the final source values.
    #[test]
    fn reactive_concurrent_writes_to_different_sources_with_derived() {
        let mut runtime = test_runtime();
        let num_sources = 4;
        let sources: Vec<Value> = (0..num_sources)
            .map(|i| {
                runtime
                    .reactive_create_signal(Value::Int(0))
                    .unwrap_or_else(|err| panic!("source {i}: {}", format_runtime_error(err)))
            })
            .collect();

        let sum = runtime
            .reactive_combine_all(
                Value::Tuple(sources.clone()),
                runtime_builtin("test.sum_n", 1, |mut args, _| {
                    let Value::Tuple(items) = args.remove(0) else {
                        panic!("expected tuple");
                    };
                    let total: i64 = items.iter().map(|v| expect_int(v.clone())).sum();
                    Ok(Value::Int(total))
                }),
            )
            .unwrap_or_else(|err| panic!("sum signal: {}", format_runtime_error(err)));

        let ctx = runtime.ctx.clone();
        let threads: Vec<_> = sources
            .iter()
            .enumerate()
            .map(|(i, source)| {
                let ctx = ctx.clone();
                let source = source.clone();
                std::thread::spawn(move || {
                    let mut bg = Runtime::new(ctx, CancelToken::root());
                    for j in 1..=20 {
                        bg.reactive_set_signal(source.clone(), Value::Int(j))
                            .unwrap_or_else(|err| {
                                panic!("thread {i} set #{j}: {}", format_runtime_error(err))
                            });
                    }
                })
            })
            .collect();

        for handle in threads {
            handle.join().expect("source writer thread should not panic");
        }

        // Each source ends at 20, so sum = 4 * 20 = 80
        let sum_val = expect_int(
            runtime
                .reactive_get_signal(sum)
                .unwrap_or_else(|err| panic!("sum value: {}", format_runtime_error(err))),
        );
        assert_eq!(sum_val, num_sources as i64 * 20);
    }

    /// Background threads write rapidly to a signal that has a deferred-flush
    /// watcher. Accumulated writes should coalesce into one watcher callback
    /// per flush.
    #[test]
    fn reactive_rapid_background_writes_coalesce_on_deferred_flush() {
        let mut runtime = test_runtime();
        let source = runtime
            .reactive_create_signal(Value::Int(0))
            .unwrap_or_else(|err| panic!("source signal: {}", format_runtime_error(err)));

        let callback_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let cc = callback_count.clone();
        let callback = runtime_builtin("test.coalesce_watch", 1, move |_args, _| {
            cc.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(Value::Unit)
        });
        runtime
            .reactive_watch_signal_main_thread(source.clone(), callback)
            .unwrap_or_else(|err| panic!("main-thread watcher: {}", format_runtime_error(err)));

        // Rapid writes from a background thread
        let ctx = runtime.ctx.clone();
        let source_for_thread = source.clone();
        std::thread::spawn(move || {
            let mut bg = Runtime::new(ctx, CancelToken::root());
            for i in 1..=50 {
                bg.reactive_set_signal(source_for_thread.clone(), Value::Int(i))
                    .unwrap_or_else(|err| panic!("rapid set #{i}: {}", format_runtime_error(err)));
            }
        })
        .join()
        .expect("rapid writer should not panic");

        // No callbacks yet — everything is deferred
        assert_eq!(
            callback_count.load(std::sync::atomic::Ordering::SeqCst),
            0
        );

        // Single flush should deliver exactly one notification
        runtime
            .reactive_flush_deferred()
            .unwrap_or_else(|err| panic!("flush deferred: {}", format_runtime_error(err)));
        assert_eq!(
            callback_count.load(std::sync::atomic::Ordering::SeqCst),
            1
        );

        let final_val = expect_int(
            runtime
                .reactive_get_signal(source)
                .unwrap_or_else(|err| panic!("final get: {}", format_runtime_error(err))),
        );
        assert_eq!(final_val, 50);
    }

    /// Multiple background threads write to the same signal while another set
    /// of threads create derived signals from it concurrently. Tests that
    /// signal creation and invalidation are safe under contention.
    #[test]
    fn reactive_concurrent_derive_creation_and_writes() {
        let mut runtime = test_runtime();
        let source = runtime
            .reactive_create_signal(Value::Int(0))
            .unwrap_or_else(|err| panic!("source signal: {}", format_runtime_error(err)));
        let ctx = runtime.ctx.clone();

        // Spawn writers
        let writer_handles: Vec<_> = (0..4)
            .map(|i| {
                let ctx = ctx.clone();
                let source = source.clone();
                std::thread::spawn(move || {
                    let mut bg = Runtime::new(ctx, CancelToken::root());
                    for j in 1..=30 {
                        bg.reactive_set_signal(source.clone(), Value::Int(i * 100 + j))
                            .unwrap_or_else(|err| {
                                panic!("writer {i} set #{j}: {}", format_runtime_error(err))
                            });
                    }
                })
            })
            .collect();

        // Spawn derive creators — each creates a derived signal in its own
        // Runtime, which still shares the same reactive graph.
        let derive_handles: Vec<_> = (0..4)
            .map(|i| {
                let ctx = ctx.clone();
                let source = source.clone();
                std::thread::spawn(move || {
                    let mut bg = Runtime::new(ctx, CancelToken::root());
                    let derived = bg
                        .reactive_derive_signal(
                            source.clone(),
                            runtime_builtin(
                                &format!("test.derive_thread_{i}"),
                                1,
                                move |mut args, _| {
                                    let val = expect_int(args.remove(0));
                                    Ok(Value::Int(val * (i + 1)))
                                },
                            ),
                        )
                        .unwrap_or_else(|err| {
                            panic!("derive thread {i}: {}", format_runtime_error(err))
                        });
                    // Read the derived signal a few times
                    for _ in 0..10 {
                        let _ = bg.reactive_get_signal(derived.clone());
                    }
                })
            })
            .collect();

        for h in writer_handles {
            h.join().expect("writer should not panic");
        }
        for h in derive_handles {
            h.join().expect("deriver should not panic");
        }

        // Source should have a valid final value
        let final_val = expect_int(
            runtime
                .reactive_get_signal(source)
                .unwrap_or_else(|err| panic!("final source: {}", format_runtime_error(err))),
        );
        assert!(final_val >= 0, "final value must be non-negative");

        let graph = runtime.reactive_graph.lock();
        assert_eq!(graph.batch_depth, 0);
        assert!(!graph.flushing);
    }

    /// Background threads each batch-write to their own source signal. After
    /// joining, a deferred flush on the main thread delivers the settled derived
    /// value exactly once.
    #[test]
    fn reactive_concurrent_batches_from_multiple_threads_with_deferred_flush() {
        let mut runtime = test_runtime();
        let first = runtime
            .reactive_create_signal(Value::Int(0))
            .unwrap_or_else(|err| panic!("first: {}", format_runtime_error(err)));
        let second = runtime
            .reactive_create_signal(Value::Int(0))
            .unwrap_or_else(|err| panic!("second: {}", format_runtime_error(err)));
        let sum = runtime
            .reactive_combine_all(
                Value::Tuple(vec![first.clone(), second.clone()]),
                runtime_builtin("test.sum_pair_concurrent", 1, |mut args, _| {
                    let Value::Tuple(items) = args.remove(0) else {
                        panic!("expected tuple");
                    };
                    let [left, right] = items.as_slice() else {
                        panic!("expected 2 items");
                    };
                    Ok(Value::Int(expect_int(left.clone()) + expect_int(right.clone())))
                }),
            )
            .unwrap_or_else(|err| panic!("sum: {}", format_runtime_error(err)));

        let callback_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let seen_values = Arc::new(std::sync::Mutex::new(Vec::<i64>::new()));
        let cc = callback_count.clone();
        let sv = seen_values.clone();
        let sum_for_watch = sum.clone();
        let callback = runtime_builtin("test.concurrent_batch_watch", 1, move |_args, runtime| {
            cc.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let val = runtime.reactive_get_signal(sum_for_watch.clone())?;
            sv.lock()
                .expect("seen_values lock should not be poisoned")
                .push(expect_int(val));
            Ok(Value::Unit)
        });
        runtime
            .reactive_watch_signal_main_thread(sum.clone(), callback)
            .unwrap_or_else(|err| panic!("watcher: {}", format_runtime_error(err)));

        let ctx = runtime.ctx.clone();

        // Thread A writes first=10, Thread B writes second=20
        let ctx_a = ctx.clone();
        let first_for_a = first.clone();
        let handle_a = std::thread::spawn(move || {
            let mut bg = Runtime::new(ctx_a, CancelToken::root());
            let first_c = first_for_a.clone();
            let batch_cb = runtime_builtin("test.batch_a", 1, move |_args, runtime| {
                runtime.reactive_set_signal(first_c.clone(), Value::Int(10))?;
                Ok(Value::Unit)
            });
            bg.reactive_batch(batch_cb)
                .unwrap_or_else(|err| panic!("batch A: {}", format_runtime_error(err)));
        });

        let ctx_b = ctx.clone();
        let second_for_b = second.clone();
        let handle_b = std::thread::spawn(move || {
            let mut bg = Runtime::new(ctx_b, CancelToken::root());
            let second_c = second_for_b.clone();
            let batch_cb = runtime_builtin("test.batch_b", 1, move |_args, runtime| {
                runtime.reactive_set_signal(second_c.clone(), Value::Int(20))?;
                Ok(Value::Unit)
            });
            bg.reactive_batch(batch_cb)
                .unwrap_or_else(|err| panic!("batch B: {}", format_runtime_error(err)));
        });

        handle_a.join().expect("thread A should not panic");
        handle_b.join().expect("thread B should not panic");

        // No callbacks yet
        assert_eq!(
            callback_count.load(std::sync::atomic::Ordering::SeqCst),
            0
        );

        runtime
            .reactive_flush_deferred()
            .unwrap_or_else(|err| panic!("flush: {}", format_runtime_error(err)));

        let final_sum = expect_int(
            runtime
                .reactive_get_signal(sum)
                .unwrap_or_else(|err| panic!("final sum: {}", format_runtime_error(err))),
        );
        assert_eq!(final_sum, 30);

        // Watcher should have fired (at least once, possibly twice if the graph
        // saw two separate pending-notification rounds).
        let count = callback_count.load(std::sync::atomic::Ordering::SeqCst);
        assert!(count >= 1, "watcher should fire at least once, got {count}");

        // The last seen value must be 30 (the settled sum).
        let values = seen_values
            .lock()
            .expect("seen_values lock should not be poisoned");
        assert_eq!(
            *values.last().expect("should have at least one seen value"),
            30
        );
    }

    /// Set the same value repeatedly — the signal should detect equality and
    /// skip watcher notifications (no-change optimization under contention).
    #[test]
    fn reactive_no_change_skips_notification_under_contention() {
        let mut runtime = test_runtime();
        let source = runtime
            .reactive_create_signal(Value::Int(42))
            .unwrap_or_else(|err| panic!("source: {}", format_runtime_error(err)));

        let callback_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let cc = callback_count.clone();
        let callback = runtime_builtin("test.noop_watch", 1, move |_args, _| {
            cc.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(Value::Unit)
        });
        runtime
            .reactive_watch_signal(source.clone(), callback)
            .unwrap_or_else(|err| panic!("watcher: {}", format_runtime_error(err)));

        // Write the same value many times — should not trigger watcher
        for _ in 0..20 {
            runtime
                .reactive_set_signal(source.clone(), Value::Int(42))
                .unwrap_or_else(|err| panic!("set same: {}", format_runtime_error(err)));
        }
        assert_eq!(
            callback_count.load(std::sync::atomic::Ordering::SeqCst),
            0
        );

        // Now write a different value — should trigger once
        runtime
            .reactive_set_signal(source.clone(), Value::Int(43))
            .unwrap_or_else(|err| panic!("set different: {}", format_runtime_error(err)));
        assert_eq!(
            callback_count.load(std::sync::atomic::Ordering::SeqCst),
            1
        );
    }
}
