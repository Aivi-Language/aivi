use crate::diagnostics::FileDiagnostic;
use crate::surface::{DomainItem, Module, ModuleItem};

use super::checker::TypeChecker;
use super::global::collect_global_type_info;
use super::ordering::ordered_module_indices;
use super::{module_interface_from_setup, setup_module, ModuleInterface, ModuleInterfaceMaps};

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
            &state.module_domain_exports,
            &state.module_class_exports,
            &state.module_instance_exports,
            diagnostics,
        );

        // Rewrite user modules only. Embedded stdlib modules are not guaranteed to typecheck in v0.1,
        // but we still want their type signatures, classes, and instances in scope for elaboration.
        if !module.path.starts_with("<embedded:") {
            let mut elab_errors = Vec::new();
            for item in module.items.iter_mut() {
                match item {
                    ModuleItem::Def(def) => {
                        if let Err(err) = checker.elaborate_def_expr(def, &setup.sigs, &setup.env) {
                            elab_errors.push(err);
                        }
                    }
                    ModuleItem::InstanceDecl(instance) => {
                        for def in instance.defs.iter_mut() {
                            if let Err(err) =
                                checker.elaborate_def_expr(def, &setup.sigs, &setup.env)
                            {
                                elab_errors.push(err);
                            }
                        }
                    }
                    ModuleItem::DomainDecl(domain) => {
                        for domain_item in domain.items.iter_mut() {
                            match domain_item {
                                DomainItem::Def(def) | DomainItem::LiteralDef(def) => {
                                    if let Err(err) =
                                        checker.elaborate_def_expr(def, &setup.sigs, &setup.env)
                                    {
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
        }

        let interface = module_interface_from_setup(module, checker, &setup);
        state.apply_module_interface(&module.name.name, &interface);
        module_results.push(ElaboratedModule {
            module_name: module.name.name.clone(),
            diagnostics: diagnostics[start..].to_vec(),
            interface,
        });
    }
    module_results
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
