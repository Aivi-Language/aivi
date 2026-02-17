use std::collections::{HashMap, HashSet};

use crate::diagnostics::FileDiagnostic;
use crate::surface::{DomainItem, Module, ModuleItem};

use super::checker::TypeChecker;
use super::types::Scheme;

use super::class_env::{
    collect_exported_class_env, collect_imported_class_env, collect_local_class_env,
    expand_classes, InstanceDeclInfo,
};
use super::global::collect_global_type_info;
use super::ordering::ordered_module_indices;

pub fn elaborate_expected_coercions(modules: &mut [Module]) -> Vec<FileDiagnostic> {
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

    for idx in ordered_module_indices(modules) {
        let module = &mut modules[idx];
        checker.reset_module_context(module);

        let mut env = checker.builtins.clone();
        checker.register_module_types(module);
        diagnostics.extend(checker.collect_type_expr_diags(module));
        let sigs = checker.collect_type_sigs(module);
        checker.register_module_constructors(module, &mut env);
        checker.register_imports(module, &module_exports, &module_domain_exports, &mut env);

        let (imported_classes, imported_instances) =
            collect_imported_class_env(module, &module_class_exports, &module_instance_exports);
        let (local_classes, local_instances) = collect_local_class_env(module);
        let local_class_names: HashSet<String> = local_classes.keys().cloned().collect();
        let mut classes = imported_classes;
        classes.extend(local_classes);
        let classes = expand_classes(classes);
        let mut instances: Vec<InstanceDeclInfo> = imported_instances
            .into_iter()
            .filter(|instance| !local_class_names.contains(&instance.class_name))
            .collect();
        instances.extend(local_instances);
        checker.set_class_env(classes, instances);

        checker.register_module_defs(module, &sigs, &mut env);

        // Rewrite user modules only. Embedded stdlib modules are not guaranteed to typecheck in v0.1,
        // but we still want their type signatures, classes, and instances in scope for elaboration.
        if !module.path.starts_with("<embedded:") {
            let mut elab_errors = Vec::new();
            for item in module.items.iter_mut() {
                match item {
                    ModuleItem::Def(def) => {
                        if let Err(err) = checker.elaborate_def_expr(def, &sigs, &env) {
                            elab_errors.push(err);
                        }
                    }
                    ModuleItem::InstanceDecl(instance) => {
                        for def in instance.defs.iter_mut() {
                            if let Err(err) = checker.elaborate_def_expr(def, &sigs, &env) {
                                elab_errors.push(err);
                            }
                        }
                    }
                    ModuleItem::DomainDecl(domain) => {
                        for domain_item in domain.items.iter_mut() {
                            match domain_item {
                                DomainItem::Def(def) | DomainItem::LiteralDef(def) => {
                                    if let Err(err) = checker.elaborate_def_expr(def, &sigs, &env) {
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

        let mut exports = HashMap::new();
        for export in &module.exports {
            if export.kind != crate::surface::ScopeItemKind::Value {
                continue;
            }
            if let Some(schemes) = sigs.get(&export.name.name) {
                exports.insert(export.name.name.clone(), schemes.clone());
            } else if let Some(schemes) = env.get_all(&export.name.name) {
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
