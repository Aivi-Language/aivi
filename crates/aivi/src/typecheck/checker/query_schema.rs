#[derive(Clone, Debug)]
enum TableColumnType {
    Int,
    Bool,
    Timestamp,
    Varchar,
}

#[derive(Clone, Debug)]
enum TableColumnDefault {
    Bool,
    Int,
    Text,
    Now,
}

#[derive(Clone, Debug)]
struct TableColumnSchema {
    name: String,
    ty: TableColumnType,
    not_null: bool,
    default: Option<TableColumnDefault>,
    span: Span,
}

#[derive(Clone, Debug)]
struct TableSchemaSnapshot {
    table_name: String,
    columns: Vec<TableColumnSchema>,
    row_fields: BTreeMap<String, Type>,
}

impl TypeChecker {
    pub(super) fn validate_schema_aware_def(
        &mut self,
        def: &Def,
        expr: &Expr,
        resolved_ty: &Type,
        env: &TypeEnv,
    ) {
        let table_ty = self
            .extract_table_row_fields(resolved_ty)
            .or_else(|| env.get(&def.name.name).and_then(|scheme| self.extract_table_row_fields(&scheme.ty)));
        if let (Some(row_fields), Some((table_name, columns_expr))) =
            (table_ty, Self::extract_table_call(expr))
        {
            if let Some(columns) = self.extract_static_table_columns(columns_expr) {
                let snapshot = TableSchemaSnapshot {
                    table_name,
                    columns,
                    row_fields,
                };
                self.validate_table_schema_snapshot(def, &snapshot);
            }
        }
        if Self::expr_mentions_query_surface(expr) {
            self.validate_query_schema_expr(expr, env, &BTreeMap::new());
        }
    }

    pub(super) fn validate_query_call_args(
        &mut self,
        func: &Expr,
        args: &[Expr],
        arg_tys: &[Type],
        env: &mut TypeEnv,
    ) -> Result<(), TypeError> {
        let Some(callee) = Self::callee_leaf_name(func) else {
            return Ok(());
        };
        if args.len() != 2 || arg_tys.len() != 2 {
            return Ok(());
        }
        let Some(row_ty) = self.extract_query_row_type(arg_tys[1].clone()) else {
            return Ok(());
        };
        let Some(expected) = self.query_projection_expected_type(callee, row_ty) else {
            return Ok(());
        };
        self.validate_expr_against_expected(&args[0], expected, env)
    }

    pub(super) fn validate_query_pipe_transformer(
        &mut self,
        transformer: &Expr,
        input_ty: &Type,
        env: &mut TypeEnv,
    ) -> Result<(), TypeError> {
        let Expr::Call { func, args, .. } = transformer else {
            return Ok(());
        };
        if args.len() != 1 {
            return Ok(());
        }
        let Some(callee) = Self::callee_leaf_name(func) else {
            return Ok(());
        };
        let Some(row_ty) = self.extract_query_row_type(input_ty.clone()) else {
            return Ok(());
        };
        let Some(expected) = self.query_projection_expected_type(callee, row_ty) else {
            return Ok(());
        };
        self.validate_expr_against_expected(&args[0], expected, env)
    }

    fn validate_expr_against_expected(
        &mut self,
        expr: &Expr,
        expected: Type,
        env: &mut TypeEnv,
    ) -> Result<(), TypeError> {
        let expr = if matches!(expr, Expr::Lambda { .. }) {
            expr.clone()
        } else if expr_contains_placeholder(expr) {
            self.rewrite_placeholder_lambda(expr, "__query")
        } else if let Some(lifted) = lift_predicate_expr(expr, env, "__query") {
            lifted
        } else {
            expr.clone()
        };
        let next_var = self.next_var;
        let subst = self.subst.clone();
        let var_names = self.var_names.clone();
        let constraints = self.constraints.clone();
        let span_types_len = self.span_types.len();
        let poly_len = self.poly_instantiations.len();
        let source_schema_len = self.load_source_schemas.len();
        let extra_diag_len = self.extra_diagnostics.len();
        let mut local_env = env.clone();
        let result = (|| {
            let _ = self.elab_expr(expr, Some(expected), &mut local_env)?;
            self.solve_deferred_constraints()
        })();
        self.next_var = next_var;
        self.subst = subst;
        self.var_names = var_names;
        self.constraints = constraints;
        self.span_types.truncate(span_types_len);
        self.poly_instantiations.truncate(poly_len);
        self.load_source_schemas.truncate(source_schema_len);
        self.extra_diagnostics.truncate(extra_diag_len);
        result
    }

    fn rewrite_placeholder_lambda(&self, expr: &Expr, param_name: &str) -> Expr {
        fn rewrite(expr: Expr, param: &SpannedName) -> Expr {
            match expr {
                Expr::Ident(name) if name.name == "_" => Expr::Ident(param.clone()),
                Expr::UnaryNeg { expr, span } => Expr::UnaryNeg {
                    expr: Box::new(rewrite(*expr, param)),
                    span,
                },
                Expr::Suffixed { base, suffix, span } => Expr::Suffixed {
                    base: Box::new(rewrite(*base, param)),
                    suffix,
                    span,
                },
                Expr::TextInterpolate { parts, span } => Expr::TextInterpolate {
                    parts: parts
                        .into_iter()
                        .map(|part| match part {
                            TextPart::Text { .. } => part,
                            TextPart::Expr { expr, span } => TextPart::Expr {
                                expr: Box::new(rewrite(*expr, param)),
                                span,
                            },
                        })
                        .collect(),
                    span,
                },
                Expr::List { items, span } => Expr::List {
                    items: items
                        .into_iter()
                        .map(|mut item| {
                            item.expr = rewrite(item.expr, param);
                            item
                        })
                        .collect(),
                    span,
                },
                Expr::Tuple { items, span } => Expr::Tuple {
                    items: items.into_iter().map(|item| rewrite(item, param)).collect(),
                    span,
                },
                Expr::Record { fields, span } => Expr::Record {
                    fields: fields
                        .into_iter()
                        .map(|mut field| {
                            field.value = rewrite(field.value, param);
                            field
                        })
                        .collect(),
                    span,
                },
                Expr::PatchLit { fields, span } => Expr::PatchLit {
                    fields: fields
                        .into_iter()
                        .map(|mut field| {
                            field.value = rewrite(field.value, param);
                            field
                        })
                        .collect(),
                    span,
                },
                Expr::FieldAccess { base, field, span } => Expr::FieldAccess {
                    base: Box::new(rewrite(*base, param)),
                    field,
                    span,
                },
                Expr::Index { base, index, span } => Expr::Index {
                    base: Box::new(rewrite(*base, param)),
                    index: Box::new(rewrite(*index, param)),
                    span,
                },
                Expr::Call { func, args, span } => Expr::Call {
                    func: Box::new(rewrite(*func, param)),
                    args: args.into_iter().map(|arg| rewrite(arg, param)).collect(),
                    span,
                },
                Expr::Lambda { params, body, span } => Expr::Lambda {
                    params,
                    body,
                    span,
                },
                Expr::Match {
                    scrutinee,
                    arms,
                    span,
                } => Expr::Match {
                    scrutinee: scrutinee.map(|expr| Box::new(rewrite(*expr, param))),
                    arms: arms
                        .into_iter()
                        .map(|mut arm| {
                            arm.guard = arm.guard.map(|guard| rewrite(guard, param));
                            arm.body = rewrite(arm.body, param);
                            arm
                        })
                        .collect(),
                    span,
                },
                Expr::If {
                    cond,
                    then_branch,
                    else_branch,
                    span,
                } => Expr::If {
                    cond: Box::new(rewrite(*cond, param)),
                    then_branch: Box::new(rewrite(*then_branch, param)),
                    else_branch: Box::new(rewrite(*else_branch, param)),
                    span,
                },
                Expr::Binary {
                    op,
                    left,
                    right,
                    span,
                } => Expr::Binary {
                    op,
                    left: Box::new(rewrite(*left, param)),
                    right: Box::new(rewrite(*right, param)),
                    span,
                },
                other => other,
            }
        }

        let span = expr_span(expr);
        let param = SpannedName {
            name: param_name.to_string(),
            span: span.clone(),
        };
        Expr::Lambda {
            params: vec![Pattern::Ident(param.clone())],
            body: Box::new(rewrite(expr.clone(), &param)),
            span,
        }
    }

    fn expr_mentions_query_surface(expr: &Expr) -> bool {
        match expr {
            Expr::Call { func, args, .. } => {
                matches!(
                    Self::callee_leaf_name(func),
                    Some(
                        "from"
                            | "where_"
                            | "select"
                            | "orderBy"
                            | "guard_"
                            | "queryOf"
                            | "count"
                            | "exists"
                            | "limit"
                            | "offset"
                    )
                ) || args.iter().any(Self::expr_mentions_query_surface)
                    || Self::expr_mentions_query_surface(func)
            }
            Expr::Block { kind, items, .. } => {
                matches!(kind, BlockKind::Do { monad } if monad.name == "Query")
                    || items.iter().any(|item| match item {
                        BlockItem::Bind { expr, .. }
                        | BlockItem::Let { expr, .. }
                        | BlockItem::Filter { expr, .. }
                        | BlockItem::Yield { expr, .. }
                        | BlockItem::Recurse { expr, .. }
                        | BlockItem::Expr { expr, .. } => Self::expr_mentions_query_surface(expr),
                        BlockItem::When { cond, effect, .. }
                        | BlockItem::Unless { cond, effect, .. } => {
                            Self::expr_mentions_query_surface(cond)
                                || Self::expr_mentions_query_surface(effect)
                        }
                        BlockItem::Given { cond, fail_expr, .. } => {
                            Self::expr_mentions_query_surface(cond)
                                || Self::expr_mentions_query_surface(fail_expr)
                        }
                        BlockItem::On { transition, handler, .. } => {
                            Self::expr_mentions_query_surface(transition)
                                || Self::expr_mentions_query_surface(handler)
                        }
                    })
            }
            Expr::Binary { left, right, .. } => {
                Self::expr_mentions_query_surface(left) || Self::expr_mentions_query_surface(right)
            }
            Expr::FieldAccess { base, .. }
            | Expr::UnaryNeg { expr: base, .. }
            | Expr::Suffixed { base, .. } => Self::expr_mentions_query_surface(base),
            Expr::Index { base, index, .. } => {
                Self::expr_mentions_query_surface(base) || Self::expr_mentions_query_surface(index)
            }
            Expr::List { items, .. } => items
                .iter()
                .any(|item| Self::expr_mentions_query_surface(&item.expr)),
            Expr::Tuple { items, .. } => items.iter().any(Self::expr_mentions_query_surface),
            Expr::Record { fields, .. } | Expr::PatchLit { fields, .. } => fields
                .iter()
                .any(|field| Self::expr_mentions_query_surface(&field.value)),
            Expr::Lambda { body, .. } => Self::expr_mentions_query_surface(body),
            Expr::Match {
                scrutinee, arms, ..
            } => {
                scrutinee
                    .as_ref()
                    .is_some_and(|expr| Self::expr_mentions_query_surface(expr))
                    || arms.iter().any(|arm| {
                        arm.guard
                            .as_ref()
                            .is_some_and(Self::expr_mentions_query_surface)
                            || Self::expr_mentions_query_surface(&arm.body)
                    })
            }
            Expr::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => {
                Self::expr_mentions_query_surface(cond)
                    || Self::expr_mentions_query_surface(then_branch)
                    || Self::expr_mentions_query_surface(else_branch)
            }
            Expr::TextInterpolate { parts, .. } => parts.iter().any(|part| match part {
                TextPart::Text { .. } => false,
                TextPart::Expr { expr, .. } => Self::expr_mentions_query_surface(expr),
            }),
            Expr::Mock { substitutions, body, .. } => {
                substitutions.iter().any(|sub| {
                    sub.value
                        .as_ref()
                        .is_some_and(Self::expr_mentions_query_surface)
                }) || Self::expr_mentions_query_surface(body)
            }
            Expr::Ident(_) | Expr::Literal(_) | Expr::FieldSection { .. } | Expr::Raw { .. } => {
                false
            }
        }
    }

    fn validate_query_schema_expr(
        &mut self,
        expr: &Expr,
        env: &TypeEnv,
        bindings: &BTreeMap<String, BTreeMap<String, Type>>,
    ) -> Option<BTreeMap<String, Type>> {
        match expr {
            Expr::Call { func, args, .. } => {
                let callee = Self::callee_leaf_name(func);
                match (callee, args.as_slice()) {
                    (Some("from"), [table]) => self.table_row_fields_from_expr(table, env),
                    (Some("where_"), [pred, query]) | (Some("orderBy"), [pred, query]) => {
                        let row = self.validate_query_schema_expr(query, env, bindings);
                        if let Some(fields) = row.as_ref() {
                            self.validate_query_row_expr(pred, Some(fields), bindings);
                        }
                        row
                    }
                    (Some("select"), [projection, query]) => {
                        let row = self.validate_query_schema_expr(query, env, bindings);
                        if let Some(fields) = row.as_ref() {
                            self.validate_query_row_expr(projection, Some(fields), bindings);
                        }
                        None
                    }
                    (Some("limit"), [_, query]) | (Some("offset"), [_, query]) => {
                        self.validate_query_schema_expr(query, env, bindings)
                    }
                    (Some("count"), [query]) | (Some("exists"), [query]) => {
                        self.validate_query_schema_expr(query, env, bindings);
                        None
                    }
                    (Some("guard_"), [guard]) | (Some("queryOf"), [guard]) => {
                        self.validate_query_row_expr(guard, None, bindings);
                        None
                    }
                    _ => {
                        self.validate_query_schema_expr(func, env, bindings);
                        for arg in args {
                            self.validate_query_schema_expr(arg, env, bindings);
                        }
                        None
                    }
                }
            }
            Expr::Binary { op, left, right, .. } if op == "|>" => {
                let left_row = self.validate_query_schema_expr(left, env, bindings);
                if let Some(fields) = left_row.as_ref() {
                    match &**right {
                        Expr::Call { func, args, .. } => match (Self::callee_leaf_name(func), args.as_slice()) {
                            (Some("where_"), [pred]) | (Some("orderBy"), [pred]) => {
                                self.validate_query_row_expr(pred, Some(fields), bindings);
                                left_row
                            }
                            (Some("select"), [projection]) => {
                                self.validate_query_row_expr(projection, Some(fields), bindings);
                                None
                            }
                            (Some("limit"), [_]) | (Some("offset"), [_]) => left_row,
                            (Some("count"), []) | (Some("exists"), []) => None,
                            _ => None,
                        },
                        _ => None,
                    }
                } else {
                    None
                }
            }
            Expr::Block {
                kind: BlockKind::Do { monad },
                items,
                ..
            } if monad.name == "Query" => {
                let mut local_bindings = bindings.clone();
                for item in items {
                    match item {
                        BlockItem::Bind { pattern, expr, .. } => {
                            let row = self.validate_query_schema_expr(expr, env, &local_bindings);
                            if let Some(fields) = row {
                                match pattern {
                                    Pattern::Ident(name) | Pattern::SubjectIdent(name) => {
                                        local_bindings.insert(name.name.clone(), fields);
                                    }
                                    _ => {}
                                }
                            }
                        }
                        BlockItem::Let { expr, .. } | BlockItem::Expr { expr, .. } => {
                            self.validate_query_schema_expr(expr, env, &local_bindings);
                            self.validate_query_row_expr(expr, None, &local_bindings);
                        }
                        BlockItem::Filter { expr, .. }
                        | BlockItem::Yield { expr, .. }
                        | BlockItem::Recurse { expr, .. } => {
                            self.validate_query_row_expr(expr, None, &local_bindings);
                        }
                        BlockItem::When { cond, effect, .. }
                        | BlockItem::Unless { cond, effect, .. } => {
                            self.validate_query_row_expr(cond, None, &local_bindings);
                            self.validate_query_row_expr(effect, None, &local_bindings);
                        }
                        BlockItem::Given { cond, fail_expr, .. } => {
                            self.validate_query_row_expr(cond, None, &local_bindings);
                            self.validate_query_row_expr(fail_expr, None, &local_bindings);
                        }
                        BlockItem::On { transition, handler, .. } => {
                            self.validate_query_row_expr(transition, None, &local_bindings);
                            self.validate_query_row_expr(handler, None, &local_bindings);
                        }
                    }
                }
                None
            }
            _ => None,
        }
    }

    fn table_row_fields_from_expr(
        &mut self,
        expr: &Expr,
        env: &TypeEnv,
    ) -> Option<BTreeMap<String, Type>> {
        let Expr::Ident(name) = expr else {
            return None;
        };
        let scheme = env.get(&name.name)?;
        self.extract_table_row_fields(&scheme.ty)
    }

    fn validate_query_row_expr(
        &mut self,
        expr: &Expr,
        placeholder: Option<&BTreeMap<String, Type>>,
        bindings: &BTreeMap<String, BTreeMap<String, Type>>,
    ) {
        match expr {
            Expr::FieldSection { field, .. } => {
                if let Some(fields) = placeholder {
                    self.ensure_field_exists(fields, field, &field.span);
                }
            }
            Expr::FieldAccess { base, field, span } => {
                if let Some(base_ty) = self.resolve_query_expr_type(base, placeholder, bindings) {
                    self.ensure_nested_field_exists(base_ty, field, span);
                }
                self.validate_query_row_expr(base, placeholder, bindings);
            }
            Expr::Lambda { params, body, .. } => {
                if let Some(fields) = placeholder {
                    match params.as_slice() {
                        [Pattern::Ident(name)] | [Pattern::SubjectIdent(name)] => {
                            let mut nested = bindings.clone();
                            nested.insert(name.name.clone(), fields.clone());
                            self.validate_query_row_expr(body, None, &nested);
                        }
                        _ => self.validate_query_row_expr(body, placeholder, bindings),
                    }
                } else {
                    self.validate_query_row_expr(body, placeholder, bindings);
                }
            }
            Expr::Call { func, args, .. } => {
                self.validate_query_row_expr(func, placeholder, bindings);
                for arg in args {
                    self.validate_query_row_expr(arg, placeholder, bindings);
                }
            }
            Expr::Binary { left, right, .. } => {
                self.validate_query_row_expr(left, placeholder, bindings);
                self.validate_query_row_expr(right, placeholder, bindings);
            }
            Expr::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => {
                self.validate_query_row_expr(cond, placeholder, bindings);
                self.validate_query_row_expr(then_branch, placeholder, bindings);
                self.validate_query_row_expr(else_branch, placeholder, bindings);
            }
            Expr::Record { fields, .. } | Expr::PatchLit { fields, .. } => {
                for field in fields {
                    self.validate_query_row_expr(&field.value, placeholder, bindings);
                }
            }
            Expr::List { items, .. } => {
                for item in items {
                    self.validate_query_row_expr(&item.expr, placeholder, bindings);
                }
            }
            Expr::Tuple { items, .. } => {
                for item in items {
                    self.validate_query_row_expr(item, placeholder, bindings);
                }
            }
            Expr::Match {
                scrutinee, arms, ..
            } => {
                if let Some(scrutinee) = scrutinee {
                    self.validate_query_row_expr(scrutinee, placeholder, bindings);
                }
                for arm in arms {
                    if let Some(guard) = arm.guard.as_ref() {
                        self.validate_query_row_expr(guard, placeholder, bindings);
                    }
                    self.validate_query_row_expr(&arm.body, placeholder, bindings);
                }
            }
            Expr::Index { base, index, .. } => {
                self.validate_query_row_expr(base, placeholder, bindings);
                self.validate_query_row_expr(index, placeholder, bindings);
            }
            Expr::TextInterpolate { parts, .. } => {
                for part in parts {
                    if let TextPart::Expr { expr, .. } = part {
                        self.validate_query_row_expr(expr, placeholder, bindings);
                    }
                }
            }
            Expr::Mock { substitutions, body, .. } => {
                for sub in substitutions {
                    if let Some(value) = &sub.value {
                        self.validate_query_row_expr(value, placeholder, bindings);
                    }
                }
                self.validate_query_row_expr(body, placeholder, bindings);
            }
            Expr::UnaryNeg { expr, .. } | Expr::Suffixed { base: expr, .. } => {
                self.validate_query_row_expr(expr, placeholder, bindings);
            }
            Expr::Block { items, .. } => {
                for item in items {
                    match item {
                        BlockItem::Bind { expr, .. }
                        | BlockItem::Let { expr, .. }
                        | BlockItem::Filter { expr, .. }
                        | BlockItem::Yield { expr, .. }
                        | BlockItem::Recurse { expr, .. }
                        | BlockItem::Expr { expr, .. } => {
                            self.validate_query_row_expr(expr, placeholder, bindings)
                        }
                        BlockItem::When { cond, effect, .. }
                        | BlockItem::Unless { cond, effect, .. } => {
                            self.validate_query_row_expr(cond, placeholder, bindings);
                            self.validate_query_row_expr(effect, placeholder, bindings);
                        }
                        BlockItem::Given { cond, fail_expr, .. } => {
                            self.validate_query_row_expr(cond, placeholder, bindings);
                            self.validate_query_row_expr(fail_expr, placeholder, bindings);
                        }
                        BlockItem::On { transition, handler, .. } => {
                            self.validate_query_row_expr(transition, placeholder, bindings);
                            self.validate_query_row_expr(handler, placeholder, bindings);
                        }
                    }
                }
            }
            Expr::Ident(_) | Expr::Literal(_) | Expr::Raw { .. } => {}
        }
    }

    fn resolve_query_expr_type(
        &mut self,
        expr: &Expr,
        placeholder: Option<&BTreeMap<String, Type>>,
        bindings: &BTreeMap<String, BTreeMap<String, Type>>,
    ) -> Option<Type> {
        match expr {
            Expr::Ident(name) if name.name == "_" => {
                Some(Type::Record { fields: placeholder?.clone() })
            }
            Expr::Ident(name) => bindings
                .get(&name.name)
                .cloned()
                .map(|fields| Type::Record { fields }),
            Expr::FieldAccess { base, field, .. } => {
                let base_ty = self.resolve_query_expr_type(base, placeholder, bindings)?;
                self.resolve_field_type(base_ty, field)
            }
            _ => None,
        }
    }

    fn resolve_field_type(&mut self, base_ty: Type, field: &SpannedName) -> Option<Type> {
        let applied = self.apply(base_ty);
        let expanded = self.expand_alias(applied);
        let Type::Record { fields } = expanded else {
            return None;
        };
        fields.get(&field.name).cloned()
    }

    fn ensure_nested_field_exists(&mut self, base_ty: Type, field: &SpannedName, span: &Span) {
        if self.resolve_field_type(base_ty, field).is_none() {
            self.emit_extra_diag(
                "E3302",
                crate::diagnostics::DiagnosticSeverity::Error,
                format!("record has no field '{}'", field.name),
                span.clone(),
            );
        }
    }

    fn ensure_field_exists(
        &mut self,
        fields: &BTreeMap<String, Type>,
        field: &SpannedName,
        span: &Span,
    ) {
        if !fields.contains_key(&field.name) {
            self.emit_extra_diag(
                "E3302",
                crate::diagnostics::DiagnosticSeverity::Error,
                format!("record has no field '{}'", field.name),
                span.clone(),
            );
        }
    }

    fn query_projection_expected_type(&mut self, callee: &str, row_ty: Type) -> Option<Type> {
        match callee {
            "where_" => Some(Type::Func(Box::new(row_ty), Box::new(Type::con("Bool")))),
            "select" | "orderBy" => Some(Type::Func(
                Box::new(row_ty),
                Box::new(self.fresh_var()),
            )),
            _ => None,
        }
    }

    fn extract_query_row_type(&mut self, ty: Type) -> Option<Type> {
        let applied = self.apply(ty);
        if let Some(row_ty) = Self::extract_named_type_arg(&applied, "Query") {
            let applied_row = self.apply(row_ty);
            return Some(self.expand_alias(applied_row));
        }

        let expanded = self.expand_alias(applied);
        let Type::Record { fields } = expanded else {
            return None;
        };
        let run_ty = fields.get("run")?.clone();
        let applied_run = self.apply(run_ty);
        let expanded_run = self.expand_alias(applied_run);
        let Type::Func(_, result_ty) = expanded_run else {
            return None;
        };
        let effect_result = Self::extract_effect_result_type(&result_ty)?;
        let list_row = Self::extract_named_type_arg(&effect_result, "List")?;
        let applied_row = self.apply(list_row);
        Some(self.expand_alias(applied_row))
    }

    fn extract_table_row_fields(&mut self, ty: &Type) -> Option<BTreeMap<String, Type>> {
        let applied = self.apply(ty.clone());
        let row_ty = if let Some(row_ty) = Self::extract_named_type_arg(&applied, "Table") {
            row_ty
        } else {
            let expanded = self.expand_alias(applied);
            let Type::Record { fields } = expanded else {
                return None;
            };
            let rows_ty = fields.get("rows")?.clone();
            let applied_rows = self.apply(rows_ty);
            let expanded_rows = self.expand_alias(applied_rows);
            Self::extract_named_type_arg(&expanded_rows, "List")?
        };
        let applied_row = self.apply(row_ty);
        let expanded = self.expand_alias(applied_row);
        match expanded {
            Type::Record { fields } => Some(fields),
            _ => None,
        }
    }

    fn validate_table_schema_snapshot(&mut self, def: &Def, snapshot: &TableSchemaSnapshot) {
        let mut seen_columns = std::collections::BTreeSet::new();
        for column in &snapshot.columns {
            if !seen_columns.insert(column.name.clone()) {
                self.emit_extra_diag(
                    "E3301",
                    crate::diagnostics::DiagnosticSeverity::Error,
                    format!(
                        "table '{}' declares column '{}' more than once",
                        snapshot.table_name, column.name
                    ),
                    column.span.clone(),
                );
                continue;
            }

            let Some(field_ty) = snapshot.row_fields.get(&column.name).cloned() else {
                self.emit_extra_diag(
                    "E3301",
                    crate::diagnostics::DiagnosticSeverity::Error,
                    format!(
                        "table '{}' column '{}' has no matching field in row type '{}'",
                        snapshot.table_name, column.name, def.name.name
                    ),
                    column.span.clone(),
                );
                continue;
            };

            let Some((field_base_ty, field_is_option)) = self.normalize_table_field_type(field_ty) else {
                continue;
            };

            if column.not_null && field_is_option {
                self.emit_extra_diag(
                    "E3301",
                    crate::diagnostics::DiagnosticSeverity::Error,
                    format!(
                        "table '{}' column '{}' is marked NotNull but row field '{}' is Option",
                        snapshot.table_name, column.name, column.name
                    ),
                    column.span.clone(),
                );
            } else if !column.not_null && !field_is_option {
                self.emit_extra_diag(
                    "E3301",
                    crate::diagnostics::DiagnosticSeverity::Error,
                    format!(
                        "table '{}' column '{}' is nullable but row field '{}' is not Option",
                        snapshot.table_name, column.name, column.name
                    ),
                    column.span.clone(),
                );
            }

            let expected_ty = self.table_column_expected_type(&column.ty);
            let field_matches = self.field_matches_column_type(&field_base_ty, &column.ty);
            if !field_matches {
                let expected_ty_str = self.type_to_string(&expected_ty);
                self.emit_extra_diag(
                    "E3301",
                    crate::diagnostics::DiagnosticSeverity::Error,
                    format!(
                        "table '{}' column '{}' expects row field '{}' to have type '{}'",
                        snapshot.table_name,
                        column.name,
                        column.name,
                        expected_ty_str
                    ),
                    column.span.clone(),
                );
            }

            if let Some(default) = &column.default {
                if !self.column_default_matches_type(default, &column.ty) {
                    self.emit_extra_diag(
                        "E3301",
                        crate::diagnostics::DiagnosticSeverity::Error,
                        format!(
                            "table '{}' column '{}' uses a default that does not match its declared column type",
                            snapshot.table_name, column.name
                        ),
                        column.span.clone(),
                    );
                }
            }
        }

        for field_name in snapshot.row_fields.keys() {
            if !seen_columns.contains(field_name) {
                self.emit_extra_diag(
                    "E3301",
                    crate::diagnostics::DiagnosticSeverity::Error,
                    format!(
                        "table '{}' row type field '{}' has no matching column declaration",
                        snapshot.table_name, field_name
                    ),
                    def.span.clone(),
                );
            }
        }
    }

    fn normalize_table_field_type(&mut self, ty: Type) -> Option<(Type, bool)> {
        let applied = self.apply(ty);
        let expanded = self.expand_alias(applied);
        match expanded {
            Type::Con(name, args) if Self::leaf_name(&name) == "Option" && args.len() == 1 => Some((
                {
                    let applied_inner = self.apply(args[0].clone());
                    self.expand_alias(applied_inner)
                },
                true,
            )),
            Type::App(base, args) => match *base {
                Type::Con(name, existing)
                    if Self::leaf_name(&name) == "Option" && existing.len() + args.len() == 1 =>
                {
                    let inner = args
                        .last()
                        .cloned()
                        .or_else(|| existing.last().cloned())?;
                    let applied_inner = self.apply(inner);
                    Some((self.expand_alias(applied_inner), true))
                }
                _ => Some((Type::App(base, args), false)),
            },
            other => Some((other, false)),
        }
    }

    fn field_matches_column_type(&mut self, field_ty: &Type, column_ty: &TableColumnType) -> bool {
        let applied = self.apply(field_ty.clone());
        let expanded = self.expand_alias(applied);
        match column_ty {
            TableColumnType::Int => {
                matches!(expanded, Type::Con(ref name, ref args) if Self::leaf_name(name) == "Int" && args.is_empty())
            }
            TableColumnType::Bool => {
                matches!(expanded, Type::Con(ref name, ref args) if Self::leaf_name(name) == "Bool" && args.is_empty())
            }
            TableColumnType::Timestamp => {
                matches!(expanded, Type::Con(ref name, ref args) if Self::leaf_name(name) == "DateTime" && args.is_empty())
            }
            TableColumnType::Varchar => {
                matches!(expanded, Type::Con(ref name, ref args) if Self::leaf_name(name) == "Text" && args.is_empty())
            }
        }
    }

    fn table_column_expected_type(&mut self, column_ty: &TableColumnType) -> Type {
        match column_ty {
            TableColumnType::Int => Type::con("Int"),
            TableColumnType::Bool => Type::con("Bool"),
            TableColumnType::Timestamp => Type::con("DateTime"),
            TableColumnType::Varchar => Type::con("Text"),
        }
    }

    fn column_default_matches_type(
        &self,
        default: &TableColumnDefault,
        column_ty: &TableColumnType,
    ) -> bool {
        matches!(
            (default, column_ty),
            (TableColumnDefault::Bool, TableColumnType::Bool)
                | (TableColumnDefault::Int, TableColumnType::Int)
                | (TableColumnDefault::Text, TableColumnType::Varchar)
                | (TableColumnDefault::Now, TableColumnType::Timestamp)
        )
    }

    fn extract_table_call(expr: &Expr) -> Option<(String, &Expr)> {
        let Expr::Call { func, args, .. } = expr else {
            return None;
        };
        if args.len() != 2 || Self::callee_leaf_name(func)? != "table" {
            return None;
        }
        let table_name = match &args[0] {
            Expr::Literal(Literal::String { text, .. }) => text.clone(),
            _ => return None,
        };
        Some((table_name, &args[1]))
    }

    fn extract_static_table_columns(&self, expr: &Expr) -> Option<Vec<TableColumnSchema>> {
        let Expr::List { items, .. } = expr else {
            return None;
        };
        items.iter()
            .map(|item| self.extract_static_table_column(item))
            .collect()
    }

    fn extract_static_table_column(&self, item: &ListItem) -> Option<TableColumnSchema> {
        if item.spread {
            return None;
        }
        let Expr::Record { fields, .. } = &item.expr else {
            return None;
        };
        let name = self.extract_column_name(fields)?;
        let ty = self.extract_column_type(fields)?;
        let not_null = self.extract_column_not_null(fields)?;
        let default = self.extract_column_default(fields)?;
        Some(TableColumnSchema {
            name,
            ty,
            not_null,
            default,
            span: item.span.clone(),
        })
    }

    fn extract_column_name(&self, fields: &[RecordField]) -> Option<String> {
        let field = Self::record_field_value(fields, "name")?;
        match field {
            Expr::Literal(Literal::String { text, .. }) => Some(text.clone()),
            _ => None,
        }
    }

    fn extract_column_type(&self, fields: &[RecordField]) -> Option<TableColumnType> {
        let field = Self::record_field_value(fields, "type")?;
        match field {
            Expr::Ident(name) if name.name == "IntType" => Some(TableColumnType::Int),
            Expr::Ident(name) if name.name == "BoolType" => Some(TableColumnType::Bool),
            Expr::Ident(name) if name.name == "TimestampType" => Some(TableColumnType::Timestamp),
            Expr::Call { func, args, .. }
                if args.len() == 1 && Self::callee_leaf_name(func) == Some("Varchar") =>
            {
                Some(TableColumnType::Varchar)
            }
            _ => None,
        }
    }

    fn extract_column_not_null(&self, fields: &[RecordField]) -> Option<bool> {
        let field = Self::record_field_value(fields, "constraints")?;
        let Expr::List { items, .. } = field else {
            return None;
        };
        let mut not_null = false;
        for item in items {
            if item.spread {
                return None;
            }
            match &item.expr {
                Expr::Ident(name) if name.name == "NotNull" => not_null = true,
                Expr::Ident(name) if name.name == "AutoIncrement" => {}
                _ => return None,
            }
        }
        Some(not_null)
    }

    fn extract_column_default(&self, fields: &[RecordField]) -> Option<Option<TableColumnDefault>> {
        let field = Self::record_field_value(fields, "default")?;
        match field {
            Expr::Ident(name) if name.name == "None" => Some(None),
            Expr::Call { func, args, .. } if args.len() == 1 && Self::callee_leaf_name(func) == Some("Some") => {
                self.extract_column_default_ctor(&args[0]).map(Some)
            }
            Expr::Ident(name) if name.name == "DefaultNow" => Some(Some(TableColumnDefault::Now)),
            _ => None,
        }
    }

    fn extract_column_default_ctor(&self, expr: &Expr) -> Option<TableColumnDefault> {
        match expr {
            Expr::Ident(name) if name.name == "DefaultNow" => Some(TableColumnDefault::Now),
            Expr::Call { func, args, .. }
                if args.len() == 1 && Self::callee_leaf_name(func) == Some("DefaultBool") =>
            {
                Some(TableColumnDefault::Bool)
            }
            Expr::Call { func, args, .. }
                if args.len() == 1 && Self::callee_leaf_name(func) == Some("DefaultInt") =>
            {
                Some(TableColumnDefault::Int)
            }
            Expr::Call { func, args, .. }
                if args.len() == 1 && Self::callee_leaf_name(func) == Some("DefaultText") =>
            {
                Some(TableColumnDefault::Text)
            }
            _ => None,
        }
    }

    fn record_field_value<'a>(fields: &'a [RecordField], name: &str) -> Option<&'a Expr> {
        fields.iter().find_map(|field| {
            if field.spread || field.path.len() != 1 {
                return None;
            }
            match &field.path[0] {
                PathSegment::Field(segment) if segment.name == name => Some(&field.value),
                _ => None,
            }
        })
    }

    fn callee_leaf_name(expr: &Expr) -> Option<&str> {
        match expr {
            Expr::Ident(name) => Some(Self::leaf_name(&name.name)),
            Expr::FieldAccess { field, .. } => Some(field.name.as_str()),
            _ => None,
        }
    }

    fn extract_named_type_arg(ty: &Type, name: &str) -> Option<Type> {
        match ty {
            Type::Con(con_name, args) if Self::leaf_name(con_name) == name && args.len() == 1 => {
                Some(args[0].clone())
            }
            Type::App(base, args) => match &**base {
                Type::Con(con_name, existing)
                    if Self::leaf_name(con_name) == name && existing.len() + args.len() == 1 =>
                {
                    args.last().cloned().or_else(|| existing.last().cloned())
                }
                _ => None,
            },
            _ => None,
        }
    }

    fn extract_effect_result_type(ty: &Type) -> Option<Type> {
        match ty {
            Type::Con(name, args) if Self::leaf_name(name) == "Effect" && args.len() == 2 => {
                Some(args[1].clone())
            }
            Type::App(base, args) => match &**base {
                Type::Con(name, existing)
                    if Self::leaf_name(name) == "Effect" && existing.len() + args.len() == 2 =>
                {
                    args.last().cloned().or_else(|| existing.last().cloned())
                }
                _ => None,
            },
            _ => None,
        }
    }

    fn leaf_name(name: &str) -> &str {
        name.rsplit('.').next().unwrap_or(name)
    }

    fn missing_record_field_message(&mut self, ty: &Type, path: &[PathSegment]) -> Option<String> {
        let mut current = ty;
        for segment in path {
            match segment {
                PathSegment::Field(name) => {
                    let Type::Record { fields } = current else {
                        return None;
                    };
                    if let Some(next) = fields.get(&name.name) {
                        current = next;
                    } else {
                        let available = if fields.is_empty() {
                            String::new()
                        } else {
                            format!(
                                " (available: {})",
                                fields.keys().cloned().collect::<Vec<_>>().join(", ")
                            )
                        };
                        return Some(format!("record has no field '{}'{}", name.name, available));
                    }
                }
                PathSegment::Index(_, _) | PathSegment::All(_) => return None,
            }
        }
        None
    }
}
