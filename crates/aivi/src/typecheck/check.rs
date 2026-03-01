use std::collections::HashMap;

use crate::diagnostics::FileDiagnostic;
use crate::surface::{DomainItem, Module, ModuleItem};

use super::checker::TypeChecker;
use super::setup_module;
use super::types::Scheme;

use super::class_env::{collect_exported_class_env, ClassDeclInfo, InstanceDeclInfo};
use super::global::collect_global_type_info;
use super::ordering::ordered_modules;

pub fn check_types(modules: &[Module]) -> Vec<FileDiagnostic> {
    check_types_impl(modules, false)
}

pub fn check_types_including_stdlib(modules: &[Module]) -> Vec<FileDiagnostic> {
    check_types_impl(modules, true)
}

/// Cached stdlib type-setup maps for `check_types`.
/// Avoids re-running `setup_module` on all embedded stdlib modules per keystroke.
#[derive(Clone)]
pub struct CheckTypesCheckpoint {
    module_exports: HashMap<String, HashMap<String, Vec<Scheme>>>,
    module_domain_exports: HashMap<String, HashMap<String, Vec<String>>>,
    module_class_exports: HashMap<String, HashMap<String, ClassDeclInfo>>,
    module_instance_exports: HashMap<String, Vec<InstanceDeclInfo>>,
}

/// Build a checkpoint by running type setup on stdlib (embedded) modules only.
/// Intended to be built once at LSP startup and reused for every keystroke.
pub fn check_types_stdlib_checkpoint(stdlib_modules: &[Module]) -> CheckTypesCheckpoint {
    let mut checker = TypeChecker::new();
    let mut module_exports: HashMap<String, HashMap<String, Vec<Scheme>>> = HashMap::new();
    let mut module_domain_exports: HashMap<String, HashMap<String, Vec<String>>> = HashMap::new();
    let mut module_class_exports: HashMap<String, HashMap<String, ClassDeclInfo>> = HashMap::new();
    let mut module_instance_exports: HashMap<String, Vec<InstanceDeclInfo>> = HashMap::new();

    let (global_type_constructors, global_aliases) =
        collect_global_type_info(&mut checker, stdlib_modules);
    checker.set_global_type_info(global_type_constructors, global_aliases);

    let mut discarded = Vec::new();
    for module in ordered_modules(stdlib_modules) {
        let setup = setup_module(
            &mut checker,
            module,
            &module_exports,
            &module_domain_exports,
            &module_class_exports,
            &module_instance_exports,
            &mut discarded,
        );
        // Don't check_module_defs — stdlib bodies may be incomplete in v0.1.
        collect_exports(
            module,
            &checker,
            &setup,
            &mut module_exports,
            &mut module_domain_exports,
            &mut module_class_exports,
            &mut module_instance_exports,
        );
    }

    CheckTypesCheckpoint {
        module_exports,
        module_domain_exports,
        module_class_exports,
        module_instance_exports,
    }
}

/// Run type-checking using a pre-built stdlib checkpoint.
/// Skips `setup_module` for embedded stdlib modules; only processes user modules.
/// `modules` must contain all modules (stdlib + user).
pub fn check_types_with_checkpoint(
    modules: &[Module],
    checkpoint: &CheckTypesCheckpoint,
) -> Vec<FileDiagnostic> {
    let mut checker = TypeChecker::new();
    let mut diagnostics = Vec::new();
    let mut module_exports = checkpoint.module_exports.clone();
    let mut module_domain_exports = checkpoint.module_domain_exports.clone();
    let mut module_class_exports = checkpoint.module_class_exports.clone();
    let mut module_instance_exports = checkpoint.module_instance_exports.clone();

    // collect_global_type_info is cheap (just extracts type names); run on all modules
    // so user-defined types are visible alongside stdlib types.
    let (global_type_constructors, global_aliases) =
        collect_global_type_info(&mut checker, modules);
    checker.set_global_type_info(global_type_constructors, global_aliases);

    for module in ordered_modules(modules) {
        if module.path.starts_with("<embedded:") {
            // Stdlib already registered via checkpoint; skip setup_module entirely.
            continue;
        }
        let setup = setup_module(
            &mut checker,
            module,
            &module_exports,
            &module_domain_exports,
            &module_class_exports,
            &module_instance_exports,
            &mut diagnostics,
        );
        let mut module_diags =
            checker.check_module_defs(module, &setup.sigs, &mut setup.env.clone());
        diagnostics.append(&mut module_diags);
        collect_exports(
            module,
            &checker,
            &setup,
            &mut module_exports,
            &mut module_domain_exports,
            &mut module_class_exports,
            &mut module_instance_exports,
        );
    }

    diagnostics
}

fn check_types_impl(modules: &[Module], check_embedded_stdlib: bool) -> Vec<FileDiagnostic> {
    let mut checker = TypeChecker::new();
    let mut diagnostics = Vec::new();
    let mut module_exports: HashMap<String, HashMap<String, Vec<Scheme>>> = HashMap::new();
    let mut module_domain_exports: HashMap<String, HashMap<String, Vec<String>>> = HashMap::new();
    let mut module_class_exports: HashMap<String, HashMap<String, ClassDeclInfo>> = HashMap::new();
    let mut module_instance_exports: HashMap<String, Vec<InstanceDeclInfo>> = HashMap::new();

    let (global_type_constructors, global_aliases) =
        collect_global_type_info(&mut checker, modules);
    checker.set_global_type_info(global_type_constructors, global_aliases);

    for module in ordered_modules(modules) {
        let setup = setup_module(
            &mut checker,
            module,
            &module_exports,
            &module_domain_exports,
            &module_class_exports,
            &module_instance_exports,
            &mut diagnostics,
        );

        // v0.1 embedded stdlib is allowed to be incomplete; typechecking its bodies can hang/crash.
        // Still collect its signatures/classes/instances so user modules can typecheck.
        if check_embedded_stdlib || !module.path.starts_with("<embedded:") {
            let mut module_diags =
                checker.check_module_defs(module, &setup.sigs, &mut setup.env.clone());
            diagnostics.append(&mut module_diags);
        }

        collect_exports(
            module,
            &checker,
            &setup,
            &mut module_exports,
            &mut module_domain_exports,
            &mut module_class_exports,
            &mut module_instance_exports,
        );
    }

    diagnostics
}

/// Collect and record all export maps for a module after it has been set up.
#[allow(clippy::too_many_arguments)]
fn collect_exports(
    module: &Module,
    checker: &TypeChecker,
    setup: &super::ModuleSetup,
    module_exports: &mut HashMap<String, HashMap<String, Vec<Scheme>>>,
    module_domain_exports: &mut HashMap<String, HashMap<String, Vec<String>>>,
    module_class_exports: &mut HashMap<String, HashMap<String, ClassDeclInfo>>,
    module_instance_exports: &mut HashMap<String, Vec<InstanceDeclInfo>>,
) {
    let mut exports = HashMap::new();
    for export in &module.exports {
        if export.kind != crate::surface::ScopeItemKind::Value {
            continue;
        }
        if let Some(schemes) = setup.sigs.get(&export.name.name) {
            exports.insert(export.name.name.clone(), schemes.clone());
        } else if let Some(schemes) = setup.env.get_all(&export.name.name) {
            exports.insert(export.name.name.clone(), schemes.to_vec());
        }
    }
    module_exports.insert(module.name.name.clone(), exports);

    let mut domain_exports = HashMap::new();
    for export in &module.exports {
        if export.kind != crate::surface::ScopeItemKind::Domain {
            continue;
        }
        let domain_name = export.name.name.as_str();
        let mut members = Vec::new();
        for item in &module.items {
            let ModuleItem::DomainDecl(domain) = item else {
                continue;
            };
            if domain.name.name != domain_name {
                continue;
            }
            for domain_item in &domain.items {
                match domain_item {
                    DomainItem::Def(def) | DomainItem::LiteralDef(def) => {
                        members.push(def.name.name.clone());
                    }
                    DomainItem::TypeAlias(_) | DomainItem::TypeSig(_) => {}
                }
            }
        }
        domain_exports.insert(domain_name.to_string(), members);
    }
    module_domain_exports.insert(module.name.name.clone(), domain_exports);

    let (class_exports, instance_exports) =
        collect_exported_class_env(module, &checker.classes, &checker.instances);
    module_class_exports.insert(module.name.name.clone(), class_exports);
    module_instance_exports.insert(module.name.name.clone(), instance_exports);
}
