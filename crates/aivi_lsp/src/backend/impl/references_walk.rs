impl Backend {
    fn collect_record_field_references(
        field: &RecordField,
        ident: &str,
        text: &str,
        uri: &Url,
        locations: &mut Vec<Location>,
    ) {
        for segment in field.path.iter() {
            Self::collect_path_segment_references(segment, ident, text, uri, locations);
        }
        Self::collect_expr_references(&field.value, ident, text, uri, locations);
    }

    fn collect_path_segment_references(
        segment: &PathSegment,
        ident: &str,
        text: &str,
        uri: &Url,
        locations: &mut Vec<Location>,
    ) {
        match segment {
            PathSegment::Field(name) => {
                if name.name == ident {
                    locations.push(Location::new(
                        uri.clone(),
                        Self::span_to_range(name.span.clone()),
                    ));
                }
            }
            PathSegment::Index(expr, _) => {
                Self::collect_expr_references(expr, ident, text, uri, locations);
            }
            PathSegment::All(_) => {}
        }
    }

    fn collect_match_arm_references(
        arm: &MatchArm,
        ident: &str,
        text: &str,
        uri: &Url,
        locations: &mut Vec<Location>,
    ) {
        Self::collect_pattern_references(&arm.pattern, ident, text, uri, locations);
        if let Some(guard) = &arm.guard {
            Self::collect_expr_references(guard, ident, text, uri, locations);
        }
        Self::collect_expr_references(&arm.body, ident, text, uri, locations);
    }

    fn collect_block_item_references(
        item: &BlockItem,
        ident: &str,
        text: &str,
        uri: &Url,
        locations: &mut Vec<Location>,
    ) {
        match item {
            BlockItem::Bind { pattern, expr, .. } => {
                Self::collect_pattern_references(pattern, ident, text, uri, locations);
                Self::collect_expr_references(expr, ident, text, uri, locations);
            }
            BlockItem::Let { pattern, expr, .. } => {
                Self::collect_pattern_references(pattern, ident, text, uri, locations);
                Self::collect_expr_references(expr, ident, text, uri, locations);
            }
            BlockItem::Filter { expr, .. }
            | BlockItem::Yield { expr, .. }
            | BlockItem::Recurse { expr, .. }
            | BlockItem::Expr { expr, .. } => {
                Self::collect_expr_references(expr, ident, text, uri, locations);
            }
            BlockItem::When { cond, effect, .. }
            | BlockItem::Unless { cond, effect, .. } => {
                Self::collect_expr_references(cond, ident, text, uri, locations);
                Self::collect_expr_references(effect, ident, text, uri, locations);
            }
            BlockItem::Given { cond, fail_expr, .. } => {
                Self::collect_expr_references(cond, ident, text, uri, locations);
                Self::collect_expr_references(fail_expr, ident, text, uri, locations);
            }
            BlockItem::On { transition, handler, .. } => {
                Self::collect_expr_references(transition, ident, text, uri, locations);
                Self::collect_expr_references(handler, ident, text, uri, locations);
            }
        }
    }

    fn format_type_decl(decl: &TypeDecl) -> String {
        let params = Self::format_type_params(&decl.params);
        let ctors = decl
            .constructors
            .iter()
            .map(Self::format_type_ctor)
            .collect::<Vec<_>>()
            .join(" | ");
        if ctors.is_empty() {
            format!("type {}{}", decl.name.name, params)
        } else {
            format!("type {}{} = {}", decl.name.name, params, ctors)
        }
    }

    fn format_type_alias(alias: &TypeAlias) -> String {
        let params = Self::format_type_params(&alias.params);
        let aliased = Self::type_expr_to_string(&alias.aliased);
        format!("type {}{} = {}", alias.name.name, params, aliased)
    }

    fn format_class_decl(class_decl: &ClassDecl) -> String {
        let params = class_decl
            .params
            .iter()
            .map(Self::type_expr_to_string)
            .collect::<Vec<_>>()
            .join(" ");
        if params.is_empty() {
            format!("class {}", class_decl.name.name)
        } else {
            format!("class {} {}", class_decl.name.name, params)
        }
    }

    fn format_instance_decl(instance_decl: &InstanceDecl) -> String {
        let params = instance_decl
            .params
            .iter()
            .map(Self::type_expr_to_string)
            .collect::<Vec<_>>()
            .join(" ");
        if params.is_empty() {
            format!("instance {}", instance_decl.name.name)
        } else {
            format!("instance {} {}", instance_decl.name.name, params)
        }
    }

    fn format_type_ctor(ctor: &TypeCtor) -> String {
        let args = ctor
            .args
            .iter()
            .map(Self::type_expr_to_string)
            .collect::<Vec<_>>()
            .join(" ");
        if args.is_empty() {
            ctor.name.name.clone()
        } else {
            format!("{} {}", ctor.name.name, args)
        }
    }

    fn format_type_params(params: &[SpannedName]) -> String {
        if params.is_empty() {
            String::new()
        } else {
            format!(
                " {}",
                params
                    .iter()
                    .map(|param| param.name.clone())
                    .collect::<Vec<_>>()
                    .join(" ")
            )
        }
    }

    pub(super) fn type_expr_to_string(expr: &TypeExpr) -> String {
        match expr {
            TypeExpr::Name(name) => name.name.clone(),
            TypeExpr::And { items, .. } => items
                .iter()
                .map(Self::type_expr_to_string)
                .collect::<Vec<_>>()
                .join(" with "),
            TypeExpr::Apply { base, args, .. } => {
                let base_str = match **base {
                    TypeExpr::Func { .. } => format!("({})", Self::type_expr_to_string(base)),
                    _ => Self::type_expr_to_string(base),
                };
                if args.is_empty() {
                    base_str
                } else {
                    let args_str = args
                        .iter()
                        .map(Self::type_expr_to_string)
                        .collect::<Vec<_>>()
                        .join(" ");
                    format!("{} {}", base_str, args_str)
                }
            }
            TypeExpr::Func { params, result, .. } => {
                let params_str = params
                    .iter()
                    .map(|param| match param {
                        TypeExpr::Func { .. } => format!("({})", Self::type_expr_to_string(param)),
                        _ => Self::type_expr_to_string(param),
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                let result_str = Self::type_expr_to_string(result);
                if params_str.is_empty() {
                    format!("-> {}", result_str)
                } else {
                    format!("{} -> {}", params_str, result_str)
                }
            }
            TypeExpr::Record { fields, .. } => {
                let fields_str = fields
                    .iter()
                    .map(|(name, ty)| format!("{}: {}", name.name, Self::type_expr_to_string(ty)))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{{{}}}", fields_str)
            }
            TypeExpr::Tuple { items, .. } => {
                let items_str = items
                    .iter()
                    .map(Self::type_expr_to_string)
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("({})", items_str)
            }
            TypeExpr::Star { .. } | TypeExpr::Unknown { .. } => "*".to_string(),
        }
    }

    pub(super) fn module_member_definition_range(module: &Module, ident: &str) -> Option<Range> {
        let matches = |name: &str| name == ident || name == format!("({})", ident);

        for item in module.items.iter() {
            match item {
                ModuleItem::Def(def) => {
                    if matches(&def.name.name) {
                        return Some(Self::span_to_range(def.name.span.clone()));
                    }
                }
                ModuleItem::TypeSig(sig) => {
                    if matches(&sig.name.name) {
                        return Some(Self::span_to_range(sig.name.span.clone()));
                    }
                }
                ModuleItem::TypeDecl(decl) => {
                    if decl.name.name == ident {
                        return Some(Self::span_to_range(decl.name.span.clone()));
                    }
                    for ctor in decl.constructors.iter() {
                        if ctor.name.name == ident {
                            return Some(Self::span_to_range(ctor.name.span.clone()));
                        }
                    }
                }
                ModuleItem::TypeAlias(alias) => {
                    if alias.name.name == ident {
                        return Some(Self::span_to_range(alias.name.span.clone()));
                    }
                }
                ModuleItem::ClassDecl(class_decl) => {
                    if class_decl.name.name == ident {
                        return Some(Self::span_to_range(class_decl.name.span.clone()));
                    }
                    for member in class_decl.members.iter() {
                        if matches(&member.name.name) {
                            return Some(Self::span_to_range(member.name.span.clone()));
                        }
                    }
                }
                ModuleItem::InstanceDecl(instance_decl) => {
                    if instance_decl.name.name == ident {
                        return Some(Self::span_to_range(instance_decl.name.span.clone()));
                    }
                    for def in instance_decl.defs.iter() {
                        if matches(&def.name.name) {
                            return Some(Self::span_to_range(def.name.span.clone()));
                        }
                    }
                }
                ModuleItem::MachineDecl(machine_decl) => {
                    if machine_decl.name.name == ident {
                        return Some(Self::span_to_range(machine_decl.name.span.clone()));
                    }
                    for state in machine_decl.states.iter() {
                        if state.name.name == ident {
                            return Some(Self::span_to_range(state.name.span.clone()));
                        }
                    }
                    for transition in machine_decl.transitions.iter() {
                        if transition.name.name == ident {
                            return Some(Self::span_to_range(transition.name.span.clone()));
                        }
                    }
                }
                ModuleItem::DomainDecl(domain_decl) => {
                    if domain_decl.name.name == ident {
                        return Some(Self::span_to_range(domain_decl.name.span.clone()));
                    }
                    for domain_item in domain_decl.items.iter() {
                        match domain_item {
                            DomainItem::TypeAlias(type_decl) => {
                                if type_decl.name.name == ident {
                                    return Some(Self::span_to_range(type_decl.name.span.clone()));
                                }
                                for ctor in type_decl.constructors.iter() {
                                    if ctor.name.name == ident {
                                        return Some(Self::span_to_range(ctor.name.span.clone()));
                                    }
                                }
                            }
                            DomainItem::TypeSig(_) => {}
                            DomainItem::Def(def) | DomainItem::LiteralDef(def) => {
                                if matches(&def.name.name) {
                                    return Some(Self::span_to_range(def.name.span.clone()));
                                }
                            }
                        }
                    }
                }
            }
        }
        None
    }

    pub(super) fn module_at_position(modules: &[Module], position: Position) -> Option<&Module> {
        modules.iter().find(|module| {
            let range = Self::span_to_range(module.span.clone());
            Self::range_contains_position(&range, position)
        })
    }

    pub(super) fn range_contains_position(range: &Range, position: Position) -> bool {
        let after_start = position.line > range.start.line
            || (position.line == range.start.line && position.character >= range.start.character);
        let before_end = position.line < range.end.line
            || (position.line == range.end.line && position.character < range.end.character);
        after_start && before_end
    }

    pub(super) fn path_from_uri(uri: &Url) -> String {
        uri.to_file_path()
            .unwrap_or_else(|_| PathBuf::from(uri.to_string()))
            .display()
            .to_string()
    }

    pub(super) fn stdlib_uri(name: &str) -> Url {
        Url::parse(&format!("aivi://stdlib/{name}")).expect("stdlib uri should be valid")
    }
}
