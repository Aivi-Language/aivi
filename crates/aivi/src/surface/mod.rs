mod arena;
mod ast;
mod desugar;
mod parser;

pub use arena::*;
pub use ast::*;
pub use desugar::desugar_effect_sugars;
pub use parser::{parse_modules, parse_modules_from_tokens, resolve_import_names};

/// Compute the value-level import map for a module: bare_name → qualified_name.
///
/// Shared logic consumed by `resolve_import_names`, the resolver's `check_defs`,
/// and the type checker's `import_names_into_env`.  Encapsulates:
///
/// - Wildcard vs selective distinction
/// - Aliased imports are skipped (handled by `expand_module_aliases`)
/// - Local definitions shadow imports
/// - Later imports shadow earlier ones (last-wins)
///
/// `available_names` maps each module name to the set of importable value names.
/// Callers build this from their own data (module map, type exports, etc.).
pub fn compute_import_pairs(
    uses: &[UseDecl],
    available_names: &std::collections::HashMap<String, std::collections::HashSet<String>>,
    local_defs: &std::collections::HashSet<String>,
) -> std::collections::HashMap<String, String> {
    let mut import_map = std::collections::HashMap::new();
    for use_decl in uses {
        if use_decl.alias.is_some() {
            continue;
        }
        let target_module = &use_decl.module.name;
        let Some(target_names) = available_names.get(target_module.as_str()) else {
            continue;
        };
        if use_decl.items.is_empty() {
            // Wildcard import.
            for name in target_names {
                if !local_defs.contains(name) {
                    import_map.insert(name.clone(), format!("{target_module}.{name}"));
                }
            }
        } else {
            // Selective import.
            for item in &use_decl.items {
                if item.kind == ScopeItemKind::Value
                    && target_names.contains(&item.name.name)
                    && !local_defs.contains(&item.name.name)
                {
                    import_map.insert(
                        item.name.name.clone(),
                        format!("{target_module}.{}", item.name.name),
                    );
                }
            }
        }
    }
    import_map
}

#[cfg(test)]
mod tests;
