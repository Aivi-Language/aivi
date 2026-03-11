use std::collections::HashMap;

use crate::surface::{DomainItem, Module, ModuleItem};

use super::checker::TypeChecker;
use super::ordering::ordered_modules;
use super::types::{AliasInfo, Kind};
use super::TypeSurface;

pub(super) fn collect_global_type_info(
    checker: &mut TypeChecker,
    modules: &[Module],
) -> (
    HashMap<String, Kind>,
    HashMap<String, AliasInfo>,
    HashMap<String, String>,
) {
    let mut type_constructors = checker.builtin_type_constructors();
    let mut opaque_types: HashMap<String, String> = HashMap::new();

    let kind_for_params = |params_len: usize| {
        let mut kind = Kind::Star;
        for _ in 0..params_len {
            kind = Kind::Arrow(Box::new(Kind::Star), Box::new(kind));
        }
        kind
    };

    for module in modules {
        for item in &module.items {
            match item {
                ModuleItem::TypeDecl(type_decl) => {
                    let internal_name = format!("{}.{}", module.name.name, type_decl.name.name);
                    type_constructors.insert(
                        internal_name.clone(),
                        kind_for_params(type_decl.params.len()),
                    );
                    if type_decl.opaque {
                        opaque_types.insert(internal_name, module.name.name.clone());
                    }
                }
                ModuleItem::TypeAlias(_) => {}
                ModuleItem::DomainDecl(domain) => {
                    for domain_item in &domain.items {
                        if let DomainItem::TypeAlias(type_decl) = domain_item {
                            let internal_name =
                                format!("{}.{}", module.name.name, type_decl.name.name);
                            type_constructors
                                .insert(internal_name, kind_for_params(type_decl.params.len()));
                        }
                    }
                }
                _ => {}
            }
        }
    }

    let mut aliases = HashMap::new();
    let mut module_type_exports: HashMap<String, HashMap<String, TypeSurface>> = HashMap::new();
    for module in ordered_modules(modules) {
        checker.reset_module_context(module);
        checker.register_module_types(module);
        checker.register_imported_type_names(module, &module_type_exports);

        for item in &module.items {
            match item {
                ModuleItem::TypeDecl(type_decl) => {
                    let Some(internal_name) = checker.resolve_type_binding(&type_decl.name.name)
                    else {
                        continue;
                    };
                    let Some(kind) = checker.type_kind(internal_name).cloned() else {
                        continue;
                    };
                    type_constructors.insert(internal_name.to_string(), kind);
                    if type_decl.opaque {
                        opaque_types.insert(internal_name.to_string(), module.name.name.clone());
                    }
                }
                ModuleItem::TypeAlias(alias) => {
                    let Some(internal_name) = checker.resolve_type_binding(&alias.name.name) else {
                        continue;
                    };
                    let Some(kind) = checker.type_kind(internal_name).cloned() else {
                        continue;
                    };
                    let Some(alias_info) = checker.alias_info_for_name(internal_name).cloned()
                    else {
                        continue;
                    };
                    type_constructors.insert(internal_name.to_string(), kind);
                    aliases.insert(internal_name.to_string(), alias_info);
                    if alias.opaque {
                        opaque_types.insert(internal_name.to_string(), module.name.name.clone());
                    }
                }
                _ => {}
            }
        }

        let mut export_surfaces = HashMap::new();
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
            export_surfaces.insert(
                export.name.name.clone(),
                TypeSurface {
                    internal_name: internal_name.to_string(),
                    kind,
                    alias,
                    opaque_origin: checker.opaque_origin_for_name(internal_name).cloned(),
                },
            );
        }
        module_type_exports.insert(module.name.name.clone(), export_surfaces);
    }

    (type_constructors, aliases, opaque_types)
}
