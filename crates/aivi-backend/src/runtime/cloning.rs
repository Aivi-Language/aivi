// Clone runtime trees with typed worklists and uniquely owned output slots.
// Values and task plans share the traversal because either can contain the
// other. Maps use postorder assembly so every key is complete before insertion.

enum RuntimeCloneNode<'a> {
    Value(&'a RuntimeValue),
    Task(&'a RuntimeTaskPlan),
}

enum RuntimeCloneOutput {
    Value(RuntimeValue),
    Task(RuntimeTaskPlan),
}

enum RuntimeCloneWork<'a> {
    Visit(RuntimeCloneNode<'a>),
    Finish {
        source: RuntimeCloneNode<'a>,
        base: usize,
    },
}

trait RuntimeCloneFields {
    fn value(&mut self, source: &RuntimeValue) -> RuntimeValue;
    fn task(&mut self, source: &RuntimeTaskPlan) -> RuntimeTaskPlan;

    fn values(&mut self, source: &[RuntimeValue]) -> Vec<RuntimeValue> {
        source.iter().map(|value| self.value(value)).collect()
    }
}

struct CompletedRuntimeClones<'a> {
    children: std::vec::Drain<'a, RuntimeCloneOutput>,
}

impl RuntimeCloneFields for CompletedRuntimeClones<'_> {
    fn value(&mut self, _: &RuntimeValue) -> RuntimeValue {
        match self.children.next() {
            Some(RuntimeCloneOutput::Value(value)) => value,
            _ => unreachable!("clone traversal must preserve the value child order"),
        }
    }

    fn task(&mut self, _: &RuntimeTaskPlan) -> RuntimeTaskPlan {
        match self.children.next() {
            Some(RuntimeCloneOutput::Task(task)) => task,
            _ => unreachable!("clone traversal must preserve the task child order"),
        }
    }
}

fn clone_runtime_tree(source: RuntimeCloneNode<'_>) -> RuntimeCloneOutput {
    let mut pending = vec![RuntimeCloneWork::Visit(source)];
    let mut completed = Vec::new();
    while let Some(work) = pending.pop() {
        match work {
            RuntimeCloneWork::Visit(source) => {
                let leaf = match &source {
                    RuntimeCloneNode::Value(value) => {
                        clone_runtime_scalar(value).map(RuntimeCloneOutput::Value)
                    }
                    RuntimeCloneNode::Task(task) => {
                        clone_runtime_task_leaf(task).map(RuntimeCloneOutput::Task)
                    }
                };
                if let Some(leaf) = leaf {
                    completed.push(leaf);
                    continue;
                }
                let base = completed.len();
                // Enqueue the parent first, then its children in reverse field
                // order. The completed suffix is exactly this parent's children.
                pending.push(RuntimeCloneWork::Finish {
                    source: match &source {
                        RuntimeCloneNode::Value(value) => RuntimeCloneNode::Value(value),
                        RuntimeCloneNode::Task(task) => RuntimeCloneNode::Task(task),
                    },
                    base,
                });
                enqueue_runtime_clone_children(source, &mut pending);
            }
            RuntimeCloneWork::Finish { source, base } => {
                let result = {
                    let mut fields = CompletedRuntimeClones {
                        children: completed.drain(base..),
                    };
                    let result = match source {
                        RuntimeCloneNode::Value(value) => RuntimeCloneOutput::Value(
                            clone_runtime_value_fields(value, &mut fields),
                        ),
                        RuntimeCloneNode::Task(task) => {
                            RuntimeCloneOutput::Task(clone_runtime_task_fields(task, &mut fields))
                        }
                    };
                    debug_assert_eq!(fields.children.len(), 0);
                    result
                };
                completed.push(result);
            }
        }
    }
    debug_assert_eq!(completed.len(), 1);
    completed.pop().expect("clone traversal produces its root")
}

impl Clone for RuntimeValue {
    // Inline scalar element copies in containers; tree traversal stays outlined.
    #[inline(always)]
    fn clone(&self) -> Self {
        match self {
            RuntimeValue::Unit => RuntimeValue::Unit,
            RuntimeValue::Bool(value) => RuntimeValue::Bool(*value),
            RuntimeValue::Int(value) => RuntimeValue::Int(*value),
            RuntimeValue::Float(value) => RuntimeValue::Float(*value),
            RuntimeValue::Decimal(value) => RuntimeValue::Decimal(value.clone()),
            RuntimeValue::BigInt(value) => RuntimeValue::BigInt(value.clone()),
            RuntimeValue::Text(value) => RuntimeValue::Text(value.clone()),
            RuntimeValue::Bytes(value) => RuntimeValue::Bytes(value.clone()),
            RuntimeValue::OptionNone => RuntimeValue::OptionNone,
            RuntimeValue::SuffixedInteger { raw, suffix } => RuntimeValue::SuffixedInteger {
                raw: raw.clone(),
                suffix: suffix.clone(),
            },
            _ => {
                let mut destination = RuntimeValue::Unit;
                run_runtime_clone(RuntimeCloneInto::Value {
                    source: self,
                    destination: &mut destination,
                });
                destination
            }
        }
    }
}

impl Clone for RuntimeTaskPlan {
    fn clone(&self) -> Self {
        clone_runtime_task(self)
    }
}

// Unary composition needs only borrowed frames. Construct each owned edge once
// while unwinding the worklist; no recursive call follows the task spine.
fn clone_runtime_task(mut source: &RuntimeTaskPlan) -> RuntimeTaskPlan {
    let mut spine = Vec::new();
    loop {
        let inner = match source {
            RuntimeTaskPlan::Join { outer } => Some(outer.as_ref()),
            RuntimeTaskPlan::Map { inner, .. } | RuntimeTaskPlan::Chain { inner, .. } => {
                Some(inner.as_ref())
            }
            RuntimeTaskPlan::Pure { value } => match value.as_ref() {
                RuntimeValue::Task(inner) => Some(inner),
                _ => None,
            },
            _ => None,
        };
        match inner {
            Some(inner) => {
                spine.push(source);
                source = inner;
            }
            None => break,
        }
    }
    let mut task = clone_runtime_task_branch(source);
    while let Some(source) = spine.pop() {
        task = match source {
            RuntimeTaskPlan::Join { .. } => RuntimeTaskPlan::Join {
                outer: Box::new(task),
            },
            RuntimeTaskPlan::Map { function, .. } => RuntimeTaskPlan::Map {
                function: Box::new(function.as_ref().clone()),
                inner: Box::new(task),
            },
            RuntimeTaskPlan::Chain { function, .. } => RuntimeTaskPlan::Chain {
                function: Box::new(function.as_ref().clone()),
                inner: Box::new(task),
            },
            RuntimeTaskPlan::Pure { .. } => RuntimeTaskPlan::Pure {
                value: Box::new(RuntimeValue::Task(task)),
            },
            _ => unreachable!("task spine contains only unary composition"),
        };
    }
    task
}

fn clone_runtime_task_branch(source: &RuntimeTaskPlan) -> RuntimeTaskPlan {
    if let RuntimeTaskPlan::Pure { value } = source {
        return RuntimeTaskPlan::Pure {
            value: Box::new(value.as_ref().clone()),
        };
    }
    if let Some(task) = clone_runtime_task_leaf(source) {
        return task;
    }
    let mut destination = RuntimeTaskPlan::TimeNowMs;
    run_runtime_clone(RuntimeCloneInto::Task {
        source,
        destination: &mut destination,
    });
    destination
}

#[inline]
fn clone_runtime_scalar(value: &RuntimeValue) -> Option<RuntimeValue> {
    Some(match value {
        RuntimeValue::Unit => RuntimeValue::Unit,
        RuntimeValue::Bool(value) => RuntimeValue::Bool(*value),
        RuntimeValue::Int(value) => RuntimeValue::Int(*value),
        RuntimeValue::Float(value) => RuntimeValue::Float(*value),
        RuntimeValue::Decimal(value) => RuntimeValue::Decimal(value.clone()),
        RuntimeValue::BigInt(value) => RuntimeValue::BigInt(value.clone()),
        RuntimeValue::Text(value) => RuntimeValue::Text(value.clone()),
        RuntimeValue::Bytes(value) => RuntimeValue::Bytes(value.clone()),
        RuntimeValue::OptionNone => RuntimeValue::OptionNone,
        RuntimeValue::SuffixedInteger { raw, suffix } => RuntimeValue::SuffixedInteger {
            raw: raw.clone(),
            suffix: suffix.clone(),
        },
        RuntimeValue::Tuple(_)
        | RuntimeValue::List(_)
        | RuntimeValue::Map(_)
        | RuntimeValue::Set(_)
        | RuntimeValue::Record(_)
        | RuntimeValue::Sum(_)
        | RuntimeValue::OptionSome(_)
        | RuntimeValue::ResultOk(_)
        | RuntimeValue::ResultErr(_)
        | RuntimeValue::ValidationValid(_)
        | RuntimeValue::ValidationInvalid(_)
        | RuntimeValue::Signal(_)
        | RuntimeValue::Task(_)
        | RuntimeValue::Callable(_) => return None,
    })
}

fn clone_runtime_task_leaf(task: &RuntimeTaskPlan) -> Option<RuntimeTaskPlan> {
    Some(match task {
        RuntimeTaskPlan::RandomInt { low, high } => RuntimeTaskPlan::RandomInt {
            low: *low,
            high: *high,
        },
        RuntimeTaskPlan::RandomBytes { count } => RuntimeTaskPlan::RandomBytes { count: *count },
        RuntimeTaskPlan::StdoutWrite { text } => {
            RuntimeTaskPlan::StdoutWrite { text: text.clone() }
        }
        RuntimeTaskPlan::StderrWrite { text } => {
            RuntimeTaskPlan::StderrWrite { text: text.clone() }
        }
        RuntimeTaskPlan::FsWriteText { path, text } => RuntimeTaskPlan::FsWriteText {
            path: path.clone(),
            text: text.clone(),
        },
        RuntimeTaskPlan::FsWriteBytes { path, bytes } => RuntimeTaskPlan::FsWriteBytes {
            path: path.clone(),
            bytes: bytes.clone(),
        },
        RuntimeTaskPlan::FsCreateDirAll { path } => {
            RuntimeTaskPlan::FsCreateDirAll { path: path.clone() }
        }
        RuntimeTaskPlan::FsDeleteFile { path } => {
            RuntimeTaskPlan::FsDeleteFile { path: path.clone() }
        }
        RuntimeTaskPlan::FsReadText { path } => RuntimeTaskPlan::FsReadText { path: path.clone() },
        RuntimeTaskPlan::FsReadDir { path } => RuntimeTaskPlan::FsReadDir { path: path.clone() },
        RuntimeTaskPlan::FsExists { path } => RuntimeTaskPlan::FsExists { path: path.clone() },
        RuntimeTaskPlan::FsReadBytes { path } => {
            RuntimeTaskPlan::FsReadBytes { path: path.clone() }
        }
        RuntimeTaskPlan::FsRename { from, to } => RuntimeTaskPlan::FsRename {
            from: from.clone(),
            to: to.clone(),
        },
        RuntimeTaskPlan::FsCopy { from, to } => RuntimeTaskPlan::FsCopy {
            from: from.clone(),
            to: to.clone(),
        },
        RuntimeTaskPlan::FsDeleteDir { path } => {
            RuntimeTaskPlan::FsDeleteDir { path: path.clone() }
        }
        RuntimeTaskPlan::JsonValidate { json } => {
            RuntimeTaskPlan::JsonValidate { json: json.clone() }
        }
        RuntimeTaskPlan::JsonGet { json, key } => RuntimeTaskPlan::JsonGet {
            json: json.clone(),
            key: key.clone(),
        },
        RuntimeTaskPlan::JsonAt { json, index } => RuntimeTaskPlan::JsonAt {
            json: json.clone(),
            index: *index,
        },
        RuntimeTaskPlan::JsonKeys { json } => RuntimeTaskPlan::JsonKeys { json: json.clone() },
        RuntimeTaskPlan::JsonPretty { json } => RuntimeTaskPlan::JsonPretty { json: json.clone() },
        RuntimeTaskPlan::JsonMinify { json } => RuntimeTaskPlan::JsonMinify { json: json.clone() },
        RuntimeTaskPlan::EnvGet { name } => RuntimeTaskPlan::EnvGet { name: name.clone() },
        RuntimeTaskPlan::EnvList { prefix } => RuntimeTaskPlan::EnvList {
            prefix: prefix.clone(),
        },
        RuntimeTaskPlan::LogEmit { level, message } => RuntimeTaskPlan::LogEmit {
            level: level.clone(),
            message: message.clone(),
        },
        RuntimeTaskPlan::LogEmitContext {
            level,
            message,
            context,
        } => RuntimeTaskPlan::LogEmitContext {
            level: level.clone(),
            message: message.clone(),
            context: context.clone(),
        },
        RuntimeTaskPlan::RegexIsMatch { pattern, text } => RuntimeTaskPlan::RegexIsMatch {
            pattern: pattern.clone(),
            text: text.clone(),
        },
        RuntimeTaskPlan::RegexFind { pattern, text } => RuntimeTaskPlan::RegexFind {
            pattern: pattern.clone(),
            text: text.clone(),
        },
        RuntimeTaskPlan::RegexFindText { pattern, text } => RuntimeTaskPlan::RegexFindText {
            pattern: pattern.clone(),
            text: text.clone(),
        },
        RuntimeTaskPlan::RegexFindAll { pattern, text } => RuntimeTaskPlan::RegexFindAll {
            pattern: pattern.clone(),
            text: text.clone(),
        },
        RuntimeTaskPlan::RegexReplace {
            pattern,
            replacement,
            text,
        } => RuntimeTaskPlan::RegexReplace {
            pattern: pattern.clone(),
            replacement: replacement.clone(),
            text: text.clone(),
        },
        RuntimeTaskPlan::RegexReplaceAll {
            pattern,
            replacement,
            text,
        } => RuntimeTaskPlan::RegexReplaceAll {
            pattern: pattern.clone(),
            replacement: replacement.clone(),
            text: text.clone(),
        },
        RuntimeTaskPlan::HttpGet { url } => RuntimeTaskPlan::HttpGet { url: url.clone() },
        RuntimeTaskPlan::HttpGetBytes { url } => RuntimeTaskPlan::HttpGetBytes { url: url.clone() },
        RuntimeTaskPlan::HttpGetStatus { url } => {
            RuntimeTaskPlan::HttpGetStatus { url: url.clone() }
        }
        RuntimeTaskPlan::HttpPost {
            url,
            content_type,
            body,
        } => RuntimeTaskPlan::HttpPost {
            url: url.clone(),
            content_type: content_type.clone(),
            body: body.clone(),
        },
        RuntimeTaskPlan::HttpPut {
            url,
            content_type,
            body,
        } => RuntimeTaskPlan::HttpPut {
            url: url.clone(),
            content_type: content_type.clone(),
            body: body.clone(),
        },
        RuntimeTaskPlan::HttpDelete { url } => RuntimeTaskPlan::HttpDelete { url: url.clone() },
        RuntimeTaskPlan::HttpHead { url } => RuntimeTaskPlan::HttpHead { url: url.clone() },
        RuntimeTaskPlan::HttpPostJson { url, body } => RuntimeTaskPlan::HttpPostJson {
            url: url.clone(),
            body: body.clone(),
        },
        RuntimeTaskPlan::SecretLookup {
            service,
            attributes,
        } => RuntimeTaskPlan::SecretLookup {
            service: service.clone(),
            attributes: attributes.clone(),
        },
        RuntimeTaskPlan::SecretStore {
            service,
            label,
            attributes,
            value,
        } => RuntimeTaskPlan::SecretStore {
            service: service.clone(),
            label: label.clone(),
            attributes: attributes.clone(),
            value: value.clone(),
        },
        RuntimeTaskPlan::SecretDelete {
            service,
            attributes,
        } => RuntimeTaskPlan::SecretDelete {
            service: service.clone(),
            attributes: attributes.clone(),
        },
        RuntimeTaskPlan::NotificationClose {
            app_name,
            id,
            bus,
            address,
        } => RuntimeTaskPlan::NotificationClose {
            app_name: app_name.clone(),
            id: *id,
            bus: bus.clone(),
            address: address.clone(),
        },
        RuntimeTaskPlan::TimeNowMs => RuntimeTaskPlan::TimeNowMs,
        RuntimeTaskPlan::TimeMonotonicMs => RuntimeTaskPlan::TimeMonotonicMs,
        RuntimeTaskPlan::RandomFloat => RuntimeTaskPlan::RandomFloat,
        RuntimeTaskPlan::Pure { .. }
        | RuntimeTaskPlan::Map { .. }
        | RuntimeTaskPlan::Apply { .. }
        | RuntimeTaskPlan::Chain { .. }
        | RuntimeTaskPlan::Join { .. }
        | RuntimeTaskPlan::DbusCall { .. }
        | RuntimeTaskPlan::NotificationSend { .. }
        | RuntimeTaskPlan::AuthPkce { .. }
        | RuntimeTaskPlan::AuthRefresh { .. }
        | RuntimeTaskPlan::Database(_)
        | RuntimeTaskPlan::CustomCapabilityCommand(_) => return None,
    })
}

fn callable_arguments(callable: &RuntimeCallable) -> &[RuntimeValue] {
    match callable {
        RuntimeCallable::ItemBody {
            bound_arguments, ..
        }
        | RuntimeCallable::BuiltinConstructor {
            bound_arguments, ..
        }
        | RuntimeCallable::SumConstructor {
            bound_arguments, ..
        }
        | RuntimeCallable::DomainMember {
            bound_arguments, ..
        }
        | RuntimeCallable::BuiltinClassMember {
            bound_arguments, ..
        }
        | RuntimeCallable::IntrinsicValue {
            bound_arguments, ..
        } => bound_arguments,
    }
}

fn enqueue_runtime_clone_children<'a>(
    source: RuntimeCloneNode<'a>,
    pending: &mut Vec<RuntimeCloneWork<'a>>,
) {
    let value = |value| RuntimeCloneWork::Visit(RuntimeCloneNode::Value(value));
    let task = |task| RuntimeCloneWork::Visit(RuntimeCloneNode::Task(task));
    match source {
        RuntimeCloneNode::Value(source) => match source {
            RuntimeValue::Tuple(values)
            | RuntimeValue::List(values)
            | RuntimeValue::Set(values) => pending.extend(values.iter().rev().map(value)),
            RuntimeValue::Map(map) => {
                for (key, entry) in map.iter().rev() {
                    pending.push(value(entry));
                    pending.push(value(key));
                }
            }
            RuntimeValue::Record(fields) => {
                pending.extend(fields.iter().rev().map(|field| value(&field.value)))
            }
            RuntimeValue::Sum(sum) => pending.extend(sum.fields.iter().rev().map(value)),
            RuntimeValue::Callable(callable) => {
                pending.extend(callable_arguments(callable).iter().rev().map(value))
            }
            RuntimeValue::OptionSome(inner)
            | RuntimeValue::ResultOk(inner)
            | RuntimeValue::ResultErr(inner)
            | RuntimeValue::ValidationValid(inner)
            | RuntimeValue::ValidationInvalid(inner)
            | RuntimeValue::Signal(inner) => pending.push(value(inner)),
            RuntimeValue::Task(inner) => pending.push(task(inner)),
            _ => unreachable!("scalar values are cloned before child traversal"),
        },
        RuntimeCloneNode::Task(source) => match source {
            RuntimeTaskPlan::Pure { value: inner } => pending.push(value(inner)),
            RuntimeTaskPlan::Map { function, inner }
            | RuntimeTaskPlan::Chain { function, inner } => {
                pending.push(task(inner));
                pending.push(value(function));
            }
            RuntimeTaskPlan::Apply {
                function_task,
                value_task,
            } => {
                pending.push(task(value_task));
                pending.push(task(function_task));
            }
            RuntimeTaskPlan::Join { outer } => pending.push(task(outer)),
            RuntimeTaskPlan::DbusCall { body, .. } => pending.extend(body.iter().rev().map(value)),
            RuntimeTaskPlan::NotificationSend { notification, .. } => {
                pending.push(value(notification))
            }
            RuntimeTaskPlan::AuthPkce { config } | RuntimeTaskPlan::AuthRefresh { config, .. } => {
                pending.push(value(config))
            }
            RuntimeTaskPlan::Database(RuntimeDbTaskPlan::Query(plan)) => {
                pending.extend(plan.statement.arguments.iter().rev().map(value))
            }
            RuntimeTaskPlan::Database(RuntimeDbTaskPlan::Commit(plan)) => {
                for statement in plan.statements.iter().rev() {
                    pending.extend(statement.arguments.iter().rev().map(value));
                }
            }
            RuntimeTaskPlan::CustomCapabilityCommand(plan) => {
                for argument in plan
                    .provider_arguments
                    .iter()
                    .chain(plan.options.iter())
                    .chain(plan.arguments.iter())
                    .rev()
                {
                    pending.push(value(&argument.value));
                }
            }
            _ => unreachable!("task leaves are cloned before child traversal"),
        },
    }
}

fn clone_runtime_value_fields(
    source: &RuntimeValue,
    fields: &mut impl RuntimeCloneFields,
) -> RuntimeValue {
    match source {
        RuntimeValue::Tuple(values) => RuntimeValue::Tuple(fields.values(values)),
        RuntimeValue::List(values) => RuntimeValue::List(fields.values(values)),
        RuntimeValue::Set(values) => RuntimeValue::Set(fields.values(values)),
        RuntimeValue::Map(map) => RuntimeValue::Map(RuntimeMap::from_entries(
            map.iter()
                .map(|(key, value)| RuntimeMapEntry {
                    key: fields.value(key),
                    value: fields.value(value),
                })
                .collect(),
        )),
        RuntimeValue::Record(values) => RuntimeValue::Record(
            values
                .iter()
                .map(|field| RuntimeRecordField {
                    label: field.label.clone(),
                    value: fields.value(&field.value),
                })
                .collect(),
        ),
        RuntimeValue::Sum(sum) => RuntimeValue::Sum(RuntimeSumValue {
            item: sum.item,
            type_name: sum.type_name.clone(),
            variant_name: sum.variant_name.clone(),
            fields: fields.values(&sum.fields),
        }),
        RuntimeValue::OptionSome(value) => RuntimeValue::OptionSome(Box::new(fields.value(value))),
        RuntimeValue::ResultOk(value) => RuntimeValue::ResultOk(Box::new(fields.value(value))),
        RuntimeValue::ResultErr(value) => RuntimeValue::ResultErr(Box::new(fields.value(value))),
        RuntimeValue::ValidationValid(value) => {
            RuntimeValue::ValidationValid(Box::new(fields.value(value)))
        }
        RuntimeValue::ValidationInvalid(value) => {
            RuntimeValue::ValidationInvalid(Box::new(fields.value(value)))
        }
        RuntimeValue::Signal(value) => RuntimeValue::Signal(Box::new(fields.value(value))),
        RuntimeValue::Task(task) => RuntimeValue::Task(fields.task(task)),
        RuntimeValue::Callable(callable) => RuntimeValue::Callable(match callable {
            RuntimeCallable::ItemBody {
                item,
                kernel,
                parameters,
                bound_arguments,
            } => RuntimeCallable::ItemBody {
                item: *item,
                kernel: *kernel,
                parameters: parameters.clone(),
                bound_arguments: fields.values(bound_arguments),
            },
            RuntimeCallable::BuiltinConstructor {
                constructor,
                bound_arguments,
            } => RuntimeCallable::BuiltinConstructor {
                constructor: constructor.clone(),
                bound_arguments: fields.values(bound_arguments),
            },
            RuntimeCallable::SumConstructor {
                handle,
                bound_arguments,
            } => RuntimeCallable::SumConstructor {
                handle: handle.clone(),
                bound_arguments: fields.values(bound_arguments),
            },
            RuntimeCallable::DomainMember {
                handle,
                parameters,
                result,
                bound_arguments,
            } => RuntimeCallable::DomainMember {
                handle: handle.clone(),
                parameters: parameters.clone(),
                result: *result,
                bound_arguments: fields.values(bound_arguments),
            },
            RuntimeCallable::BuiltinClassMember {
                intrinsic,
                bound_arguments,
            } => RuntimeCallable::BuiltinClassMember {
                intrinsic: intrinsic.clone(),
                bound_arguments: fields.values(bound_arguments),
            },
            RuntimeCallable::IntrinsicValue {
                value,
                bound_arguments,
            } => RuntimeCallable::IntrinsicValue {
                value: *value,
                bound_arguments: fields.values(bound_arguments),
            },
        }),
        _ => {
            clone_runtime_scalar(source).expect("all composite value variants have a clone builder")
        }
    }
}

fn clone_runtime_db_statement(
    source: &RuntimeDbStatement,
    fields: &mut impl RuntimeCloneFields,
) -> RuntimeDbStatement {
    RuntimeDbStatement {
        sql: source.sql.clone(),
        arguments: fields.values(&source.arguments),
    }
}

fn clone_runtime_named_values(
    source: &[RuntimeNamedValue],
    fields: &mut impl RuntimeCloneFields,
) -> Box<[RuntimeNamedValue]> {
    source
        .iter()
        .map(|argument| RuntimeNamedValue {
            name: argument.name.clone(),
            value: fields.value(&argument.value),
        })
        .collect()
}

fn clone_runtime_task_fields(
    source: &RuntimeTaskPlan,
    fields: &mut impl RuntimeCloneFields,
) -> RuntimeTaskPlan {
    match source {
        RuntimeTaskPlan::Pure { value } => RuntimeTaskPlan::Pure {
            value: Box::new(fields.value(value)),
        },
        RuntimeTaskPlan::Map { function, inner } => RuntimeTaskPlan::Map {
            function: Box::new(fields.value(function)),
            inner: Box::new(fields.task(inner)),
        },
        RuntimeTaskPlan::Chain { function, inner } => RuntimeTaskPlan::Chain {
            function: Box::new(fields.value(function)),
            inner: Box::new(fields.task(inner)),
        },
        RuntimeTaskPlan::Apply {
            function_task,
            value_task,
        } => RuntimeTaskPlan::Apply {
            function_task: Box::new(fields.task(function_task)),
            value_task: Box::new(fields.task(value_task)),
        },
        RuntimeTaskPlan::Join { outer } => RuntimeTaskPlan::Join {
            outer: Box::new(fields.task(outer)),
        },
        RuntimeTaskPlan::DbusCall {
            destination,
            path,
            interface,
            member,
            body,
            bus,
            address,
        } => RuntimeTaskPlan::DbusCall {
            destination: destination.clone(),
            path: path.clone(),
            interface: interface.clone(),
            member: member.clone(),
            body: fields.values(body).into_boxed_slice(),
            bus: bus.clone(),
            address: address.clone(),
        },
        RuntimeTaskPlan::NotificationSend {
            app_name,
            notification,
            bus,
            address,
        } => RuntimeTaskPlan::NotificationSend {
            app_name: app_name.clone(),
            notification: Box::new(fields.value(notification)),
            bus: bus.clone(),
            address: address.clone(),
        },
        RuntimeTaskPlan::AuthPkce { config } => RuntimeTaskPlan::AuthPkce {
            config: Box::new(fields.value(config)),
        },
        RuntimeTaskPlan::AuthRefresh {
            config,
            refresh_token,
        } => RuntimeTaskPlan::AuthRefresh {
            config: Box::new(fields.value(config)),
            refresh_token: refresh_token.clone(),
        },
        RuntimeTaskPlan::Database(plan) => RuntimeTaskPlan::Database(match plan {
            RuntimeDbTaskPlan::Query(plan) => RuntimeDbTaskPlan::Query(RuntimeDbQueryPlan {
                connection: plan.connection.clone(),
                statement: clone_runtime_db_statement(&plan.statement, fields),
            }),
            RuntimeDbTaskPlan::Commit(plan) => RuntimeDbTaskPlan::Commit(RuntimeDbCommitPlan {
                connection: plan.connection.clone(),
                statements: plan
                    .statements
                    .iter()
                    .map(|statement| clone_runtime_db_statement(statement, fields))
                    .collect(),
                changed_tables: plan.changed_tables.clone(),
            }),
        }),
        RuntimeTaskPlan::CustomCapabilityCommand(plan) => {
            RuntimeTaskPlan::CustomCapabilityCommand(RuntimeCustomCapabilityCommandPlan {
                provider_key: plan.provider_key.clone(),
                command: plan.command.clone(),
                provider_arguments: clone_runtime_named_values(&plan.provider_arguments, fields),
                options: clone_runtime_named_values(&plan.options, fields),
                arguments: clone_runtime_named_values(&plan.arguments, fields),
            })
        }
        _ => clone_runtime_task_leaf(source)
            .expect("all composite task variants have a clone builder"),
    }
}

// Fill uniquely owned child slots directly. This avoids a second value buffer
// and lets flat containers keep the same allocation count as their derived clone.
// Maps use postorder assembly because keys must be complete before insertion.
enum RuntimeCloneInto<'a> {
    Value {
        source: &'a RuntimeValue,
        destination: &'a mut RuntimeValue,
    },
    Task {
        source: &'a RuntimeTaskPlan,
        destination: &'a mut RuntimeTaskPlan,
    },
}

struct ShallowRuntimeClone {
    deferred: bool,
}

#[inline]
fn runtime_value_is_scalar(value: &RuntimeValue) -> bool {
    matches!(
        value,
        RuntimeValue::Unit
            | RuntimeValue::Bool(_)
            | RuntimeValue::Int(_)
            | RuntimeValue::Float(_)
            | RuntimeValue::Decimal(_)
            | RuntimeValue::BigInt(_)
            | RuntimeValue::Text(_)
            | RuntimeValue::Bytes(_)
            | RuntimeValue::OptionNone
            | RuntimeValue::SuffixedInteger { .. }
    )
}

impl RuntimeCloneFields for ShallowRuntimeClone {
    fn value(&mut self, source: &RuntimeValue) -> RuntimeValue {
        if runtime_value_is_scalar(source) {
            source.clone()
        } else {
            self.deferred = true;
            RuntimeValue::Unit
        }
    }

    fn task(&mut self, source: &RuntimeTaskPlan) -> RuntimeTaskPlan {
        clone_runtime_task_leaf(source).unwrap_or_else(|| {
            self.deferred = true;
            RuntimeTaskPlan::TimeNowMs
        })
    }
}

fn enqueue_value_copy<'a>(
    source: &'a RuntimeValue,
    destination: &'a mut RuntimeValue,
    pending: &mut Vec<RuntimeCloneInto<'a>>,
) {
    if !runtime_value_is_scalar(source) {
        pending.push(RuntimeCloneInto::Value {
            source,
            destination,
        });
    }
}

fn enqueue_value_copies<'a>(
    source: &'a [RuntimeValue],
    destination: &'a mut [RuntimeValue],
    pending: &mut Vec<RuntimeCloneInto<'a>>,
) {
    for (source, destination) in source.iter().zip(destination) {
        enqueue_value_copy(source, destination, pending);
    }
}

fn enqueue_task_copy<'a>(
    source: &'a RuntimeTaskPlan,
    destination: &'a mut RuntimeTaskPlan,
    pending: &mut Vec<RuntimeCloneInto<'a>>,
) {
    // Leaf plans have already been copied by ShallowRuntimeClone.
    if !runtime_task_is_leaf(source) {
        pending.push(RuntimeCloneInto::Task {
            source,
            destination,
        });
    }
}

#[inline]
fn runtime_task_is_leaf(source: &RuntimeTaskPlan) -> bool {
    !matches!(
        source,
        RuntimeTaskPlan::Pure { .. }
            | RuntimeTaskPlan::Map { .. }
            | RuntimeTaskPlan::Apply { .. }
            | RuntimeTaskPlan::Chain { .. }
            | RuntimeTaskPlan::Join { .. }
            | RuntimeTaskPlan::DbusCall { .. }
            | RuntimeTaskPlan::NotificationSend { .. }
            | RuntimeTaskPlan::AuthPkce { .. }
            | RuntimeTaskPlan::AuthRefresh { .. }
            | RuntimeTaskPlan::Database(_)
            | RuntimeTaskPlan::CustomCapabilityCommand(_)
    )
}

fn callable_arguments_mut(callable: &mut RuntimeCallable) -> &mut [RuntimeValue] {
    match callable {
        RuntimeCallable::ItemBody {
            bound_arguments, ..
        }
        | RuntimeCallable::BuiltinConstructor {
            bound_arguments, ..
        }
        | RuntimeCallable::SumConstructor {
            bound_arguments, ..
        }
        | RuntimeCallable::DomainMember {
            bound_arguments, ..
        }
        | RuntimeCallable::BuiltinClassMember {
            bound_arguments, ..
        }
        | RuntimeCallable::IntrinsicValue {
            bound_arguments, ..
        } => bound_arguments,
    }
}

fn fill_runtime_clone<'a>(
    work: RuntimeCloneInto<'a>,
    pending: &mut Vec<RuntimeCloneInto<'a>>,
) -> bool {
    match work {
        RuntimeCloneInto::Value {
            source,
            destination,
        } => {
            if let RuntimeValue::Map(map) = source {
                let shallow = map.iter().all(|(key, value)| {
                    runtime_value_is_scalar(key) && runtime_value_is_scalar(value)
                });
                *destination = if shallow {
                    RuntimeValue::Map(map.clone())
                } else {
                    match clone_runtime_tree(RuntimeCloneNode::Value(source)) {
                        RuntimeCloneOutput::Value(value) => value,
                        RuntimeCloneOutput::Task(_) => unreachable!("map clone has a value root"),
                    }
                };
                return !shallow;
            }
            let mut fields = ShallowRuntimeClone { deferred: false };
            *destination = clone_runtime_value_fields(source, &mut fields);
            if !fields.deferred {
                return false;
            }
            match (source, destination) {
                (RuntimeValue::Tuple(source), RuntimeValue::Tuple(destination))
                | (RuntimeValue::List(source), RuntimeValue::List(destination))
                | (RuntimeValue::Set(source), RuntimeValue::Set(destination)) => {
                    enqueue_value_copies(source, destination, pending)
                }
                (RuntimeValue::Record(source), RuntimeValue::Record(destination)) => {
                    for (source, destination) in source.iter().zip(destination) {
                        enqueue_value_copy(&source.value, &mut destination.value, pending);
                    }
                }
                (RuntimeValue::Sum(source), RuntimeValue::Sum(destination)) => {
                    enqueue_value_copies(&source.fields, &mut destination.fields, pending)
                }
                (RuntimeValue::Callable(source), RuntimeValue::Callable(destination)) => {
                    enqueue_value_copies(
                        callable_arguments(source),
                        callable_arguments_mut(destination),
                        pending,
                    )
                }
                (RuntimeValue::OptionSome(source), RuntimeValue::OptionSome(destination))
                | (RuntimeValue::ResultOk(source), RuntimeValue::ResultOk(destination))
                | (RuntimeValue::ResultErr(source), RuntimeValue::ResultErr(destination))
                | (
                    RuntimeValue::ValidationValid(source),
                    RuntimeValue::ValidationValid(destination),
                )
                | (
                    RuntimeValue::ValidationInvalid(source),
                    RuntimeValue::ValidationInvalid(destination),
                )
                | (RuntimeValue::Signal(source), RuntimeValue::Signal(destination)) => {
                    enqueue_value_copy(source, destination, pending)
                }
                (RuntimeValue::Task(source), RuntimeValue::Task(destination)) => {
                    enqueue_task_copy(source, destination, pending)
                }
                _ => debug_assert!(runtime_value_is_scalar(source)),
            }
        }
        RuntimeCloneInto::Task {
            source,
            destination,
        } => {
            let mut fields = ShallowRuntimeClone { deferred: false };
            *destination = clone_runtime_task_fields(source, &mut fields);
            if !fields.deferred {
                return false;
            }
            match (source, destination) {
                (
                    RuntimeTaskPlan::Pure { value: source },
                    RuntimeTaskPlan::Pure { value: destination },
                ) => enqueue_value_copy(source, destination, pending),
                (
                    RuntimeTaskPlan::Map {
                        function: sf,
                        inner: si,
                    },
                    RuntimeTaskPlan::Map {
                        function: df,
                        inner: di,
                    },
                )
                | (
                    RuntimeTaskPlan::Chain {
                        function: sf,
                        inner: si,
                    },
                    RuntimeTaskPlan::Chain {
                        function: df,
                        inner: di,
                    },
                ) => {
                    enqueue_value_copy(sf, df, pending);
                    enqueue_task_copy(si, di, pending);
                }
                (
                    RuntimeTaskPlan::Apply {
                        function_task: sf,
                        value_task: sv,
                    },
                    RuntimeTaskPlan::Apply {
                        function_task: df,
                        value_task: dv,
                    },
                ) => {
                    enqueue_task_copy(sf, df, pending);
                    enqueue_task_copy(sv, dv, pending);
                }
                (
                    RuntimeTaskPlan::Join { outer: source },
                    RuntimeTaskPlan::Join { outer: destination },
                ) => enqueue_task_copy(source, destination, pending),
                (
                    RuntimeTaskPlan::DbusCall { body: source, .. },
                    RuntimeTaskPlan::DbusCall {
                        body: destination, ..
                    },
                ) => enqueue_value_copies(source, destination, pending),
                (
                    RuntimeTaskPlan::NotificationSend {
                        notification: source,
                        ..
                    },
                    RuntimeTaskPlan::NotificationSend {
                        notification: destination,
                        ..
                    },
                ) => enqueue_value_copy(source, destination, pending),
                (
                    RuntimeTaskPlan::AuthPkce { config: source },
                    RuntimeTaskPlan::AuthPkce {
                        config: destination,
                    },
                )
                | (
                    RuntimeTaskPlan::AuthRefresh { config: source, .. },
                    RuntimeTaskPlan::AuthRefresh {
                        config: destination,
                        ..
                    },
                ) => enqueue_value_copy(source, destination, pending),
                (
                    RuntimeTaskPlan::Database(RuntimeDbTaskPlan::Query(source)),
                    RuntimeTaskPlan::Database(RuntimeDbTaskPlan::Query(destination)),
                ) => enqueue_value_copies(
                    &source.statement.arguments,
                    &mut destination.statement.arguments,
                    pending,
                ),
                (
                    RuntimeTaskPlan::Database(RuntimeDbTaskPlan::Commit(source)),
                    RuntimeTaskPlan::Database(RuntimeDbTaskPlan::Commit(destination)),
                ) => {
                    for (source, destination) in
                        source.statements.iter().zip(&mut destination.statements)
                    {
                        enqueue_value_copies(
                            &source.arguments,
                            &mut destination.arguments,
                            pending,
                        );
                    }
                }
                (
                    RuntimeTaskPlan::CustomCapabilityCommand(source),
                    RuntimeTaskPlan::CustomCapabilityCommand(destination),
                ) => {
                    for (source, destination) in source
                        .provider_arguments
                        .iter()
                        .chain(source.options.iter())
                        .chain(source.arguments.iter())
                        .zip(
                            destination
                                .provider_arguments
                                .iter_mut()
                                .chain(destination.options.iter_mut())
                                .chain(destination.arguments.iter_mut()),
                        )
                    {
                        enqueue_value_copy(&source.value, &mut destination.value, pending);
                    }
                }
                _ => debug_assert!(runtime_task_is_leaf(source)),
            }
        }
    }
    true
}

fn run_runtime_clone(work: RuntimeCloneInto<'_>) -> bool {
    let mut pending = Vec::new();
    let deferred = fill_runtime_clone(work, &mut pending);
    while let Some(work) = pending.pop() {
        fill_runtime_clone(work, &mut pending);
    }
    deferred
}

#[inline(always)]
fn clone_runtime_snapshot(source: &RuntimeValue) -> DetachedRuntimeValue {
    let value = match source {
        RuntimeValue::Unit => RuntimeValue::Unit,
        RuntimeValue::Bool(value) => RuntimeValue::Bool(*value),
        RuntimeValue::Int(value) => RuntimeValue::Int(*value),
        RuntimeValue::Float(value) => RuntimeValue::Float(*value),
        RuntimeValue::OptionNone => RuntimeValue::OptionNone,
        _ => return clone_runtime_composite_snapshot(source),
    };
    DetachedRuntimeValue {
        value,
        iterative_drop: false,
    }
}

// Keep the allocation-free leaf dispatch small enough to inline at boundary
// call sites. Owning scalar payloads and trees share the remaining copy path.
fn clone_runtime_composite_snapshot(source: &RuntimeValue) -> DetachedRuntimeValue {
    if let Some(value) = clone_runtime_scalar(source) {
        return DetachedRuntimeValue {
            value,
            iterative_drop: false,
        };
    }
    if let RuntimeValue::Map(map) = source
        && map
            .iter()
            .all(|(key, value)| runtime_value_is_scalar(key) && runtime_value_is_scalar(value))
    {
        return DetachedRuntimeValue {
            value: RuntimeValue::Map(map.clone()),
            iterative_drop: false,
        };
    }
    if let RuntimeValue::Task(task) = source {
        return DetachedRuntimeValue {
            value: RuntimeValue::Task(clone_runtime_task(task)),
            iterative_drop: true,
        };
    }
    let mut value = RuntimeValue::Unit;
    let iterative_drop = run_runtime_clone(RuntimeCloneInto::Value {
        source,
        destination: &mut value,
    });
    DetachedRuntimeValue {
        value,
        iterative_drop,
    }
}

#[cfg(test)]
mod clone_tests {
    use super::*;

    fn nested_value(depth: usize) -> RuntimeValue {
        let mut value = RuntimeValue::Int(17);
        for index in 0..depth {
            value = match index % 12 {
                0 => RuntimeValue::List(vec![RuntimeValue::Int(index as i64), value]),
                1 => RuntimeValue::OptionSome(Box::new(value)),
                2 => RuntimeValue::Task(RuntimeTaskPlan::Pure {
                    value: Box::new(value),
                }),
                3 => RuntimeValue::Record(vec![RuntimeRecordField {
                    label: "value".into(),
                    value,
                }]),
                4 => RuntimeValue::Callable(RuntimeCallable::BuiltinConstructor {
                    constructor: RuntimeConstructor::Some,
                    bound_arguments: vec![value],
                }),
                5 => RuntimeValue::Map(RuntimeMap::from_entries(vec![RuntimeMapEntry {
                    key: RuntimeValue::Int(index as i64),
                    value,
                }])),
                6 => RuntimeValue::Tuple(vec![value]),
                7 => RuntimeValue::Set(vec![value]),
                8 => RuntimeValue::ResultOk(Box::new(value)),
                9 => RuntimeValue::ValidationInvalid(Box::new(value)),
                10 => RuntimeValue::Signal(Box::new(value)),
                _ => RuntimeValue::Sum(RuntimeSumValue {
                    item: HirItemId::from_raw(2),
                    type_name: "Nested".into(),
                    variant_name: "Node".into(),
                    fields: vec![value],
                }),
            };
        }
        value
    }

    fn check_nested_value(mut value: &RuntimeValue, depth: usize) {
        for index in (0..depth).rev() {
            value = match (index % 12, value) {
                (0, RuntimeValue::List(values)) => {
                    assert_eq!(values[0], RuntimeValue::Int(index as i64));
                    &values[1]
                }
                (1, RuntimeValue::OptionSome(value))
                | (8, RuntimeValue::ResultOk(value))
                | (9, RuntimeValue::ValidationInvalid(value))
                | (10, RuntimeValue::Signal(value)) => value,
                (2, RuntimeValue::Task(RuntimeTaskPlan::Pure { value })) => value,
                (3, RuntimeValue::Record(fields)) => {
                    assert_eq!(&*fields[0].label, "value");
                    &fields[0].value
                }
                (4, RuntimeValue::Callable(callable)) => &callable_arguments(callable)[0],
                (5, RuntimeValue::Map(map)) => map.get(&RuntimeValue::Int(index as i64)).unwrap(),
                (6, RuntimeValue::Tuple(values)) | (7, RuntimeValue::Set(values)) => &values[0],
                (11, RuntimeValue::Sum(sum)) => {
                    assert_eq!(&*sum.variant_name, "Node");
                    &sum.fields[0]
                }
                _ => panic!("cloning changed the value shape at depth {index}"),
            };
        }
        assert_eq!(*value, RuntimeValue::Int(17));
    }

    #[test]
    fn clone_deep_mixed_values_and_detached_snapshots_on_a_small_stack() {
        std::thread::Builder::new()
            .stack_size(256 * 1024)
            .spawn(|| {
                let original = DetachedRuntimeValue::from_runtime_owned(nested_value(20_000));
                let copy = original.clone();
                check_nested_value(copy.as_runtime(), 20_000);
                let transferred = copy.into_runtime();
                check_nested_value(&transferred, 20_000);
                transferred.discard();
                let copy = original.to_runtime();
                check_nested_value(&copy, 20_000);
                copy.discard();
                drop(original); // boundary teardown must also be iterative
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn clone_lawful_join_pure_task_layers_on_a_small_stack() {
        std::thread::Builder::new().stack_size(256 * 1024).spawn(|| {
            let mut original = RuntimeTaskPlan::Pure { value: Box::new(RuntimeValue::Int(1)) };
            for _ in 0..10_000 {
                original = RuntimeTaskPlan::Join { outer: Box::new(RuntimeTaskPlan::Pure {
                    value: Box::new(RuntimeValue::Task(original)),
                }) };
            }
            let copy = original.clone();
            let mut cursor = &copy;
            for _ in 0..10_000 {
                let RuntimeTaskPlan::Join { outer } = cursor else { panic!("missing Join") };
                let RuntimeTaskPlan::Pure { value } = &**outer else { panic!("missing Pure") };
                let RuntimeValue::Task(inner) = &**value else { panic!("missing Task") };
                cursor = inner;
            }
            assert!(matches!(cursor, RuntimeTaskPlan::Pure { value } if **value == RuntimeValue::Int(1)));
            RuntimeValue::Task(copy).discard();
            RuntimeValue::Task(original).discard();
        }).unwrap().join().unwrap();
    }

    #[test]
    fn clone_map_keys_without_inserting_unfinished_keys() {
        std::thread::Builder::new()
            .stack_size(256 * 1024)
            .spawn(|| {
                let mut key = RuntimeValue::Int(91);
                for _ in 0..128 {
                    key = RuntimeValue::OptionSome(Box::new(key));
                }
                // Keep construction within the existing recursive key-hashing depth;
                // the clone must preserve the complete key before inserting it.
                let source = DetachedRuntimeValue::from_runtime_owned(RuntimeValue::Map(
                    RuntimeMap::from_entries(vec![RuntimeMapEntry {
                        key,
                        value: RuntimeValue::Int(5),
                    }]),
                ));
                let copy = source.clone();
                let RuntimeValue::Map(map) = copy.as_runtime() else {
                    panic!("missing Map")
                };
                let (mut cursor, value) = map.iter().next().unwrap();
                assert_eq!(*value, RuntimeValue::Int(5));
                for _ in 0..128 {
                    let RuntimeValue::OptionSome(inner) = cursor else {
                        panic!("missing key layer")
                    };
                    cursor = inner;
                }
                assert_eq!(*cursor, RuntimeValue::Int(91));
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn cloning_preserves_branch_order_metadata_and_wire_representation() {
        let value = nested_value(48);
        let copy = value.clone();
        assert_eq!(copy, value);
        assert_eq!(
            postcard::to_stdvec(&copy).unwrap(),
            postcard::to_stdvec(&value).unwrap()
        );
        let branch = RuntimeTaskPlan::Apply {
            function_task: Box::new(RuntimeTaskPlan::Map {
                function: Box::new(RuntimeValue::Text("first".into())),
                inner: Box::new(RuntimeTaskPlan::Pure {
                    value: Box::new(value),
                }),
            }),
            value_task: Box::new(RuntimeTaskPlan::Chain {
                function: Box::new(RuntimeValue::Text("second".into())),
                inner: Box::new(RuntimeTaskPlan::Pure {
                    value: Box::new(RuntimeValue::Int(29)),
                }),
            }),
        };
        let copy = branch.clone();
        assert_eq!(copy, branch);
        assert_eq!(
            postcard::to_stdvec(&copy).unwrap(),
            postcard::to_stdvec(&branch).unwrap()
        );
        RuntimeValue::Task(branch).discard();
        RuntimeValue::Task(copy).discard();
    }
    #[test]
    fn every_task_leaf_preserves_metadata_and_wire_representation() {
        for task in [
            RuntimeTaskPlan::RandomInt { low: 1, high: 7 },
            RuntimeTaskPlan::RandomBytes { count: 7 },
            RuntimeTaskPlan::StdoutWrite {
                text: "StdoutWrite.text".into(),
            },
            RuntimeTaskPlan::StderrWrite {
                text: "StderrWrite.text".into(),
            },
            RuntimeTaskPlan::FsWriteText {
                path: "FsWriteText.path".into(),
                text: "FsWriteText.text".into(),
            },
            RuntimeTaskPlan::FsWriteBytes {
                path: "FsWriteBytes.path".into(),
                bytes: vec![1, 2, 3].into_boxed_slice(),
            },
            RuntimeTaskPlan::FsCreateDirAll {
                path: "FsCreateDirAll.path".into(),
            },
            RuntimeTaskPlan::FsDeleteFile {
                path: "FsDeleteFile.path".into(),
            },
            RuntimeTaskPlan::FsReadText {
                path: "FsReadText.path".into(),
            },
            RuntimeTaskPlan::FsReadDir {
                path: "FsReadDir.path".into(),
            },
            RuntimeTaskPlan::FsExists {
                path: "FsExists.path".into(),
            },
            RuntimeTaskPlan::FsReadBytes {
                path: "FsReadBytes.path".into(),
            },
            RuntimeTaskPlan::FsRename {
                from: "FsRename.from".into(),
                to: "FsRename.to".into(),
            },
            RuntimeTaskPlan::FsCopy {
                from: "FsCopy.from".into(),
                to: "FsCopy.to".into(),
            },
            RuntimeTaskPlan::FsDeleteDir {
                path: "FsDeleteDir.path".into(),
            },
            RuntimeTaskPlan::JsonValidate {
                json: "JsonValidate.json".into(),
            },
            RuntimeTaskPlan::JsonGet {
                json: "JsonGet.json".into(),
                key: "JsonGet.key".into(),
            },
            RuntimeTaskPlan::JsonAt {
                json: "JsonAt.json".into(),
                index: 7,
            },
            RuntimeTaskPlan::JsonKeys {
                json: "JsonKeys.json".into(),
            },
            RuntimeTaskPlan::JsonPretty {
                json: "JsonPretty.json".into(),
            },
            RuntimeTaskPlan::JsonMinify {
                json: "JsonMinify.json".into(),
            },
            RuntimeTaskPlan::EnvGet {
                name: "EnvGet.name".into(),
            },
            RuntimeTaskPlan::EnvList {
                prefix: "EnvList.prefix".into(),
            },
            RuntimeTaskPlan::LogEmit {
                level: "LogEmit.level".into(),
                message: "LogEmit.message".into(),
            },
            RuntimeTaskPlan::LogEmitContext {
                level: "LogEmitContext.level".into(),
                message: "LogEmitContext.message".into(),
                context: vec![("context.key".into(), "context.value".into())].into_boxed_slice(),
            },
            RuntimeTaskPlan::RegexIsMatch {
                pattern: "RegexIsMatch.pattern".into(),
                text: "RegexIsMatch.text".into(),
            },
            RuntimeTaskPlan::RegexFind {
                pattern: "RegexFind.pattern".into(),
                text: "RegexFind.text".into(),
            },
            RuntimeTaskPlan::RegexFindText {
                pattern: "RegexFindText.pattern".into(),
                text: "RegexFindText.text".into(),
            },
            RuntimeTaskPlan::RegexFindAll {
                pattern: "RegexFindAll.pattern".into(),
                text: "RegexFindAll.text".into(),
            },
            RuntimeTaskPlan::RegexReplace {
                pattern: "RegexReplace.pattern".into(),
                replacement: "RegexReplace.replacement".into(),
                text: "RegexReplace.text".into(),
            },
            RuntimeTaskPlan::RegexReplaceAll {
                pattern: "RegexReplaceAll.pattern".into(),
                replacement: "RegexReplaceAll.replacement".into(),
                text: "RegexReplaceAll.text".into(),
            },
            RuntimeTaskPlan::HttpGet {
                url: "HttpGet.url".into(),
            },
            RuntimeTaskPlan::HttpGetBytes {
                url: "HttpGetBytes.url".into(),
            },
            RuntimeTaskPlan::HttpGetStatus {
                url: "HttpGetStatus.url".into(),
            },
            RuntimeTaskPlan::HttpPost {
                url: "HttpPost.url".into(),
                content_type: "HttpPost.content_type".into(),
                body: "HttpPost.body".into(),
            },
            RuntimeTaskPlan::HttpPut {
                url: "HttpPut.url".into(),
                content_type: "HttpPut.content_type".into(),
                body: "HttpPut.body".into(),
            },
            RuntimeTaskPlan::HttpDelete {
                url: "HttpDelete.url".into(),
            },
            RuntimeTaskPlan::HttpHead {
                url: "HttpHead.url".into(),
            },
            RuntimeTaskPlan::HttpPostJson {
                url: "HttpPostJson.url".into(),
                body: "HttpPostJson.body".into(),
            },
            RuntimeTaskPlan::SecretLookup {
                service: "SecretLookup.service".into(),
                attributes: vec![("attributes.key".into(), "attributes.value".into())]
                    .into_boxed_slice(),
            },
            RuntimeTaskPlan::SecretStore {
                service: "SecretStore.service".into(),
                label: "SecretStore.label".into(),
                attributes: vec![("attributes.key".into(), "attributes.value".into())]
                    .into_boxed_slice(),
                value: "SecretStore.value".into(),
            },
            RuntimeTaskPlan::SecretDelete {
                service: "SecretDelete.service".into(),
                attributes: vec![("attributes.key".into(), "attributes.value".into())]
                    .into_boxed_slice(),
            },
            RuntimeTaskPlan::NotificationClose {
                app_name: "NotificationClose.app_name".into(),
                id: 7,
                bus: "NotificationClose.bus".into(),
                address: "NotificationClose.address".into(),
            },
            RuntimeTaskPlan::TimeNowMs,
            RuntimeTaskPlan::TimeMonotonicMs,
            RuntimeTaskPlan::RandomFloat,
        ] {
            let copy = task.clone();
            assert_eq!(copy, task);
            assert_eq!(
                postcard::to_stdvec(&copy).unwrap(),
                postcard::to_stdvec(&task).unwrap()
            );
        }
    }

    #[test]
    fn snapshot_disposal_metadata_is_private_and_wire_compatible() {
        for value in [
            RuntimeValue::Int(3),
            RuntimeValue::OptionSome(Box::new(RuntimeValue::List(vec![
                RuntimeValue::Text("snapshot".into()),
                RuntimeValue::Int(7),
            ]))),
        ] {
            let copied = DetachedRuntimeValue::from_runtime_copy(&value);
            let owned = DetachedRuntimeValue::from_runtime_owned(value.clone());
            let bytes = postcard::to_stdvec(&value).unwrap();
            assert_eq!(postcard::to_stdvec(&copied).unwrap(), bytes);
            assert_eq!(postcard::to_stdvec(&owned).unwrap(), bytes);
            let decoded: DetachedRuntimeValue = postcard::from_bytes(&bytes).unwrap();
            assert!(decoded.iterative_drop);
            assert_eq!(decoded, copied);
            assert_eq!(owned, copied);
            assert_eq!(format!("{owned:?}"), format!("{copied:?}"));
            assert_eq!(
                serde_json::to_value(&copied).unwrap(),
                serde_json::to_value(&value).unwrap()
            );
            let decoded: DetachedRuntimeValue =
                serde_json::from_value(serde_json::to_value(&value).unwrap()).unwrap();
            assert!(decoded.iterative_drop);
            assert_eq!(decoded, copied);
            value.discard();
        }
    }

    #[test]
    fn evaluator_cache_releases_deep_task_values_on_a_small_stack() {
        std::thread::Builder::new()
            .stack_size(256 * 1024)
            .spawn(|| {
                let program = crate::Program::default();
                let mut evaluator = KernelEvaluator::new(&program);
                evaluator.item_cache.insert(
                    ItemId::from_raw(0),
                    DetachedRuntimeValue::from_runtime_owned(nested_value(20_000)),
                );
                let value = DetachedRuntimeValue::from_runtime_owned(nested_value(20_000));
                evaluator.last_kernel_call = Some(LastKernelCall {
                    kernel_id: KernelId::from_raw(0),
                    input_subject: Some(value.clone()),
                    environment: vec![value.clone()].into_boxed_slice(),
                    result: value,
                    result_layout: LayoutId::from_raw(0),
                });
                evaluator.last_kernel_call = None;
                drop(evaluator);
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn task_payloads_preserve_all_argument_groups_and_metadata() {
        let statement = |sql: &str, number| RuntimeDbStatement {
            sql: sql.into(),
            arguments: vec![nested_value(4), RuntimeValue::Int(number)],
        };
        let named = |name: &str| {
            vec![RuntimeNamedValue {
                name: name.into(),
                value: nested_value(4),
            }]
            .into_boxed_slice()
        };
        for task in [
            RuntimeTaskPlan::Database(RuntimeDbTaskPlan::Query(RuntimeDbQueryPlan {
                connection: RuntimeDbConnection {
                    database: "query.db".into(),
                },
                statement: statement("SELECT ?", 1),
            })),
            RuntimeTaskPlan::Database(RuntimeDbTaskPlan::Commit(RuntimeDbCommitPlan {
                connection: RuntimeDbConnection {
                    database: "commit.db".into(),
                },
                statements: vec![statement("INSERT ?", 2), statement("UPDATE ?", 3)],
                changed_tables: BTreeSet::from(["first".into(), "second".into()]),
            })),
            RuntimeTaskPlan::CustomCapabilityCommand(RuntimeCustomCapabilityCommandPlan {
                provider_key: "provider".into(),
                command: "command".into(),
                provider_arguments: named("provider-argument"),
                options: named("option"),
                arguments: named("argument"),
            }),
            RuntimeTaskPlan::DbusCall {
                destination: "destination".into(),
                path: "/path".into(),
                interface: "interface".into(),
                member: "member".into(),
                body: vec![nested_value(4), RuntimeValue::Int(4)].into_boxed_slice(),
                bus: "session".into(),
                address: "address".into(),
            },
            RuntimeTaskPlan::NotificationSend {
                app_name: "app".into(),
                notification: Box::new(nested_value(4)),
                bus: "session".into(),
                address: "address".into(),
            },
            RuntimeTaskPlan::AuthPkce {
                config: Box::new(nested_value(4)),
            },
            RuntimeTaskPlan::AuthRefresh {
                config: Box::new(nested_value(4)),
                refresh_token: "refresh".into(),
            },
        ] {
            let copy = task.clone();
            assert_eq!(copy, task);
            assert_eq!(
                postcard::to_stdvec(&copy).unwrap(),
                postcard::to_stdvec(&task).unwrap()
            );
            RuntimeValue::Task(copy).discard();
            RuntimeValue::Task(task).discard();
        }
    }
}
