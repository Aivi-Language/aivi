impl Backend {
    fn doc_block_above(text: &str, line: usize) -> Option<String> {
        let lines: Vec<&str> = text.lines().collect();
        let mut index = line.checked_sub(2)?;
        let mut docs = Vec::new();
        loop {
            let current = lines.get(index)?.trim_start();
            if current.is_empty() {
                break;
            }
            let Some(body) = current.strip_prefix("//") else {
                break;
            };
            docs.push(body.strip_prefix(' ').unwrap_or(body).to_string());
            if index == 0 {
                break;
            }
            index -= 1;
        }
        docs.reverse();
        (!docs.is_empty()).then_some(docs.join("\n"))
    }

    fn decl_line_for_ident(module: &Module, ident: &str) -> Option<usize> {
        if module.name.name == ident {
            return Some(module.name.span.start.line);
        }
        for item in module.items.iter() {
            match item {
                ModuleItem::Def(def) if def.name.name == ident => {
                    return Some(def.name.span.start.line);
                }
                ModuleItem::TypeSig(sig) if sig.name.name == ident => {
                    return Some(sig.name.span.start.line);
                }
                ModuleItem::TypeDecl(decl) if decl.name.name == ident => {
                    return Some(decl.name.span.start.line);
                }
                ModuleItem::TypeAlias(alias) if alias.name.name == ident => {
                    return Some(alias.name.span.start.line);
                }
                ModuleItem::ClassDecl(class_decl) if class_decl.name.name == ident => {
                    return Some(class_decl.name.span.start.line);
                }
                ModuleItem::InstanceDecl(instance_decl) if instance_decl.name.name == ident => {
                    return Some(instance_decl.name.span.start.line);
                }
                ModuleItem::DomainDecl(domain_decl) if domain_decl.name.name == ident => {
                    return Some(domain_decl.name.span.start.line);
                }
                ModuleItem::DomainDecl(domain_decl) => {
                    for domain_item in domain_decl.items.iter() {
                        match domain_item {
                            DomainItem::TypeAlias(type_decl) if type_decl.name.name == ident => {
                                return Some(type_decl.name.span.start.line);
                            }
                            DomainItem::TypeSig(sig) if sig.name.name == ident => {
                                return Some(sig.name.span.start.line);
                            }
                            DomainItem::Def(def) | DomainItem::LiteralDef(def)
                                if def.name.name == ident =>
                            {
                                return Some(def.name.span.start.line);
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
        None
    }

    pub(super) fn doc_for_ident(text: &str, module: &Module, ident: &str) -> Option<String> {
        let line = Self::decl_line_for_ident(module, ident)?;
        Self::doc_block_above(text, line)
    }

    pub(super) fn hover_contents_for_module(
        module: &Module,
        ident: &str,
        inferred: Option<&HashMap<String, String>>,
        doc: Option<&str>,
        doc_index: &DocIndex,
    ) -> Option<String> {
        if let Some(entry) = doc_index.lookup_best(ident, Some(module.name.name.as_str())) {
            return Some(Self::format_quick_info(entry, module, ident, inferred));
        }

        let mut base = Self::hover_base_for_module(module, ident, inferred)?;
        if let Some(doc) = doc {
            let doc = doc.trim();
            if !doc.is_empty() {
                base.push_str("\n\n");
                base.push_str(doc);
            }
        }
        Some(base)
    }

    fn hover_base_for_module(
        module: &Module,
        ident: &str,
        inferred: Option<&HashMap<String, String>>,
    ) -> Option<String> {
        let mut base = None;
        if module.name.name == ident {
            base = Some(format!("module `{}`", module.name.name));
        }
        let mut type_signatures = HashMap::new();
        for item in module.items.iter() {
            if let ModuleItem::TypeSig(sig) = item {
                type_signatures.insert(
                    sig.name.name.clone(),
                    format!(
                        "`{}` : `{}`",
                        sig.name.name,
                        Self::type_expr_to_string(&sig.ty)
                    ),
                );
            }
        }
        if base.is_none() {
            if let Some(sig) = type_signatures
                .get(ident)
                .or_else(|| type_signatures.get(&format!("({})", ident)))
            {
                base = Some(sig.clone());
            }
        }
        if base.is_none() {
            for item in module.items.iter() {
                if let Some(contents) =
                    Self::hover_contents_for_item(item, ident, &type_signatures, inferred)
                {
                    base = Some(contents);
                    break;
                }
            }
        }
        if base.is_none() {
            for domain in module.items.iter().filter_map(|item| match item {
                ModuleItem::DomainDecl(domain) => Some(domain),
                _ => None,
            }) {
                if let Some(contents) = Self::hover_contents_for_domain(domain, ident, inferred) {
                    base = Some(contents);
                    break;
                }
            }
        }
        base
    }

    fn format_quick_info(
        entry: &QuickInfoEntry,
        module: &Module,
        ident: &str,
        inferred: Option<&HashMap<String, String>>,
    ) -> String {
        // Prefer the existing hover logic for accurate types, but replace docs with spec-derived docs.
        let base = Self::hover_base_for_module(module, ident, inferred).unwrap_or_else(|| {
            match entry.kind {
                QuickInfoKind::Module => format!("module `{}`", entry.name),
                _ => format!("`{}`", entry.name),
            }
        });

        let mut out = base;
        if let Some(sig) = &entry.signature {
            // If the base is just a bare identifier, add a signature line.
            if !out.contains(" : `") && entry.kind != QuickInfoKind::Module {
                out = format!("`{}` : `{}`", entry.name, sig);
            }
        }

        if !entry.content.trim().is_empty() {
            out.push_str("\n\n");
            out.push_str(entry.content.trim());
        }
        out
    }

    fn hover_contents_for_item(
        item: &ModuleItem,
        ident: &str,
        type_signatures: &HashMap<String, String>,
        inferred: Option<&HashMap<String, String>>,
    ) -> Option<String> {
        let matches = |name: &str| name == ident || name == format!("({})", ident);

        match item {
            ModuleItem::Def(def) => {
                if matches(&def.name.name) {
                    if let Some(sig) = type_signatures
                        .get(ident)
                        .or_else(|| type_signatures.get(&format!("({})", ident)))
                    {
                        return Some(sig.clone());
                    }
                    if let Some(ty) = inferred.and_then(|types| {
                        types
                            .get(ident)
                            .or_else(|| types.get(&format!("({})", ident)))
                    }) {
                        return Some(format!("`{}` : `{}`", def.name.name, ty));
                    }
                    return Some(format!("`{}`", def.name.name));
                }
            }
            ModuleItem::TypeSig(sig) => {
                if matches(&sig.name.name) {
                    return Some(format!(
                        "`{}` : `{}`",
                        sig.name.name,
                        Self::type_expr_to_string(&sig.ty)
                    ));
                }
            }
            ModuleItem::TypeDecl(decl) => {
                if decl.name.name == ident {
                    return Some(format!("`{}`", Self::format_type_decl(decl)));
                }
            }
            ModuleItem::TypeAlias(alias) => {
                if alias.name.name == ident {
                    return Some(format!("`{}`", Self::format_type_alias(alias)));
                }
            }
            ModuleItem::ClassDecl(class_decl) => {
                if class_decl.name.name == ident {
                    return Some(format!("`{}`", Self::format_class_decl(class_decl)));
                }
                for member in class_decl.members.iter() {
                    if matches(&member.name.name) {
                        return Some(format!(
                            "`{}` : `{}`",
                            member.name.name,
                            Self::type_expr_to_string(&member.ty)
                        ));
                    }
                }
            }
            ModuleItem::InstanceDecl(instance_decl) => {
                if instance_decl.name.name == ident {
                    return Some(format!("`{}`", Self::format_instance_decl(instance_decl)));
                }
            }
            ModuleItem::DomainDecl(domain_decl) => {
                if domain_decl.name.name == ident {
                    return Some(format!(
                        "`domain {}` over `{}`",
                        domain_decl.name.name,
                        Self::type_expr_to_string(&domain_decl.over)
                    ));
                }
            }
            ModuleItem::MachineDecl(_) => {}
        }
        None
    }

    fn hover_contents_for_domain(
        domain_decl: &DomainDecl,
        ident: &str,
        inferred: Option<&HashMap<String, String>>,
    ) -> Option<String> {
        let matches = |name: &str| name == ident || name == format!("({})", ident);

        let mut type_signatures = HashMap::new();
        for item in domain_decl.items.iter() {
            if let DomainItem::TypeSig(sig) = item {
                type_signatures.insert(
                    sig.name.name.clone(),
                    format!(
                        "`{}` : `{}`",
                        sig.name.name,
                        Self::type_expr_to_string(&sig.ty)
                    ),
                );
            }
        }
        if let Some(sig) = type_signatures
            .get(ident)
            .or_else(|| type_signatures.get(&format!("({})", ident)))
        {
            return Some(sig.clone());
        }
        for item in domain_decl.items.iter() {
            match item {
                DomainItem::TypeAlias(type_decl) => {
                    if type_decl.name.name == ident {
                        return Some(format!("`{}`", Self::format_type_decl(type_decl)));
                    }
                }
                DomainItem::TypeSig(_) => {}
                DomainItem::Def(def) | DomainItem::LiteralDef(def) => {
                    if matches(&def.name.name) {
                        if let Some(sig) = type_signatures
                            .get(ident)
                            .or_else(|| type_signatures.get(&format!("({})", ident)))
                        {
                            return Some(sig.clone());
                        }
                        if let Some(ty) = inferred.and_then(|types| {
                            types
                                .get(ident)
                                .or_else(|| types.get(&format!("({})", ident)))
                        }) {
                            return Some(format!("`{}` : `{}`", def.name.name, ty));
                        }
                        return Some(format!("`{}`", def.name.name));
                    }
                }
            }
        }
        None
    }
}
