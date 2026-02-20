use std::collections::{HashMap, HashSet};

use crate::cg_type::CgType;
use crate::diagnostics::FileDiagnostic;
use crate::surface::{DomainItem, Module, ModuleItem};

use super::checker::TypeChecker;
use super::types::Scheme;

use super::class_env::{
    collect_exported_class_env, collect_imported_class_env, collect_local_class_env,
    expand_classes, InstanceDeclInfo,
};
use super::global::collect_global_type_info;
use super::ordering::ordered_modules;

/// Result of type inference: diagnostics, pretty-printed type strings, and codegen-friendly
/// type annotations.
pub struct InferResult {
    pub diagnostics: Vec<FileDiagnostic>,
    /// Module → definition name → rendered type string (for LSP / display).
    pub type_strings: HashMap<String, HashMap<String, String>>,
    /// Module → definition name → codegen-friendly type (for the typed codegen path).
    pub cg_types: HashMap<String, HashMap<String, CgType>>,
}

pub fn infer_value_types(
    modules: &[Module],
) -> (
    Vec<FileDiagnostic>,
    HashMap<String, HashMap<String, String>>,
) {
    let result = infer_value_types_full(modules);
    (result.diagnostics, result.type_strings)
}

pub fn infer_value_types_full(modules: &[Module]) -> InferResult {
    let mut checker = TypeChecker::new();
    let mut diagnostics = Vec::new();
    let mut module_exports: HashMap<String, HashMap<String, Vec<Scheme>>> = HashMap::new();
    let mut module_domain_exports: HashMap<String, HashMap<String, Vec<String>>> = HashMap::new();
    let mut module_class_exports: HashMap<
        String,
        HashMap<String, super::class_env::ClassDeclInfo>,
    > = HashMap::new();
    let mut module_instance_exports: HashMap<String, Vec<InstanceDeclInfo>> = HashMap::new();
    let mut inferred: HashMap<String, HashMap<String, String>> = HashMap::new();
    let mut cg_types: HashMap<String, HashMap<String, CgType>> = HashMap::new();

    let (global_type_constructors, global_aliases) =
        collect_global_type_info(&mut checker, modules);
    checker.set_global_type_info(global_type_constructors, global_aliases);

    for module in ordered_modules(modules) {
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

        let mut module_diags = checker.check_module_defs(module, &sigs, &mut env);
        diagnostics.append(&mut module_diags);

        let mut local_names = HashSet::new();
        for item in module.items.iter() {
            match item {
                ModuleItem::Def(def) => {
                    local_names.insert(def.name.name.clone());
                }
                ModuleItem::TypeSig(sig) => {
                    local_names.insert(sig.name.name.clone());
                }
                ModuleItem::DomainDecl(domain) => {
                    for domain_item in domain.items.iter() {
                        match domain_item {
                            DomainItem::TypeSig(sig) => {
                                local_names.insert(sig.name.name.clone());
                            }
                            DomainItem::Def(def) | DomainItem::LiteralDef(def) => {
                                local_names.insert(def.name.name.clone());
                            }
                            DomainItem::TypeAlias(_) => {}
                        }
                    }
                }
                _ => {}
            }
        }

        let mut module_types = HashMap::new();
        let mut module_cg_types = HashMap::new();
        for name in local_names {
            if let Some(schemes) = env.get_all(&name) {
                if schemes.len() == 1 {
                    module_types.insert(name.clone(), checker.type_to_string(&schemes[0].ty));
                    // Monomorphic (no quantified vars) → try to produce a CgType.
                    if schemes[0].vars.is_empty() {
                        module_cg_types.insert(name, checker.type_to_cg_type(&schemes[0].ty, &env));
                    } else {
                        module_cg_types.insert(name, CgType::Dynamic);
                    }
                } else {
                    let mut rendered = String::new();
                    for (idx, scheme) in schemes.iter().enumerate() {
                        if idx > 0 {
                            rendered.push_str(" | ");
                        }
                        rendered.push_str(&checker.type_to_string(&scheme.ty));
                    }
                    module_types.insert(name.clone(), rendered);
                    // Multi-clause overloads are always Dynamic for now.
                    module_cg_types.insert(name, CgType::Dynamic);
                }
            }
        }
        inferred.insert(module.name.name.clone(), module_types);
        cg_types.insert(module.name.name.clone(), module_cg_types);

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

    InferResult {
        diagnostics,
        type_strings: inferred,
        cg_types,
    }
}
