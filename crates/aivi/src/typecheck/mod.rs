use std::collections::{HashMap, HashSet};

use crate::diagnostics::FileDiagnostic;
use crate::surface::Module;

mod builtins;
mod checker;
mod constraints;
mod query_engine;
mod types;

mod check;
mod class_env;
mod elaborate;
mod global;
mod infer;
mod ordering;

#[cfg(test)]
mod builtins_parity_tests;
#[cfg(test)]
mod class_constraints_tests;
#[cfg(test)]
mod expected_coercions_tests;
#[cfg(test)]
mod infer_and_class_env_tests;

pub use check::{
    check_types, check_types_including_stdlib, check_types_stdlib_checkpoint,
    check_types_with_checkpoint, check_types_with_checkpoint_incremental,
    summarize_module_export_surface, CheckTypesCheckpoint, CheckTypesIncrementalResult,
    CheckedModule, ModuleExportSurfaceSummary,
};
pub use elaborate::{
    elaborate_expected_coercions, elaborate_stdlib_checkpoint, elaborate_with_checkpoint,
    elaborate_with_checkpoint_incremental, ElaboratedModule, ElaborationCheckpoint,
    ElaborationIncrementalResult,
};
pub use infer::{
    infer_value_types, infer_value_types_fast, infer_value_types_fast_incremental,
    infer_value_types_full, infer_value_types_full_incremental, InferCheckpoint, InferMode,
    InferModuleCache, InferResult, InferValueTypesIncrementalResult,
};
pub use ordering::{ordered_module_names, reverse_module_dependencies};

use checker::TypeChecker;
use class_env::{
    collect_exported_class_env, collect_imported_class_env, collect_local_class_env,
    expand_classes, synthesize_auto_forward_instances, ClassDeclInfo, InstanceDeclInfo,
};
use types::{AliasInfo, Kind, Scheme, TypeEnv};

/// Result of per-module registration: the local type environment and collected type signatures.
struct ModuleSetup {
    env: TypeEnv,
    sigs: HashMap<String, Vec<Scheme>>,
}

#[derive(Clone, Default)]
pub struct ModuleInterface {
    exports: HashMap<String, Vec<Scheme>>,
    type_exports: HashMap<String, TypeSurface>,
    domain_exports: HashMap<String, Vec<String>>,
    class_exports: HashMap<String, ClassDeclInfo>,
    instance_exports: Vec<InstanceDeclInfo>,
}

#[derive(Clone)]
struct TypeSurface {
    internal_name: String,
    kind: Kind,
    alias: Option<AliasInfo>,
    opaque_origin: Option<String>,
}

#[derive(Clone, Default)]
struct ModuleInterfaceMaps {
    module_exports: HashMap<String, HashMap<String, Vec<Scheme>>>,
    module_type_exports: HashMap<String, HashMap<String, TypeSurface>>,
    module_domain_exports: HashMap<String, HashMap<String, Vec<String>>>,
    module_class_exports: HashMap<String, HashMap<String, ClassDeclInfo>>,
    module_instance_exports: HashMap<String, Vec<InstanceDeclInfo>>,
}

impl ModuleInterfaceMaps {
    fn apply_module_interface(&mut self, module_name: &str, interface: &ModuleInterface) {
        self.module_exports
            .insert(module_name.to_string(), interface.exports.clone());
        self.module_type_exports
            .insert(module_name.to_string(), interface.type_exports.clone());
        self.module_domain_exports
            .insert(module_name.to_string(), interface.domain_exports.clone());
        self.module_class_exports
            .insert(module_name.to_string(), interface.class_exports.clone());
        self.module_instance_exports
            .insert(module_name.to_string(), interface.instance_exports.clone());
    }

    fn remove_module(&mut self, module_name: &str) {
        self.module_exports.remove(module_name);
        self.module_type_exports.remove(module_name);
        self.module_domain_exports.remove(module_name);
        self.module_class_exports.remove(module_name);
        self.module_instance_exports.remove(module_name);
    }
}

/// Runs the full per-module registration sequence shared by all type-checking passes:
/// reset → register types → collect type-expr diagnostics → collect signatures →
/// register constructors → register imports → build class env → register defs.
#[allow(clippy::too_many_arguments)]
fn setup_module(
    checker: &mut TypeChecker,
    module: &Module,
    module_exports: &HashMap<String, HashMap<String, Vec<Scheme>>>,
    module_type_exports: &HashMap<String, HashMap<String, TypeSurface>>,
    module_domain_exports: &HashMap<String, HashMap<String, Vec<String>>>,
    module_class_exports: &HashMap<String, HashMap<String, ClassDeclInfo>>,
    module_instance_exports: &HashMap<String, Vec<InstanceDeclInfo>>,
    diagnostics: &mut Vec<FileDiagnostic>,
) -> ModuleSetup {
    checker.reset_module_context(module);
    let mut env = checker.builtins.clone();
    checker.register_module_types(module);
    checker.register_imported_type_names(module, module_type_exports);
    diagnostics.extend(checker.collect_type_expr_diags(module));
    let sigs = checker.collect_type_sigs(module);
    checker.register_module_constructors(module, &mut env);
    checker.register_imports(module, module_exports, module_domain_exports, &mut env);

    let (imported_classes, imported_instances) =
        collect_imported_class_env(module, module_class_exports, module_instance_exports);
    let (local_classes, local_instances) = collect_local_class_env(module);
    let local_class_names: HashSet<String> = local_classes.keys().cloned().collect();
    let mut classes = imported_classes;
    classes.extend(local_classes);
    let classes = expand_classes(classes);
    let mut instances: Vec<InstanceDeclInfo> = imported_instances
        .into_iter()
        .filter(|inst| !local_class_names.contains(&inst.class_name))
        .collect();
    instances.extend(local_instances);
    instances.extend(synthesize_auto_forward_instances(module, &instances));
    checker.set_class_env(classes, instances);
    checker.register_module_defs(module, &sigs, &mut env);
    checker.rewrite_env_type_names(&mut env);

    ModuleSetup { env, sigs }
}

fn module_interface_from_setup(
    module: &Module,
    checker: &TypeChecker,
    setup: &ModuleSetup,
) -> ModuleInterface {
    build_module_interface(module, checker, &setup.sigs, &setup.env)
}

fn build_module_interface(
    module: &Module,
    checker: &TypeChecker,
    sigs: &HashMap<String, Vec<Scheme>>,
    env: &TypeEnv,
) -> ModuleInterface {
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

    let mut type_exports = HashMap::new();
    for export in &module.exports {
        if export.kind == crate::surface::ScopeItemKind::Domain {
            continue;
        }
        let Some(internal_name) = checker.resolve_type_binding(&export.name.name) else {
            continue;
        };
        let Some(kind) = checker.type_kind(internal_name).cloned() else {
            continue;
        };
        let alias = checker.alias_info_for_name(internal_name).cloned();
        type_exports.insert(
            export.name.name.clone(),
            TypeSurface {
                internal_name: internal_name.to_string(),
                kind,
                alias,
                opaque_origin: checker.opaque_origin_for_name(internal_name).cloned(),
            },
        );
    }

    let mut domain_exports = HashMap::new();
    for export in &module.exports {
        if export.kind != crate::surface::ScopeItemKind::Domain {
            continue;
        }
        let domain_name = export.name.name.as_str();
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
        domain_exports.insert(domain_name.to_string(), members);
    }

    let (class_exports, instance_exports) =
        collect_exported_class_env(module, &checker.classes, &checker.instances);

    ModuleInterface {
        exports,
        type_exports,
        domain_exports,
        class_exports,
        instance_exports,
    }
}
