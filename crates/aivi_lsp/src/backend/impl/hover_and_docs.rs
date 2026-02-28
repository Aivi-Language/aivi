impl Backend {
    pub(super) fn hover_badge_markdown(kind: &str, body: String) -> String {
        format!("`{kind}`\n\n{body}")
    }

    fn is_primitive_ident(ident: &str) -> bool {
        matches!(
            ident,
            "Int"
                | "Bool"
                | "Number"
                | "Float"
                | "Text"
                | "Char"
                | "Bytes"
                | "Unit"
                | "Date"
                | "Time"
                | "DateTime"
                | "Duration"
        )
    }

    fn is_operator_ident(ident: &str) -> bool {
        !ident.is_empty() && ident.chars().any(|ch| !ch.is_alphanumeric() && ch != '_' && ch != '.')
    }

    fn quick_info_badge(kind: &QuickInfoKind) -> &'static str {
        match kind {
            QuickInfoKind::Module => "module",
            QuickInfoKind::Function => "function",
            QuickInfoKind::Type => "type",
            QuickInfoKind::Class => "class",
            QuickInfoKind::Domain => "domain",
            QuickInfoKind::Operator => "operator",
            QuickInfoKind::ClassMember => "class-member",
            QuickInfoKind::Unknown => "value",
        }
    }

    fn hover_badge_for_module_ident(
        module: &Module,
        ident: &str,
        inferred: Option<&HashMap<String, String>>,
    ) -> Option<&'static str> {
        if module.name.name == ident {
            return Some("module");
        }
        if Self::is_operator_ident(ident) {
            return Some("operator");
        }
        if Self::is_primitive_ident(ident) {
            return Some("primitive");
        }

        for item in module.items.iter() {
            match item {
                ModuleItem::Def(def) if def.name.name == ident => {
                    return Some(if def.params.is_empty() {
                        "value"
                    } else {
                        "function"
                    });
                }
                ModuleItem::TypeSig(sig) if sig.name.name == ident => {
                    return Some("function");
                }
                ModuleItem::TypeDecl(decl) if decl.name.name == ident => return Some("type"),
                ModuleItem::TypeDecl(decl) => {
                    if decl.constructors.iter().any(|ctor| ctor.name.name == ident) {
                        return Some("constructor");
                    }
                }
                ModuleItem::TypeAlias(alias) if alias.name.name == ident => return Some("type-alias"),
                ModuleItem::ClassDecl(class_decl) if class_decl.name.name == ident => {
                    return Some("class");
                }
                ModuleItem::ClassDecl(class_decl)
                    if class_decl.members.iter().any(|member| member.name.name == ident) =>
                {
                    return Some("class-member");
                }
                ModuleItem::InstanceDecl(instance_decl) if instance_decl.name.name == ident => {
                    return Some("instance");
                }
                ModuleItem::InstanceDecl(instance_decl)
                    if instance_decl.defs.iter().any(|def| def.name.name == ident) =>
                {
                    return Some("function");
                }
                ModuleItem::DomainDecl(domain_decl) if domain_decl.name.name == ident => {
                    return Some("domain");
                }
                ModuleItem::DomainDecl(domain_decl) => {
                    for domain_item in domain_decl.items.iter() {
                        match domain_item {
                            DomainItem::TypeAlias(type_decl) if type_decl.name.name == ident => {
                                return Some("type");
                            }
                            DomainItem::TypeSig(sig) if sig.name.name == ident => {
                                return Some("function");
                            }
                            DomainItem::Def(def) | DomainItem::LiteralDef(def)
                                if def.name.name == ident =>
                            {
                                return Some(if def.params.is_empty() { "value" } else { "function" });
                            }
                            _ => {}
                        }
                    }
                }
                ModuleItem::MachineDecl(machine_decl) if machine_decl.name.name == ident => {
                    return Some("machine");
                }
                ModuleItem::MachineDecl(machine_decl)
                    if machine_decl.states.iter().any(|state| state.name.name == ident) =>
                {
                    return Some("machine-state");
                }
                ModuleItem::MachineDecl(machine_decl)
                    if machine_decl.transitions.iter().any(|transition| transition.name.name == ident) =>
                {
                    return Some("machine-transition");
                }
                _ => {}
            }
        }
        if inferred
            .is_some_and(|types| types.contains_key(ident) || types.contains_key(&format!("({ident})")))
        {
            return Some("value");
        }
        None
    }

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
                ModuleItem::MachineDecl(machine_decl) if machine_decl.name.name == ident => {
                    return Some(machine_decl.name.span.start.line);
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
                ModuleItem::MachineDecl(machine_decl) => {
                    for state in machine_decl.states.iter() {
                        if state.name.name == ident {
                            return Some(state.name.span.start.line);
                        }
                    }
                    for transition in machine_decl.transitions.iter() {
                        if transition.name.name == ident {
                            return Some(transition.name.span.start.line);
                        }
                        if transition.source.name == ident {
                            return Some(transition.source.span.start.line);
                        }
                        if transition.target.name == ident {
                            return Some(transition.target.span.start.line);
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
        let kind = Self::hover_badge_for_module_ident(module, ident, inferred).unwrap_or("value");
        Some(Self::hover_badge_markdown(kind, base))
    }

    fn hover_base_for_module(
        module: &Module,
        ident: &str,
        inferred: Option<&HashMap<String, String>>,
    ) -> Option<String> {
        let mut base = None;
        if Self::is_primitive_ident(ident) {
            base = Some(format!("`{ident}`"));
        }
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

    pub(super) fn hover_contents_for_primitive_value(token: &str) -> Option<String> {
        let ty = match token {
            "true" | "false" => "Bool",
            _ if token.parse::<i64>().is_ok() => "Int",
            _ if token.contains('.') && token.parse::<f64>().is_ok() => "Float",
            _ => return None,
        };
        Some(Self::hover_badge_markdown(
            "value",
            format!("`{token}` : `{ty}`"),
        ))
    }

    /// Fallback hover: find the smallest span in `span_types` that contains the
    /// cursor position and return the recorded type.
    pub(super) fn hover_from_span_types(
        ident: &str,
        position: Position,
        span_types: &HashMap<String, Vec<(Span, String)>>,
        module_name: &str,
    ) -> Option<String> {
        let entries = span_types.get(module_name)?;
        // LSP Position is 0-based; our Span uses 1-based lines and 1-based columns.
        let line = position.line as usize + 1;
        let col = position.character as usize + 1;
        let mut best: Option<&(Span, String)> = None;
        for entry in entries {
            let s = &entry.0;
            let start_ok = s.start.line < line || (s.start.line == line && s.start.column <= col);
            let end_ok = s.end.line > line || (s.end.line == line && s.end.column >= col);
            if start_ok && end_ok {
                if let Some(prev) = best {
                    let prev_size = Self::span_size(&prev.0);
                    let cur_size = Self::span_size(s);
                    if cur_size < prev_size {
                        best = Some(entry);
                    }
                } else {
                    best = Some(entry);
                }
            }
        }
        let (_, ty_str) = best?;
        Some(Self::hover_badge_markdown(
            "value",
            format!("`{ident}` : `{ty_str}`"),
        ))
    }

    fn span_size(s: &Span) -> usize {
        let lines = s.end.line.saturating_sub(s.start.line);
        if lines == 0 {
            s.end.column.saturating_sub(s.start.column) + 1
        } else {
            lines * 1000 + s.end.column
        }
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
        Self::hover_badge_markdown(Self::quick_info_badge(&entry.kind), out)
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
            ModuleItem::MachineDecl(machine_decl) => {
                if machine_decl.name.name == ident {
                    return Some(format!("`machine {}`", machine_decl.name.name));
                }
                for state in machine_decl.states.iter() {
                    if state.name.name == ident {
                        return Some(format!(
                            "state `{}` in machine `{}`",
                            state.name.name,
                            machine_decl.name.name
                        ));
                    }
                }
                for transition in machine_decl.transitions.iter() {
                    if transition.name.name == ident {
                        let payload = if transition.payload.is_empty() {
                            "{}".to_string()
                        } else {
                            let fields = transition
                                .payload
                                .iter()
                                .map(|(name, ty)| {
                                    format!("{}: {}", name.name, Self::type_expr_to_string(ty))
                                })
                                .collect::<Vec<_>>()
                                .join(", ");
                            format!("{{{fields}}}")
                        };
                        return Some(format!(
                            "`{} -> {} : {} {}`",
                            transition.source.name,
                            transition.target.name,
                            transition.name.name,
                            payload
                        ));
                    }
                    if transition.source.name == ident || transition.target.name == ident {
                        return Some(format!(
                            "state `{}` in machine `{}`",
                            ident,
                            machine_decl.name.name
                        ));
                    }
                }
            }
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

    /// Recursively collect non-primitive, non-variable type names from a TypeExpr.
    pub(super) fn collect_type_names(expr: &TypeExpr) -> Vec<String> {
        let mut names = Vec::new();
        Self::collect_type_names_inner(expr, &mut names);
        names
    }

    fn collect_type_names_inner(expr: &TypeExpr, names: &mut Vec<String>) {
        match expr {
            TypeExpr::Name(name) => {
                let n = &name.name;
                if !n.is_empty()
                    && n.chars().next().is_some_and(|c| c.is_uppercase())
                    && !Self::is_primitive_ident(n)
                    && !names.contains(n)
                {
                    names.push(n.clone());
                }
            }
            TypeExpr::Apply { base, args, .. } => {
                Self::collect_type_names_inner(base, names);
                for arg in args {
                    Self::collect_type_names_inner(arg, names);
                }
            }
            TypeExpr::Func { params, result, .. } => {
                for param in params {
                    Self::collect_type_names_inner(param, names);
                }
                Self::collect_type_names_inner(result, names);
            }
            TypeExpr::Record { fields, .. } => {
                for (_, ty) in fields {
                    Self::collect_type_names_inner(ty, names);
                }
            }
            TypeExpr::Tuple { items, .. } | TypeExpr::And { items, .. } => {
                for item in items {
                    Self::collect_type_names_inner(item, names);
                }
            }
            TypeExpr::Star { .. } | TypeExpr::Unknown { .. } => {}
        }
    }

    /// Look up a concise type definition for `name` in a single module.
    pub(super) fn find_type_definition_brief(module: &Module, name: &str) -> Option<String> {
        for item in module.items.iter() {
            match item {
                ModuleItem::TypeDecl(decl) if decl.name.name == name => {
                    return Some(Self::format_type_decl(decl));
                }
                ModuleItem::TypeAlias(alias) if alias.name.name == name => {
                    return Some(Self::format_type_alias(alias));
                }
                ModuleItem::ClassDecl(class_decl) if class_decl.name.name == name => {
                    return Some(Self::format_class_decl(class_decl));
                }
                ModuleItem::DomainDecl(domain_decl) => {
                    for domain_item in domain_decl.items.iter() {
                        if let DomainItem::TypeAlias(type_decl) = domain_item {
                            if type_decl.name.name == name {
                                return Some(Self::format_type_decl(type_decl));
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        None
    }
}
