impl Backend {
    pub(super) fn collect_module_references(
        module: &Module,
        ident: &str,
        text: &str,
        uri: &Url,
        include_declaration: bool,
        locations: &mut Vec<Location>,
    ) {
        if include_declaration && module.name.name == ident {
            locations.push(Location::new(
                uri.clone(),
                Self::span_to_range(module.name.span.clone()),
            ));
        }
        for export in module.exports.iter() {
            if export.name.name == ident {
                locations.push(Location::new(
                    uri.clone(),
                    Self::span_to_range(export.name.span.clone()),
                ));
            }
        }
        for annotation in module.annotations.iter() {
            if annotation.name.name == ident {
                locations.push(Location::new(
                    uri.clone(),
                    Self::span_to_range(annotation.name.span.clone()),
                ));
            }
        }
        for use_decl in module.uses.iter() {
            Self::collect_use_references(use_decl, ident, uri, locations);
        }
        for item in module.items.iter() {
            Self::collect_item_references(item, ident, text, uri, include_declaration, locations);
        }
    }

    fn collect_use_references(
        use_decl: &UseDecl,
        ident: &str,
        uri: &Url,
        locations: &mut Vec<Location>,
    ) {
        if use_decl.module.name == ident {
            locations.push(Location::new(
                uri.clone(),
                Self::span_to_range(use_decl.module.span.clone()),
            ));
        }
        for item in use_decl.items.iter() {
            if item.name.name == ident {
                locations.push(Location::new(
                    uri.clone(),
                    Self::span_to_range(item.name.span.clone()),
                ));
            }
        }
    }
}
