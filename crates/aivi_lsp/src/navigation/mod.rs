mod definition;
mod hover;
mod references;

/// For a selective import list, check if `ident` matches any item either by
/// original name or by alias. Returns the *original* (exported) name so
/// callers can look it up in the target module.
pub(crate) fn resolve_import_name<'a>(items: &'a [aivi::UseItem], ident: &str) -> Option<&'a str> {
    items.iter().find_map(|item| {
        let matches =
            item.name.name == ident || item.alias.as_ref().is_some_and(|a| a.name == ident);
        if matches {
            Some(item.name.name.as_str())
        } else {
            None
        }
    })
}

/// For a module import alias like `use foo.bar as Baz`, resolve `Baz` back to
/// the original module path `foo.bar`.
pub(crate) fn resolve_module_alias<'a>(
    use_decls: &'a [aivi::UseDecl],
    ident: &str,
) -> Option<&'a str> {
    use_decls.iter().find_map(|use_decl| {
        use_decl
            .alias
            .as_ref()
            .filter(|alias| alias.name == ident)
            .map(|_| use_decl.module.name.as_str())
    })
}
