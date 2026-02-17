impl TypeChecker {
    fn infer_expr(&mut self, expr: &Expr, env: &mut TypeEnv) -> Result<Type, TypeError> {
        match expr {
            Expr::Ident(name) => self.infer_ident(name, env),
            Expr::Literal(literal) => match literal {
                Literal::Number { text, span } => match number_kind(text) {
                    Some(NumberKind::Float) => Ok(Type::con("Float")),
                    Some(NumberKind::Int) => Ok(Type::con("Int")),
                    None => {
                        let Some((_number, suffix, kind)) = split_suffixed_number(text) else {
                            return Ok(self.fresh_var());
                        };
                        let template_name = format!("1{suffix}");
                        let scheme = env.get(&template_name).cloned().ok_or_else(|| TypeError {
                            span: span.clone(),
                            message: format!(
                                "unknown numeric literal '{text}' (suffix literals require a '{template_name}' template in scope; import the relevant domain with `use ... (domain ...)` or define '{template_name} = ...`)"
                            ),
                            expected: None,
                            found: None,
                        })?;
                        let template_ty = self.instantiate(&scheme);
                        let result_ty = self.fresh_var();
                        let arg_ty = match kind {
                            NumberKind::Int => Type::con("Int"),
                            NumberKind::Float => Type::con("Float"),
                        };
                        self.unify_with_span(
                            template_ty,
                            Type::Func(Box::new(arg_ty), Box::new(result_ty.clone())),
                            span.clone(),
                        )?;
                        Ok(result_ty)
                    }
                },
                _ => Ok(self.literal_type(literal)),
            },
            Expr::UnaryNeg { expr, span } => {
                let inner_ty = self.infer_expr(expr, env)?;

                // Try Int first; if that fails, backtrack and try Float.
                let base_subst = self.subst.clone();
                let int_ty = Type::con("Int");
                if self
                    .unify_with_span(inner_ty.clone(), int_ty.clone(), span.clone())
                    .is_ok()
                {
                    return Ok(int_ty);
                }

                self.subst = base_subst;
                let float_ty = Type::con("Float");
                if self
                    .unify_with_span(inner_ty.clone(), float_ty.clone(), span.clone())
                    .is_ok()
                {
                    return Ok(float_ty);
                }

                let found = self.type_to_string(&self.apply(inner_ty));
                Err(TypeError {
                    span: span.clone(),
                    message: format!("unary '-' expects Int or Float (found {found})"),
                    expected: None,
                    found: None,
                })
            }
            Expr::Suffixed { base, suffix, span } => {
                let arg_ty = self.infer_expr(base, env)?;
                let template_name = format!("1{}", suffix.name);
                let scheme = env.get(&template_name).cloned().ok_or_else(|| TypeError {
                    span: span.clone(),
                    message: format!(
                        "unknown suffix '{}' (suffix literals require a '{template_name}' template in scope; import the relevant domain with `use ... (domain ...)` or define '{template_name} = ...`)",
                        suffix.name
                    ),
                    expected: None,
                    found: None,
                })?;
                let template_ty = self.instantiate(&scheme);
                let result_ty = self.fresh_var();
                self.unify_with_span(
                    template_ty,
                    Type::Func(Box::new(arg_ty), Box::new(result_ty.clone())),
                    span.clone(),
                )?;
                Ok(result_ty)
            }
            Expr::TextInterpolate { parts, .. } => {
                for part in parts {
                    if let TextPart::Expr { expr, .. } = part {
                        let _ = self.infer_expr(expr, env)?;
                    }
                }
                Ok(Type::con("Text"))
            }
            Expr::List { items, .. } => self.infer_list(items, env),
            Expr::Tuple { items, .. } => self.infer_tuple(items, env),
            Expr::Record { fields, .. } => self.infer_record(fields, env),
            Expr::PatchLit { fields, .. } => self.infer_patch_literal(fields, env),
            Expr::FieldAccess { base, field, .. } => self.infer_field_access(base, field, env),
            Expr::FieldSection { field, .. } => {
                let param = SpannedName {
                    name: "_arg0".to_string(),
                    span: field.span.clone(),
                };
                let body = Expr::FieldAccess {
                    base: Box::new(Expr::Ident(param.clone())),
                    field: field.clone(),
                    span: field.span.clone(),
                };
                let lambda = Expr::Lambda {
                    params: vec![Pattern::Ident(param)],
                    body: Box::new(body),
                    span: field.span.clone(),
                };
                self.infer_expr(&lambda, env)
            }
            Expr::Index { base, index, .. } => self.infer_index(base, index, env),
            Expr::Call { func, args, .. } => self.infer_call(func, args, env),
            Expr::Lambda { params, body, .. } => self.infer_lambda(params, body, env),
            Expr::Match {
                scrutinee,
                arms,
                span,
                ..
            } => self.infer_match(scrutinee, arms, span, env),
            Expr::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => self.infer_if(cond, then_branch, else_branch, env),
            Expr::Binary {
                op, left, right, ..
            } => self.infer_binary(op, left, right, env),
            Expr::Block { kind, items, .. } => self.infer_block(kind, items, env),
            Expr::Raw { .. } => Ok(self.fresh_var()),
        }
    }

    pub(super) fn elaborate_def_expr(
        &mut self,
        def: &mut Def,
        sigs: &HashMap<String, Vec<Scheme>>,
        env: &TypeEnv,
    ) -> Result<(), TypeError> {
        let base_subst = self.subst.clone();
        let result = (|| {
            let name = def.name.name.clone();
            let expr = crate::surface::desugar_effect_sugars(desugar_holes(def.expr.clone()));

            let mut local_env = env.clone();
            // Ensure self-recursion sees the expected scheme when available.
            if let Some(sig) = sigs.get(&name).and_then(|items| (items.len() == 1).then(|| &items[0]))
            {
                let expected = self.instantiate(sig);
                local_env.insert(name.clone(), Scheme::mono(expected));
            }

            // Bind parameters in the local env.
            for pattern in &def.params {
                let _ = self.infer_pattern(pattern, &mut local_env)?;
            }

            // If a signature exists, propagate the expected result type into the body.
            let expected_body = sigs.get(&name).map(|sig| {
                let Some(sig) = (sig.len() == 1).then(|| &sig[0]) else {
                    return self.fresh_var();
                };
                let mut expected = self.instantiate(sig);
                for _ in &def.params {
                    let applied = self.apply(expected);
                    expected = match self.expand_alias(applied) {
                        Type::Func(_, rest) => *rest,
                        other => other,
                    };
                }
                expected
            });

            let (elab, _ty) = self.elab_expr(expr, expected_body, &mut local_env)?;
            def.expr = elab;
            Ok(())
        })();
        self.subst = base_subst;
        result
    }

    fn elab_expr(
        &mut self,
        expr: Expr,
        expected: Option<Type>,
        env: &mut TypeEnv,
    ) -> Result<(Expr, Type), TypeError> {
        match expr {
            Expr::Call { func, args, span } => self.elab_call(*func, args, span, expected, env),
            Expr::UnaryNeg { expr, span } => {
                let (inner, inner_ty) = self.elab_expr(*expr, None, env)?;

                // Choose Int vs Float using the same backtracking strategy as inference.
                let base_subst = self.subst.clone();
                let mut zero_text = "0".to_string();
                let int_ty = Type::con("Int");
                let float_ty = Type::con("Float");
                let chosen = if self
                    .unify_with_span(inner_ty.clone(), int_ty.clone(), span.clone())
                    .is_ok()
                {
                    int_ty
                } else {
                    self.subst = base_subst;
                    zero_text = "0.0".to_string();
                    self.unify_with_span(inner_ty.clone(), float_ty.clone(), span.clone())?;
                    float_ty
                };

                let zero = Expr::Literal(Literal::Number {
                    text: zero_text,
                    span: span.clone(),
                });
                let out = Expr::Binary {
                    op: "-".to_string(),
                    left: Box::new(zero),
                    right: Box::new(inner),
                    span,
                };
                self.check_or_coerce(out, expected.or(Some(chosen)), env)
            }
            Expr::Suffixed { base, suffix, span } => {
                let (base, _base_ty) = self.elab_expr(*base, None, env)?;
                let out = Expr::Suffixed {
                    base: Box::new(base),
                    suffix,
                    span,
                };
                self.check_or_coerce(out, expected, env)
            }
            Expr::Record { fields, span } => self.elab_record(fields, span, expected, env),
            Expr::If {
                cond,
                then_branch,
                else_branch,
                span,
            } => {
                let (cond, _cond_ty) = self.elab_expr(*cond, None, env)?;
                let (then_branch, _then_ty) =
                    self.elab_expr(*then_branch, expected.clone(), env)?;
                let (else_branch, _else_ty) =
                    self.elab_expr(*else_branch, expected.clone(), env)?;
                let out = Expr::If {
                    cond: Box::new(cond),
                    then_branch: Box::new(then_branch),
                    else_branch: Box::new(else_branch),
                    span,
                };
                self.check_or_coerce(out, expected, env)
            }
            Expr::List { items, span } => {
                let expected_elem = expected.as_ref().and_then(|ty| {
                    let applied = self.apply(ty.clone());
                    let expanded = self.expand_alias(applied);
                    match expanded {
                        Type::Con(ref name, ref args) if name == "List" && args.len() == 1 => {
                            Some(args[0].clone())
                        }
                        _ => None,
                    }
                });
                let mut new_items = Vec::new();
                for item in items {
                    let item_expected = if item.spread {
                        None
                    } else {
                        expected_elem.clone()
                    };
                    let (expr, _ty) = self.elab_expr(item.expr, item_expected, env)?;
                    new_items.push(ListItem {
                        expr,
                        spread: item.spread,
                        span: item.span,
                    });
                }
                let out = Expr::List {
                    items: new_items,
                    span,
                };
                self.check_or_coerce(out, expected, env)
            }
            Expr::Tuple { items, span } => {
                let mut new_items = Vec::new();
                for item in items {
                    let (expr, _ty) = self.elab_expr(item, None, env)?;
                    new_items.push(expr);
                }
                let out = Expr::Tuple {
                    items: new_items,
                    span,
                };
                self.check_or_coerce(out, expected, env)
            }
            Expr::Lambda { params, body, span } => {
                // Bind lambda parameters before elaborating the body so references resolve during
                // expected-coercion elaboration.
                let mut lambda_env = env.clone();
                for pattern in &params {
                    let _ = self.infer_pattern(pattern, &mut lambda_env)?;
                }

                // For now, only elaborate the body with no expected type. Expected-type coercions
                // are primarily needed at call sites (arguments/fields), not for lambda bodies.
                let (body, _ty) = self.elab_expr(*body, None, &mut lambda_env)?;
                let out = Expr::Lambda {
                    params,
                    body: Box::new(body),
                    span,
                };
                self.check_or_coerce(out, expected, env)
            }
            Expr::Match {
                scrutinee,
                arms,
                span,
            } => {
                let scrutinee = if let Some(scrutinee) = scrutinee {
                    let (scrutinee, _ty) = self.elab_expr(*scrutinee, None, env)?;
                    Some(Box::new(scrutinee))
                } else {
                    None
                };

                // Constrain arm-local bindings before elaborating arm bodies.
                //
                // Expected-type elaboration inside an arm body (e.g. operator overload selection)
                // needs the types induced by the scrutinee/pattern unification. Without this,
                // pattern-bound names start as unconstrained type variables and can trigger
                // spurious "ambiguous domain operator" errors.
                let scrutinee_ty = if let Some(scrutinee) = &scrutinee {
                    self.infer_expr(scrutinee, env)?
                } else {
                    self.fresh_var()
                };

                let mut new_arms = Vec::new();
                for arm in arms {
                    let mut arm_env = env.clone();
                    let pat_ty = self.infer_pattern(&arm.pattern, &mut arm_env)?;
                    self.unify_with_span(pat_ty, scrutinee_ty.clone(), arm.span.clone())?;
                    let guard = if let Some(guard) = arm.guard {
                        let (guard, _ty) = self.elab_expr(guard, None, &mut arm_env)?;
                        Some(guard)
                    } else {
                        None
                    };
                    let (body, _ty) = self.elab_expr(arm.body, expected.clone(), &mut arm_env)?;
                    new_arms.push(crate::surface::MatchArm {
                        pattern: arm.pattern,
                        guard,
                        body,
                        span: arm.span,
                    });
                }
                let out = Expr::Match {
                    scrutinee,
                    arms: new_arms,
                    span,
                };
                self.check_or_coerce(out, expected, env)
            }
            Expr::Block { kind, items, span } => {
                let mut local_env = env.clone();
                let mut new_items = Vec::new();
                for item in items {
                    match item {
                        BlockItem::Expr { expr, span } => {
                            let (expr, _ty) = self.elab_expr(expr, None, &mut local_env)?;
                            new_items.push(BlockItem::Expr { expr, span });
                        }
                        BlockItem::Let {
                            pattern,
                            expr,
                            span,
                        } => {
                            let (expr, expr_ty) = self.elab_expr(expr, None, &mut local_env)?;
                            let pat_ty = self.infer_pattern(&pattern, &mut local_env)?;
                            if matches!(kind, BlockKind::Plain) {
                                self.unify_with_span(pat_ty, expr_ty, pattern_span(&pattern))?;
                            }
                            new_items.push(BlockItem::Let {
                                pattern,
                                expr,
                                span,
                            });
                        }
                        BlockItem::Bind {
                            pattern,
                            expr,
                            span,
                        } => {
                            let (expr, expr_ty) = self.elab_expr(expr, None, &mut local_env)?;
                            let pat_ty = self.infer_pattern(&pattern, &mut local_env)?;
                            if matches!(kind, BlockKind::Plain) {
                                self.unify_with_span(pat_ty, expr_ty, pattern_span(&pattern))?;
                            }
                            new_items.push(BlockItem::Bind {
                                pattern,
                                expr,
                                span,
                            });
                        }
                        BlockItem::Filter { expr, span } => {
                            let (expr, _ty) = self.elab_expr(expr, None, &mut local_env)?;
                            new_items.push(BlockItem::Filter { expr, span });
                        }
                        BlockItem::Yield { expr, span } => {
                            let (expr, _ty) = self.elab_expr(expr, None, &mut local_env)?;
                            new_items.push(BlockItem::Yield { expr, span });
                        }
                        BlockItem::Recurse { expr, span } => {
                            let (expr, _ty) = self.elab_expr(expr, None, &mut local_env)?;
                            new_items.push(BlockItem::Recurse { expr, span });
                        }
                    }
                }
                let out = Expr::Block {
                    kind,
                    items: new_items,
                    span,
                };
                self.check_or_coerce(out, expected, env)
            }
            other => self.check_or_coerce(other, expected, env),
        }
    }

    fn elab_call(
        &mut self,
        func: Expr,
        args: Vec<Expr>,
        span: Span,
        expected: Option<Type>,
        env: &mut TypeEnv,
    ) -> Result<(Expr, Type), TypeError> {
        // Method calls are inferred via a dedicated path; skip expected-type propagation.
        if let Expr::Ident(name) = &func {
            if env.get(&name.name).is_none() && self.method_to_classes.contains_key(&name.name) {
                let mut new_args = Vec::new();
                for arg in args {
                    let (arg, _ty) = self.elab_expr(arg, None, env)?;
                    new_args.push(arg);
                }
                let out = Expr::Call {
                    func: Box::new(func),
                    args: new_args,
                    span: span.clone(),
                };
                return self.check_or_coerce(out, expected, env);
            }
        }

        let (func, _func_ty) = self.elab_expr(func, None, env)?;
        let func_ty = self.infer_expr(&func, env)?;

        let mut param_tys = Vec::new();
        for _ in 0..args.len() {
            param_tys.push(self.fresh_var());
        }
        let result_ty = expected.clone().unwrap_or_else(|| self.fresh_var());

        let mut expected_func_ty = result_ty.clone();
        for param in param_tys.iter().rev() {
            expected_func_ty = Type::Func(Box::new(param.clone()), Box::new(expected_func_ty));
        }
        self.unify_with_span(func_ty, expected_func_ty, span.clone())?;

        let mut new_args = Vec::new();
        for (arg, expected_arg_ty) in args.into_iter().zip(param_tys.into_iter()) {
            let expected_arg_ty = self.apply(expected_arg_ty);
            let (arg, _ty) = self.elab_expr(arg, Some(expected_arg_ty), env)?;
            new_args.push(arg);
        }
        let out = Expr::Call {
            func: Box::new(func),
            args: new_args,
            span,
        };
        Ok((out, self.apply(result_ty)))
    }

    fn elab_record(
        &mut self,
        fields: Vec<RecordField>,
        span: Span,
        expected: Option<Type>,
        env: &mut TypeEnv,
    ) -> Result<(Expr, Type), TypeError> {
        let expected_ty = if let Some(ty) = expected.as_ref() {
            let applied = self.apply(ty.clone());
            Some(self.expand_alias(applied))
        } else {
            None
        };

        let mut new_fields = Vec::new();
        for field in fields {
            let value_expected = if field.spread {
                None
            } else if let Some(base) = expected_ty.clone() {
                self.record_field_type(base, &field.path, field.span.clone())
                    .ok()
            } else {
                None
            };
            let (value, _ty) = self.elab_expr(field.value, value_expected, env)?;
            new_fields.push(RecordField {
                path: field.path,
                value,
                spread: field.spread,
                span: field.span,
            });
        }
        let out = Expr::Record {
            fields: new_fields,
            span,
        };
        self.check_or_coerce(out, expected, env)
    }

    fn check_or_coerce(
        &mut self,
        expr: Expr,
        expected: Option<Type>,
        env: &mut TypeEnv,
    ) -> Result<(Expr, Type), TypeError> {
        if let (Some(expected_ty), Expr::Ident(name)) = (expected.clone(), &expr) {
            if let Some(candidates) = env.get_all(&name.name) {
                if candidates.len() > 1 {
                    let base_subst = self.subst.clone();
                    let mut selected: Option<std::collections::HashMap<TypeVarId, Type>> = None;
                    for scheme in candidates {
                        self.subst = base_subst.clone();
                        let candidate_ty = self.instantiate(scheme);
                        if self
                            .unify_with_span(candidate_ty, expected_ty.clone(), expr_span(&expr))
                            .is_ok()
                        {
                            if selected.is_some() {
                                self.subst = base_subst;
                                return Err(TypeError {
                                    span: expr_span(&expr),
                                    message: format!(
                                        "ambiguous name '{}' (multiple overloads match)",
                                        name.name
                                    ),
                                    expected: None,
                                    found: None,
                                });
                            }
                            selected = Some(self.subst.clone());
                        }
                    }
                    if let Some(subst) = selected {
                        self.subst = subst;
                        return Ok((expr, self.apply(expected_ty)));
                    }
                    self.subst = base_subst;
                }
            }
        }

        let inferred = self.infer_expr(&expr, env)?;
        let Some(expected) = expected else {
            return Ok((expr, inferred));
        };

        let expected_applied = {
            let applied = self.apply(expected.clone());
            self.expand_alias(applied)
        };
        let base_subst = self.subst.clone();
        if self
            .unify_with_span(inferred.clone(), expected.clone(), expr_span(&expr))
            .is_ok()
        {
            return Ok((expr, self.apply(expected)));
        }

        // Reset any constraints added by the failed unify attempt before trying a coercion.
        self.subst = base_subst.clone();

        let is_text = matches!(
            expected_applied,
            Type::Con(ref name, ref args) if name == "Text" && args.is_empty()
        );
        if is_text {
            // Try inserting a `toText` call (resolved via the `ToText` class environment).
            let to_text = Expr::Ident(SpannedName {
                name: "toText".to_string(),
                span: expr_span(&expr),
            });
            let call_expr = Expr::Call {
                func: Box::new(to_text),
                args: vec![expr.clone()],
                span: expr_span(&expr),
            };
            let call_ty = self.infer_expr(&call_expr, env)?;
            let base_subst2 = self.subst.clone();
            if self
                .unify_with_span(call_ty, expected.clone(), expr_span(&call_expr))
                .is_ok()
            {
                return Ok((call_expr, self.apply(expected)));
            }
            self.subst = base_subst2;
        }

        let is_vnode = matches!(
            expected_applied,
            Type::Con(ref name, ref args) if name == "VNode" && args.len() == 1
        );
        if is_vnode {
            // Coerce into a `VNode` via `TextNode`, either directly from `Text`
            // or via `toText` when available.
            let text_node = Expr::Ident(SpannedName {
                name: "TextNode".to_string(),
                span: expr_span(&expr),
            });

            // First try `TextNode <expr>` if `<expr>` already is `Text`.
            let call_expr = Expr::Call {
                func: Box::new(text_node.clone()),
                args: vec![expr.clone()],
                span: expr_span(&expr),
            };
            let call_ty = self.infer_expr(&call_expr, env)?;
            let base_subst2 = self.subst.clone();
            if self
                .unify_with_span(call_ty, expected.clone(), expr_span(&call_expr))
                .is_ok()
            {
                return Ok((call_expr, self.apply(expected)));
            }
            self.subst = base_subst2;

            // Then try `TextNode (toText <expr>)`.
            let to_text = Expr::Ident(SpannedName {
                name: "toText".to_string(),
                span: expr_span(&expr),
            });
            let to_text_call = Expr::Call {
                func: Box::new(to_text),
                args: vec![expr.clone()],
                span: expr_span(&expr),
            };
            let call_expr = Expr::Call {
                func: Box::new(text_node),
                args: vec![to_text_call],
                span: expr_span(&expr),
            };
            let call_ty = self.infer_expr(&call_expr, env)?;
            let base_subst3 = self.subst.clone();
            if self
                .unify_with_span(call_ty, expected.clone(), expr_span(&call_expr))
                .is_ok()
            {
                return Ok((call_expr, self.apply(expected)));
            }
            self.subst = base_subst3;
        }

        // Fall back to the original mismatch (without keeping any partial unifications).
        self.subst = base_subst;
        self.unify_with_span(inferred, expected.clone(), expr_span(&expr))?;
        Ok((expr, self.apply(expected)))
    }
}
