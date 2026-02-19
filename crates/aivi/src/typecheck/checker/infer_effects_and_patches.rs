impl TypeChecker {
    fn require_effect_value(
        &mut self,
        expr_ty: Type,
        err_ty: Type,
        span: Span,
    ) -> Result<Type, TypeError> {
        let value_ty = self.fresh_var();
        let effect_ty = Type::con("Effect").app(vec![err_ty, value_ty.clone()]);
        self.unify_with_span(expr_ty, effect_ty, span)?;
        Ok(value_ty)
    }

    fn infer_effect_block(
        &mut self,
        items: &[BlockItem],
        env: &mut TypeEnv,
    ) -> Result<Type, TypeError> {
        let mut local_env = env.clone();
        let err_ty = self.fresh_var();
        let mut result_ty = Type::con("Unit");
        for (idx, item) in items.iter().enumerate() {
            match item {
                BlockItem::Bind { pattern, expr, .. } => {
                    let expr_ty = self.infer_expr(expr, &mut local_env)?;
                    let snapshot = self.subst.clone();
                    let value_ty = match self
                        .bind_effect_value(expr_ty.clone(), err_ty.clone(), expr_span(expr))
                    {
                        Ok(value_ty) => value_ty,
                        Err(_) => {
                            self.subst = snapshot;
                            expr_ty
                        }
                    };
                    let pat_ty = self.infer_pattern(pattern, &mut local_env)?;
                    self.unify_with_span(pat_ty, value_ty, pattern_span(pattern))?;
                }
                BlockItem::Let { pattern, expr, .. } => {
                    // `x = expr` inside `effect { ... }` is a pure let-binding and must not run
                    // effects. Reject effect-typed expressions (including `Resource`).
                    // Pre-add compiler-generated names for self-reference (loop desugaring).
                    if matches!(pattern, Pattern::Ident(n) if n.name.starts_with("__")) {
                        self.infer_pattern(pattern, &mut local_env)?;
                    }
                    let expr_ty = self.infer_expr(expr, &mut local_env)?;
                    let expr_ty_applied = self.apply(expr_ty.clone());
                    let expr_ty_applied = self.expand_alias(expr_ty_applied);

                    fn is_effect_like(ty: &Type) -> bool {
                        match ty {
                            Type::Con(name, args) => match name.as_str() {
                                "Effect" | "Resource" | "Source" => args.len() == 2,
                                _ => false,
                            },
                            Type::App(base, args) => {
                                if let Type::Con(name, existing) = &**base {
                                    if name == "Effect" || name == "Resource" || name == "Source" {
                                        let mut combined = existing.clone();
                                        combined.extend(args.iter().cloned());
                                        return combined.len() == 2;
                                    }
                                }
                                false
                            }
                            _ => false,
                        }
                    }

                    let mut is_impure = is_effect_like(&expr_ty_applied);
                    if !is_impure && matches!(expr_ty_applied, Type::Var(_)) {
                        // If we still don't know the type, fall back to a unification-based check.
                        // This ensures we reject effectful expressions even when they haven't been
                        // forced by later constraints yet.
                        let snapshot = self.subst.clone();
                        let let_err_ty = self.fresh_var();
                        is_impure = self
                            .bind_effect_value(expr_ty.clone(), let_err_ty, expr_span(expr))
                            .is_ok();
                        self.subst = snapshot;
                    }

                    if is_impure {
                        let mut message =
                            "use `<-` to run effects; `=` binds pure values".to_string();
                        if std::env::var("AIVI_DEBUG_TRACE").is_ok_and(|v| v == "1") {
                            let ty_str = self.type_to_string(&expr_ty);
                            message = format!("{message} (expr_ty={ty_str}, expr_ty_dbg={expr_ty:?})");
                        }
                        return Err(TypeError {
                            span: expr_span(expr),
                            message,
                            expected: None,
                            found: None,
                        });
                    }

                    let pat_ty = self.infer_pattern(pattern, &mut local_env)?;
                    self.unify_with_span(pat_ty, expr_ty, pattern_span(pattern))?;
                }
                BlockItem::Filter { expr, .. } => {
                    let expr_ty = self.infer_expr(expr, &mut local_env)?;
                    self.unify_with_span(expr_ty, Type::con("Bool"), expr_span(expr))?;
                }
                BlockItem::Yield { expr, .. } | BlockItem::Recurse { expr, .. } => {
                    let _ = self.infer_expr(expr, &mut local_env)?;
                }
                BlockItem::When { cond, effect, .. }
                | BlockItem::Unless { cond, effect, .. } => {
                    self.infer_expr(cond, &mut local_env)?;
                    self.infer_expr(effect, &mut local_env)?;
                }
                BlockItem::Given { cond, fail_expr, .. } => {
                    self.infer_expr(cond, &mut local_env)?;
                    self.infer_expr(fail_expr, &mut local_env)?;
                }
                BlockItem::On { handler, .. } => {
                    let _ = self.infer_expr(handler, &mut local_env)?;
                }
                BlockItem::Expr { expr, .. } => {
                    let expr_ty = self.infer_expr(expr, &mut local_env)?;
                    if idx + 1 == items.len() {
                        result_ty = self.fresh_var();
                        let expected =
                            Type::con("Effect").app(vec![err_ty.clone(), result_ty.clone()]);
                        self.unify_with_span(expr_ty, expected, expr_span(expr))?;
                    } else {
                        // Expression statements only auto-run effects when they return `Unit`.
                        // For non-`Unit` results, require an explicit `<-` bind.
                        let value_ty =
                            self.require_effect_value(expr_ty, err_ty.clone(), expr_span(expr))?;
                        self.unify_with_span(value_ty, Type::con("Unit"), expr_span(expr))?;
                    }
                }
            }
        }
        Ok(Type::con("Effect").app(vec![err_ty, result_ty]))
    }

    fn infer_generate_block(
        &mut self,
        items: &[BlockItem],
        env: &mut TypeEnv,
    ) -> Result<Type, TypeError> {
        let mut local_env = env.clone();
        let yield_ty = self.fresh_var();
        let mut current_elem: Option<Type> = None;
        for item in items {
            match item {
                BlockItem::Bind { pattern, expr, .. } => {
                    let expr_ty = self.infer_expr(expr, &mut local_env)?;
                    let bind_elem = self.generate_source_elem(expr_ty, expr_span(expr))?;
                    let pat_ty = self.infer_pattern(pattern, &mut local_env)?;
                    self.unify_with_span(pat_ty, bind_elem.clone(), pattern_span(pattern))?;
                    current_elem = Some(bind_elem);
                }
                BlockItem::Let { pattern, expr, .. } => {
                    if matches!(pattern, Pattern::Ident(n) if n.name.starts_with("__")) {
                        self.infer_pattern(pattern, &mut local_env)?;
                    }
                    let expr_ty = self.infer_expr(expr, &mut local_env)?;
                    let pat_ty = self.infer_pattern(pattern, &mut local_env)?;
                    self.unify_with_span(pat_ty, expr_ty, pattern_span(pattern))?;
                }
                BlockItem::Filter { expr, .. } => {
                    let mut guard_env = local_env.clone();
                    if let Some(elem) = current_elem.clone() {
                        guard_env.insert("_".to_string(), Scheme::mono(elem));
                    }
                    let expr_ty = self.infer_expr(expr, &mut guard_env)?;
                    if self
                        .unify_with_span(expr_ty.clone(), Type::con("Bool"), expr_span(expr))
                        .is_err()
                    {
                        let arg_ty = current_elem.clone().unwrap_or_else(|| self.fresh_var());
                        let func_ty = Type::Func(Box::new(arg_ty), Box::new(Type::con("Bool")));
                        self.unify_with_span(expr_ty, func_ty, expr_span(expr))?;
                    }
                }
                BlockItem::Yield { expr, .. } => {
                    let expr_ty = self.infer_expr(expr, &mut local_env)?;
                    self.unify_with_span(expr_ty, yield_ty.clone(), expr_span(expr))?;
                }
                BlockItem::Recurse { expr, .. } | BlockItem::Expr { expr, .. } => {
                    let _ = self.infer_expr(expr, &mut local_env)?;
                }
                BlockItem::When { cond, effect, .. }
                | BlockItem::Unless { cond, effect, .. } => {
                    self.infer_expr(cond, &mut local_env)?;
                    self.infer_expr(effect, &mut local_env)?;
                }
                BlockItem::Given { cond, fail_expr, .. } => {
                    self.infer_expr(cond, &mut local_env)?;
                    self.infer_expr(fail_expr, &mut local_env)?;
                }
                BlockItem::On { handler, .. } => {
                    let _ = self.infer_expr(handler, &mut local_env)?;
                }
            }
        }
        Ok(Type::con("Generator").app(vec![yield_ty]))
    }

    fn infer_resource_block(
        &mut self,
        items: &[BlockItem],
        env: &mut TypeEnv,
    ) -> Result<Type, TypeError> {
        let mut local_env = env.clone();
        let err_ty = self.fresh_var();
        let yield_ty = self.fresh_var();
        for item in items {
            match item {
                BlockItem::Bind { pattern, expr, .. } => {
                    let expr_ty = self.infer_expr(expr, &mut local_env)?;
                    let value_ty =
                        self.bind_effect_value(expr_ty, err_ty.clone(), expr_span(expr))?;
                    let pat_ty = self.infer_pattern(pattern, &mut local_env)?;
                    self.unify_with_span(pat_ty, value_ty, pattern_span(pattern))?;
                }
                BlockItem::Let { pattern, expr, .. } => {
                    if matches!(pattern, Pattern::Ident(n) if n.name.starts_with("__")) {
                        self.infer_pattern(pattern, &mut local_env)?;
                    }
                    let expr_ty = self.infer_expr(expr, &mut local_env)?;
                    let pat_ty = self.infer_pattern(pattern, &mut local_env)?;
                    self.unify_with_span(pat_ty, expr_ty, pattern_span(pattern))?;
                }
                BlockItem::Filter { expr, .. } => {
                    let expr_ty = self.infer_expr(expr, &mut local_env)?;
                    self.unify_with_span(expr_ty, Type::con("Bool"), expr_span(expr))?;
                }
                BlockItem::Yield { expr, .. } => {
                    let expr_ty = self.infer_expr(expr, &mut local_env)?;
                    self.unify_with_span(expr_ty, yield_ty.clone(), expr_span(expr))?;
                }
                BlockItem::Recurse { expr, .. } | BlockItem::Expr { expr, .. } => {
                    let _ = self.infer_expr(expr, &mut local_env)?;
                }
                BlockItem::When { cond, effect, .. }
                | BlockItem::Unless { cond, effect, .. } => {
                    self.infer_expr(cond, &mut local_env)?;
                    self.infer_expr(effect, &mut local_env)?;
                }
                BlockItem::Given { cond, fail_expr, .. } => {
                    self.infer_expr(cond, &mut local_env)?;
                    self.infer_expr(fail_expr, &mut local_env)?;
                }
                BlockItem::On { handler, .. } => {
                    let _ = self.infer_expr(handler, &mut local_env)?;
                }
            }
        }
        Ok(Type::con("Resource").app(vec![err_ty, yield_ty]))
    }

    fn infer_patch(
        &mut self,
        target_ty: Type,
        fields: &[RecordField],
        env: &mut TypeEnv,
    ) -> Result<Type, TypeError> {
        for field in fields {
            let value_ty = self.infer_expr(&field.value, env)?;
            let field_ty = self.infer_patch_path_focus(
                target_ty.clone(),
                &field.path,
                env,
                field.span.clone(),
            )?;
            let value_applied = self.apply(value_ty.clone());
            let field_applied = self.apply(field_ty.clone());
            if matches!(field_applied, Type::Func(_, _))
                && self
                    .unify_with_span(value_ty.clone(), field_ty.clone(), field.span.clone())
                    .is_ok()
            {
                continue;
            }
            if matches!(value_applied, Type::Func(_, _)) {
                let func_ty = Type::Func(Box::new(field_ty.clone()), Box::new(field_ty.clone()));
                if self
                    .unify_with_span(value_ty.clone(), func_ty, field.span.clone())
                    .is_ok()
                {
                    continue;
                }
            }
            self.unify_with_span(value_ty, field_ty, field.span.clone())?;
        }
        Ok(target_ty)
    }

    fn infer_patch_path_focus(
        &mut self,
        target_ty: Type,
        path: &[PathSegment],
        env: &mut TypeEnv,
        span: Span,
    ) -> Result<Type, TypeError> {
        if path.is_empty() {
            return Err(TypeError {
                span,
                message: "patch path must not be empty".to_string(),
                expected: None,
                found: None,
            });
        }

        let mut current_ty = target_ty;
        for segment in path {
            match segment {
                PathSegment::Field(name) => {
                    let field_ty = self.fresh_var();
                    let mut fields = BTreeMap::new();
                    fields.insert(name.name.clone(), field_ty.clone());
                    self.unify_with_span(
                        current_ty,
                        Type::Record { fields, open: true },
                        name.span.clone(),
                    )?;
                    current_ty = field_ty;
                }
                PathSegment::All(seg_span) => {
                    let checkpoint = self.subst.clone();

                    // List traversal: `List A` -> `A`
                    let elem_ty = self.fresh_var();
                    if self
                        .unify_with_span(
                            current_ty.clone(),
                            Type::con("List").app(vec![elem_ty.clone()]),
                            seg_span.clone(),
                        )
                        .is_ok()
                    {
                        current_ty = elem_ty;
                        continue;
                    }

                    // Map traversal: `Map K V` -> `V`
                    self.subst = checkpoint;
                    let key_ty = self.fresh_var();
                    let value_ty = self.fresh_var();
                    self.unify_with_span(
                        current_ty,
                        Type::con("Map").app(vec![key_ty, value_ty.clone()]),
                        seg_span.clone(),
                    )?;
                    current_ty = value_ty;
                }
                PathSegment::Index(expr, seg_span) => {
                    let unbound = collect_unbound_names(expr, env);
                    if unbound.is_empty() {
                        let idx_ty = self.infer_expr(expr, env)?;
                        let checkpoint = self.subst.clone();

                        // List index: `List A` + `Int` -> `A`
                        let elem_ty = self.fresh_var();
                        if self
                            .unify_with_span(idx_ty.clone(), Type::con("Int"), expr_span(expr))
                            .is_ok()
                            && self
                                .unify_with_span(
                                    current_ty.clone(),
                                    Type::con("List").app(vec![elem_ty.clone()]),
                                    seg_span.clone(),
                                )
                                .is_ok()
                        {
                            current_ty = elem_ty;
                            continue;
                        }

                        // Map key selector: `Map K V` + `K` -> `V`
                        self.subst = checkpoint;
                        let key_ty = self.fresh_var();
                        let value_ty = self.fresh_var();
                        self.unify_with_span(
                            current_ty,
                            Type::con("Map").app(vec![key_ty.clone(), value_ty.clone()]),
                            seg_span.clone(),
                        )?;
                        self.unify_with_span(idx_ty, key_ty, expr_span(expr))?;
                        current_ty = value_ty;
                    } else {
                        // Predicate selector: `items[price > 80]` treats unbound names as
                        // implicit field accesses on the element (`_.price > 80`).
                        let checkpoint = self.subst.clone();

                        // List predicate: element is `A`, predicate is `A -> Bool`.
                        let elem_ty = self.fresh_var();
                        if self
                            .unify_with_span(
                                current_ty.clone(),
                                Type::con("List").app(vec![elem_ty.clone()]),
                                seg_span.clone(),
                            )
                            .is_ok()
                        {
                            let param = "__it".to_string();
                            let mut env2 = env.clone();
                            env2.insert(param.clone(), Scheme::mono(elem_ty.clone()));
                            let rewritten =
                                rewrite_implicit_field_vars(expr.clone(), &param, &unbound);
                            let pred_ty = self.infer_expr(&rewritten, &mut env2)?;
                            if self
                                .unify_with_span(pred_ty, Type::con("Bool"), expr_span(&rewritten))
                                .is_ok()
                            {
                                current_ty = elem_ty;
                                continue;
                            }
                        }

                        // Map predicate: element is `{ key: K, value: V }`, focus is `V`.
                        self.subst = checkpoint;
                        let key_ty = self.fresh_var();
                        let value_ty = self.fresh_var();
                        self.unify_with_span(
                            current_ty.clone(),
                            Type::con("Map").app(vec![key_ty.clone(), value_ty.clone()]),
                            seg_span.clone(),
                        )?;
                        let mut entry_fields = BTreeMap::new();
                        entry_fields.insert("key".to_string(), key_ty);
                        entry_fields.insert("value".to_string(), value_ty.clone());
                        let entry_ty = Type::Record {
                            fields: entry_fields,
                            open: true,
                        };

                        let param = "__it".to_string();
                        let mut env2 = env.clone();
                        env2.insert(param.clone(), Scheme::mono(entry_ty));
                        let rewritten = rewrite_implicit_field_vars(expr.clone(), &param, &unbound);
                        let pred_ty = self.infer_expr(&rewritten, &mut env2)?;
                        self.unify_with_span(pred_ty, Type::con("Bool"), expr_span(&rewritten))?;
                        current_ty = value_ty;
                    }
                }
            }
        }

        Ok(self.apply(current_ty))
    }

    fn infer_patch_literal(
        &mut self,
        fields: &[RecordField],
        env: &mut TypeEnv,
    ) -> Result<Type, TypeError> {
        let mut record_ty = Type::Record {
            fields: BTreeMap::new(),
            open: true,
        };
        for field in fields {
            if field.spread {
                return Err(TypeError {
                    span: field.span.clone(),
                    message: "patch literal does not support record spread".to_string(),
                    expected: None,
                    found: None,
                });
            }
            let value_ty = self.infer_expr(&field.value, env)?;
            let field_ty = self.fresh_var();
            let requirement = self.record_from_path(&field.path, field_ty.clone());
            record_ty = self.merge_records(record_ty, requirement, field.span.clone())?;

            let value_applied = self.apply(value_ty.clone());
            if matches!(value_applied, Type::Func(_, _)) {
                let func_ty = Type::Func(Box::new(field_ty.clone()), Box::new(field_ty.clone()));
                if self
                    .unify_with_span(value_ty.clone(), func_ty, field.span.clone())
                    .is_ok()
                {
                    continue;
                }
            }
            self.unify_with_span(value_ty, field_ty, field.span.clone())?;
        }
        let record_ty = self.apply(record_ty);
        Ok(Type::Func(Box::new(record_ty.clone()), Box::new(record_ty)))
    }

    fn infer_pattern(&mut self, pattern: &Pattern, env: &mut TypeEnv) -> Result<Type, TypeError> {
        match pattern {
            Pattern::Wildcard(_) => Ok(self.fresh_var()),
            Pattern::Ident(name) => {
                let ty = self.fresh_var();
                env.insert(name.name.clone(), Scheme::mono(ty.clone()));
                Ok(ty)
            }
            Pattern::SubjectIdent(name) => {
                let ty = self.fresh_var();
                env.insert(name.name.clone(), Scheme::mono(ty.clone()));
                Ok(ty)
            }
            Pattern::Literal(literal) => Ok(self.literal_type(literal)),
            Pattern::At { name, pattern, .. } => {
                let ty = self.infer_pattern(pattern, env)?;
                env.insert(name.name.clone(), Scheme::mono(ty.clone()));
                Ok(ty)
            }
            Pattern::Constructor { name, args, span } => {
                let scheme = env.get(&name.name).cloned().ok_or_else(|| TypeError {
                    span: span.clone(),
                    message: format!("unknown constructor '{}'", name.name),
                    expected: None,
                    found: None,
                })?;
                let mut ctor_ty = self.instantiate(&scheme);
                for arg in args {
                    let arg_ty = self.infer_pattern(arg, env)?;
                    let result_ty = self.fresh_var();
                    self.unify_with_span(
                        ctor_ty,
                        Type::Func(Box::new(arg_ty), Box::new(result_ty.clone())),
                        pattern_span(arg),
                    )?;
                    ctor_ty = result_ty;
                }
                Ok(ctor_ty)
            }
            Pattern::Tuple { items, .. } => {
                let mut tys = Vec::new();
                for item in items {
                    tys.push(self.infer_pattern(item, env)?);
                }
                Ok(Type::Tuple(tys))
            }
            Pattern::List { items, rest, .. } => {
                let elem_ty = self.fresh_var();
                for item in items {
                    let item_ty = self.infer_pattern(item, env)?;
                    self.unify_with_span(item_ty, elem_ty.clone(), pattern_span(item))?;
                }
                if let Some(rest) = rest {
                    let rest_ty = self.infer_pattern(rest, env)?;
                    let list_ty = Type::con("List").app(vec![elem_ty.clone()]);
                    self.unify_with_span(rest_ty, list_ty, pattern_span(rest))?;
                }
                Ok(Type::con("List").app(vec![elem_ty]))
            }
            Pattern::Record { fields, .. } => self.infer_record_pattern(fields, env),
        }
    }

    fn infer_record_pattern(
        &mut self,
        fields: &[RecordPatternField],
        env: &mut TypeEnv,
    ) -> Result<Type, TypeError> {
        let mut record_ty = Type::Record {
            fields: BTreeMap::new(),
            open: true,
        };
        for field in fields {
            let field_ty = self.infer_pattern(&field.pattern, env)?;
            let nested = self.record_from_pattern_path(&field.path, field_ty);
            record_ty = self.merge_records(record_ty, nested, field.span.clone())?;
        }
        Ok(record_ty)
    }

    fn bind_effect_value(
        &mut self,
        expr_ty: Type,
        err_ty: Type,
        span: Span,
    ) -> Result<Type, TypeError> {
        let value_ty = self.fresh_var();
        let source_kind_ty = self.fresh_var();
        let source_ty = Type::con("Source").app(vec![source_kind_ty.clone(), value_ty.clone()]);
        if self
            .unify_with_span(expr_ty.clone(), source_ty, span.clone())
            .is_ok()
        {
            let source_err_ty = Type::con("SourceError").app(vec![source_kind_ty]);
            self.unify_with_span(err_ty, source_err_ty, span)?;
            return Ok(value_ty);
        }
        let effect_ty = Type::con("Effect").app(vec![err_ty.clone(), value_ty.clone()]);
        let resource_ty = Type::con("Resource").app(vec![err_ty, value_ty.clone()]);
        if self
            .unify_with_span(expr_ty.clone(), effect_ty, span.clone())
            .is_ok()
        {
            return Ok(value_ty);
        }
        self.unify_with_span(expr_ty, resource_ty, span)?;
        Ok(value_ty)
    }

    fn generate_source_elem(&mut self, expr_ty: Type, span: Span) -> Result<Type, TypeError> {
        let elem_ty = self.fresh_var();
        let list_ty = Type::con("List").app(vec![elem_ty.clone()]);
        let gen_ty = Type::con("Generator").app(vec![elem_ty.clone()]);
        if self
            .unify_with_span(expr_ty.clone(), list_ty, span.clone())
            .is_ok()
        {
            return Ok(elem_ty);
        }
        self.unify_with_span(expr_ty, gen_ty, span)?;
        Ok(elem_ty)
    }

    fn record_field_type(
        &mut self,
        base_ty: Type,
        path: &[PathSegment],
        span: Span,
    ) -> Result<Type, TypeError> {
        let field_ty = self.fresh_var();
        let requirement = self.record_from_path(path, field_ty.clone());
        self.unify_with_span(base_ty, requirement, span)?;
        Ok(field_ty)
    }

    fn record_from_path(&mut self, path: &[PathSegment], value: Type) -> Type {
        let mut current = value;
        for segment in path.iter().rev() {
            match segment {
                PathSegment::Field(name) => {
                    let mut fields = BTreeMap::new();
                    fields.insert(name.name.clone(), current);
                    current = Type::Record { fields, open: true };
                }
                PathSegment::Index(_, _) | PathSegment::All(_) => {
                    current = Type::con("List").app(vec![current]);
                }
            }
        }
        current
    }

    fn record_from_pattern_path(&mut self, path: &[SpannedName], value: Type) -> Type {
        let mut current = value;
        for segment in path.iter().rev() {
            let mut fields = BTreeMap::new();
            fields.insert(segment.name.clone(), current);
            current = Type::Record { fields, open: true };
        }
        current
    }

    fn merge_records(&mut self, left: Type, right: Type, span: Span) -> Result<Type, TypeError> {
        let left = self.apply(left);
        let right = self.apply(right);
        let left_clone = left.clone();
        let right_clone = right.clone();
        match (left, right) {
            (
                Type::Record { mut fields, open },
                Type::Record {
                    fields: other,
                    open: other_open,
                },
            ) => {
                for (name, ty) in other {
                    if let Some(existing) = fields.get(&name).cloned() {
                        self.unify(existing, ty.clone(), span.clone())?;
                    } else {
                        fields.insert(name, ty);
                    }
                }
                Ok(Type::Record {
                    fields,
                    open: open || other_open,
                })
            }
            (Type::Var(var), other) => {
                self.bind_var(var, other, span.clone(), true)?;
                Ok(self.apply(Type::Var(var)))
            }
            (other, Type::Var(var)) => {
                self.bind_var(var, other, span.clone(), false)?;
                Ok(self.apply(Type::Var(var)))
            }
            _ => {
                self.unify(left_clone.clone(), right_clone, span)?;
                Ok(self.apply(left_clone))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::diagnostics::{Position, Span};
    use crate::typecheck::checker::TypeChecker;
    use crate::typecheck::types::Type;

    fn dummy_span() -> Span {
        Span {
            start: Position { line: 1, column: 1 },
            end: Position { line: 1, column: 1 },
        }
    }

    #[test]
    fn bind_effect_value_rejects_pure_int() {
        let mut checker = TypeChecker::new();
        let span = dummy_span();
        let err_ty = checker.fresh_var();
        let res = checker.bind_effect_value(Type::con("Int"), err_ty, span);
        assert!(res.is_err(), "expected Err, got: {res:?}");
    }
}
