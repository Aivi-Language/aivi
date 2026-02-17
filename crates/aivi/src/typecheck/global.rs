use std::collections::HashMap;

use crate::surface::{DomainItem, Module, ModuleItem};

use super::checker::TypeChecker;
use super::types::{AliasInfo, Kind};

pub(super) fn collect_global_type_info(
    checker: &mut TypeChecker,
    modules: &[Module],
) -> (HashMap<String, Kind>, HashMap<String, AliasInfo>) {
    let mut type_constructors = checker.builtin_type_constructors();

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
                    type_constructors.insert(
                        type_decl.name.name.clone(),
                        kind_for_params(type_decl.params.len()),
                    );
                }
                ModuleItem::TypeAlias(alias) => {
                    type_constructors
                        .insert(alias.name.name.clone(), kind_for_params(alias.params.len()));
                }
                ModuleItem::DomainDecl(domain) => {
                    for domain_item in &domain.items {
                        if let DomainItem::TypeAlias(type_decl) = domain_item {
                            type_constructors.insert(
                                type_decl.name.name.clone(),
                                kind_for_params(type_decl.params.len()),
                            );
                        }
                    }
                }
                _ => {}
            }
        }
    }

    // Compute alias bodies using a context that recognizes all known type constructors, so
    // imported type aliases don't degrade into fresh type variables.
    let prev_constructors = checker.type_constructors.clone();
    checker.type_constructors = type_constructors.clone();
    let mut aliases = HashMap::new();
    for module in modules {
        for item in &module.items {
            if let ModuleItem::TypeAlias(alias) = item {
                aliases.insert(alias.name.name.clone(), checker.alias_info(alias));
            }
        }
    }
    checker.type_constructors = prev_constructors;

    (type_constructors, aliases)
}
