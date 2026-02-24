use std::collections::{HashMap, HashSet};

use crate::surface::{Module, ModuleItem, ScopeItemKind, TypeDecl, TypeExpr};

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

const AUTO_FORWARD_DECORATOR: &str = "__auto_forward";

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

fn type_expr_eq(left: &TypeExpr, right: &TypeExpr) -> bool {
    match (left, right) {
        (TypeExpr::Name(left_name), TypeExpr::Name(right_name)) => {
            left_name.name == right_name.name
        }
        (
            TypeExpr::And {
                items: left_items, ..
            },
            TypeExpr::And {
                items: right_items, ..
            },
        ) => {
            left_items.len() == right_items.len()
                && left_items
                    .iter()
                    .zip(right_items.iter())
                    .all(|(left_item, right_item)| type_expr_eq(left_item, right_item))
        }
        (
            TypeExpr::Apply {
                base: left_base,
                args: left_args,
                ..
            },
            TypeExpr::Apply {
                base: right_base,
                args: right_args,
                ..
            },
        ) => {
            left_args.len() == right_args.len()
                && type_expr_eq(left_base, right_base)
                && left_args
                    .iter()
                    .zip(right_args.iter())
                    .all(|(left_arg, right_arg)| type_expr_eq(left_arg, right_arg))
        }
        (
            TypeExpr::Func {
                params: left_params,
                result: left_result,
                ..
            },
            TypeExpr::Func {
                params: right_params,
                result: right_result,
                ..
            },
        ) => {
            left_params.len() == right_params.len()
                && left_params
                    .iter()
                    .zip(right_params.iter())
                    .all(|(left_param, right_param)| type_expr_eq(left_param, right_param))
                && type_expr_eq(left_result, right_result)
        }
        (
            TypeExpr::Record {
                fields: left_fields,
                ..
            },
            TypeExpr::Record {
                fields: right_fields,
                ..
            },
        ) => {
            left_fields.len() == right_fields.len()
                && left_fields.iter().zip(right_fields.iter()).all(
                    |((left_name, left_ty), (right_name, right_ty))| {
                        left_name.name == right_name.name && type_expr_eq(left_ty, right_ty)
                    },
                )
        }
        (
            TypeExpr::Tuple {
                items: left_items, ..
            },
            TypeExpr::Tuple {
                items: right_items, ..
            },
        ) => {
            left_items.len() == right_items.len()
                && left_items
                    .iter()
                    .zip(right_items.iter())
                    .all(|(left_item, right_item)| type_expr_eq(left_item, right_item))
        }
        (TypeExpr::Star { .. }, TypeExpr::Star { .. }) => true,
        _ => false,
    }
}

fn rewrite_type_expr(ty: &TypeExpr, from: &TypeExpr, to: &TypeExpr) -> (TypeExpr, bool) {
    if type_expr_eq(ty, from) {
        return (to.clone(), true);
    }
    match ty {
        TypeExpr::Name(_) | TypeExpr::Star { .. } | TypeExpr::Unknown { .. } => (ty.clone(), false),
        TypeExpr::And { items, span } => {
            let mut changed = false;
            let items = items
                .iter()
                .map(|item| {
                    let (rewritten, did_change) = rewrite_type_expr(item, from, to);
                    changed |= did_change;
                    rewritten
                })
                .collect();
            (
                TypeExpr::And {
                    items,
                    span: span.clone(),
                },
                changed,
            )
        }
        TypeExpr::Apply { base, args, span } => {
            let (rewritten_base, base_changed) = rewrite_type_expr(base, from, to);
            let mut changed = base_changed;
            let args = args
                .iter()
                .map(|arg| {
                    let (rewritten, did_change) = rewrite_type_expr(arg, from, to);
                    changed |= did_change;
                    rewritten
                })
                .collect();
            (
                TypeExpr::Apply {
                    base: Box::new(rewritten_base),
                    args,
                    span: span.clone(),
                },
                changed,
            )
        }
        TypeExpr::Func {
            params,
            result,
            span,
        } => {
            let mut changed = false;
            let params = params
                .iter()
                .map(|param| {
                    let (rewritten, did_change) = rewrite_type_expr(param, from, to);
                    changed |= did_change;
                    rewritten
                })
                .collect();
            let (rewritten_result, result_changed) = rewrite_type_expr(result, from, to);
            changed |= result_changed;
            (
                TypeExpr::Func {
                    params,
                    result: Box::new(rewritten_result),
                    span: span.clone(),
                },
                changed,
            )
        }
        TypeExpr::Record { fields, span } => {
            let mut changed = false;
            let fields = fields
                .iter()
                .map(|(name, field_ty)| {
                    let (rewritten, did_change) = rewrite_type_expr(field_ty, from, to);
                    changed |= did_change;
                    (name.clone(), rewritten)
                })
                .collect();
            (
                TypeExpr::Record {
                    fields,
                    span: span.clone(),
                },
                changed,
            )
        }
        TypeExpr::Tuple { items, span } => {
            let mut changed = false;
            let items = items
                .iter()
                .map(|item| {
                    let (rewritten, did_change) = rewrite_type_expr(item, from, to);
                    changed |= did_change;
                    rewritten
                })
                .collect();
            (
                TypeExpr::Tuple {
                    items,
                    span: span.clone(),
                },
                changed,
            )
        }
    }
}

fn instance_decl_eq(left: &InstanceDeclInfo, right: &InstanceDeclInfo) -> bool {
    left.class_name == right.class_name
        && left.params.len() == right.params.len()
        && left
            .params
            .iter()
            .zip(right.params.iter())
            .all(|(left_param, right_param)| type_expr_eq(left_param, right_param))
}

fn auto_forward_decl(type_decl: &TypeDecl) -> Option<(TypeExpr, TypeExpr)> {
    if !type_decl
        .decorators
        .iter()
        .any(|decorator| decorator.name.name == AUTO_FORWARD_DECORATOR)
    {
        return None;
    }
    if !type_decl.params.is_empty() {
        return None;
    }
    let constructor = type_decl.constructors.first()?;
    if type_decl.constructors.len() != 1
        || constructor.name.name != type_decl.name.name
        || constructor.args.len() != 1
    {
        return None;
    }
    let base = constructor.args.first()?.clone();
    let branded = TypeExpr::Name(type_decl.name.clone());
    Some((base, branded))
}

pub(super) fn synthesize_auto_forward_instances(
    module: &Module,
    instances: &[InstanceDeclInfo],
) -> Vec<InstanceDeclInfo> {
    let mut synthesized = Vec::new();
    for item in &module.items {
        let ModuleItem::TypeDecl(type_decl) = item else {
            continue;
        };
        let Some((base_type, branded_type)) = auto_forward_decl(type_decl) else {
            continue;
        };

        for instance in instances {
            let mut changed = false;
            let params = instance
                .params
                .iter()
                .map(|param| {
                    let (rewritten, did_change) =
                        rewrite_type_expr(param, &base_type, &branded_type);
                    changed |= did_change;
                    rewritten
                })
                .collect::<Vec<_>>();
            if !changed {
                continue;
            }

            let candidate = InstanceDeclInfo {
                class_name: instance.class_name.clone(),
                params,
            };
            if instances
                .iter()
                .any(|existing| instance_decl_eq(existing, &candidate))
                || synthesized
                    .iter()
                    .any(|existing| instance_decl_eq(existing, &candidate))
            {
                continue;
            }
            synthesized.push(candidate);
        }
    }
    synthesized
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
