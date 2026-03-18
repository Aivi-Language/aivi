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
mod bigint_domain_tests;
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
    checker.rebuild_module_alias_bodies(module);
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
    module_exports: &HashMap<String, HashMap<String, Vec<Scheme>>>,
    module_domain_exports: &HashMap<String, HashMap<String, Vec<String>>>,
) -> ModuleInterface {
    build_module_interface(
        module,
        checker,
        &setup.sigs,
        &setup.env,
        module_exports,
        module_domain_exports,
    )
}

fn checked_module_env(module: &Module, checker: &mut TypeChecker, setup: &ModuleSetup) -> TypeEnv {
    checker.with_ephemeral_state_rollback(|checker| {
        let mut env = setup.env.clone();
        let _ = checker.check_module_defs(module, &setup.sigs, &mut env);
        env
    })
}

fn build_module_interface(
    module: &Module,
    checker: &TypeChecker,
    sigs: &HashMap<String, Vec<Scheme>>,
    env: &TypeEnv,
    module_exports: &HashMap<String, HashMap<String, Vec<Scheme>>>,
    module_domain_exports: &HashMap<String, HashMap<String, Vec<String>>>,
) -> ModuleInterface {
    let mut exports = HashMap::new();
    for export in &module.exports {
        match export.kind {
            crate::surface::ScopeItemKind::Value => {
                insert_export_schemes(&mut exports, &export.name.name, sigs, env);
            }
            crate::surface::ScopeItemKind::Domain => {
                let domain_name = export.name.name.as_str();
                if let Some(members) = local_domain_members(module, domain_name) {
                    for member in &members {
                        insert_export_schemes(&mut exports, member, sigs, env);
                    }
                    continue;
                }
                for (source_module, members) in
                    imported_domain_sources(module, domain_name, module_domain_exports)
                {
                    let Some(source_exports) = module_exports.get(&source_module) else {
                        continue;
                    };
                    for member in members {
                        if let Some(schemes) = source_exports.get(&member) {
                            exports.insert(member, schemes.clone());
                        }
                    }
                }
            }
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
        let members = local_domain_members(module, domain_name).unwrap_or_else(|| {
            let mut members = Vec::new();
            let mut seen = HashSet::new();
            for (_, imported_members) in
                imported_domain_sources(module, domain_name, module_domain_exports)
            {
                for member in imported_members {
                    if seen.insert(member.clone()) {
                        members.push(member);
                    }
                }
            }
            members
        });
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

fn local_domain_members(module: &Module, domain_name: &str) -> Option<Vec<String>> {
    for item in &module.items {
        let crate::surface::ModuleItem::DomainDecl(domain) = item else {
            continue;
        };
        if domain.name.name != domain_name {
            continue;
        }
        let members = domain
            .items
            .iter()
            .filter_map(|domain_item| match domain_item {
                crate::surface::DomainItem::Def(def)
                | crate::surface::DomainItem::LiteralDef(def) => Some(def.name.name.clone()),
                crate::surface::DomainItem::TypeAlias(_)
                | crate::surface::DomainItem::TypeSig(_) => None,
            })
            .collect();
        return Some(members);
    }
    None
}

fn imported_domain_sources(
    module: &Module,
    domain_name: &str,
    module_domain_exports: &HashMap<String, HashMap<String, Vec<String>>>,
) -> Vec<(String, Vec<String>)> {
    let mut sources = Vec::new();
    let mut seen_modules = HashSet::new();
    for use_decl in &module.uses {
        let imports_domain = if use_decl.wildcard {
            !use_decl.hides_domain(domain_name)
                && module_domain_exports
                    .get(&use_decl.module.name)
                    .is_some_and(|domains| domains.contains_key(domain_name))
        } else {
            use_decl.imported_items().iter().any(|item| {
                item.kind == crate::surface::ScopeItemKind::Domain && item.name.name == domain_name
            })
        };
        if !imports_domain {
            continue;
        }
        let Some(domains) = module_domain_exports.get(&use_decl.module.name) else {
            continue;
        };
        let Some(members) = domains.get(domain_name) else {
            continue;
        };
        if seen_modules.insert(use_decl.module.name.clone()) {
            sources.push((use_decl.module.name.clone(), members.clone()));
        }
    }
    sources
}

fn insert_export_schemes(
    exports: &mut HashMap<String, Vec<Scheme>>,
    name: &str,
    sigs: &HashMap<String, Vec<Scheme>>,
    env: &TypeEnv,
) {
    if let Some(schemes) = sigs.get(name) {
        exports.insert(name.to_string(), schemes.clone());
    } else if let Some(schemes) = env.get_all(name) {
        exports.insert(name.to_string(), schemes.to_vec());
    }
}
