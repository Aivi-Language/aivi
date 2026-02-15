impl Backend {
    fn collect_item_references(
        item: &ModuleItem,
        ident: &str,
        text: &str,
        uri: &Url,
        include_declaration: bool,
        locations: &mut Vec<Location>,
    ) {
        match item {
            ModuleItem::Def(def) => {
                Self::collect_def_references(def, ident, text, uri, include_declaration, locations);
            }
            ModuleItem::TypeSig(sig) => {
                let matches = |name: &str| name == ident || name == format!("({})", ident);
                if include_declaration && matches(&sig.name.name) {
                    locations.push(Location::new(
                        uri.clone(),
                        Self::span_to_range(sig.name.span.clone()),
                    ));
                }
                Self::collect_type_expr_references(&sig.ty, ident, uri, locations);
            }
            ModuleItem::TypeDecl(decl) => {
                Self::collect_type_decl_references(
                    decl,
                    ident,
                    uri,
                    include_declaration,
                    locations,
                );
            }
            ModuleItem::TypeAlias(alias) => {
                Self::collect_type_alias_references(
                    alias,
                    ident,
                    uri,
                    include_declaration,
                    locations,
                );
            }
            ModuleItem::ClassDecl(class_decl) => {
                Self::collect_class_references(
                    class_decl,
                    ident,
                    uri,
                    include_declaration,
                    locations,
                );
            }
            ModuleItem::InstanceDecl(instance_decl) => {
                Self::collect_instance_references(
                    instance_decl,
                    ident,
                    text,
                    uri,
                    include_declaration,
                    locations,
                );
            }
            ModuleItem::DomainDecl(domain_decl) => {
                Self::collect_domain_references(
                    domain_decl,
                    ident,
                    text,
                    uri,
                    include_declaration,
                    locations,
                );
            }
        }
    }

    fn collect_def_references(
        def: &Def,
        ident: &str,
        text: &str,
        uri: &Url,
        include_declaration: bool,
        locations: &mut Vec<Location>,
    ) {
        let matches = |name: &str| name == ident || name == format!("({})", ident);
        if include_declaration && matches(&def.name.name) {
            locations.push(Location::new(
                uri.clone(),
                Self::span_to_range(def.name.span.clone()),
            ));
        }
        for param in def.params.iter() {
            Self::collect_pattern_references(param, ident, text, uri, locations);
        }
        Self::collect_expr_references(&def.expr, ident, text, uri, locations);
    }

    fn collect_type_decl_references(
        decl: &TypeDecl,
        ident: &str,
        uri: &Url,
        include_declaration: bool,
        locations: &mut Vec<Location>,
    ) {
        if include_declaration && decl.name.name == ident {
            locations.push(Location::new(
                uri.clone(),
                Self::span_to_range(decl.name.span.clone()),
            ));
        }
        for param in decl.params.iter() {
            if param.name == ident {
                locations.push(Location::new(
                    uri.clone(),
                    Self::span_to_range(param.span.clone()),
                ));
            }
        }
        for ctor in decl.constructors.iter() {
            Self::collect_type_ctor_references(ctor, ident, uri, include_declaration, locations);
        }
    }

    fn collect_type_alias_references(
        alias: &TypeAlias,
        ident: &str,
        uri: &Url,
        include_declaration: bool,
        locations: &mut Vec<Location>,
    ) {
        if include_declaration && alias.name.name == ident {
            locations.push(Location::new(
                uri.clone(),
                Self::span_to_range(alias.name.span.clone()),
            ));
        }
        for param in alias.params.iter() {
            if param.name == ident {
                locations.push(Location::new(
                    uri.clone(),
                    Self::span_to_range(param.span.clone()),
                ));
            }
        }
        Self::collect_type_expr_references(&alias.aliased, ident, uri, locations);
    }

    fn collect_class_references(
        class_decl: &ClassDecl,
        ident: &str,
        uri: &Url,
        include_declaration: bool,
        locations: &mut Vec<Location>,
    ) {
        if include_declaration && class_decl.name.name == ident {
            locations.push(Location::new(
                uri.clone(),
                Self::span_to_range(class_decl.name.span.clone()),
            ));
        }
        for param in class_decl.params.iter() {
            Self::collect_type_expr_references(param, ident, uri, locations);
        }
        let matches = |name: &str| name == ident || name == format!("({})", ident);
        for member in class_decl.members.iter() {
            if include_declaration && matches(&member.name.name) {
                locations.push(Location::new(
                    uri.clone(),
                    Self::span_to_range(member.name.span.clone()),
                ));
            }
            Self::collect_type_expr_references(&member.ty, ident, uri, locations);
        }
    }

    fn collect_instance_references(
        instance_decl: &InstanceDecl,
        ident: &str,
        text: &str,
        uri: &Url,
        include_declaration: bool,
        locations: &mut Vec<Location>,
    ) {
        if include_declaration && instance_decl.name.name == ident {
            locations.push(Location::new(
                uri.clone(),
                Self::span_to_range(instance_decl.name.span.clone()),
            ));
        }
        for param in instance_decl.params.iter() {
            Self::collect_type_expr_references(param, ident, uri, locations);
        }
        for def in instance_decl.defs.iter() {
            Self::collect_def_references(def, ident, text, uri, include_declaration, locations);
        }
    }

    fn collect_domain_references(
        domain_decl: &DomainDecl,
        ident: &str,
        text: &str,
        uri: &Url,
        include_declaration: bool,
        locations: &mut Vec<Location>,
    ) {
        if include_declaration && domain_decl.name.name == ident {
            locations.push(Location::new(
                uri.clone(),
                Self::span_to_range(domain_decl.name.span.clone()),
            ));
        }
        Self::collect_type_expr_references(&domain_decl.over, ident, uri, locations);
        for item in domain_decl.items.iter() {
            match item {
                DomainItem::TypeAlias(decl) => {
                    Self::collect_type_decl_references(
                        decl,
                        ident,
                        uri,
                        include_declaration,
                        locations,
                    );
                }
                DomainItem::TypeSig(_) => {}
                DomainItem::Def(def) | DomainItem::LiteralDef(def) => {
                    Self::collect_def_references(
                        def,
                        ident,
                        text,
                        uri,
                        include_declaration,
                        locations,
                    );
                }
            }
        }
    }

    fn collect_type_ctor_references(
        ctor: &TypeCtor,
        ident: &str,
        uri: &Url,
        include_declaration: bool,
        locations: &mut Vec<Location>,
    ) {
        if include_declaration && ctor.name.name == ident {
            locations.push(Location::new(
                uri.clone(),
                Self::span_to_range(ctor.name.span.clone()),
            ));
        }
        for arg in ctor.args.iter() {
            Self::collect_type_expr_references(arg, ident, uri, locations);
        }
    }

    fn collect_type_expr_references(
        expr: &TypeExpr,
        ident: &str,
        uri: &Url,
        locations: &mut Vec<Location>,
    ) {
        match expr {
            TypeExpr::Name(name) => {
                if name.name == ident {
                    locations.push(Location::new(
                        uri.clone(),
                        Self::span_to_range(name.span.clone()),
                    ));
                }
            }
            TypeExpr::And { items, .. } => {
                for item in items.iter() {
                    Self::collect_type_expr_references(item, ident, uri, locations);
                }
            }
            TypeExpr::Apply { base, args, .. } => {
                Self::collect_type_expr_references(base, ident, uri, locations);
                for arg in args.iter() {
                    Self::collect_type_expr_references(arg, ident, uri, locations);
                }
            }
            TypeExpr::Func { params, result, .. } => {
                for param in params.iter() {
                    Self::collect_type_expr_references(param, ident, uri, locations);
                }
                Self::collect_type_expr_references(result, ident, uri, locations);
            }
            TypeExpr::Record { fields, .. } => {
                for (name, ty) in fields.iter() {
                    if name.name == ident {
                        locations.push(Location::new(
                            uri.clone(),
                            Self::span_to_range(name.span.clone()),
                        ));
                    }
                    Self::collect_type_expr_references(ty, ident, uri, locations);
                }
            }
            TypeExpr::Tuple { items, .. } => {
                for item in items.iter() {
                    Self::collect_type_expr_references(item, ident, uri, locations);
                }
            }
            TypeExpr::Star { .. } | TypeExpr::Unknown { .. } => {}
        }
    }

    fn collect_pattern_references(
        pattern: &Pattern,
        ident: &str,
        text: &str,
        uri: &Url,
        locations: &mut Vec<Location>,
    ) {
        match pattern {
            Pattern::Ident(name) => {
                if name.name == ident {
                    locations.push(Location::new(
                        uri.clone(),
                        Self::span_to_range(name.span.clone()),
                    ));
                }
            }
            Pattern::SubjectIdent(name) => {
                if name.name == ident {
                    locations.push(Location::new(
                        uri.clone(),
                        Self::span_to_range(name.span.clone()),
                    ));
                }
            }
            Pattern::At { name, pattern, .. } => {
                if name.name == ident {
                    locations.push(Location::new(
                        uri.clone(),
                        Self::span_to_range(name.span.clone()),
                    ));
                }
                Self::collect_pattern_references(pattern, ident, text, uri, locations);
            }
            Pattern::Constructor { name, args, .. } => {
                if name.name == ident {
                    locations.push(Location::new(
                        uri.clone(),
                        Self::span_to_range(name.span.clone()),
                    ));
                }
                for arg in args.iter() {
                    Self::collect_pattern_references(arg, ident, text, uri, locations);
                }
            }
            Pattern::Tuple { items, .. } => {
                for item in items.iter() {
                    Self::collect_pattern_references(item, ident, text, uri, locations);
                }
            }
            Pattern::List { items, rest, .. } => {
                for item in items.iter() {
                    Self::collect_pattern_references(item, ident, text, uri, locations);
                }
                if let Some(rest) = rest {
                    Self::collect_pattern_references(rest, ident, text, uri, locations);
                }
            }
            Pattern::Record { fields, .. } => {
                for field in fields.iter() {
                    Self::collect_record_pattern_references(field, ident, text, uri, locations);
                }
            }
            Pattern::Wildcard(_) | Pattern::Literal(_) => {}
        }
    }

    fn collect_record_pattern_references(
        field: &RecordPatternField,
        ident: &str,
        text: &str,
        uri: &Url,
        locations: &mut Vec<Location>,
    ) {
        for segment in field.path.iter() {
            if segment.name == ident {
                locations.push(Location::new(
                    uri.clone(),
                    Self::span_to_range(segment.span.clone()),
                ));
            }
        }
        Self::collect_pattern_references(&field.pattern, ident, text, uri, locations);
    }

    fn collect_expr_references(
        expr: &Expr,
        ident: &str,
        text: &str,
        uri: &Url,
        locations: &mut Vec<Location>,
    ) {
        match expr {
            Expr::TextInterpolate { parts, .. } => {
                for part in parts {
                    if let aivi::TextPart::Expr { expr, .. } = part {
                        Self::collect_expr_references(expr, ident, text, uri, locations);
                    }
                }
            }
            Expr::Ident(name) => {
                let matches = |name: &str| name == ident || name == format!("({})", ident);
                if matches(&name.name) {
                    locations.push(Location::new(
                        uri.clone(),
                        Self::span_to_range(name.span.clone()),
                    ));
                }
            }
            Expr::Literal(_) => {}
            Expr::Suffixed { base, .. } => {
                Self::collect_expr_references(base, ident, text, uri, locations);
            }
            Expr::List { items, .. } => {
                for item in items.iter() {
                    Self::collect_list_item_references(item, ident, text, uri, locations);
                }
            }
            Expr::Tuple { items, .. } => {
                for item in items.iter() {
                    Self::collect_expr_references(item, ident, text, uri, locations);
                }
            }
            Expr::Record { fields, .. } => {
                for field in fields.iter() {
                    Self::collect_record_field_references(field, ident, text, uri, locations);
                }
            }
            Expr::PatchLit { fields, .. } => {
                for field in fields.iter() {
                    Self::collect_record_field_references(field, ident, text, uri, locations);
                }
            }
            Expr::FieldAccess { base, field, .. } => {
                Self::collect_expr_references(base, ident, text, uri, locations);
                if field.name == ident {
                    locations.push(Location::new(
                        uri.clone(),
                        Self::span_to_range(field.span.clone()),
                    ));
                }
            }
            Expr::FieldSection { field, .. } => {
                if field.name == ident {
                    locations.push(Location::new(
                        uri.clone(),
                        Self::span_to_range(field.span.clone()),
                    ));
                }
            }
            Expr::Index { base, index, .. } => {
                Self::collect_expr_references(base, ident, text, uri, locations);
                Self::collect_expr_references(index, ident, text, uri, locations);
            }
            Expr::Call { func, args, .. } => {
                Self::collect_expr_references(func, ident, text, uri, locations);
                for arg in args.iter() {
                    Self::collect_expr_references(arg, ident, text, uri, locations);
                }
            }
            Expr::Lambda { params, body, .. } => {
                for param in params.iter() {
                    Self::collect_pattern_references(param, ident, text, uri, locations);
                }
                Self::collect_expr_references(body, ident, text, uri, locations);
            }
            Expr::Match {
                scrutinee, arms, ..
            } => {
                if let Some(scrutinee) = scrutinee {
                    Self::collect_expr_references(scrutinee, ident, text, uri, locations);
                }
                for arm in arms.iter() {
                    Self::collect_match_arm_references(arm, ident, text, uri, locations);
                }
            }
            Expr::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => {
                Self::collect_expr_references(cond, ident, text, uri, locations);
                Self::collect_expr_references(then_branch, ident, text, uri, locations);
                Self::collect_expr_references(else_branch, ident, text, uri, locations);
            }
            Expr::Binary {
                op, left, right, ..
            } => {
                Self::collect_expr_references(left, ident, text, uri, locations);

                let matches_op = op == ident || format!("({})", op) == ident;
                if matches_op {
                    let left_end = Self::span_to_range(Self::expr_span(left).clone()).end;
                    let right_start = Self::span_to_range(Self::expr_span(right).clone()).start;

                    let left_offset = Self::offset_at(text, left_end);
                    let right_offset = Self::offset_at(text, right_start);

                    if left_offset < text.len()
                        && right_offset <= text.len()
                        && left_offset < right_offset
                    {
                        let range_text = &text[left_offset..right_offset];
                        if let Some(idx) = range_text.find(op) {
                            let mut line = left_end.line;
                            let mut char_idx = left_end.character;

                            let prefix = &range_text[..idx];
                            for c in prefix.chars() {
                                if c == '\n' {
                                    line += 1;
                                    char_idx = 0;
                                } else {
                                    char_idx += c.len_utf16() as u32;
                                }
                            }
                            let start_pos = Position::new(line, char_idx);

                            let mut end_line = line;
                            let mut end_char = char_idx;
                            for c in op.chars() {
                                if c == '\n' {
                                    end_line += 1;
                                    end_char = 0;
                                } else {
                                    end_char += c.len_utf16() as u32;
                                }
                            }
                            let end_pos = Position::new(end_line, end_char);

                            locations
                                .push(Location::new(uri.clone(), Range::new(start_pos, end_pos)));
                        }
                    }
                }

                Self::collect_expr_references(right, ident, text, uri, locations);
            }
            Expr::Block { items, .. } => {
                for item in items.iter() {
                    Self::collect_block_item_references(item, ident, text, uri, locations);
                }
            }
            Expr::Raw { .. } => {}
        }
    }

    fn collect_list_item_references(
        item: &ListItem,
        ident: &str,
        text: &str,
        uri: &Url,
        locations: &mut Vec<Location>,
    ) {
        Self::collect_expr_references(&item.expr, ident, text, uri, locations);
    }
}
