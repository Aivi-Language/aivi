use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{Datelike, NaiveDate, TimeZone as ChronoTimeZone};
use parking_lot::Mutex as ParkingMutex;
use regex::RegexBuilder;
use url::Url;

use crate::hir::{HirExpr, HirProgram};
use crate::i18n::{parse_message_template, validate_key_text, MessagePart};
use crate::AiviError;

mod builtins;
mod constructors;
pub(crate) mod environment;
mod http;
pub(crate) mod json_schema;
pub(crate) mod snapshot;
pub(crate) mod values;

use self::builtins::register_builtins;
use self::constructors::{core_constructor_ordinals, insert_constructor_ordinal};
use self::environment::{Env, RuntimeContext};
use self::values::{
    BuiltinImpl, BuiltinValue, EffectValue, SourceValue, ThunkFunc, ThunkValue, Value,
};

pub use self::constructors::{TestFailure, TestReport, TestSuccess};
pub(crate) use self::constructors::collect_surface_constructor_ordinals;

#[derive(Debug)]
pub(crate) struct CancelToken {
    local: AtomicBool,
    parent: Option<Arc<CancelToken>>,
}

impl CancelToken {
    pub(crate) fn root() -> Arc<Self> {
        Arc::new(Self {
            local: AtomicBool::new(false),
            parent: None,
        })
    }

    fn child(parent: Arc<CancelToken>) -> Arc<Self> {
        Arc::new(Self {
            local: AtomicBool::new(false),
            parent: Some(parent),
        })
    }

    pub(crate) fn cancel(&self) {
        self.local.store(true, Ordering::Release);
    }

    fn parent(&self) -> Option<Arc<CancelToken>> {
        self.parent.clone()
    }

    fn is_cancelled(&self) -> bool {
        if self.local.load(Ordering::Relaxed) {
            return true;
        }
        self.parent
            .as_ref()
            .is_some_and(|parent| parent.is_cancelled())
    }
}

pub(crate) struct Runtime {
    pub(crate) ctx: Arc<RuntimeContext>,
    cancel: Arc<CancelToken>,
    cancel_mask: usize,
    pub(crate) fuel: Option<u64>,
    rng_state: u64,
    /// Counter used to amortize cancel-token checks (checked every 64 evals).
    check_counter: u32,
    /// JIT call-depth counter — prevents stack overflow from infinite JIT recursion.
    pub(crate) jit_call_depth: u32,
    /// Maximum JIT call depth before bailing out.
    pub(crate) jit_max_call_depth: u32,
    /// Flag set by JIT match fallthrough to signal "non-exhaustive match" to
    /// `make_jit_builtin`, enabling `apply_multi_clause` to try the next clause.
    pub(crate) jit_match_failed: bool,
    /// Pending error from JIT-compiled code. Set by `rt_apply` / `rt_run_effect`
    /// when a builtin or effect fails inside JIT code, so that the enclosing
    /// closure wrapper can propagate it as `Err` instead of swallowing it.
    pub(crate) jit_pending_error: Option<RuntimeError>,
    /// Name of the currently executing JIT-compiled function, set by
    /// `rt_enter_fn` at the start of each compiled function body.
    pub(crate) jit_current_fn: Option<Box<str>>,
    /// Source location of the most recently instrumented expression, set by
    /// `rt_set_location` before potentially-failing operations.
    pub(crate) jit_current_loc: Option<Box<str>>,
    /// Warning counter incremented by `rt_warn`. Used by `rt_binary_op` to
    /// detect when a MultiClause operator clause produced warnings (e.g. wrong
    /// field access) so the next clause can be tried instead.
    pub(crate) jit_rt_warning_count: u64,
    /// When true, `rt_warn` increments the counter but does NOT print to stderr.
    /// Used during MultiClause trial dispatch where wrong-type clauses are
    /// expected to fail silently.
    pub(crate) jit_suppress_warnings: bool,
    /// Guard to prevent recursive MultiClause dispatch in `rt_binary_op`.
    /// When true, nested binary ops use the first matching clause directly.
    pub(crate) jit_binary_op_dispatching: bool,
    /// Whether `--update-snapshots` was passed. When true, `assertSnapshot`
    /// writes new snapshots and `mock snapshot` records real calls.
    pub(crate) update_snapshots: bool,
    /// Qualified name of the currently running `@test` (e.g. `"mod.testFn"`).
    pub(crate) current_test_name: Option<String>,
    /// Project root directory for resolving `__snapshots__/` paths.
    pub(crate) project_root: Option<std::path::PathBuf>,
    /// Recorded snapshot mock call results, keyed by binding path.
    /// Populated during `--update-snapshots`; flushed to disk after the test.
    pub(crate) snapshot_recordings: HashMap<String, Vec<String>>,
    /// Replay cursors for snapshot mock playback, keyed by binding path.
    pub(crate) snapshot_replay_cursors: HashMap<String, usize>,
    /// Snapshot assertion failure message. Set by `assertSnapshot` in verify
    /// mode when a mismatch is detected. Checked by the test runner after the
    /// test Effect completes, since the JIT cannot propagate Effect errors.
    pub(crate) snapshot_failure: Option<String>,
    /// Stack of resource cleanup closures. Cleanups are run in LIFO order when
    /// a do-block scope exits. Scope boundaries are explicit markers.
    pub(crate) resource_cleanups: Vec<ResourceCleanupEntry>,
    pub(crate) reactive_host: Option<ReactiveHostState>,
    pub(crate) reactive_graph: Arc<ParkingMutex<ReactiveGraphState>>,
}

pub(crate) enum ResourceCleanupEntry {
    ScopeBoundary,
    Cleanup { cleanup: Arc<ThunkFunc> },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum ReactiveDependency {
    WholeModel,
    RootField(String),
    Signal(String),
}

#[derive(Clone, Debug)]
pub(crate) struct ReactiveDependencyVersion {
    pub(crate) dependency: ReactiveDependency,
    pub(crate) revision: u64,
}

#[derive(Clone)]
pub(crate) struct ReactiveSignalEntry {
    pub(crate) derive: Value,
    pub(crate) cached: Option<Value>,
    pub(crate) dependencies: Vec<ReactiveDependencyVersion>,
    pub(crate) dirty: bool,
    pub(crate) revision: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct ReactiveEvalFrame {
    pub(crate) key: String,
    pub(crate) dependencies: HashSet<ReactiveDependency>,
}

#[derive(Clone)]
pub(crate) struct ReactiveHostState {
    pub(crate) current_model: Value,
    pub(crate) model_revision: u64,
    pub(crate) root_field_revisions: HashMap<String, u64>,
    pub(crate) signals: HashMap<String, ReactiveSignalEntry>,
    pub(crate) dependents: HashMap<String, HashSet<String>>,
    pub(crate) eval_stack: Vec<ReactiveEvalFrame>,
}

pub(crate) struct ReactiveGraphState {
    pub(crate) next_signal_id: usize,
    pub(crate) next_watcher_id: usize,
    pub(crate) batch_depth: usize,
    pub(crate) flushing: bool,
    /// Set when a background thread finishes a batch with pending notifications
    /// but cannot run watcher callbacks (GTK not active on that thread).
    /// The GTK main thread checks this flag and flushes on the next pump.
    pub(crate) deferred_flush: bool,
    /// True once any watcher has been registered that requires flushing on the
    /// main thread (e.g. GTK live-binding watchers). When false, all flushes
    /// happen immediately regardless of which thread triggers them.
    pub(crate) main_thread_flush: bool,
    pub(crate) signals: HashMap<usize, ReactiveCellEntry>,
    pub(crate) watchers: HashMap<usize, ReactiveWatcherEntry>,
    pub(crate) watchers_by_signal: HashMap<usize, HashSet<usize>>,
    pub(crate) pending_notifications: HashSet<usize>,
}

pub(crate) struct ReactiveCellEntry {
    pub(crate) kind: ReactiveCellKind,
    pub(crate) value: Value,
    pub(crate) revision: u64,
    pub(crate) dirty: bool,
    pub(crate) dependents: HashSet<usize>,
}

pub(crate) enum ReactiveCellKind {
    Source,
    Derived {
        dependencies: Vec<usize>,
        compute: Value,
    },
    DerivedRecord {
        dependencies: Vec<usize>,
        field_names: Vec<String>,
        compute: Value,
    },
}

pub(crate) struct ReactiveWatcherEntry {
    pub(crate) signal_id: usize,
    pub(crate) callback: Value,
    pub(crate) active: bool,
    pub(crate) last_revision: u64,
}

#[derive(Clone)]
#[allow(dead_code)]
pub(crate) enum RuntimeError {
    Error(Value),
    Cancelled,
    Message(String),
    TypeError {
        context: String,
        expected: String,
        got: String,
    },
    DivisionByZero {
        context: String,
    },
    Overflow {
        context: String,
    },
    IndexOutOfBounds {
        context: String,
        index: i64,
        length: usize,
    },
    NonExhaustiveMatch {
        scrutinee: Option<String>,
    },
    StackOverflow {
        depth: u32,
    },
    IOError {
        context: String,
        cause: String,
    },
    InvalidArgument {
        context: String,
        reason: String,
    },
    ParseError {
        context: String,
        input: String,
    },
}

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", format_runtime_error(self.clone()))
    }
}

pub(crate) fn run_main_effect(runtime: &mut Runtime) -> Result<(), AiviError> {
    let main = runtime
        .ctx
        .globals
        .get("main")
        .ok_or_else(|| AiviError::Runtime("missing main definition".to_string()))?;
    let main_value = match runtime.force_value(main) {
        Ok(value) => value,
        Err(err) => return Err(AiviError::Runtime(format_runtime_error(err))),
    };
    let effect = match main_value {
        Value::Effect(effect) => Value::Effect(effect),
        other => {
            return Err(AiviError::Runtime(format!(
                "main must be an Effect value, got {}",
                format_value(&other)
            )))
        }
    };

    match runtime.run_effect_value(effect) {
        Ok(_) => {
            // Surface any errors that JIT runtime helpers caught but couldn't
            // propagate through native code boundaries (e.g. assertEq failures).
            if let Some(err) = runtime.jit_pending_error.take() {
                Err(AiviError::Runtime(format_runtime_error(err)))
            } else {
                Ok(())
            }
        }
        Err(err) => Err(AiviError::Runtime(format_runtime_error(err))),
    }
}

pub(crate) fn build_runtime_from_program(program: &HirProgram) -> Result<Runtime, AiviError> {
    if program.modules.is_empty() {
        return Err(AiviError::Runtime("no modules to run".to_string()));
    }

    // With the interpreter removed, we can't create Thunks wrapping HIR expressions.
    // Instead, register placeholders that the JIT will overwrite with compiled builtins.
    // Definitions that the JIT can't compile will remain as Unit (and error at runtime).
    let mut grouped: HashMap<String, usize> = HashMap::new();
    for module in &program.modules {
        let module_name = &module.name;
        for def in &module.defs {
            *grouped.entry(def.name.clone()).or_default() += 1;
            *grouped
                .entry(format!("{module_name}.{}", def.name))
                .or_default() += 1;
        }
    }

    let globals = Env::new(None);
    register_builtins(&globals);
    // Don't create thunks — the JIT will register compiled builtins directly

    let ctx = Arc::new(RuntimeContext::new_with_constructor_ordinals(
        globals,
        core_constructor_ordinals(),
    ));
    let cancel = CancelToken::root();
    Ok(Runtime::new(ctx, cancel))
}

/// Like [`build_runtime_from_program`] but uses an externally provided cancel
/// token so the caller can cancel from another thread.
pub(crate) fn build_runtime_from_program_with_cancel(
    program: &HirProgram,
    cancel: Arc<CancelToken>,
) -> Result<Runtime, AiviError> {
    if program.modules.is_empty() {
        return Err(AiviError::Runtime("no modules to run".to_string()));
    }

    let mut grouped: HashMap<String, usize> = HashMap::new();
    for module in &program.modules {
        let module_name = &module.name;
        for def in &module.defs {
            *grouped.entry(def.name.clone()).or_default() += 1;
            *grouped
                .entry(format!("{module_name}.{}", def.name))
                .or_default() += 1;
        }
    }

    let globals = Env::new(None);
    register_builtins(&globals);

    let ctx = Arc::new(RuntimeContext::new_with_constructor_ordinals(
        globals,
        core_constructor_ordinals(),
    ));
    Ok(Runtime::new(ctx, cancel))
}

/// Create a runtime with only builtins registered (no user program).
/// Used by the AOT path where compiled functions are registered separately.
pub(crate) fn build_runtime_base() -> Runtime {
    let globals = Env::new(None);
    register_builtins(&globals);
    let ctx = Arc::new(RuntimeContext::new_with_constructor_ordinals(
        globals,
        core_constructor_ordinals(),
    ));
    let cancel = CancelToken::root();
    Runtime::new(ctx, cancel)
}

#[allow(dead_code)]
fn build_runtime_from_program_scoped(
    program: HirProgram,
    surface_modules: &[crate::surface::Module],
) -> Result<Runtime, AiviError> {
    if program.modules.is_empty() {
        return Err(AiviError::Runtime("no modules to run".to_string()));
    }

    let globals = Env::new(None);
    register_builtins(&globals);

    // Build a map of surface module metadata for import scoping.
    let mut surface_by_name: HashMap<String, &crate::surface::Module> = HashMap::new();
    for module in surface_modules {
        surface_by_name.insert(module.name.name.clone(), module);
    }
    let mut value_exports: HashMap<String, Vec<String>> = HashMap::new();
    let mut domain_members: HashMap<(String, String), Vec<String>> = HashMap::new();
    let mut method_names: HashSet<String> = HashSet::new();
    for module in surface_modules {
        value_exports.insert(
            module.name.name.clone(),
            module
                .exports
                .iter()
                .filter(|e| e.kind == crate::surface::ScopeItemKind::Value)
                .map(|e| e.name.name.clone())
                .collect(),
        );
        for export in &module.exports {
            if export.kind != crate::surface::ScopeItemKind::Domain {
                continue;
            }
            let domain_name = export.name.name.clone();
            let mut members = Vec::new();
            for item in &module.items {
                let crate::surface::ModuleItem::DomainDecl(domain) = item else {
                    continue;
                };
                if domain.name.name != domain_name {
                    continue;
                }
                for domain_item in &domain.items {
                    match domain_item {
                        crate::surface::DomainItem::Def(def)
                        | crate::surface::DomainItem::LiteralDef(def) => {
                            members.push(def.name.name.clone());
                        }
                        crate::surface::DomainItem::TypeAlias(_)
                        | crate::surface::DomainItem::TypeSig(_) => {}
                    }
                }
            }
            domain_members.insert((module.name.name.clone(), domain_name), members);
        }

        // Methods (class members) behave like open multi-clause functions at runtime: instances can
        // add new clauses. When importing, we merge method bindings instead of overwriting locals.
        for item in &module.items {
            let crate::surface::ModuleItem::ClassDecl(class_decl) = item else {
                continue;
            };
            for member in &class_decl.members {
                method_names.insert(member.name.name.clone());
            }
        }
    }

    fn merge_method_binding(existing: Value, imported: Value) -> Value {
        fn flatten(value: Value, out: &mut Vec<Value>) {
            match value {
                Value::MultiClause(clauses) => out.extend(clauses),
                other => out.push(other),
            }
        }

        let mut clauses = Vec::new();
        // Keep local clauses first so user-defined instances override defaults.
        flatten(existing, &mut clauses);
        flatten(imported, &mut clauses);
        Value::MultiClause(clauses)
    }

    // Create a per-module environment rooted at the global environment. Each top-level def thunk
    // captures its module env so runtime evaluation respects lexical imports and avoids global
    // collisions (especially for operator names like `(+)`).
    let mut module_envs: HashMap<String, Env> = HashMap::new();
    for module in &program.modules {
        module_envs.insert(module.name.clone(), Env::new(Some(globals.clone())));
    }

    // First pass: register qualified globals for every definition, preserving multi-clause
    // functions (same qualified name defined multiple times).
    let mut grouped: HashMap<String, (Env, Vec<HirExpr>)> = HashMap::new();
    for module in &program.modules {
        let module_name = module.name.clone();
        let module_env = module_envs
            .get(&module_name)
            .cloned()
            .unwrap_or_else(|| Env::new(Some(globals.clone())));
        for def in &module.defs {
            let name = format!("{module_name}.{}", def.name);
            grouped
                .entry(name)
                .or_insert_with(|| (module_env.clone(), Vec::new()))
                .1
                .push(def.expr.clone());
        }
    }
    for (name, (_module_env, exprs)) in grouped {
        if globals.get(&name).is_some() {
            continue;
        }
        if exprs.len() == 1 {
            let thunk = ThunkValue {
                cached: Mutex::new(None),
            };
            globals.set(name, Value::Thunk(Arc::new(thunk)));
        } else {
            let mut clauses = Vec::new();
            for _expr in exprs {
                let thunk = ThunkValue {
                    cached: Mutex::new(None),
                };
                clauses.push(Value::Thunk(Arc::new(thunk)));
            }
            globals.set(name, Value::MultiClause(clauses));
        }
    }

    // Second pass: populate each module env with its local defs and imports.
    for module in &program.modules {
        let module_name = module.name.clone();
        let module_env = module_envs
            .get(&module_name)
            .cloned()
            .unwrap_or_else(|| Env::new(Some(globals.clone())));

        // Local defs in the module are always in scope unqualified.
        for def in &module.defs {
            let qualified = format!("{module_name}.{}", def.name);
            if let Some(value) = globals.get(&qualified) {
                module_env.set(def.name.clone(), value);
            }
        }

        // Import exported values and domain members.
        let Some(surface_module) = surface_by_name.get(&module_name).copied() else {
            continue;
        };
        for use_decl in &surface_module.uses {
            let imported_mod = use_decl.module.name.clone();
            if use_decl.wildcard {
                if let Some(names) = value_exports.get(&imported_mod) {
                    for name in names {
                        let qualified = format!("{imported_mod}.{name}");
                        if let Some(value) = globals.get(&qualified) {
                            if let Some(existing) = module_env.get(name) {
                                if method_names.contains(name) {
                                    module_env
                                        .set(name.clone(), merge_method_binding(existing, value));
                                    continue;
                                }
                            }
                            // Non-methods: last import wins (allows more-specific modules to shadow)
                            module_env.set(name.clone(), value);
                        }
                    }
                }
                continue;
            }
            for item in &use_decl.items {
                match item.kind {
                    crate::surface::ScopeItemKind::Value => {
                        let original = item.name.name.clone();
                        let local = item
                            .alias
                            .as_ref()
                            .map(|a| a.name.clone())
                            .unwrap_or_else(|| original.clone());
                        let qualified = format!("{imported_mod}.{original}");
                        if let Some(value) = globals.get(&qualified) {
                            if let Some(existing) = module_env.get(&local) {
                                if method_names.contains(&local) {
                                    module_env
                                        .set(local.clone(), merge_method_binding(existing, value));
                                    continue;
                                }
                            }
                            module_env.set(local, value);
                        }
                    }
                    crate::surface::ScopeItemKind::Domain => {
                        let domain_name = item.name.name.clone();
                        let key = (imported_mod.clone(), domain_name);
                        if let Some(members) = domain_members.get(&key) {
                            for member in members {
                                let qualified = format!("{imported_mod}.{member}");
                                if let Some(value) = globals.get(&qualified) {
                                    if let Some(existing) = module_env.get(member) {
                                        if method_names.contains(member) {
                                            module_env.set(
                                                member.clone(),
                                                merge_method_binding(existing, value),
                                            );
                                            continue;
                                        }
                                    }
                                    module_env.set(member.clone(), value);
                                }
                            }
                        }
                    }
                }
            }
        }

        // Re-apply local defs after imports so that local definitions always
        // shadow imported names (including domain members).  Without this,
        // a wildcard `use` that brings in a domain method with the same name
        // as a local binding would silently overwrite the local definition.
        for def in &module.defs {
            let qualified = format!("{module_name}.{}", def.name);
            if let Some(value) = globals.get(&qualified) {
                module_env.set(def.name.clone(), value);
            }
        }

        // Re-export forwarding: a module can `export x` where `x` is brought into scope via `use`
        // (e.g. facade modules like `aivi.linalg`). Ensure qualified access `Module.x` resolves by
        // registering exported bindings that exist in the module env, even when they aren't local
        // definitions.
        for export in &surface_module.exports {
            if export.kind != crate::surface::ScopeItemKind::Value {
                continue;
            }
            let name = export.name.name.clone();
            let qualified = format!("{module_name}.{name}");
            if globals.get(&qualified).is_some() {
                continue;
            }
            if let Some(value) = module_env.get(&name) {
                globals.set(qualified, value);
            }
        }
    }

    let mut constructor_ordinals = core_constructor_ordinals();
    for (name, ordinal) in collect_surface_constructor_ordinals(surface_modules) {
        match ordinal {
            Some(idx) => insert_constructor_ordinal(&mut constructor_ordinals, name, idx),
            None => {
                constructor_ordinals.insert(name, None);
            }
        }
    }
    let ctx = Arc::new(RuntimeContext::new_with_constructor_ordinals(
        globals,
        constructor_ordinals,
    ));
    let cancel = CancelToken::root();
    Ok(Runtime::new(ctx, cancel))
}

pub(crate) fn format_runtime_error(err: RuntimeError) -> String {
    match err {
        RuntimeError::Cancelled => "execution cancelled".to_string(),
        RuntimeError::Message(message) => message,
        RuntimeError::Error(value) => format!("runtime error: {}", format_value(&value)),
        RuntimeError::TypeError {
            context,
            expected,
            got,
        } => format!("{context}: expected {expected}, got {got}"),
        RuntimeError::DivisionByZero { context } => {
            format!("{context}: division by zero")
        }
        RuntimeError::Overflow { context } => {
            format!("{context}: arithmetic overflow")
        }
        RuntimeError::IndexOutOfBounds {
            context,
            index,
            length,
        } => format!("{context}: index {index} out of bounds (length {length})"),
        RuntimeError::NonExhaustiveMatch { scrutinee } => match scrutinee {
            Some(val) => format!("non-exhaustive match: no pattern matched {val}"),
            None => "non-exhaustive match".to_string(),
        },
        RuntimeError::StackOverflow { depth } => {
            format!("stack overflow: exceeded maximum call depth of {depth}")
        }
        RuntimeError::IOError { context, cause } => {
            format!("{context}: {cause}")
        }
        RuntimeError::InvalidArgument { context, reason } => {
            format!("{context}: {reason}")
        }
        RuntimeError::ParseError { context, input } => {
            format!("{context}: failed to parse \"{input}\"")
        }
    }
}

#[cfg(test)]
fn runtime_builtin(
    name: &str,
    arity: usize,
    func: impl Fn(Vec<Value>, &mut Runtime) -> Result<Value, RuntimeError> + Send + Sync + 'static,
) -> Value {
    Value::Builtin(BuiltinValue {
        imp: Arc::new(BuiltinImpl {
            name: name.to_string(),
            arity,
            func: Arc::new(func),
        }),
        args: Vec::new(),
        tagged_args: Some(Vec::new()),
    })
}

include!("runtime_impl/lifecycle_and_cancel.rs");
include!("runtime_impl/eval_and_apply.rs");
include!("runtime_impl/reactive.rs");
include!("runtime_impl/reactive_signals.rs");
include!("runtime_impl/resources.rs");
include!("runtime_impl/trampoline.rs");
