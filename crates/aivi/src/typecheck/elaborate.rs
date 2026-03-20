use std::collections::{HashMap, HashSet};

use crate::diagnostics::{Diagnostic, DiagnosticSeverity, FileDiagnostic, Span};
use crate::surface::{Def, DomainItem, Expr, Module, ModuleItem};

use super::checker::TypeChecker;
use super::global::collect_global_type_info;
use super::ordering::ordered_module_indices;
use super::{
    build_module_interface, checked_module_env, setup_module, ModuleInterface, ModuleInterfaceMaps,
};

pub fn elaborate_expected_coercions(modules: &mut [Module]) -> Vec<FileDiagnostic> {
    let mut checker = TypeChecker::new();
    let mut diagnostics = Vec::new();
    let mut state = ModuleInterfaceMaps::default();

    let (global_type_constructors, global_aliases, global_opaque_types) =
        collect_global_type_info(&mut checker, modules);
    checker.set_global_type_info(
        global_type_constructors,
        global_aliases,
        global_opaque_types,
    );

    elaborate_modules(modules, &mut checker, &mut diagnostics, &mut state, false);

    diagnostics
}

const CYCLIC_SIGNAL_DIAGNOSTIC_CODE: &str = "E3001";

#[derive(Clone)]
enum ReactiveSignalBinding {
    Derived { span: Span, deps: Vec<String> },
    Alias { span: Span, target: String },
}

impl ReactiveSignalBinding {
    fn span(&self) -> &Span {
        match self {
            ReactiveSignalBinding::Derived { span, .. }
            | ReactiveSignalBinding::Alias { span, .. } => span,
        }
    }

    fn deps(&self) -> Vec<String> {
        match self {
            ReactiveSignalBinding::Derived { deps, .. } => deps.clone(),
            ReactiveSignalBinding::Alias { target, .. } => vec![target.clone()],
        }
    }
}

/// Cached stdlib export maps. Avoids re-processing embedded modules during elaboration
/// when the same stdlib is shared across many user files.
#[derive(Clone)]
pub struct ElaborationCheckpoint {
    state: ModuleInterfaceMaps,
}

#[derive(Clone)]
pub struct ElaboratedModule {
    pub module_name: String,
    pub diagnostics: Vec<FileDiagnostic>,
    interface: ModuleInterface,
}

#[derive(Clone)]
pub struct ElaborationIncrementalResult {
    pub diagnostics: Vec<FileDiagnostic>,
    pub checkpoint: ElaborationCheckpoint,
    pub modules: Vec<ElaboratedModule>,
}

/// Build a checkpoint by elaborating only stdlib (embedded) modules.
/// The returned checkpoint can be cloned and reused for multiple user files.
pub fn elaborate_stdlib_checkpoint(stdlib_modules: &mut [Module]) -> ElaborationCheckpoint {
    let mut checker = TypeChecker::new();
    let mut state = ModuleInterfaceMaps::default();

    let (global_type_constructors, global_aliases, global_opaque_types) =
        collect_global_type_info(&mut checker, stdlib_modules);
    checker.set_global_type_info(
        global_type_constructors,
        global_aliases,
        global_opaque_types,
    );

    let mut diagnostics = Vec::new();
    elaborate_modules(
        stdlib_modules,
        &mut checker,
        &mut diagnostics,
        &mut state,
        false,
    );

    ElaborationCheckpoint { state }
}

/// Elaborate user modules using a pre-built stdlib checkpoint.
/// `modules` must contain all modules (stdlib + user); stdlib modules are skipped during
/// elaboration and their cached exports are used instead.
pub fn elaborate_with_checkpoint(
    modules: &mut [Module],
    checkpoint: &ElaborationCheckpoint,
) -> Vec<FileDiagnostic> {
    let all_modules = modules.to_vec();
    elaborate_with_checkpoint_incremental(&all_modules, modules, checkpoint).diagnostics
}

/// Elaborate a subset of modules using a checkpoint built from earlier modules in dependency
/// order. `all_modules` must contain the full active workspace so global type info stays current.
pub fn elaborate_with_checkpoint_incremental(
    all_modules: &[Module],
    modules: &mut [Module],
    checkpoint: &ElaborationCheckpoint,
) -> ElaborationIncrementalResult {
    let mut checker = TypeChecker::new();
    let mut diagnostics = Vec::new();
    let mut state = checkpoint.state.clone();

    let (global_type_constructors, global_aliases, global_opaque_types) =
        collect_global_type_info(&mut checker, all_modules);
    checker.set_global_type_info(
        global_type_constructors,
        global_aliases,
        global_opaque_types,
    );

    let modules = elaborate_modules(modules, &mut checker, &mut diagnostics, &mut state, true);

    ElaborationIncrementalResult {
        diagnostics,
        checkpoint: ElaborationCheckpoint { state },
        modules,
    }
}

/// Core elaboration loop shared by both the full and checkpoint paths.
/// When `skip_embedded` is true, modules whose path starts with `<embedded:` are skipped
/// (their exports are assumed to already be present in the export maps).
fn elaborate_modules(
    modules: &mut [Module],
    checker: &mut TypeChecker,
    diagnostics: &mut Vec<FileDiagnostic>,
    state: &mut ModuleInterfaceMaps,
    skip_embedded: bool,
) -> Vec<ElaboratedModule> {
    let mut module_results = Vec::new();
    for idx in ordered_module_indices(modules) {
        let is_embedded = modules[idx].path.starts_with("<embedded:");
        if skip_embedded && is_embedded {
            continue;
        }

        let start = diagnostics.len();
        let module = &mut modules[idx];
        let setup = setup_module(
            checker,
            module,
            &state.module_exports,
            &state.module_type_exports,
            &state.module_domain_exports,
            &state.module_class_exports,
            &state.module_instance_exports,
            diagnostics,
        );
        let elaboration_env = if is_embedded {
            setup.env.clone()
        } else {
            checked_module_env(module, checker, &setup)
        };

        // Rewrite user modules only. Embedded stdlib modules are not guaranteed to typecheck in v0.1,
        // but we still want their type signatures, classes, and instances in scope for elaboration.
        if !is_embedded {
            let mut elab_errors = Vec::new();
            for item in module.items.iter_mut() {
                match item {
                    ModuleItem::Def(def) => {
                        if let Err(err) =
                            checker.elaborate_def_expr(def, &setup.sigs, &elaboration_env)
                        {
                            elab_errors.push(err);
                        }
                    }
                    ModuleItem::InstanceDecl(instance) => {
                        for def in instance.defs.iter_mut() {
                            if let Err(err) =
                                checker.elaborate_def_expr(def, &setup.sigs, &elaboration_env)
                            {
                                elab_errors.push(err);
                            }
                        }
                    }
                    ModuleItem::DomainDecl(domain) => {
                        for domain_item in domain.items.iter_mut() {
                            match domain_item {
                                DomainItem::Def(def) | DomainItem::LiteralDef(def) => {
                                    if let Err(err) = checker.elaborate_def_expr(
                                        def,
                                        &setup.sigs,
                                        &elaboration_env,
                                    ) {
                                        elab_errors.push(err);
                                    }
                                }
                                DomainItem::TypeAlias(_) | DomainItem::TypeSig(_) => {}
                            }
                        }
                    }
                    _ => {}
                }
            }
            for err in elab_errors {
                diagnostics.push(checker.error_to_diag(module, err));
            }

            diagnostics.extend(check_reactive_signal_cycles(module));
        }

        let interface = build_module_interface(
            module,
            checker,
            &setup.sigs,
            &elaboration_env,
            &state.module_exports,
            &state.module_domain_exports,
        );
        state.apply_module_interface(&module.name.name, &interface);
        module_results.push(ElaboratedModule {
            module_name: module.name.name.clone(),
            diagnostics: diagnostics[start..].to_vec(),
            interface,
        });
    }
    module_results
}

fn check_reactive_signal_cycles(module: &Module) -> Vec<FileDiagnostic> {
    let mut bindings = HashMap::new();
    for item in &module.items {
        let ModuleItem::Def(def) = item else {
            continue;
        };
        if !def.params.is_empty() {
            continue;
        }
        if let Some(binding) = reactive_signal_binding(def) {
            bindings.insert(def.name.name.clone(), binding);
        }
    }

    let mut reactive_names: HashSet<String> = bindings
        .iter()
        .filter_map(|(name, binding)| match binding {
            ReactiveSignalBinding::Derived { .. } => Some(name.clone()),
            ReactiveSignalBinding::Alias { .. } => None,
        })
        .collect();

    loop {
        let mut changed = false;
        for (name, binding) in &bindings {
            if reactive_names.contains(name) {
                continue;
            }
            let ReactiveSignalBinding::Alias { target, .. } = binding else {
                continue;
            };
            if reactive_names.contains(target) {
                reactive_names.insert(name.clone());
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    if reactive_names.is_empty() {
        return Vec::new();
    }

    let mut graph = HashMap::new();
    for (name, binding) in &bindings {
        if !reactive_names.contains(name) {
            continue;
        }
        let mut deps = binding
            .deps()
            .into_iter()
            .filter(|dep| reactive_names.contains(dep))
            .collect::<Vec<_>>();
        deps.sort();
        deps.dedup();
        graph.insert(name.clone(), deps);
    }

    let cycle_nodes = detect_named_cycles(&graph);
    cycle_nodes
        .into_iter()
        .filter_map(|name| {
            bindings.get(&name).map(|binding| {
                file_diag(
                    module,
                    Diagnostic {
                        code: CYCLIC_SIGNAL_DIAGNOSTIC_CODE.to_string(),
                        severity: DiagnosticSeverity::Error,
                        message: format!("cyclic signal dependency involving '{name}'"),
                        span: binding.span().clone(),
                        labels: Vec::new(),
                        hints: vec![
                            "top-level derived and combined signals must form an acyclic graph".to_string(),
                            "break the cycle by introducing a source `signal ...` or by removing the back-edge".to_string(),
                        ],
                        suggestion: None,
                    },
                )
            })
        })
        .collect()
}

fn reactive_signal_binding(def: &Def) -> Option<ReactiveSignalBinding> {
    reactive_signal_dependencies(&def.expr)
        .map(|deps| ReactiveSignalBinding::Derived {
            span: def.name.span.clone(),
            deps,
        })
        .or_else(|| {
            reactive_signal_alias(&def.expr).map(|target| ReactiveSignalBinding::Alias {
                span: def.name.span.clone(),
                target,
            })
        })
}

fn reactive_signal_dependencies(expr: &Expr) -> Option<Vec<String>> {
    let Expr::Call { func, args, .. } = expr else {
        return None;
    };

    if is_reactive_builtin(func, "signal") && args.len() == 1 {
        return Some(Vec::new());
    }

    if is_reactive_builtin(func, "derive") && args.len() == 2 {
        return Some(collect_reactive_source_refs(&args[0]));
    }

    if is_reactive_builtin(func, "combineAll") && args.len() == 2 {
        return Some(collect_reactive_source_refs(&args[0]));
    }

    None
}

fn reactive_signal_alias(expr: &Expr) -> Option<String> {
    let Expr::Ident(name) = expr else {
        return None;
    };
    if name.name.contains('.') {
        return None;
    }
    Some(name.name.clone())
}

fn is_reactive_builtin(func: &Expr, field: &str) -> bool {
    match func {
        Expr::Ident(name) => reactive_builtin_name_matches(&name.name, field),
        Expr::FieldAccess {
            base, field: name, ..
        } => {
            name.name == field
                && matches!(
                    base.as_ref(),
                    Expr::Ident(base_name)
                        if base_name.name == "reactive"
                            || base_name.name.ends_with(".reactive")
                )
        }
        _ => false,
    }
}

fn reactive_builtin_name_matches(name: &str, field: &str) -> bool {
    name == field
        || name == format!("reactive.{field}")
        || name.ends_with(&format!(".reactive.{field}"))
}

fn collect_reactive_source_refs(expr: &Expr) -> Vec<String> {
    let mut refs = Vec::new();
    collect_reactive_source_refs_inner(expr, &mut refs);
    refs
}

fn collect_reactive_source_refs_inner(expr: &Expr, refs: &mut Vec<String>) {
    match expr {
        Expr::Ident(name) => {
            if !name.name.contains('.') {
                refs.push(name.name.clone());
            }
        }
        Expr::UnaryNeg { expr, .. } | Expr::Suffixed { base: expr, .. } => {
            collect_reactive_source_refs_inner(expr, refs);
        }
        Expr::List { items, .. } => {
            for item in items {
                collect_reactive_source_refs_inner(&item.expr, refs);
            }
        }
        Expr::Tuple { items, .. } => {
            for item in items {
                collect_reactive_source_refs_inner(item, refs);
            }
        }
        Expr::Record { fields, .. } | Expr::PatchLit { fields, .. } => {
            for field in fields {
                collect_reactive_source_refs_inner(&field.value, refs);
            }
        }
        Expr::FieldAccess { base, .. } => collect_reactive_source_refs_inner(base, refs),
        Expr::Index { base, index, .. } => {
            collect_reactive_source_refs_inner(base, refs);
            collect_reactive_source_refs_inner(index, refs);
        }
        Expr::Call { func, args, .. } => {
            collect_reactive_source_refs_inner(func, refs);
            for arg in args {
                collect_reactive_source_refs_inner(arg, refs);
            }
        }
        Expr::Flow { root, .. } => {
            collect_reactive_source_refs_inner(root, refs);
        }
        Expr::Binary { left, right, .. } => {
            collect_reactive_source_refs_inner(left, refs);
            collect_reactive_source_refs_inner(right, refs);
        }
        Expr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            collect_reactive_source_refs_inner(cond, refs);
            collect_reactive_source_refs_inner(then_branch, refs);
            collect_reactive_source_refs_inner(else_branch, refs);
        }
        Expr::TextInterpolate { .. }
        | Expr::Literal(_)
        | Expr::Lambda { .. }
        | Expr::Match { .. }
        | Expr::Block { .. }
        | Expr::Raw { .. }
        | Expr::FieldSection { .. }
        | Expr::Mock { .. } => {}
    }
}

fn detect_named_cycles(graph: &HashMap<String, Vec<String>>) -> HashSet<String> {
    fn dfs(
        name: &str,
        graph: &HashMap<String, Vec<String>>,
        visiting: &mut HashSet<String>,
        visited: &mut HashSet<String>,
        stack: &mut Vec<String>,
        in_cycle: &mut HashSet<String>,
    ) {
        if !visiting.insert(name.to_string()) {
            if let Some(pos) = stack.iter().position(|entry| entry == name) {
                for entry in &stack[pos..] {
                    in_cycle.insert(entry.clone());
                }
            } else {
                in_cycle.insert(name.to_string());
            }
            return;
        }

        stack.push(name.to_string());
        if let Some(deps) = graph.get(name) {
            for dep in deps {
                if !graph.contains_key(dep) {
                    continue;
                }
                if visiting.contains(dep) {
                    if let Some(pos) = stack.iter().position(|entry| entry == dep) {
                        for entry in &stack[pos..] {
                            in_cycle.insert(entry.clone());
                        }
                    } else {
                        in_cycle.insert(dep.clone());
                    }
                    continue;
                }
                if !visited.contains(dep) {
                    dfs(dep, graph, visiting, visited, stack, in_cycle);
                }
            }
        }
        stack.pop();
        visiting.remove(name);
        visited.insert(name.to_string());
    }

    let mut in_cycle = HashSet::new();
    let mut visiting = HashSet::new();
    let mut visited = HashSet::new();
    let mut stack = Vec::new();
    for name in graph.keys() {
        if !visited.contains(name) {
            dfs(
                name,
                graph,
                &mut visiting,
                &mut visited,
                &mut stack,
                &mut in_cycle,
            );
        }
    }
    in_cycle
}

fn file_diag(module: &Module, diagnostic: Diagnostic) -> FileDiagnostic {
    FileDiagnostic {
        path: module.path.clone(),
        diagnostic,
    }
}

impl ElaborationCheckpoint {
    pub fn empty() -> Self {
        Self {
            state: ModuleInterfaceMaps::default(),
        }
    }

    pub fn apply_cached_module(&mut self, module: &ElaboratedModule) {
        self.state
            .apply_module_interface(&module.module_name, &module.interface);
    }

    pub fn remove_module(&mut self, module_name: &str) {
        self.state.remove_module(module_name);
    }
}
