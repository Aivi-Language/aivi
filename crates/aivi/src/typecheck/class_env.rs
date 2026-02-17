use std::collections::{HashMap, HashSet};

use crate::surface::{Module, ModuleItem, ScopeItemKind, TypeExpr};

#[derive(Clone, Debug)]
pub(super) struct ClassDeclInfo {
    pub(super) params: Vec<TypeExpr>,
    pub(super) supers: Vec<TypeExpr>,
    pub(super) constraints: Vec<(String, String)>,
    // Members declared directly on the class (excluding inherited members). Used for resolving
    // method names without introducing ambiguity from superclass expansion.
    pub(super) direct_members: HashMap<String, TypeExpr>,
    pub(super) members: HashMap<String, TypeExpr>,
}

#[derive(Clone, Debug)]
pub(super) struct InstanceDeclInfo {
    pub(super) class_name: String,
    pub(super) params: Vec<TypeExpr>,
}

pub(super) fn collect_local_class_env(
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
                let direct_members = members.clone();
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
                        direct_members,
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

pub(super) fn expand_classes(
    mut classes: HashMap<String, ClassDeclInfo>,
) -> HashMap<String, ClassDeclInfo> {
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

pub(super) fn collect_imported_class_env(
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
            if item.kind != ScopeItemKind::Value {
                continue;
            }
            if let Some(info) = class_exports.get(&item.name.name) {
                classes.insert(item.name.name.clone(), info.clone());
                imported_classes.insert(item.name.name.clone());
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

pub(super) fn collect_exported_class_env(
    module: &Module,
    classes: &HashMap<String, ClassDeclInfo>,
    instances: &[InstanceDeclInfo],
) -> (HashMap<String, ClassDeclInfo>, Vec<InstanceDeclInfo>) {
    let mut class_exports = HashMap::new();
    let mut exported_class_names = HashSet::new();
    for export in &module.exports {
        if export.kind != ScopeItemKind::Value {
            continue;
        }
        if let Some(info) = classes.get(&export.name.name) {
            class_exports.insert(export.name.name.clone(), info.clone());
            exported_class_names.insert(export.name.name.clone());
        }
    }
    let instance_exports = instances
        .iter()
        .filter(|instance| exported_class_names.contains(&instance.class_name))
        .cloned()
        .collect();
    (class_exports, instance_exports)
}
