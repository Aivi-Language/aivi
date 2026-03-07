impl Runtime {
    pub(crate) fn reactive_init_host(&mut self, model: Value) {
        let mut root_field_revisions = HashMap::new();
        if let Value::Record(fields) = &model {
            for field in fields.keys() {
                root_field_revisions.insert(field.clone(), 1);
            }
        }
        self.reactive_host = Some(ReactiveHostState {
            current_model: model,
            model_revision: 1,
            root_field_revisions,
            signals: HashMap::new(),
            dependents: HashMap::new(),
            eval_stack: Vec::new(),
        });
    }

    pub(crate) fn reactive_commit_host(&mut self, previous: Value, model: Value) {
        if self.reactive_host.is_none() {
            self.reactive_init_host(model);
            return;
        }

        let model_changed = !reactive_values_match(&previous, &model);
        let changed_fields = reactive_changed_root_fields(&previous, &model);
        let mut dirty_keys = Vec::new();

        {
            let state = self.reactive_host.as_mut().expect("reactive host state");
            if model_changed {
                state.model_revision = state.model_revision.saturating_add(1);
            }

            match (&previous, &model) {
                (Value::Record(_), Value::Record(new_fields)) => {
                    for field in changed_fields.iter() {
                        state
                            .root_field_revisions
                            .insert(field.clone(), state.model_revision);
                    }
                    for field in new_fields.keys() {
                        state
                            .root_field_revisions
                            .entry(field.clone())
                            .or_insert(state.model_revision);
                    }
                }
                _ if model_changed => {
                    state.root_field_revisions.clear();
                    if let Value::Record(fields) = &model {
                        for field in fields.keys() {
                            state
                                .root_field_revisions
                                .insert(field.clone(), state.model_revision);
                        }
                    }
                }
                _ => {}
            }

            if model_changed {
                for (key, entry) in &state.signals {
                    let depends_on_changed_source = entry.dependencies.iter().any(|dep| {
                        matches!(dep.dependency, ReactiveDependency::WholeModel)
                            || matches!(
                                &dep.dependency,
                                ReactiveDependency::RootField(field) if changed_fields.contains(field)
                            )
                    });
                    if depends_on_changed_source {
                        dirty_keys.push(key.clone());
                    }
                }
            }

            state.current_model = model;
        }

        for key in dirty_keys {
            self.reactive_mark_signal_dirty(&key);
        }
    }

    pub(crate) fn reactive_note_root_field_access(&mut self, field: &str) {
        let Some(state) = self.reactive_host.as_mut() else {
            return;
        };
        let Some(frame) = state.eval_stack.last_mut() else {
            return;
        };
        frame.dependencies
            .insert(ReactiveDependency::RootField(field.to_string()));
    }

    pub(crate) fn reactive_note_record_field_access(&mut self, base: &Value, field: &str) {
        let should_note = match (&self.reactive_host, base) {
            (Some(state), Value::Record(base_fields)) => match &state.current_model {
                Value::Record(current_fields) => Arc::ptr_eq(current_fields, base_fields),
                _ => false,
            },
            _ => false,
        };
        if should_note {
            self.reactive_note_root_field_access(field);
        }
    }

    pub(crate) fn reactive_read_computed(
        &mut self,
        key: &str,
        derive: Value,
        model: Value,
    ) -> Result<Value, RuntimeError> {
        if !self.reactive_model_matches(&model) {
            return self.apply(derive, model);
        }

        let key_owned = key.to_string();
        self.reactive_note_signal_dependency(&key_owned);
        self.reactive_store_derive(&key_owned, derive.clone());

        if self.reactive_stack_contains(&key_owned) {
            return Err(RuntimeError::InvalidArgument {
                context: "gtk4.computed".to_string(),
                reason: format!("reactive cycle detected while evaluating `{key}`"),
            });
        }

        if self.reactive_signal_is_fresh(&key_owned) {
            if let Some(cached) = self
                .reactive_host
                .as_ref()
                .and_then(|state| state.signals.get(&key_owned))
                .and_then(|entry| entry.cached.clone())
            {
                return Ok(cached);
            }
        }

        self.reactive_push_frame(key_owned.clone());
        let result = self.apply(derive, model);
        let dependencies = self.reactive_pop_frame_dependencies(&key_owned);

        match result {
            Ok(value) => {
                self.reactive_finish_signal(&key_owned, value.clone(), dependencies);
                Ok(value)
            }
            Err(err) => Err(err),
        }
    }

    fn reactive_model_matches(&self, model: &Value) -> bool {
        self.reactive_host
            .as_ref()
            .map(|state| reactive_values_match(&state.current_model, model))
            .unwrap_or(false)
    }

    fn reactive_store_derive(&mut self, key: &str, derive: Value) {
        let Some(state) = self.reactive_host.as_mut() else {
            return;
        };
        let entry = state
            .signals
            .entry(key.to_string())
            .or_insert_with(|| ReactiveSignalEntry {
                derive: derive.clone(),
                cached: None,
                dependencies: Vec::new(),
                dirty: true,
                revision: 0,
            });
        entry.derive = derive;
    }

    fn reactive_stack_contains(&self, key: &str) -> bool {
        self.reactive_host
            .as_ref()
            .map(|state| state.eval_stack.iter().any(|frame| frame.key == key))
            .unwrap_or(false)
    }

    fn reactive_signal_is_fresh(&self, key: &str) -> bool {
        let Some(state) = self.reactive_host.as_ref() else {
            return false;
        };
        let Some(entry) = state.signals.get(key) else {
            return false;
        };
        if entry.dirty || entry.cached.is_none() {
            return false;
        }
        entry.dependencies.iter().all(|dep| match &dep.dependency {
            ReactiveDependency::WholeModel => state.model_revision == dep.revision,
            ReactiveDependency::RootField(field) => state
                .root_field_revisions
                .get(field)
                .copied()
                .unwrap_or(0)
                == dep.revision,
            ReactiveDependency::Signal(other) => state
                .signals
                .get(other)
                .is_some_and(|other_entry| !other_entry.dirty && other_entry.revision == dep.revision),
        })
    }

    fn reactive_push_frame(&mut self, key: String) {
        let Some(state) = self.reactive_host.as_mut() else {
            return;
        };
        state.eval_stack.push(ReactiveEvalFrame {
            key,
            dependencies: HashSet::new(),
        });
    }

    fn reactive_pop_frame_dependencies(&mut self, key: &str) -> HashSet<ReactiveDependency> {
        let Some(state) = self.reactive_host.as_mut() else {
            return HashSet::new();
        };
        match state.eval_stack.pop() {
            Some(frame) if frame.key == key => frame.dependencies,
            Some(frame) => {
                state.eval_stack.push(frame);
                HashSet::new()
            }
            None => HashSet::new(),
        }
    }

    fn reactive_note_signal_dependency(&mut self, dependency_key: &str) {
        let Some(state) = self.reactive_host.as_mut() else {
            return;
        };
        let Some(frame) = state.eval_stack.last_mut() else {
            return;
        };
        if frame.key != dependency_key {
            frame.dependencies
                .insert(ReactiveDependency::Signal(dependency_key.to_string()));
        }
    }

    fn reactive_finish_signal(
        &mut self,
        key: &str,
        value: Value,
        mut dependencies: HashSet<ReactiveDependency>,
    ) {
        let Some(state) = self.reactive_host.as_mut() else {
            return;
        };

        if dependencies.is_empty() {
            dependencies.insert(ReactiveDependency::WholeModel);
        }

        let old_signal_dependencies = state
            .signals
            .get(key)
            .map(|entry| {
                entry
                    .dependencies
                    .iter()
                    .filter_map(|dep| match &dep.dependency {
                        ReactiveDependency::Signal(name) => Some(name.clone()),
                        _ => None,
                    })
                    .collect::<HashSet<_>>()
            })
            .unwrap_or_default();

        let new_signal_dependencies = dependencies
            .iter()
            .filter_map(|dep| match dep {
                ReactiveDependency::Signal(name) => Some(name.clone()),
                _ => None,
            })
            .collect::<HashSet<_>>();

        for dependency in old_signal_dependencies.difference(&new_signal_dependencies) {
            if let Some(dependents) = state.dependents.get_mut(dependency) {
                dependents.remove(key);
                if dependents.is_empty() {
                    state.dependents.remove(dependency);
                }
            }
        }

        for dependency in &new_signal_dependencies {
            state
                .dependents
                .entry(dependency.clone())
                .or_default()
                .insert(key.to_string());
        }

        let dependency_versions = dependencies
            .into_iter()
            .map(|dependency| ReactiveDependencyVersion {
                revision: reactive_dependency_revision(state, &dependency),
                dependency,
            })
            .collect::<Vec<_>>();

        let entry = state
            .signals
            .entry(key.to_string())
            .or_insert_with(|| ReactiveSignalEntry {
                derive: Value::Unit,
                cached: None,
                dependencies: Vec::new(),
                dirty: true,
                revision: 0,
            });
        entry.cached = Some(value);
        entry.dependencies = dependency_versions;
        entry.dirty = false;
        entry.revision = entry.revision.saturating_add(1).max(1);
    }

    fn reactive_mark_signal_dirty(&mut self, key: &str) {
        let Some(state) = self.reactive_host.as_mut() else {
            return;
        };

        let mut stack = vec![key.to_string()];
        let mut seen = HashSet::new();
        while let Some(current) = stack.pop() {
            if !seen.insert(current.clone()) {
                continue;
            }

            if let Some(entry) = state.signals.get_mut(&current) {
                entry.dirty = true;
            }

            if let Some(dependents) = state.dependents.get(&current) {
                stack.extend(dependents.iter().cloned());
            }
        }
    }
}

fn reactive_dependency_revision(state: &ReactiveHostState, dependency: &ReactiveDependency) -> u64 {
    match dependency {
        ReactiveDependency::WholeModel => state.model_revision,
        ReactiveDependency::RootField(field) => {
            state.root_field_revisions.get(field).copied().unwrap_or(0)
        }
        ReactiveDependency::Signal(signal) => state
            .signals
            .get(signal)
            .map(|entry| entry.revision)
            .unwrap_or(0),
    }
}

fn reactive_changed_root_fields(previous: &Value, current: &Value) -> HashSet<String> {
    match (previous, current) {
        (Value::Record(previous_fields), Value::Record(current_fields)) => previous_fields
            .keys()
            .chain(current_fields.keys())
            .cloned()
            .collect::<HashSet<_>>()
            .into_iter()
            .filter(|field| {
                previous_fields.get(field).map_or(true, |previous_value| {
                    current_fields
                        .get(field)
                        .map(|current_value| !reactive_values_match(previous_value, current_value))
                        .unwrap_or(true)
                })
            })
            .collect(),
        _ => HashSet::new(),
    }
}

fn reactive_values_match(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Record(left_fields), Value::Record(right_fields)) => {
            Arc::ptr_eq(left_fields, right_fields) || values_equal(left, right)
        }
        (Value::List(left_items), Value::List(right_items)) => {
            Arc::ptr_eq(left_items, right_items) || values_equal(left, right)
        }
        (Value::Map(left_items), Value::Map(right_items)) => {
            Arc::ptr_eq(left_items, right_items) || values_equal(left, right)
        }
        (Value::Set(left_items), Value::Set(right_items)) => {
            Arc::ptr_eq(left_items, right_items) || values_equal(left, right)
        }
        (Value::Queue(left_items), Value::Queue(right_items)) => {
            Arc::ptr_eq(left_items, right_items) || values_equal(left, right)
        }
        (Value::Deque(left_items), Value::Deque(right_items)) => {
            Arc::ptr_eq(left_items, right_items) || values_equal(left, right)
        }
        (Value::Heap(left_items), Value::Heap(right_items)) => {
            Arc::ptr_eq(left_items, right_items) || values_equal(left, right)
        }
        (Value::Bytes(left_bytes), Value::Bytes(right_bytes)) => {
            Arc::ptr_eq(left_bytes, right_bytes) || values_equal(left, right)
        }
        (Value::Text(left_text), Value::Text(right_text)) => left_text == right_text,
        (Value::Int(left_int), Value::Int(right_int)) => left_int == right_int,
        (Value::Bool(left_bool), Value::Bool(right_bool)) => left_bool == right_bool,
        (Value::Float(left_float), Value::Float(right_float)) => left_float == right_float,
        (Value::Unit, Value::Unit) => true,
        _ => values_equal(left, right),
    }
}
