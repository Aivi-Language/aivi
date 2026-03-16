use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{Datelike, NaiveDate, TimeZone as ChronoTimeZone};
use parking_lot::Mutex as ParkingMutex;
use regex::RegexBuilder;
use url::Url;

use aivi_driver::{RuntimeFrame, RuntimeFrameKind, RuntimeLabel, RuntimeNoteKind, RuntimeReport};

use crate::hir::{HirExpr, HirProgram};
use crate::i18n::{parse_message_template, validate_key_text, MessagePart};
use crate::{Position, SourceOrigin, Span};
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
    /// Captured JIT stack/origin for the most recent non-exhaustive match.
    pub(crate) jit_match_snapshot: Option<RuntimeSnapshot>,
    /// Pending error from JIT-compiled code. Set by `rt_apply` / `rt_run_effect`
    /// when a builtin or effect fails inside JIT code, so that the enclosing
    /// closure wrapper can propagate it as `Err` instead of swallowing it.
    pub(crate) jit_pending_error: Option<RuntimeError>,
    /// Captured JIT stack/origin at the moment `jit_pending_error` was set.
    pub(crate) jit_pending_snapshot: Option<RuntimeSnapshot>,
    /// Name of the currently executing JIT-compiled function, set by
    /// `rt_enter_fn` at the start of each compiled function body.
    pub(crate) jit_current_fn: Option<Box<str>>,
    /// Source location of the most recently instrumented expression, set by
    /// `rt_set_location` before potentially-failing operations.
    pub(crate) jit_current_loc: Option<SourceOrigin>,
    /// Live JIT stack used for runtime reports and warnings.
    pub(crate) jit_frame_stack: Vec<RuntimeFrame>,
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

#[derive(Clone, Debug, Default)]
pub(crate) struct RuntimeSnapshot {
    pub(crate) origin: Option<SourceOrigin>,
    pub(crate) frames: Vec<RuntimeFrame>,
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
    pub(crate) next_change_seq: u64,
    pub(crate) batch_depth: usize,
    pub(crate) flushing: bool,
    /// Set when a background thread finishes a batch with pending notifications
    /// but cannot run watcher callbacks (GTK not active on that thread).
    /// The GTK main thread checks this flag and flushes on the next pump.
    pub(crate) deferred_flush: bool,
    /// When set, watcher callbacks must run on this specific thread (e.g. the
    /// GTK main thread). Background threads defer their flush so this thread
    /// picks it up during the next pump/recv cycle. When `None`, all flushes
    /// happen immediately on whichever thread triggers them.
    pub(crate) flush_thread: Option<std::thread::ThreadId>,
    pub(crate) signals: HashMap<usize, ReactiveCellEntry>,
    pub(crate) watchers: HashMap<usize, ReactiveWatcherEntry>,
    pub(crate) watchers_by_signal: HashMap<usize, HashSet<usize>>,
    pub(crate) pending_notifications: HashSet<usize>,
}

pub(crate) struct ReactiveCellEntry {
    pub(crate) kind: ReactiveCellKind,
    pub(crate) value: Value,
    pub(crate) revision: u64,
    pub(crate) last_change_seq: u64,
    pub(crate) last_change_timestamp_ms: u64,
    pub(crate) dirty: bool,
    pub(crate) dependents: HashSet<usize>,
}

pub(crate) enum ReactiveCellKind {
    Source,
    Derived {
        dependencies: Vec<usize>,
        compute: Value,
    },
    DerivedTuple {
        dependencies: Vec<usize>,
        compute: Value,
    },
}

pub(crate) struct ReactiveWatcherEntry {
    pub(crate) signal_id: usize,
    pub(crate) callback: Value,
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
    Context {
        context: String,
        source: Box<RuntimeError>,
    },
}

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", format_runtime_error(self.clone()))
    }
}

impl Runtime {
    pub(crate) fn capture_runtime_snapshot(&self) -> RuntimeSnapshot {
        RuntimeSnapshot {
            origin: self
                .jit_current_loc
                .clone()
                .or_else(|| self.jit_frame_stack.last().and_then(|frame| frame.origin.clone())),
            frames: self.jit_frame_stack.clone(),
        }
    }

    pub(crate) fn capture_match_failure(&mut self) {
        self.jit_match_failed = true;
        self.jit_match_snapshot = Some(self.capture_runtime_snapshot());
    }

    pub(crate) fn clear_match_failure(&mut self) {
        self.jit_match_failed = false;
        self.jit_match_snapshot = None;
    }

    pub(crate) fn clear_pending_runtime_error(&mut self) {
        self.jit_pending_error = None;
        self.jit_pending_snapshot = None;
    }

    pub(crate) fn take_pending_runtime_error(
        &mut self,
    ) -> Option<(RuntimeError, Option<RuntimeSnapshot>)> {
        let err = self.jit_pending_error.take()?;
        let snapshot = self
            .jit_pending_snapshot
            .take()
            .or_else(|| Some(self.capture_runtime_snapshot()));
        Some((err, snapshot))
    }

    fn take_snapshot_for_error(&mut self, err: &RuntimeError) -> Option<RuntimeSnapshot> {
        let pending = self.jit_pending_snapshot.take();
        let match_snapshot = match runtime_error_leaf(err) {
            RuntimeError::NonExhaustiveMatch { .. } => self.jit_match_snapshot.take(),
            _ => None,
        };
        pending.or(match_snapshot).or_else(|| {
            if self.jit_current_loc.is_none() && self.jit_frame_stack.is_empty() {
                None
            } else {
                Some(self.capture_runtime_snapshot())
            }
        })
    }

    pub(crate) fn runtime_report(&mut self, err: RuntimeError) -> RuntimeReport {
        let snapshot = self.take_snapshot_for_error(&err);
        self.runtime_report_with_snapshot(err, snapshot)
    }

    pub(crate) fn runtime_report_with_snapshot(
        &mut self,
        err: RuntimeError,
        snapshot: Option<RuntimeSnapshot>,
    ) -> RuntimeReport {
        runtime_error_report(err, snapshot)
    }

    pub(crate) fn take_pending_aivi_error(&mut self) -> Option<AiviError> {
        let (err, snapshot) = self.take_pending_runtime_error()?;
        Some(AiviError::Runtime(Box::new(
            self.runtime_report_with_snapshot(err, snapshot),
        )))
    }

    pub(crate) fn aivi_runtime_error(&mut self, err: RuntimeError) -> AiviError {
        AiviError::Runtime(Box::new(self.runtime_report(err)))
    }
}

pub(crate) fn run_main_effect(runtime: &mut Runtime) -> Result<(), AiviError> {
    let main = runtime
        .ctx
        .globals
        .get("main")
        .ok_or_else(|| AiviError::runtime_message("missing main definition"))?;
    let main_value = match runtime.force_value(main) {
        Ok(value) => value,
        Err(err) => return Err(runtime.aivi_runtime_error(err)),
    };
    let effect = match main_value {
        Value::Effect(effect) => Value::Effect(effect),
        other => {
            return Err(AiviError::runtime_message(format!(
                "main must be an Effect value, got {}",
                format_value(&other)
            )))
        }
    };

    match runtime.run_effect_value(effect) {
        Ok(_) => {
            // Surface any errors that JIT runtime helpers caught but couldn't
            // propagate through native code boundaries (e.g. assertEq failures).
            if let Some(err) = runtime.take_pending_aivi_error() {
                Err(err)
            } else {
                Ok(())
            }
        }
        Err(err) => Err(runtime.aivi_runtime_error(err)),
    }
}

pub(crate) fn build_runtime_from_program(program: &HirProgram) -> Result<Runtime, AiviError> {
    if program.modules.is_empty() {
        return Err(AiviError::runtime_message("no modules to run"));
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
        return Err(AiviError::runtime_message("no modules to run"));
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
        return Err(AiviError::runtime_message("no modules to run"));
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

fn runtime_error_leaf(err: &RuntimeError) -> &RuntimeError {
    match err {
        RuntimeError::Context { source, .. } => runtime_error_leaf(source),
        other => other,
    }
}

fn collect_runtime_contexts<'a>(err: &'a RuntimeError, contexts: &mut Vec<String>) -> &'a RuntimeError {
    match err {
        RuntimeError::Context { context, source } => {
            contexts.push(context.clone());
            collect_runtime_contexts(source, contexts)
        }
        other => other,
    }
}

fn parse_origin_text(text: &str) -> Option<SourceOrigin> {
    let mut parts = text.rsplitn(3, ':');
    let column = parts.next()?.parse().ok()?;
    let line = parts.next()?.parse().ok()?;
    let path = parts.next()?;
    Some(SourceOrigin::new(
        path.to_string(),
        Span {
            start: Position { line, column },
            end: Position {
                line,
                column: column.saturating_add(1),
            },
        },
    ))
}

fn parse_context_origin(context: &str) -> Option<SourceOrigin> {
    let rest = context.strip_prefix("at ")?;
    let location = rest.split(" in `").next().unwrap_or(rest).trim();
    parse_origin_text(location)
}

fn parse_context_frame(context: &str) -> Option<RuntimeFrame> {
    let (_, tail) = context.split_once("in `")?;
    let name = tail.strip_suffix('`')?;
    Some(RuntimeFrame {
        kind: RuntimeFrameKind::Function,
        name: name.to_string(),
        origin: None,
    })
}

fn push_unique_frame(frames: &mut Vec<RuntimeFrame>, frame: RuntimeFrame) {
    if frames
        .last()
        .is_some_and(|prev| prev.name == frame.name && prev.origin == frame.origin)
    {
        return;
    }
    frames.push(frame);
}

fn display_runtime_name(name: &str) -> &str {
    name.strip_prefix("aivi.").unwrap_or(name)
}

fn text_join_help() -> &'static str {
    "pass a `List Text` to `text.join`, or convert the items first with `map text.toText`"
}

fn preview_text(value: &str) -> String {
    const LIMIT: usize = 60;
    let mut chars = value.chars();
    let preview: String = chars.by_ref().take(LIMIT).collect();
    if chars.next().is_some() {
        format!("{preview}…")
    } else {
        preview
    }
}

fn build_runtime_frames(
    snapshot: Option<&RuntimeSnapshot>,
    contexts: &[String],
    scrutinee: Option<&str>,
) -> Vec<RuntimeFrame> {
    let mut frames = Vec::new();
    if let Some(snapshot) = snapshot {
        for frame in snapshot.frames.iter().rev() {
            push_unique_frame(&mut frames, frame.clone());
        }
    }
    for context in contexts.iter().rev() {
        if let Some(frame) = parse_context_frame(context) {
            push_unique_frame(&mut frames, frame);
        }
    }
    if let Some(scrutinee) = scrutinee {
        if let Some(frame) = parse_context_frame(scrutinee) {
            push_unique_frame(&mut frames, frame);
        }
    }
    frames
}

fn select_primary_origin(
    snapshot: Option<&RuntimeSnapshot>,
    contexts: &[String],
    scrutinee: Option<&str>,
) -> Option<SourceOrigin> {
    let context_origins = || {
        contexts
            .iter()
            .rev()
            .filter_map(|context| parse_context_origin(context))
            .chain(scrutinee.into_iter().filter_map(parse_context_origin))
    };

    if let Some(snapshot) = snapshot {
        if let Some(origin) = snapshot
            .origin
            .clone()
            .filter(|origin| origin.source_kind == aivi_core::SourceKind::User)
        {
            return Some(origin);
        }
        if let Some(origin) = snapshot
            .frames
            .iter()
            .rev()
            .filter_map(|frame| frame.origin.clone())
            .find(|origin| origin.source_kind == aivi_core::SourceKind::User)
        {
            return Some(origin);
        }
    }
    if let Some(origin) = context_origins().find(|origin| origin.source_kind == aivi_core::SourceKind::User) {
        return Some(origin);
    }
    if let Some(snapshot) = snapshot {
        if let Some(origin) = snapshot.origin.clone() {
            return Some(origin);
        }
        if let Some(origin) = snapshot
            .frames
            .iter()
            .rev()
            .filter_map(|frame| frame.origin.clone())
            .next()
        {
            return Some(origin);
        }
    }
    context_origins().next()
}

fn runtime_error_report(err: RuntimeError, snapshot: Option<RuntimeSnapshot>) -> RuntimeReport {
    let mut contexts = Vec::new();
    let leaf = collect_runtime_contexts(&err, &mut contexts);
    let scrutinee = match leaf {
        RuntimeError::NonExhaustiveMatch { scrutinee } => scrutinee.as_deref(),
        _ => None,
    };
    let frames = build_runtime_frames(snapshot.as_ref(), &contexts, scrutinee);
    let primary = select_primary_origin(snapshot.as_ref(), &contexts, scrutinee);
    let active_frame_name = frames.first().map(|frame| frame.name.as_str());

    let mut report = match leaf {
        RuntimeError::Cancelled => RuntimeReport::new("RT1000", "execution cancelled"),
        RuntimeError::Message(message) => RuntimeReport::new("RT1001", message.clone()),
        RuntimeError::Error(value) => RuntimeReport::new("RT1002", "runtime error")
            .with_note(format!("value: {}", format_value(value))),
        RuntimeError::TypeError {
            context,
            expected,
            got,
        } if context == "text.join" => RuntimeReport::new("RT1203", "`text.join` expected a list of `Text`")
            .with_note(format!("received `{got}`"))
            .with_hint(text_join_help()),
        RuntimeError::TypeError {
            context,
            expected,
            got,
        } => RuntimeReport::new(
            "RT1200",
            format!("`{context}` expected `{expected}`, got `{got}`"),
        ),
        RuntimeError::DivisionByZero { context } => RuntimeReport::new(
            "RT1204",
            format!("`{context}` attempted to divide by zero"),
        )
        .with_hint("check that the divisor is non-zero before dividing"),
        RuntimeError::Overflow { context } => RuntimeReport::new(
            "RT1205",
            format!("`{context}` overflowed during arithmetic"),
        ),
        RuntimeError::IndexOutOfBounds {
            context,
            index,
            length,
        } => RuntimeReport::new(
            "RT1206",
            format!("`{context}` index {index} is out of bounds for length {length}"),
        )
        .with_hint("guard the index with a length check before indexing"),
        RuntimeError::NonExhaustiveMatch { scrutinee } => {
            let in_text_join = active_frame_name.is_some_and(|name| name.ends_with("text.join"));
            let mut report = if in_text_join {
                RuntimeReport::new("RT1203", "`text.join` expected a list of `Text`")
                    .with_hint(text_join_help())
            } else {
                RuntimeReport::new("RT1208", "non-exhaustive match: no pattern matched")
                    .with_hint("add a wildcard (`_`) arm or cover the missing cases explicitly")
            };
            if let Some(value) = scrutinee {
                if parse_context_origin(value).is_none() && parse_context_frame(value).is_none() {
                    let label = if in_text_join {
                        "received value".to_string()
                    } else {
                        "unmatched value".to_string()
                    };
                    report = report.with_note(format!("{label}: {value}"));
                }
            }
            if !in_text_join {
                if let Some(name) = active_frame_name {
                    report = report.with_note(format!(
                        "while evaluating `{}`",
                        display_runtime_name(name)
                    ));
                }
            }
            report
        }
        RuntimeError::StackOverflow { depth } => RuntimeReport::new(
            "RT1209",
            format!("stack overflow: exceeded maximum call depth of {depth}"),
        ),
        RuntimeError::IOError { context, cause } => {
            RuntimeReport::new("RT1210", format!("`{context}` failed")).with_note(cause.clone())
        }
        RuntimeError::InvalidArgument { context, reason } if context == "text.join" => {
            RuntimeReport::new("RT1203", "`text.join` expected a list of `Text`")
                .with_note(reason.clone())
                .with_hint(text_join_help())
        }
        RuntimeError::InvalidArgument { context, reason } => {
            RuntimeReport::new("RT1211", format!("`{context}` rejected its arguments"))
                .with_note(reason.clone())
        }
        RuntimeError::ParseError { context, input } => RuntimeReport::new(
            "RT1212",
            format!("`{context}` failed to parse the input"),
        )
        .with_note(format!("input: {:?}", preview_text(input))),
        RuntimeError::Context { .. } => unreachable!("context wrappers are stripped above"),
    };

    if let Some(primary_origin) = primary {
        report.primary = Some(primary_origin.clone());
        if let Some(snapshot) = snapshot.as_ref() {
            if let Some(inner_frame) = snapshot.frames.last() {
                if let Some(inner_origin) = inner_frame.origin.clone() {
                    if inner_origin != primary_origin {
                        report.labels.push(RuntimeLabel {
                            message: format!("runtime raised inside `{}`", inner_frame.name),
                            origin: inner_origin,
                        });
                    }
                }
            }
        }
    }

    report.frames = frames;

    for context in contexts {
        if parse_context_origin(&context).is_none() && parse_context_frame(&context).is_none() {
            report.notes.push(aivi_driver::RuntimeNote {
                kind: RuntimeNoteKind::Note,
                message: context,
            });
        }
    }

    report
}

pub(crate) fn format_runtime_error(err: RuntimeError) -> String {
    aivi_driver::render_runtime_report(&runtime_error_report(err, None), false)
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

#[cfg(test)]
mod runtime_report_tests {
    use super::*;

    fn origin(path: &str, line: usize, column: usize) -> SourceOrigin {
        SourceOrigin::new(
            path.to_string(),
            Span {
                start: Position { line, column },
                end: Position {
                    line,
                    column: column + 1,
                },
            },
        )
    }

    #[test]
    fn non_exhaustive_text_join_prefers_user_callsite_and_keeps_stdlib_frame() {
        let user_origin = origin("src/app.aivi", 42, 15);
        let stdlib_origin = origin("<embedded:aivi.text>", 173, 14);
        let snapshot = RuntimeSnapshot {
            origin: Some(stdlib_origin.clone()),
            frames: vec![
                RuntimeFrame {
                    kind: RuntimeFrameKind::Function,
                    name: "app.main.renderNames".to_string(),
                    origin: Some(user_origin.clone()),
                },
                RuntimeFrame {
                    kind: RuntimeFrameKind::Function,
                    name: "aivi.text.join".to_string(),
                    origin: Some(stdlib_origin.clone()),
                },
            ],
        };

        let report = runtime_error_report(
            RuntimeError::NonExhaustiveMatch { scrutinee: None },
            Some(snapshot),
        );
        let rendered = aivi_driver::render_runtime_report(&report, false);

        assert_eq!(report.code, "RT1203");
        assert_eq!(report.primary.as_ref().map(|origin| origin.path.as_str()), Some("src/app.aivi"));
        assert_eq!(report.frames.first().map(|frame| frame.name.as_str()), Some("aivi.text.join"));
        assert!(
            report
                .labels
                .iter()
                .any(|label| label.origin.path == "<embedded:aivi.text>"),
            "expected embedded stdlib label, got {report:#?}"
        );
        assert!(rendered.contains("error[RT1203]: `text.join` expected a list of `Text`"));
        assert!(rendered.contains("src/app.aivi:42:15"));
        assert!(rendered.contains("runtime raised inside `aivi.text.join`"));
        assert!(rendered.contains("0: aivi.text.join at <embedded:aivi.text>:173:14"));
    }

    #[test]
    fn text_join_item_error_surfaces_index_and_help() {
        let user_origin = origin("src/app.aivi", 42, 15);
        let snapshot = RuntimeSnapshot {
            origin: Some(user_origin.clone()),
            frames: vec![RuntimeFrame {
                kind: RuntimeFrameKind::Function,
                name: "app.main.renderNames".to_string(),
                origin: Some(user_origin),
            }],
        };

        let report = runtime_error_report(
            RuntimeError::InvalidArgument {
                context: "text.join".to_string(),
                reason: "list item at index 2 has type `Int`".to_string(),
            },
            Some(snapshot),
        );
        let rendered = aivi_driver::render_runtime_report(&report, false);

        assert_eq!(report.code, "RT1203");
        assert!(rendered.contains("note: list item at index 2 has type `Int`"));
        assert!(rendered.contains("help: pass a `List Text` to `text.join`"));
    }
}

include!("runtime_impl/lifecycle_and_cancel.rs");
include!("runtime_impl/eval_and_apply.rs");
include!("runtime_impl/reactive.rs");
include!("runtime_impl/reactive_signals.rs");
include!("runtime_impl/resources.rs");
include!("runtime_impl/trampoline.rs");
