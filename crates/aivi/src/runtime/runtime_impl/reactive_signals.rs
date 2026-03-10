impl Default for ReactiveGraphState {
    fn default() -> Self {
        Self {
            next_signal_id: 1,
            next_watcher_id: 1,
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
        self.reactive_graph.lock().signals.insert(
            signal_id,
            ReactiveCellEntry {
                kind: ReactiveCellKind::Source,
                value: initial,
                revision: 1,
                dirty: false,
                dependents: HashSet::new(),
            },
        );
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
        signals_record: Value,
        combine: Value,
    ) -> Result<Value, RuntimeError> {
        let signals_record = self.force_value(signals_record)?;
        let Value::Record(fields) = signals_record else {
            return Err(RuntimeError::InvalidArgument {
                context: "reactive.combineAll".to_string(),
                reason: "expected a record of signals".to_string(),
            });
        };
        let mut sorted_fields: Vec<(String, Value)> = fields
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        sorted_fields.sort_by(|a, b| a.0.cmp(&b.0));

        let mut signal_ids = Vec::with_capacity(sorted_fields.len());
        let mut field_names = Vec::with_capacity(sorted_fields.len());
        for (name, value) in &sorted_fields {
            let id = self.reactive_signal_id_from_value(
                value.clone(),
                &format!("reactive.combineAll field '{name}'"),
            )?;
            signal_ids.push(id);
            field_names.push(name.clone());
        }

        // Create a derived signal that reconstructs the record with resolved values,
        // then applies the combine function.
        self.reactive_create_derived_signal_with_record(signal_ids, field_names, combine)
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
                    active: true,
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
        self.reactive_graph.lock().flush_thread = Some(std::thread::current().id());
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

    fn reactive_create_derived_signal_with_record(
        &mut self,
        dependencies: Vec<usize>,
        field_names: Vec<String>,
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
                    kind: ReactiveCellKind::DerivedRecord {
                        dependencies: dependencies.clone(),
                        field_names,
                        compute,
                    },
                    value: Value::Unit,
                    revision: 0,
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
            let entry = graph
                .signals
                .get_mut(&signal_id)
                .expect("source signal exists");
            entry.value = next_value;
            entry.revision = entry.revision.saturating_add(1).max(1);
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
                ReactiveCellKind::DerivedRecord {
                    dependencies,
                    field_names,
                    compute,
                } => ReactiveCellKind::DerivedRecord {
                    dependencies: dependencies.clone(),
                    field_names: field_names.clone(),
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
                    let entry = graph.signals.get_mut(&signal_id).expect("derived signal exists");
                    if !reactive_values_match(&entry.value, &value) {
                        entry.value = value;
                        entry.revision = entry.revision.saturating_add(1).max(1);
                        changed = true;
                    }
                    entry.dirty = false;
                }
                if changed {
                    self.reactive_graph
                        .lock()
                        .pending_notifications
                        .insert(signal_id);
                }
                Ok(())
            }
            ReactiveCellKind::DerivedRecord {
                dependencies,
                field_names,
                compute,
            } => {
                stack.push(signal_id);
                let mut record_fields = HashMap::new();
                for (i, dependency) in dependencies.iter().enumerate() {
                    self.reactive_ensure_signal_fresh(*dependency, stack)?;
                    let dep_value = self
                        .reactive_graph
                        .lock()
                        .signals
                        .get(dependency)
                        .expect("dependency exists")
                        .value
                        .clone();
                    record_fields.insert(field_names[i].clone(), dep_value);
                }
                let record = Value::Record(Arc::new(record_fields));
                let value = self.apply(compute, record)?;
                stack.pop();

                let mut changed = false;
                {
                    let mut graph = self.reactive_graph.lock();
                    let entry = graph.signals.get_mut(&signal_id).expect("derived signal exists");
                    if !reactive_values_match(&entry.value, &value) {
                        entry.value = value;
                        entry.revision = entry.revision.saturating_add(1).max(1);
                        changed = true;
                    }
                    entry.dirty = false;
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
        {
            let mut graph = self.reactive_graph.lock();
            if graph.flushing {
                return Ok(());
            }
            graph.flushing = true;
        }
        let result = (|| {
            loop {
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
                                watcher.active,
                                watcher.signal_id,
                                watcher.callback.clone(),
                                watcher.last_revision,
                                revision,
                            )
                        })
                    };
                    let Some(snapshot) = snapshot else {
                        continue;
                    };

                    let (active, signal_id, callback, last_revision, revision) = snapshot;
                    if !active || revision == last_revision {
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

    fn expect_int(value: Value) -> i64 {
        match value {
            Value::Int(value) => value,
            other => panic!("expected Int, got {}", format_value(&other)),
        }
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

        let combine = runtime_builtin("test.combine_record", 1, |mut args, _| {
            let record = args.remove(0);
            let Value::Record(fields) = record else {
                panic!("expected record");
            };
            let left = expect_int(fields.get("first").unwrap().clone());
            let right = expect_int(fields.get("second").unwrap().clone());
            Ok(Value::Int(left + right))
        });

        let mut signals_fields = HashMap::new();
        signals_fields.insert("first".to_string(), first.clone());
        signals_fields.insert("second".to_string(), second.clone());
        let signals_record = Value::Record(Arc::new(signals_fields));

        let sum = runtime
            .reactive_combine_all(signals_record, combine)
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
}
