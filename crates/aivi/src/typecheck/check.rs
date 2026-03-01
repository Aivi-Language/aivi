use std::collections::HashMap;

use crate::diagnostics::FileDiagnostic;
use crate::surface::{DomainItem, Module, ModuleItem};

use super::checker::TypeChecker;
use super::setup_module;
use super::types::Scheme;

use super::class_env::{collect_exported_class_env, InstanceDeclInfo};
use super::global::collect_global_type_info;
use super::ordering::ordered_modules;

pub fn check_types(modules: &[Module]) -> Vec<FileDiagnostic> {
    check_types_impl(modules, false)
}

pub fn check_types_including_stdlib(modules: &[Module]) -> Vec<FileDiagnostic> {
    check_types_impl(modules, true)
}

fn check_types_impl(modules: &[Module], check_embedded_stdlib: bool) -> Vec<FileDiagnostic> {
    let mut checker = TypeChecker::new();
    let mut diagnostics = Vec::new();
    let mut module_exports: HashMap<String, HashMap<String, Vec<Scheme>>> = HashMap::new();
    let mut module_domain_exports: HashMap<String, HashMap<String, Vec<String>>> = HashMap::new();
    let mut module_class_exports: HashMap<
        String,
        HashMap<String, super::class_env::ClassDeclInfo>,
    > = HashMap::new();
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

    diagnostics
}
