use std::collections::{HashMap, HashSet};

use crate::diagnostics::FileDiagnostic;
use crate::surface::{DomainItem, Module, ModuleItem, TypeExpr};

mod builtins;
mod checker;
#[cfg(test)]
mod class_constraints_tests;
#[cfg(test)]
mod expected_coercions_tests;
mod types;

use self::checker::TypeChecker;
use self::types::Scheme;

#[derive(Clone, Debug)]
struct ClassDeclInfo {
    params: Vec<TypeExpr>,
    supers: Vec<TypeExpr>,
    constraints: Vec<(String, String)>,
    members: HashMap<String, TypeExpr>,
}

#[derive(Clone, Debug)]
struct InstanceDeclInfo {
    class_name: String,
    params: Vec<TypeExpr>,
}

fn collect_local_class_env(
    module: &Module,
) -> (HashMap<String, ClassDeclInfo>, Vec<InstanceDeclInfo>) {
    let mut classes = HashMap::new();
    let mut instances = Vec::new();
    for item in &module.items {
        match item {
            ModuleItem::ClassDecl(class_decl) => {
                let mut members = HashMap::new();
                for member in &class_decl.members {
                    members.insert(member.name.name.clone(), member.ty.clone());
                }
                let constraints = class_decl
                    .constraints
                    .iter()
                    .map(|constraint| (constraint.var.name.clone(), constraint.class.name.clone()))
                    .collect();
                classes.insert(
                    class_decl.name.name.clone(),
                    ClassDeclInfo {
                        params: class_decl.params.clone(),
                        supers: class_decl.supers.clone(),
                        constraints,
                        members,
                    },
                );
            }
            ModuleItem::InstanceDecl(instance_decl) => {
                instances.push(InstanceDeclInfo {
                    class_name: instance_decl.name.name.clone(),
                    params: instance_decl.params.clone(),
                });
            }
            _ => {}
        }
    }
    (classes, instances)
}

fn class_name_from_type_expr(ty: &TypeExpr) -> Option<&str> {
    match ty {
        TypeExpr::Name(name) => Some(name.name.as_str()),
        TypeExpr::Apply { base, .. } => match base.as_ref() {
            TypeExpr::Name(name) => Some(name.name.as_str()),
            _ => None,
        },
        _ => None,
    }
}

fn expand_class_members(
    name: &str,
    classes: &HashMap<String, ClassDeclInfo>,
    visiting: &mut HashSet<String>,
    cache: &mut HashMap<String, HashMap<String, TypeExpr>>,
) -> HashMap<String, TypeExpr> {
    if let Some(members) = cache.get(name) {
        return members.clone();
    }
    let Some(info) = classes.get(name) else {
        return HashMap::new();
    };
    if !visiting.insert(name.to_string()) {
        // Cycle: stop expanding to avoid infinite recursion.
        return info.members.clone();
    }

    let mut merged = HashMap::new();
    for sup in &info.supers {
        let Some(super_name) = class_name_from_type_expr(sup) else {
            continue;
        };
        if !classes.contains_key(super_name) {
            continue;
        };
        let inherited = expand_class_members(super_name, classes, visiting, cache);
        for (member, ty) in inherited {
            merged.entry(member).or_insert(ty);
        }
    }
    // Explicit members override inherited ones when names overlap.
    for (member, ty) in &info.members {
        merged.insert(member.clone(), ty.clone());
    }

    visiting.remove(name);
    cache.insert(name.to_string(), merged.clone());
    merged
}

fn expand_classes(mut classes: HashMap<String, ClassDeclInfo>) -> HashMap<String, ClassDeclInfo> {
    let mut visiting = HashSet::new();
    let mut cache: HashMap<String, HashMap<String, TypeExpr>> = HashMap::new();
    let names: Vec<String> = classes.keys().cloned().collect();
    for name in names {
        let expanded = expand_class_members(&name, &classes, &mut visiting, &mut cache);
        if let Some(info) = classes.get_mut(&name) {
            info.members = expanded;
        }
    }
    classes
}

fn collect_imported_class_env(
    module: &Module,
    module_class_exports: &HashMap<String, HashMap<String, ClassDeclInfo>>,
    module_instance_exports: &HashMap<String, Vec<InstanceDeclInfo>>,
) -> (HashMap<String, ClassDeclInfo>, Vec<InstanceDeclInfo>) {
    let mut classes = HashMap::new();
    let mut instances = Vec::new();
    for use_decl in &module.uses {
        let Some(class_exports) = module_class_exports.get(&use_decl.module.name) else {
            continue;
        };
        if use_decl.wildcard {
            for (name, info) in class_exports {
                classes.insert(name.clone(), info.clone());
            }
            if let Some(instance_exports) = module_instance_exports.get(&use_decl.module.name) {
                instances.extend(instance_exports.iter().cloned());
            }
            continue;
        }
        let mut imported_classes = HashSet::new();
        for item in &use_decl.items {
            if let Some(info) = class_exports.get(&item.name) {
                classes.insert(item.name.clone(), info.clone());
                imported_classes.insert(item.name.clone());
            }
        }
        if let Some(instance_exports) = module_instance_exports.get(&use_decl.module.name) {
            for instance in instance_exports {
                if imported_classes.contains(&instance.class_name) {
                    instances.push(instance.clone());
                }
            }
        }
    }
    (classes, instances)
}

fn collect_exported_class_env(
    module: &Module,
    classes: &HashMap<String, ClassDeclInfo>,
    instances: &[InstanceDeclInfo],
) -> (HashMap<String, ClassDeclInfo>, Vec<InstanceDeclInfo>) {
    let mut class_exports = HashMap::new();
    let mut exported_class_names = HashSet::new();
    for export in &module.exports {
        if let Some(info) = classes.get(&export.name) {
            class_exports.insert(export.name.clone(), info.clone());
            exported_class_names.insert(export.name.clone());
        }
    }
    let instance_exports = instances
        .iter()
        .filter(|instance| exported_class_names.contains(&instance.class_name))
        .cloned()
        .collect();
    (class_exports, instance_exports)
}

fn ordered_modules(modules: &[Module]) -> Vec<&Module> {
    let mut name_to_index = HashMap::new();
    for (idx, module) in modules.iter().enumerate() {
        name_to_index
            .entry(module.name.name.as_str())
            .or_insert(idx);
    }

    let mut indegree = vec![0usize; modules.len()];
    let mut edges: Vec<Vec<usize>> = vec![Vec::new(); modules.len()];

    for (idx, module) in modules.iter().enumerate() {
        for use_decl in module.uses.iter() {
            let Some(&dep_idx) = name_to_index.get(use_decl.module.name.as_str()) else {
                continue;
            };
            if dep_idx == idx {
                continue;
            }
            edges[dep_idx].push(idx);
            indegree[idx] += 1;
        }
    }

    let mut ready: Vec<usize> = indegree
        .iter()
        .enumerate()
        .filter_map(|(idx, &deg)| (deg == 0).then_some(idx))
        .collect();
    ready.sort_by(|a, b| modules[*a].name.name.cmp(&modules[*b].name.name));

    let mut out = Vec::new();
    let mut processed = vec![false; modules.len()];
    while let Some(idx) = ready.first().copied() {
        ready.remove(0);
        if processed[idx] {
            continue;
        }
        processed[idx] = true;
        out.push(&modules[idx]);
        for &next in edges[idx].iter() {
            indegree[next] = indegree[next].saturating_sub(1);
            if indegree[next] == 0 && !processed[next] {
                ready.push(next);
                ready.sort_by(|a, b| modules[*a].name.name.cmp(&modules[*b].name.name));
            }
        }
    }

    let mut remaining: Vec<usize> = processed
        .iter()
        .enumerate()
        .filter_map(|(idx, done)| (!done).then_some(idx))
        .collect();
    remaining.sort_by(|a, b| modules[*a].name.name.cmp(&modules[*b].name.name));
    for idx in remaining {
        out.push(&modules[idx]);
    }

    out
}

fn ordered_module_indices(modules: &[Module]) -> Vec<usize> {
    let mut name_to_index = HashMap::new();
    for (idx, module) in modules.iter().enumerate() {
        name_to_index
            .entry(module.name.name.as_str())
            .or_insert(idx);
    }

    let mut indegree = vec![0usize; modules.len()];
    let mut edges: Vec<Vec<usize>> = vec![Vec::new(); modules.len()];

    for (idx, module) in modules.iter().enumerate() {
        for use_decl in module.uses.iter() {
            let Some(&dep_idx) = name_to_index.get(use_decl.module.name.as_str()) else {
                continue;
            };
            if dep_idx == idx {
                continue;
            }
            edges[dep_idx].push(idx);
            indegree[idx] += 1;
        }
    }

    let mut ready: Vec<usize> = indegree
        .iter()
        .enumerate()
        .filter_map(|(idx, &deg)| (deg == 0).then_some(idx))
        .collect();
    ready.sort_by(|a, b| modules[*a].name.name.cmp(&modules[*b].name.name));

    let mut out = Vec::new();
    let mut processed = vec![false; modules.len()];
    while let Some(idx) = ready.first().copied() {
        ready.remove(0);
        if processed[idx] {
            continue;
        }
        processed[idx] = true;
        out.push(idx);
        for &next in edges[idx].iter() {
            indegree[next] = indegree[next].saturating_sub(1);
            if indegree[next] == 0 && !processed[next] {
                ready.push(next);
                ready.sort_by(|a, b| modules[*a].name.name.cmp(&modules[*b].name.name));
            }
        }
    }

    let mut remaining: Vec<usize> = processed
        .iter()
        .enumerate()
        .filter_map(|(idx, done)| (!done).then_some(idx))
        .collect();
    remaining.sort_by(|a, b| modules[*a].name.name.cmp(&modules[*b].name.name));
    out.extend(remaining);
    out
}

pub fn check_types(modules: &[Module]) -> Vec<FileDiagnostic> {
    let mut checker = TypeChecker::new();
    let mut diagnostics = Vec::new();
    let mut module_exports: HashMap<String, HashMap<String, Scheme>> = HashMap::new();
    let mut module_class_exports: HashMap<String, HashMap<String, ClassDeclInfo>> = HashMap::new();
    let mut module_instance_exports: HashMap<String, Vec<InstanceDeclInfo>> = HashMap::new();

    for module in ordered_modules(modules) {
        checker.reset_module_context(module);
        let mut env = checker.builtins.clone();
        checker.register_module_types(module);
        diagnostics.extend(checker.collect_type_expr_diags(module));
        let sigs = checker.collect_type_sigs(module);
        checker.register_module_constructors(module, &mut env);
        checker.register_imports(module, &module_exports, &mut env);
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

        let mut exports = HashMap::new();
        for export in &module.exports {
            if let Some(scheme) = env.get(&export.name) {
                exports.insert(export.name.clone(), scheme.clone());
            }
        }
        module_exports.insert(module.name.name.clone(), exports);
        let (class_exports, instance_exports) =
            collect_exported_class_env(module, &checker.classes, &checker.instances);
        module_class_exports.insert(module.name.name.clone(), class_exports);
        module_instance_exports.insert(module.name.name.clone(), instance_exports);
    }

    diagnostics
}

pub fn elaborate_expected_coercions(modules: &mut [Module]) -> Vec<FileDiagnostic> {
    let mut checker = TypeChecker::new();
    let mut diagnostics = Vec::new();
    let mut module_exports: HashMap<String, HashMap<String, Scheme>> = HashMap::new();
    let mut module_class_exports: HashMap<String, HashMap<String, ClassDeclInfo>> = HashMap::new();
    let mut module_instance_exports: HashMap<String, Vec<InstanceDeclInfo>> = HashMap::new();

    for idx in ordered_module_indices(modules) {
        let module = &mut modules[idx];
        checker.reset_module_context(module);

        let mut env = checker.builtins.clone();
        checker.register_module_types(module);
        diagnostics.extend(checker.collect_type_expr_diags(module));
        let sigs = checker.collect_type_sigs(module);
        checker.register_module_constructors(module, &mut env);
        checker.register_imports(module, &module_exports, &mut env);

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
            if let Some(scheme) = env.get(&export.name) {
                exports.insert(export.name.clone(), scheme.clone());
            }
        }
        module_exports.insert(module.name.name.clone(), exports);
        let (class_exports, instance_exports) =
            collect_exported_class_env(module, &checker.classes, &checker.instances);
        module_class_exports.insert(module.name.name.clone(), class_exports);
        module_instance_exports.insert(module.name.name.clone(), instance_exports);
    }

    diagnostics
}

pub fn infer_value_types(
    modules: &[Module],
) -> (
    Vec<FileDiagnostic>,
    HashMap<String, HashMap<String, String>>,
) {
    let mut checker = TypeChecker::new();
    let mut diagnostics = Vec::new();
    let mut module_exports: HashMap<String, HashMap<String, Scheme>> = HashMap::new();
    let mut module_class_exports: HashMap<String, HashMap<String, ClassDeclInfo>> = HashMap::new();
    let mut module_instance_exports: HashMap<String, Vec<InstanceDeclInfo>> = HashMap::new();
    let mut inferred: HashMap<String, HashMap<String, String>> = HashMap::new();

    for module in ordered_modules(modules) {
        checker.reset_module_context(module);
        let mut env = checker.builtins.clone();
        checker.register_module_types(module);
        diagnostics.extend(checker.collect_type_expr_diags(module));
        let sigs = checker.collect_type_sigs(module);
        checker.register_module_constructors(module, &mut env);
        checker.register_imports(module, &module_exports, &mut env);
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
        for name in local_names {
            if let Some(scheme) = env.get(&name) {
                module_types.insert(name, checker.type_to_string(&scheme.ty));
            }
        }
        inferred.insert(module.name.name.clone(), module_types);

        let mut exports = HashMap::new();
        for export in &module.exports {
            if let Some(scheme) = env.get(&export.name) {
                exports.insert(export.name.clone(), scheme.clone());
            }
        }
        module_exports.insert(module.name.name.clone(), exports);
        let (class_exports, instance_exports) =
            collect_exported_class_env(module, &checker.classes, &checker.instances);
        module_class_exports.insert(module.name.name.clone(), class_exports);
        module_instance_exports.insert(module.name.name.clone(), instance_exports);
    }

    (diagnostics, inferred)
}
