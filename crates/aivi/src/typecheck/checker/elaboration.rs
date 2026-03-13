impl TypeChecker {
    fn infer_expr(&mut self, expr: &Expr, env: &mut TypeEnv) -> Result<Type, TypeError> {
        let result = match expr {
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
                        if env.get_all(&template_name).is_some_and(|s| s.len() > 1) {
                            return Err(TypeError {
                                span: span.clone(),
                                message: format!(
                                    "ambiguous suffix literal '{text}': multiple domains define '{template_name}' — use a qualified form or import only one domain"
                                ),
                                expected: None,
                                found: None,
                            });
                        }
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

                let applied_inner = self.apply(inner_ty);
                let found = self.type_to_string(&applied_inner);
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
                if env.get_all(&template_name).is_some_and(|s| s.len() > 1) {
                    return Err(TypeError {
                        span: span.clone(),
                        message: format!(
                            "ambiguous suffix '{}': multiple domains define '{template_name}' — use a qualified form or import only one domain",
                            suffix.name
                        ),
                        expected: None,
                        found: None,
                    });
                }
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
            Expr::FieldAccess { base, field, span } => {
                self.infer_field_access(base, field, span, env)
            }
            Expr::FieldSection { field, .. } => {
                let param = SpannedName {
                    name: "_arg0".into(),
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
            Expr::Mock { body, .. } => self.infer_expr(body, env),
        };
        if let Ok(ref ty) = result {
            let resolved = self.apply(ty.clone());
            self.span_types.push((expr_span(expr), resolved));
        }
        result
    }

    pub(super) fn elaborate_def_expr(
        &mut self,
        def: &mut Def,
        sigs: &HashMap<String, Vec<Scheme>>,
        env: &TypeEnv,
    ) -> Result<(), TypeError> {
        // @native defs have auto-generated bodies; skip elaboration to avoid false errors.
        if def.decorators.iter().any(|d| d.name.name == "native") {
            return Ok(());
        }
        let debug_elab = std::env::var("AIVI_DEBUG_ELAB").is_ok_and(|v| v == "1");
        if debug_elab {
            eprintln!("[ELAB_DEBUG] elaborate_def_expr: {}", def.name.name);
        }
        let base_subst = self.subst.clone();
        let result = (|| {
            let name = def.name.name.clone();
            let expr = desugar_holes(def.expr.clone());

            let mut local_env = env.clone();
            // Ensure self-recursion sees the expected scheme when available.
            if let Some(sig) = sigs
                .get(&name)
                .and_then(|items| (items.len() == 1).then(|| &items[0]))
            {
                let expected = self.instantiate(sig);
                local_env.insert(name.clone(), Scheme::mono(expected));
            }

            // Bind parameters in the local env, constraining them from the
            // type signature when available so that class method dispatch in the
            // body sees concrete container types (e.g. `List Text` instead of a
            // fresh type variable).
            let sig_scheme = sigs
                .get(&name)
                .and_then(|items| (items.len() == 1).then(|| &items[0]));
            let mut remaining_sig_ty = sig_scheme.map(|s| self.instantiate(s));

            for pattern in &def.params {
                let param_ty = self.infer_pattern(pattern, &mut local_env)?;
                let param_ty_for_span = param_ty.clone();
                if let Some(sig_ty) = remaining_sig_ty.take() {
                    let applied = self.apply(sig_ty);
                    match self.expand_alias(applied) {
                        Type::Func(expected_param, rest) => {
                            if self
                                .unify_with_span(param_ty, *expected_param, def.name.span.clone())
                                .is_ok()
                            {
                                remaining_sig_ty = Some(*rest);
                            } else {
                                remaining_sig_ty = None;
                            }
                        }
                        _ => {
                            // Signature/parameter mismatch: stop propagating
                            // signature-driven expectations during elaboration.
                            remaining_sig_ty = None;
                        }
                    }
                }
                let resolved = self.apply(param_ty_for_span);
                self.span_types
                    .push((pattern_span(pattern), resolved));
            }

            let expected_body = remaining_sig_ty;

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
            Expr::TextInterpolate { parts, span } => {
                // Each splice `{ expr }` behaves like an expected-`Text` position.
                // Elaborate splice expressions against `Text` so expected-type coercions
                // (notably `toText`) can be inserted.
                let mut new_parts = Vec::with_capacity(parts.len());
                for part in parts {
                    match part {
                        TextPart::Text { .. } => new_parts.push(part),
                        TextPart::Expr {
                            expr,
                            span: part_span,
                        } => {
                            let (expr, _ty) =
                                self.elab_expr(*expr, Some(Type::con("Text")), env)?;
                            new_parts.push(TextPart::Expr {
                                expr: Box::new(expr),
                                span: part_span,
                            });
                        }
                    }
                }
                let out = Expr::TextInterpolate {
                    parts: new_parts,
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
                // Bind lambda parameters, constraining them from the expected
                // type when available so that class method dispatch in the body
                // sees concrete types (e.g. `List Text` rather than a fresh var).
                let mut lambda_env = env.clone();
                let mut remaining_expected = expected.clone();
                let mut param_tys = Vec::new();
                for pattern in &params {
                    let param_ty = self.infer_pattern(pattern, &mut lambda_env)?;
                    let param_ty_for_span = param_ty.clone();
                    if let Some(exp_ty) = remaining_expected.take() {
                        let applied = self.apply(exp_ty);
                        match self.expand_alias(applied) {
                            Type::Func(expected_param, rest) => {
                                let expected_param_ty = *expected_param;
                                if self
                                    .unify_with_span(
                                        param_ty.clone(),
                                        expected_param_ty,
                                        span.clone(),
                                    )
                                    .is_ok()
                                {
                                    remaining_expected = Some(*rest);
                                } else {
                                    remaining_expected = None;
                                }
                            }
                            _ => {
                                // Mismatch with the expected lambda type:
                                // keep elaborating, but do not propagate a
                                // potentially wrong expected body type.
                                remaining_expected = None;
                            }
                        }
                    }
                    let resolved = self.apply(param_ty_for_span);
                    self.span_types
                        .push((pattern_span(pattern), resolved));
                    param_tys.push(param_ty);
                }

                let (body, body_ty) =
                    self.elab_expr(*body, remaining_expected, &mut lambda_env)?;
                let out = Expr::Lambda {
                    params,
                    body: Box::new(body),
                    span,
                };

                // Build the lambda type from the (now-constrained) param types
                // and the elaborated body type. This avoids re-inferring from
                // scratch via check_or_coerce which would lose the param types.
                let mut lambda_ty = body_ty;
                for param_ty in param_tys.into_iter().rev() {
                    lambda_ty =
                        Type::Func(Box::new(self.apply(param_ty)), Box::new(lambda_ty));
                }
                if let Some(expected_ty) = expected {
                    let _ = self.unify_with_span(
                        lambda_ty.clone(),
                        expected_ty,
                        expr_span(&out),
                    );
                }
                Ok((out, self.apply(lambda_ty)))
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

                // For multi-clause function sugar (no scrutinee), the `expected` type is the full
                // function type `A -> B`. Each arm body should be checked against the RETURN type
                // `B`, not the full function type. Passing the full function type to arm bodies
                // causes spurious type errors (e.g. "expected A -> B, found B").
                let arm_body_expected = if scrutinee.is_none() {
                    expected.as_ref().and_then(|ty| {
                        let applied = self.apply(ty.clone());
                        let expanded = self.expand_alias(applied);
                        if let Type::Func(_, ret) = expanded {
                            Some(*ret)
                        } else {
                            None
                        }
                    })
                } else {
                    expected.clone()
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
                    let (body, _ty) =
                        self.elab_expr(arm.body, arm_body_expected.clone(), &mut arm_env)?;
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
            Expr::Binary {
                op,
                left,
                right,
                span,
            } if op == "->>" || op == "<<-" => {
                self.elab_binary_expr(op, *left, *right, span, expected, env)
            }
            Expr::Block { kind, items, span } => {
                // For generic do-monad blocks (do Result, do Option, etc.), skip
                // item-by-item elaboration and defer to infer_generic_do_block
                // via check_or_coerce. The elaboration's bind processing uses
                // shared substitution state that conflicts with the inference pass.
                if matches!(
                    &kind,
                    BlockKind::Do { monad }
                        if monad.name != "Effect" && monad.name != "Event" && monad.name != "Query"
                ) {
                    let out = Expr::Block { kind, items, span };
                    return self.check_or_coerce(out, expected, env);
                }
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
                            // Compiler-generated let bindings (e.g. __loop from
                            // loop/recurse desugaring) may be self-referential.
                            // Pre-add them to scope so the recursive reference
                            // inside the lambda body can be elaborated.
                            if matches!(&pattern, Pattern::Ident(n) if n.name.starts_with("__")) {
                                self.infer_pattern(&pattern, &mut local_env)?;
                            }
                            let (expr, expr_ty) = self.elab_expr(expr, None, &mut local_env)?;
                            let pat_ty = self.infer_pattern(&pattern, &mut local_env)?;
                            // Always unify Let pattern with expression type so that
                            // later items have accurate constraints during elaboration.
                            // (Let bindings are pure in all block kinds.)
                            self.unify_with_span(pat_ty, expr_ty, pattern_span(&pattern))?;
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
                            } else if matches!(kind, BlockKind::Generate | BlockKind::Resource) {
                                // For generate/resource blocks, try to extract the element type
                                // from List or Generator. Plain unify on failure.
                                let backup = self.subst.clone();
                                let elem_ty = self.fresh_var();
                                let list_ty = Type::con("List").app(vec![elem_ty.clone()]);
                                if self
                                    .unify_with_span(expr_ty.clone(), list_ty, span.clone())
                                    .is_ok()
                                {
                                    let _ = self.unify_with_span(
                                        pat_ty,
                                        elem_ty,
                                        pattern_span(&pattern),
                                    );
                                } else {
                                    self.subst = backup.clone();
                                    let gen_ty =
                                        Type::con("Generator").app(vec![elem_ty.clone()]);
                                    if self
                                        .unify_with_span(expr_ty.clone(), gen_ty, span.clone())
                                        .is_ok()
                                    {
                                        let _ = self.unify_with_span(
                                            pat_ty,
                                            elem_ty,
                                            pattern_span(&pattern),
                                        );
                                    } else {
                                        self.subst = backup;
                                    }
                                }
                            } else {
                                // For Effect do-blocks, try to extract the value type from the
                                // Effect wrapper and unify with the pattern. This propagates
                                // constraints during elaboration so that later items (e.g.
                                // operator overload selection) see accurate types.
                                let backup = self.subst.clone();
                                let value_ty = self.fresh_var();
                                let eff_err_ty = self.fresh_var();
                                let effect_ty =
                                    Type::con("Effect").app(vec![eff_err_ty, value_ty.clone()]);
                                if self
                                    .unify_with_span(expr_ty.clone(), effect_ty, span.clone())
                                    .is_ok()
                                {
                                    let _ = self.unify_with_span(
                                        pat_ty,
                                        value_ty,
                                        pattern_span(&pattern),
                                    );
                                } else {
                                    self.subst = backup;
                                }
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
                        BlockItem::When { cond, effect, span } => {
                            let (cond, _) = self.elab_expr(cond, None, &mut local_env)?;
                            let (effect, _) = self.elab_expr(effect, None, &mut local_env)?;
                            new_items.push(BlockItem::When { cond, effect, span });
                        }
                        BlockItem::Unless { cond, effect, span } => {
                            let (cond, _) = self.elab_expr(cond, None, &mut local_env)?;
                            let (effect, _) = self.elab_expr(effect, None, &mut local_env)?;
                            new_items.push(BlockItem::Unless { cond, effect, span });
                        }
                        BlockItem::Given {
                            cond,
                            fail_expr,
                            span,
                        } => {
                            let (cond, _) = self.elab_expr(cond, None, &mut local_env)?;
                            let (fail_expr, _) = self.elab_expr(fail_expr, None, &mut local_env)?;
                            new_items.push(BlockItem::Given {
                                cond,
                                fail_expr,
                                span,
                            });
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
            Expr::FieldAccess { base, field, span } => {
                let base = *base;
                let checkpoint = self.subst.clone();
                match self.infer_plain_field_access(&base, &field, env) {
                    Ok(_) => {
                        self.subst = checkpoint;
                        let (base, _base_ty) = self.elab_expr(base, None, env)?;
                        let out = Expr::FieldAccess {
                            base: Box::new(base),
                            field,
                            span,
                        };
                        self.check_or_coerce(out, expected, env)
                    }
                    Err(record_err) => {
                        self.subst = checkpoint.clone();
                        let rewritten = self.callable_field_access_expr(base, field, span);
                        match self.elab_expr(rewritten, expected, env) {
                            Ok(result) => Ok(result),
                            Err(_) => {
                                self.subst = checkpoint;
                                Err(record_err)
                            }
                        }
                    }
                }
            }
            other => self.check_or_coerce(other, expected, env),
        }
    }

    fn elab_binary_expr(
        &mut self,
        op: String,
        left: Expr,
        right: Expr,
        span: Span,
        expected: Option<Type>,
        env: &mut TypeEnv,
    ) -> Result<(Expr, Type), TypeError> {
        let right = if matches!(op.as_str(), "|>" | "->>") {
            self.normalize_pipe_transformer(&right, env)
        } else {
            right
        };
        if op == "|>" {
            let (left, _left_ty) = self.elab_expr(left, None, env)?;
            let (right, _right_ty) = self.elab_expr(right, None, env)?;
            let out = Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
                span,
            };
            return self.check_or_coerce(out, expected, env);
        }
        let left_ty = self.infer_expr(&left, env)?;
        if let Some(signal_item_ty) = self.extract_signal_item_type(left_ty) {
            match op.as_str() {
                "->>" => return self.elab_signal_pipe(left, right, span, signal_item_ty, expected, env),
                "<<-" => return self.elab_signal_write(left, right, span, signal_item_ty, expected, env),
                _ => {}
            }
        }

        let out = Expr::Binary {
            op,
            left: Box::new(left),
            right: Box::new(right),
            span,
        };
        self.check_or_coerce(out, expected, env)
    }

    fn elab_signal_pipe(
        &mut self,
        left: Expr,
        right: Expr,
        span: Span,
        signal_item_ty: Type,
        expected: Option<Type>,
        env: &mut TypeEnv,
    ) -> Result<(Expr, Type), TypeError> {
        let (left, _left_ty) = self.elab_expr(left, None, env)?;

        let param = SpannedName {
            name: "__signalValue".to_string(),
            span: span.clone(),
        };
        let piped_body = Expr::Binary {
            op: "|>".to_string(),
            left: Box::new(Expr::Ident(param.clone())),
            right: Box::new(right),
            span: span.clone(),
        };
        let mut body_env = env.clone();
        body_env.insert(param.name.clone(), Scheme::mono(signal_item_ty));
        let (body, _body_ty) = self.elab_expr(piped_body, None, &mut body_env)?;
        let lambda = Expr::Lambda {
            params: vec![Pattern::Ident(param)],
            body: Box::new(body),
            span: span.clone(),
        };

        let call = self.reactive_call_expr("derive", vec![left, lambda], &span);
        self.check_or_coerce(call, expected, env)
    }

    fn elab_signal_write(
        &mut self,
        left: Expr,
        right: Expr,
        span: Span,
        signal_item_ty: Type,
        expected: Option<Type>,
        env: &mut TypeEnv,
    ) -> Result<(Expr, Type), TypeError> {
        let (left, _left_ty) = self.elab_expr(left, None, env)?;

        let call = match right {
            Expr::Record { .. } | Expr::PatchLit { .. } => {
                let param = SpannedName {
                    name: "__signalState".to_string(),
                    span: span.clone(),
                };
                let patch_body = Expr::Binary {
                    op: "<|".to_string(),
                    left: Box::new(Expr::Ident(param.clone())),
                    right: Box::new(right),
                    span: span.clone(),
                };
                let mut body_env = env.clone();
                body_env.insert(param.name.clone(), Scheme::mono(signal_item_ty.clone()));
                let (body, _body_ty) =
                    self.elab_expr(patch_body, Some(signal_item_ty.clone()), &mut body_env)?;
                let lambda = Expr::Lambda {
                    params: vec![Pattern::Ident(param)],
                    body: Box::new(body),
                    span: span.clone(),
                };
                self.reactive_call_expr("update", vec![left, lambda], &span)
            }
            right => match self.infer_signal_write_kind(&right, &signal_item_ty, env)? {
                SignalWriteKind::Set => {
                    let (right, _right_ty) =
                        self.elab_expr(right, Some(signal_item_ty.clone()), env)?;
                    self.reactive_call_expr("set", vec![left, right], &span)
                }
                SignalWriteKind::Update => {
                    let updater_ty = Type::Func(
                        Box::new(signal_item_ty.clone()),
                        Box::new(signal_item_ty),
                    );
                    let (right, _right_ty) = self.elab_expr(right, Some(updater_ty), env)?;
                    self.reactive_call_expr("update", vec![left, right], &span)
                }
            },
        };

        self.check_or_coerce(call, expected, env)
    }

    fn reactive_call_expr(&self, field: &str, args: Vec<Expr>, span: &Span) -> Expr {
        let reactive = Expr::Ident(SpannedName {
            name: "reactive".to_string(),
            span: span.clone(),
        });
        let func = Expr::FieldAccess {
            base: Box::new(reactive),
            field: SpannedName {
                name: field.to_string(),
                span: span.clone(),
            },
            span: span.clone(),
        };
        Expr::Call {
            func: Box::new(func),
            args,
            span: span.clone(),
        }
    }

}

include!("elaboration/coerce.rs");
