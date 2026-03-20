impl TypeChecker {
    fn collect_applicative_pattern_binders(pattern: &Pattern, out: &mut Vec<String>) {
        match pattern {
            Pattern::Wildcard(_) | Pattern::Literal(_) => {}
            Pattern::Ident(name) | Pattern::SubjectIdent(name) => out.push(name.name.clone()),
            Pattern::At { name, pattern, .. } => {
                out.push(name.name.clone());
                Self::collect_applicative_pattern_binders(pattern, out);
            }
            Pattern::Constructor { args, .. } | Pattern::Tuple { items: args, .. } => {
                for arg in args {
                    Self::collect_applicative_pattern_binders(arg, out);
                }
            }
            Pattern::List { items, rest, .. } => {
                for item in items {
                    Self::collect_applicative_pattern_binders(item, out);
                }
                if let Some(rest) = rest.as_deref() {
                    Self::collect_applicative_pattern_binders(rest, out);
                }
            }
            Pattern::Record { fields, .. } => {
                for field in fields {
                    Self::collect_applicative_pattern_binders(&field.pattern, out);
                }
            }
        }
    }

    fn collect_applicative_references(
        expr: &Expr,
        interesting: &HashSet<String>,
        bound: &mut Vec<String>,
        out: &mut HashSet<String>,
    ) {
        match expr {
            Expr::Ident(name) => {
                if interesting.contains(&name.name)
                    && !bound.iter().rev().any(|bound_name| bound_name == &name.name)
                {
                    out.insert(name.name.clone());
                }
            }
            Expr::Suffixed { base, .. } => {
                Self::collect_applicative_references(base, interesting, bound, out);
            }
            Expr::Literal(_) | Expr::Raw { .. } | Expr::FieldSection { .. } => {}
            Expr::UnaryNeg { expr, .. } => {
                Self::collect_applicative_references(expr, interesting, bound, out);
            }
            Expr::TextInterpolate { parts, .. } => {
                for part in parts {
                    if let TextPart::Expr { expr, .. } = part {
                        Self::collect_applicative_references(expr, interesting, bound, out);
                    }
                }
            }
            Expr::List { items, .. } => {
                for item in items {
                    Self::collect_applicative_references(&item.expr, interesting, bound, out);
                }
            }
            Expr::Tuple { items, .. } => {
                for item in items {
                    Self::collect_applicative_references(item, interesting, bound, out);
                }
            }
            Expr::Record { fields, .. } | Expr::PatchLit { fields, .. } => {
                for field in fields {
                    for segment in &field.path {
                        if let PathSegment::Index(expr, _) = segment {
                            Self::collect_applicative_references(expr, interesting, bound, out);
                        }
                    }
                    Self::collect_applicative_references(&field.value, interesting, bound, out);
                }
            }
            Expr::FieldAccess { base, .. } => {
                Self::collect_applicative_references(base, interesting, bound, out);
            }
            Expr::Index { base, index, .. } => {
                Self::collect_applicative_references(base, interesting, bound, out);
                Self::collect_applicative_references(index, interesting, bound, out);
            }
            Expr::Call { func, args, .. } => {
                Self::collect_applicative_references(func, interesting, bound, out);
                for arg in args {
                    Self::collect_applicative_references(arg, interesting, bound, out);
                }
            }
            Expr::Lambda { params, body, .. } => {
                let before = bound.len();
                for param in params {
                    Self::collect_applicative_pattern_binders(param, bound);
                }
                Self::collect_applicative_references(body, interesting, bound, out);
                bound.truncate(before);
            }
            Expr::Match {
                scrutinee, arms, ..
            } => {
                if let Some(scrutinee) = scrutinee.as_deref() {
                    Self::collect_applicative_references(scrutinee, interesting, bound, out);
                }
                for arm in arms {
                    let before = bound.len();
                    Self::collect_applicative_pattern_binders(&arm.pattern, bound);
                    if let Some(guard) = arm.guard.as_ref() {
                        Self::collect_applicative_references(guard, interesting, bound, out);
                    }
                    Self::collect_applicative_references(&arm.body, interesting, bound, out);
                    bound.truncate(before);
                }
            }
            Expr::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => {
                Self::collect_applicative_references(cond, interesting, bound, out);
                Self::collect_applicative_references(then_branch, interesting, bound, out);
                Self::collect_applicative_references(else_branch, interesting, bound, out);
            }
            Expr::Binary { left, right, .. } => {
                Self::collect_applicative_references(left, interesting, bound, out);
                Self::collect_applicative_references(right, interesting, bound, out);
            }
            Expr::Flow { root, .. } => {
                Self::collect_applicative_references(root, interesting, bound, out);
            }
            Expr::Block { items, .. } => {
                let before = bound.len();
                for item in items {
                    match item {
                        BlockItem::Bind { pattern, expr, .. }
                        | BlockItem::Let { pattern, expr, .. } => {
                            Self::collect_applicative_references(expr, interesting, bound, out);
                            Self::collect_applicative_pattern_binders(pattern, bound);
                        }
                        BlockItem::Filter { expr, .. }
                        | BlockItem::Yield { expr, .. }
                        | BlockItem::Recurse { expr, .. }
                        | BlockItem::Expr { expr, .. } => {
                            Self::collect_applicative_references(expr, interesting, bound, out);
                        }
                        BlockItem::When { cond, effect, .. }
                        | BlockItem::Unless { cond, effect, .. } => {
                            Self::collect_applicative_references(cond, interesting, bound, out);
                            Self::collect_applicative_references(effect, interesting, bound, out);
                        }
                        BlockItem::Given { cond, fail_expr, .. } => {
                            Self::collect_applicative_references(cond, interesting, bound, out);
                            Self::collect_applicative_references(
                                fail_expr,
                                interesting,
                                bound,
                                out,
                            );
                        }
                    }
                }
                bound.truncate(before);
            }
            Expr::Mock { substitutions, body, .. } => {
                for sub in substitutions {
                    if let Some(value) = &sub.value {
                        Self::collect_applicative_references(value, interesting, bound, out);
                    }
                }
                Self::collect_applicative_references(body, interesting, bound, out);
            }
        }
    }

    fn applicative_references(expr: &Expr, interesting: &HashSet<String>) -> HashSet<String> {
        let mut out = HashSet::new();
        let mut bound = Vec::new();
        Self::collect_applicative_references(expr, interesting, &mut bound, &mut out);
        out
    }

    fn make_applicative_lambda(pattern: Pattern, body: Expr, span: Span) -> Expr {
        Expr::Lambda {
            params: vec![pattern],
            body: Box::new(body),
            span,
        }
    }

    fn make_applicative_call(func_name: &str, args: Vec<Expr>, span: Span) -> Expr {
        Expr::Call {
            func: Box::new(Expr::Ident(SpannedName {
                name: func_name.to_string(),
                span: span.clone(),
            })),
            args,
            span,
        }
    }

    fn desugar_applicative_do_expr(
        &self,
        items: &[BlockItem],
        block_span: &Span,
    ) -> Result<Expr, TypeError> {
        if items.is_empty() {
            return Ok(Self::make_applicative_call(
                "of",
                vec![Expr::Ident(SpannedName {
                    name: "Unit".to_string(),
                    span: block_span.clone(),
                })],
                block_span.clone(),
            ));
        }

        let Some(BlockItem::Expr {
            expr: final_expr,
            span: final_span,
        }) = items.last()
        else {
            return Err(TypeError {
                span: block_span.clone(),
                message: "`do Applicative { ... }` requires a final pure result expression".to_string(),
                expected: None,
                found: None,
            });
        };

        let mut body = final_expr.clone();
        let mut applicative_inputs_rev = Vec::new();

        for item in items[..items.len() - 1].iter().rev() {
            match item {
                BlockItem::Bind {
                    pattern,
                    expr,
                    span,
                } => {
                    body = Self::make_applicative_lambda(pattern.clone(), body, span.clone());
                    applicative_inputs_rev.push((expr.clone(), span.clone()));
                }
                BlockItem::Let {
                    pattern,
                    expr,
                    span,
                } => {
                    let lambda =
                        Self::make_applicative_lambda(pattern.clone(), body, span.clone());
                    body = Expr::Call {
                        func: Box::new(lambda),
                        args: vec![expr.clone()],
                        span: span.clone(),
                    };
                }
                BlockItem::Expr { span, .. } => {
                    return Err(TypeError {
                        span: span.clone(),
                        message: "non-final expression statements are not allowed in `do Applicative { ... }`; keep a single final result expression".to_string(),
                        expected: None,
                        found: None,
                    });
                }
                BlockItem::When { span, .. } | BlockItem::Unless { span, .. } => {
                    return Err(TypeError {
                        span: span.clone(),
                        message: "`when`/`unless` is only available in `do Effect { ... }` blocks, not `do Applicative { ... }`".to_string(),
                        expected: None,
                        found: None,
                    });
                }
                BlockItem::Given { span, .. } => {
                    return Err(TypeError {
                        span: span.clone(),
                        message: "`given` is only available in `do Effect { ... }` blocks, not `do Applicative { ... }`".to_string(),
                        expected: None,
                        found: None,
                    });
                }
                BlockItem::Recurse { span, .. } => {
                    return Err(TypeError {
                        span: span.clone(),
                        message: "`recurse` is only available in `do Effect { ... }` or `generate { ... }` blocks, not `do Applicative { ... }`".to_string(),
                        expected: None,
                        found: None,
                    });
                }
                BlockItem::Filter { span, .. } | BlockItem::Yield { span, .. } => {
                    return Err(TypeError {
                        span: span.clone(),
                        message: "`do Applicative { ... }` only supports `<-`, `=`, and a final result expression".to_string(),
                        expected: None,
                        found: None,
                    });
                }
            }
        }

        if applicative_inputs_rev.is_empty() {
            return Ok(Self::make_applicative_call(
                "of",
                vec![body],
                final_span.clone(),
            ));
        }

        let mut applicative_inputs = applicative_inputs_rev;
        applicative_inputs.reverse();

        let mut acc = Self::make_applicative_call(
            "map",
            vec![body, applicative_inputs[0].0.clone()],
            applicative_inputs[0].1.clone(),
        );
        for (expr, span) in applicative_inputs.into_iter().skip(1) {
            acc = Self::make_applicative_call("ap", vec![acc, expr], span);
        }
        Ok(acc)
    }

    fn infer_applicative_do_block(
        &mut self,
        applicative_span: &Span,
        items: &[BlockItem],
        env: &mut TypeEnv,
    ) -> Result<Type, TypeError> {
        let mut prior_applicative_names = HashSet::new();
        let mut tainted_let_names = HashSet::new();

        for item in items {
            match item {
                BlockItem::Bind { pattern, expr, .. } => {
                    let mut forbidden = prior_applicative_names.clone();
                    forbidden.extend(tainted_let_names.iter().cloned());
                    let refs = Self::applicative_references(expr, &forbidden);
                    if !refs.is_empty() {
                        let mut names: Vec<String> = refs.into_iter().collect();
                        names.sort();
                        return Err(TypeError {
                            span: expr_span(expr),
                            message: format!(
                                "applicative bindings must be independent; this `<-` expression depends on earlier applicative names: {}",
                                names.join(", ")
                            ),
                            expected: None,
                            found: None,
                        });
                    }

                    let mut binders = Vec::new();
                    Self::collect_applicative_pattern_binders(pattern, &mut binders);
                    for binder in binders {
                        prior_applicative_names.remove(&binder);
                        tainted_let_names.remove(&binder);
                        prior_applicative_names.insert(binder);
                    }
                }
                BlockItem::Let { pattern, expr, .. } => {
                    let mut forbidden = prior_applicative_names.clone();
                    forbidden.extend(tainted_let_names.iter().cloned());
                    let refs = Self::applicative_references(expr, &forbidden);
                    let mut binders = Vec::new();
                    Self::collect_applicative_pattern_binders(pattern, &mut binders);
                    for binder in &binders {
                        prior_applicative_names.remove(binder);
                        tainted_let_names.remove(binder);
                    }
                    if !refs.is_empty() {
                        for binder in binders {
                            tainted_let_names.insert(binder);
                        }
                    }
                }
                BlockItem::Expr { .. } => {}
                BlockItem::When { span, .. } | BlockItem::Unless { span, .. } => {
                    return Err(TypeError {
                        span: span.clone(),
                        message: "`when`/`unless` is only available in `do Effect { ... }` blocks, not `do Applicative { ... }`".to_string(),
                        expected: None,
                        found: None,
                    });
                }
                BlockItem::Given { span, .. } => {
                    return Err(TypeError {
                        span: span.clone(),
                        message: "`given` is only available in `do Effect { ... }` blocks, not `do Applicative { ... }`".to_string(),
                        expected: None,
                        found: None,
                    });
                }
                BlockItem::Recurse { span, .. } => {
                    return Err(TypeError {
                        span: span.clone(),
                        message: "`recurse` is only available in `do Effect { ... }` or `generate { ... }` blocks, not `do Applicative { ... }`".to_string(),
                        expected: None,
                        found: None,
                    });
                }
                BlockItem::Filter { span, .. } | BlockItem::Yield { span, .. } => {
                    return Err(TypeError {
                        span: span.clone(),
                        message: "`do Applicative { ... }` only supports `<-`, `=`, and a final result expression".to_string(),
                        expected: None,
                        found: None,
                    });
                }
            }
        }

        let desugared = self.desugar_applicative_do_expr(items, applicative_span)?;
        self.infer_expr(&desugared, env)
    }

    fn require_effect_value(
        &mut self,
        expr_ty: Type,
        err_ty: Type,
        span: Span,
    ) -> Result<Type, TypeError> {
        if let Some(value_ty) = self.infallible_effect_value_ty(expr_ty.clone()) {
            return Ok(value_ty);
        }
        let value_ty = self.fresh_var();
        let effect_ty = Type::con("Effect").app(vec![err_ty, value_ty.clone()]);
        self.unify_with_span(expr_ty, effect_ty, span)?;
        Ok(value_ty)
    }

    fn effect_type_args(&mut self, ty: &Type) -> Option<Vec<Type>> {
        match ty {
            Type::Con(name, args) if self.type_name_matches(name, "Effect") => Some(args.clone()),
            Type::App(base, args) => {
                let mut collected = self.effect_type_args(base)?;
                collected.extend(args.iter().cloned());
                Some(collected)
            }
            _ => None,
        }
    }

    fn infallible_effect_value_ty(&mut self, expr_ty: Type) -> Option<Type> {
        let applied = self.apply(expr_ty);
        let expanded = self.expand_alias(applied);
        let args = self.effect_type_args(&expanded)?;
        if args.len() == 1 {
            Some(args[0].clone())
        } else {
            None
        }
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
                    let pat_ty_for_span = pat_ty.clone();
                    self.unify_with_span(pat_ty, value_ty, pattern_span(pattern))?;
                    let resolved = self.apply(pat_ty_for_span);
                    self.span_types.push((pattern_span(pattern), resolved));
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

                    let is_impure = is_effect_like(&expr_ty_applied);

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
                    let pat_ty_for_span = pat_ty.clone();
                    self.unify_with_span(pat_ty, expr_ty, pattern_span(pattern))?;
                    let resolved = self.apply(pat_ty_for_span);
                    self.span_types.push((pattern_span(pattern), resolved));
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
                BlockItem::Expr { expr, .. } => {
                    let expr_ty = self.infer_expr(expr, &mut local_env)?;
                    if idx + 1 == items.len() {
                        result_ty = self.fresh_var();
                        if let Some(value_ty) = self.infallible_effect_value_ty(expr_ty.clone()) {
                            self.unify_with_span(value_ty, result_ty.clone(), expr_span(expr))?;
                        } else {
                            let expected =
                                Type::con("Effect").app(vec![err_ty.clone(), result_ty.clone()]);
                            self.push_deferred_constraint(expr_ty, expected, expr_span(expr));
                        }
                    } else {
                        // Bare expression desugars to `chain (λ_. body) expr`; the value is
                        // discarded, so expr need only be `Effect E A` for any A (like `_ <- expr`).
                        self.require_effect_value(expr_ty, err_ty.clone(), expr_span(expr))?;
                    }
                }
            }
        }
        self.solve_deferred_constraints()?;
        Ok(Type::con("Effect").app(vec![err_ty, result_ty]))
    }

    /// Type-check a `do Event { ... }` block.
    ///
    /// The body is type-checked as an effect block (same rules as `do Effect`),
    /// but the overall type is `EventHandle E A` rather than `Effect E A`.
    /// At the HIR level, `do Event { body }` desugars to `reactive.event(do Effect { body })`.
    pub(crate) fn infer_event_block(
        &mut self,
        items: &[BlockItem],
        env: &mut TypeEnv,
    ) -> Result<Type, TypeError> {
        let effect_ty = self.infer_effect_block(items, env)?;
        // effect_ty is Effect E A; extract E and A to produce EventHandle E A
        match effect_ty {
            Type::Con(ref name, ref args) if self.type_name_matches(name, "Effect") && args.len() == 2 => {
                Ok(self
                    .named_type("EventHandle")
                    .app(vec![args[0].clone(), args[1].clone()]))
            }
            _ => {
                // Fallback: create fresh vars
                let err_ty = self.fresh_var();
                let result_ty = self.fresh_var();
                Ok(self.named_type("EventHandle").app(vec![err_ty, result_ty]))
            }
        }
    }

    /// Type-check a generic `do M { ... }` block where `M` is not `Effect`.
    ///
    /// The block's type is `M result_ty`. Binds (`x <- expr`) unify `expr` with
    /// `M A` and bind `x : A`. Let-bindings (`x = expr`) are pure. Expression
    /// statements must be `M A` (non-final, value discarded) or `M A` (final, determines result).
    ///
        /// Effect-specific statements (`when`, `unless`, `given`, `recurse`)
    /// produce type errors since they are not available in generic monadic blocks.
    fn infer_generic_do_block(
        &mut self,
        monad_name: &str,
        _monad_span: &Span,
        items: &[BlockItem],
        env: &mut TypeEnv,
    ) -> Result<Type, TypeError> {
        // Verify the monad type constructor exists (it must be a known type).
        // For now, we accept Option and Result as known monadic types.
        // In the future this will resolve a Monad instance via class resolution.
        let monad_con = self.resolved_type_name(monad_name).to_string();
        let is_result_monad = self.type_name_matches(&monad_con, "Result");

        let mut local_env = env.clone();
        let mut result_ty = self.named_type("Unit");

        for (idx, item) in items.iter().enumerate() {
            match item {
                BlockItem::Bind { pattern, expr, .. } => {
                    let expr_ty = self.infer_expr(expr, &mut local_env)?;
                    let value_ty = self.fresh_var();
                    let expected = if is_result_monad {
                        // Result E A — partially applied, error type is inferred
                        let err_var = self.fresh_var();
                        Type::con(&monad_con).app(vec![err_var, value_ty.clone()])
                    } else {
                        // Option A, List A, etc.
                        Type::con(&monad_con).app(vec![value_ty.clone()])
                    };
                    self.unify_with_span(expr_ty, expected, expr_span(expr))?;
                    let pat_ty = self.infer_pattern(pattern, &mut local_env)?;
                    let pat_ty_for_span = pat_ty.clone();
                    self.unify_with_span(pat_ty, value_ty, pattern_span(pattern))?;
                    let resolved = self.apply(pat_ty_for_span);
                    self.span_types.push((pattern_span(pattern), resolved));
                }
                BlockItem::Let { pattern, expr, .. } => {
                    let expr_ty = self.infer_expr(expr, &mut local_env)?;
                    let pat_ty = self.infer_pattern(pattern, &mut local_env)?;
                    let pat_ty_for_span = pat_ty.clone();
                    self.unify_with_span(pat_ty, expr_ty, pattern_span(pattern))?;
                    let resolved = self.apply(pat_ty_for_span);
                    self.span_types.push((pattern_span(pattern), resolved));
                }
                BlockItem::Expr { expr, .. } => {
                    let expr_ty = self.infer_expr(expr, &mut local_env)?;
                    if idx + 1 == items.len() {
                        // Final expression: must be M A, determines the block's result type
                        result_ty = self.fresh_var();
                        let expected = if is_result_monad {
                            let err_var = self.fresh_var();
                            Type::con(&monad_con).app(vec![err_var, result_ty.clone()])
                        } else {
                            Type::con(&monad_con).app(vec![result_ty.clone()])
                        };
                        self.push_deferred_constraint(expr_ty, expected, expr_span(expr));
                    } else {
                        // Non-final expression: desugars to `chain (λ_. body) expr`, so
                        // expr must be `M A` for any A (the value is discarded, like `_ <- expr`).
                        let discarded = self.fresh_var();
                        let expected = if is_result_monad {
                            let err_var = self.fresh_var();
                            Type::con(&monad_con).app(vec![err_var, discarded])
                        } else {
                            Type::con(&monad_con).app(vec![discarded])
                        };
                        self.push_deferred_constraint(expr_ty, expected, expr_span(expr));
                    }
                }
                // Effect-specific statements are not allowed in generic do blocks
                BlockItem::When { span, .. }
                | BlockItem::Unless { span, .. } => {
                    return Err(TypeError {
                        span: span.clone(),
                        message: format!(
                            "`when`/`unless` is only available in `do Effect {{ ... }}` blocks, not `do {monad_name} {{ ... }}`"
                        ),
                        expected: None,
                        found: None,
                    });
                }
                BlockItem::Given { span, .. } => {
                    return Err(TypeError {
                        span: span.clone(),
                        message: format!(
                            "`given` is only available in `do Effect {{ ... }}` blocks, not `do {monad_name} {{ ... }}`"
                        ),
                        expected: None,
                        found: None,
                    });
                }
                BlockItem::Recurse { span, .. } => {
                    return Err(TypeError {
                        span: span.clone(),
                        message: format!(
                            "`recurse` is only available in `do Effect {{ ... }}` or `generate {{ ... }}` blocks, not `do {monad_name} {{ ... }}`"
                        ),
                        expected: None,
                        found: None,
                    });
                }
                BlockItem::Filter { span, .. } => {
                    return Err(TypeError {
                        span: span.clone(),
                        message: format!(
                            "guards are only available in `generate {{ ... }}` blocks, not `do {monad_name} {{ ... }}`"
                        ),
                        expected: None,
                        found: None,
                    });
                }
                BlockItem::Yield { span, .. } => {
                    return Err(TypeError {
                        span: span.clone(),
                        message: format!(
                            "`yield` is only available in `generate {{ ... }}` blocks, not `do {monad_name} {{ ... }}`"
                        ),
                        expected: None,
                        found: None,
                    });
                }
            }
        }
        self.solve_deferred_constraints()?;

        // Build the block's return type: M result_ty
        let block_ty = if is_result_monad {
            let err_var = self.fresh_var();
            Type::con(&monad_con).app(vec![err_var, result_ty])
        } else {
            Type::con(&monad_con).app(vec![result_ty])
        };
        Ok(block_ty)
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
                    let pat_ty_for_span = pat_ty.clone();
                    self.unify_with_span(pat_ty, bind_elem.clone(), pattern_span(pattern))?;
                    let resolved = self.apply(pat_ty_for_span);
                    self.span_types.push((pattern_span(pattern), resolved));
                    current_elem = Some(bind_elem);
                }
                BlockItem::Let { pattern, expr, .. } => {
                    if matches!(pattern, Pattern::Ident(n) if n.name.starts_with("__")) {
                        self.infer_pattern(pattern, &mut local_env)?;
                    }
                    let expr_ty = self.infer_expr(expr, &mut local_env)?;
                    let pat_ty = self.infer_pattern(pattern, &mut local_env)?;
                    let pat_ty_for_span = pat_ty.clone();
                    self.unify_with_span(pat_ty, expr_ty, pattern_span(pattern))?;
                    let resolved = self.apply(pat_ty_for_span);
                    self.span_types.push((pattern_span(pattern), resolved));
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
            }
        }
        Ok(Type::con("aivi.generator.Generator").app(vec![yield_ty]))
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
                    let pat_ty_for_span = pat_ty.clone();
                    self.unify_with_span(pat_ty, value_ty, pattern_span(pattern))?;
                    let resolved = self.apply(pat_ty_for_span);
                    self.span_types.push((pattern_span(pattern), resolved));
                }
                BlockItem::Let { pattern, expr, .. } => {
                    if matches!(pattern, Pattern::Ident(n) if n.name.starts_with("__")) {
                        self.infer_pattern(pattern, &mut local_env)?;
                    }
                    let expr_ty = self.infer_expr(expr, &mut local_env)?;
                    let pat_ty = self.infer_pattern(pattern, &mut local_env)?;
                    let pat_ty_for_span = pat_ty.clone();
                    self.unify_with_span(pat_ty, expr_ty, pattern_span(pattern))?;
                    let resolved = self.apply(pat_ty_for_span);
                    self.span_types.push((pattern_span(pattern), resolved));
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
                        Type::Record {
                            fields,
                        },
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
                    let unbound = collect_implicit_field_names(expr, env);
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
                        // implicit field accesses on the element (`price > 80`).
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
        if let Some(value_ty) = self.infallible_effect_value_ty(expr_ty.clone()) {
            return Ok(value_ty);
        }
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
        let gen_ty = Type::con("aivi.generator.Generator").app(vec![elem_ty.clone()]);
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
        if let (Type::Var(var), [PathSegment::Field(name)]) = (base_ty.clone(), path) {
            let applied = self.apply(Type::Var(var));
            if let Type::Record { mut fields } = applied {
                if let Some(existing) = fields.get(&name.name) {
                    return Ok(existing.clone());
                }
                let fresh = self.fresh_var();
                fields.insert(name.name.clone(), fresh.clone());
                self.subst.insert(var, Type::Record { fields });
                return Ok(fresh);
            }
        }

        let applied_base = self.apply(base_ty.clone());
        let expanded_base = self.expand_alias(applied_base);
        if let Some(ty) = self.try_resolve_field_path(&expanded_base, path) {
            return Ok(ty);
        }
        if let Some(message) = self.missing_record_field_message(&expanded_base, path) {
            return Err(TypeError {
                span,
                message,
                expected: None,
                found: None,
            });
        }
        let field_ty = self.fresh_var();
        let requirement = self.record_from_path(path, field_ty.clone());
        self.unify_with_span(base_ty, requirement, span)?;
        Ok(field_ty)
    }

    fn try_resolve_field_path(&self, ty: &Type, path: &[PathSegment]) -> Option<Type> {
        let mut current = ty;
        for segment in path {
            match segment {
                PathSegment::Field(name) => {
                    let Type::Record { fields } = current else {
                        return None;
                    };
                    current = fields.get(&name.name)?;
                }
                PathSegment::Index(_, _) | PathSegment::All(_) => return None,
            }
        }
        Some(current.clone())
    }

    fn record_from_path(&mut self, path: &[PathSegment], value: Type) -> Type {
        let mut current = value;
        for segment in path.iter().rev() {
            match segment {
                PathSegment::Field(name) => {
                    let mut fields = BTreeMap::new();
                    fields.insert(name.name.clone(), current);
                    current = Type::Record { fields };
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
            current = Type::Record { fields };
        }
        current
    }

    fn merge_records(&mut self, left: Type, right: Type, span: Span) -> Result<Type, TypeError> {
        let left = self.apply(left);
        let right = self.apply(right);
        let left_clone = left.clone();
        let right_clone = right.clone();
        match (left, right) {
            (Type::Record { mut fields }, Type::Record { fields: other }) => {
                for (name, ty) in other {
                    if let Some(existing) = fields.get(&name).cloned() {
                        self.unify(existing, ty.clone(), span.clone())?;
                    } else {
                        fields.insert(name, ty);
                    }
                }
                Ok(Type::Record { fields })
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

    #[test]
    fn bind_effect_value_accepts_infallible_effect_partial() {
        let mut checker = TypeChecker::new();
        let span = dummy_span();
        let err_ty = checker.fresh_var();
        let partial_effect = Type::con("Effect").app(vec![Type::con("DateTime")]);
        let res = checker
            .bind_effect_value(partial_effect, err_ty, span)
            .expect("expected infallible effect bind to succeed");
        assert_eq!(checker.type_to_string(&res), "DateTime");
    }
}
