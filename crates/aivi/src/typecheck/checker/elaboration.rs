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
            self.span_types.push((expr_span(expr), ty.clone()));
        }
        result
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
            if let Some(sig) = sigs
                .get(&name)
                .and_then(|items| (items.len() == 1).then(|| &items[0]))
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
            Expr::Block { kind, items, span } => {
                // For generic do-monad blocks (do Result, do Option, etc.), skip
                // item-by-item elaboration and defer to infer_generic_do_block
                // via check_or_coerce. The elaboration's bind processing uses
                // shared substitution state that conflicts with the inference pass.
                if matches!(&kind, BlockKind::Do { monad } if monad.name != "Effect") {
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
                        BlockItem::On {
                            transition,
                            handler,
                            span,
                        } => {
                            let (transition, _) =
                                self.elab_expr(transition, None, &mut local_env)?;
                            let (handler, _) = self.elab_expr(handler, None, &mut local_env)?;
                            new_items.push(BlockItem::On {
                                transition,
                                handler,
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
                    let (arg, _ty) = match self.elab_expr(arg.clone(), None, env) {
                        Ok(value) => value,
                        Err(err) if err.message.starts_with("unknown name '") => {
                            let Some(rewritten) = lift_predicate_expr(&arg, env, "__pred") else {
                                return Err(err);
                            };
                            self.elab_expr(rewritten, None, env)?
                        }
                        Err(err) => return Err(err),
                    };
                    new_args.push(arg);
                }
                let result_ty = self.infer_method_call(name, &new_args, expected.clone(), env)?;
                let out = Expr::Call {
                    func: Box::new(func),
                    args: new_args,
                    span: span.clone(),
                };
                return Ok((out, result_ty));
            }
        }

        // Overloaded (non-method) identifiers: resolve by inferring argument types
        // and selecting the unique matching overload, mirroring infer_call logic.
        if let Expr::Ident(name) = &func {
            if env.get_all(&name.name).is_some_and(|items| items.len() > 1) {
                // Infer argument types first (on the original exprs) to select the
                // right overload, then elaborate arguments with the resolved param types.
                let arg_tys: Vec<Type> = args
                    .iter()
                    .map(|arg| self.infer_arg_with_predicate_fallback(arg, env))
                    .collect::<Result<_, _>>()?;

                let Some(candidates) = env.get_all(&name.name) else {
                    return Err(TypeError {
                        span: name.span.clone(),
                        message: format!("unknown name '{}'", name.name),
                        expected: None,
                        found: None,
                    });
                };

                // Save substitution state AFTER arg inference so operand type
                // constraints (e.g. Vec2 from domain `-`) are preserved.
                let base_subst = self.subst.clone();
                let mut selected: Option<(
                    Type,
                    Vec<Type>,
                    std::collections::HashMap<TypeVarId, Type>,
                )> = None;

                for scheme in candidates {
                    self.subst = base_subst.clone();
                    let mut func_ty = self.instantiate(scheme);
                    let mut ok = true;
                    let mut param_tys = Vec::new();
                    for (arg_ty, arg_expr) in arg_tys.iter().zip(args.iter()) {
                        // Before unification, structurally check record field sets.
                        // An open record `{ x, y, .. }` should NOT match a candidate
                        // expecting `{ x, y, z }` — the extra field `z` disqualifies it.
                        let func_ty_applied = self.apply(func_ty.clone());
                        let func_ty_expanded = self.expand_alias(func_ty_applied);
                        if let Type::Func(ref param, _) = func_ty_expanded {
                            let param_expanded = self.expand_alias((**param).clone());
                            let arg_applied = self.apply(arg_ty.clone());
                            let arg_expanded = self.expand_alias(arg_applied);
                            if let (
                                Type::Record {
                                    fields: param_fields,
                                    ..
                                },
                                Type::Record {
                                    fields: arg_fields, ..
                                },
                            ) = (&param_expanded, &arg_expanded)
                            {
                                let param_has_extra =
                                    param_fields.keys().any(|k| !arg_fields.contains_key(k));
                                if param_has_extra {
                                    ok = false;
                                    break;
                                }
                            }
                        }

                        let result_ty = self.fresh_var();
                        if self
                            .unify_with_span(
                                func_ty.clone(),
                                Type::Func(Box::new(arg_ty.clone()), Box::new(result_ty.clone())),
                                expr_span(arg_expr),
                            )
                            .is_err()
                        {
                            ok = false;
                            break;
                        }
                        param_tys.push(self.apply(arg_ty.clone()));
                        func_ty = result_ty;
                    }
                    if !ok {
                        continue;
                    }
                    let applied = self.apply(func_ty.clone());
                    if selected.is_some() {
                        self.subst = base_subst;
                        return Err(TypeError {
                            span: expr_span(&func),
                            message: format!(
                                "ambiguous call to '{}' (multiple overloads match)",
                                name.name
                            ),
                            expected: None,
                            found: None,
                        });
                    }
                    selected = Some((applied, param_tys, self.subst.clone()));
                }

                if let Some((result_ty, param_tys, subst)) = selected {
                    self.subst = subst;
                    let mut new_args = Vec::new();
                    for (arg, expected_arg_ty) in args.into_iter().zip(param_tys.into_iter()) {
                        let expected_arg_ty = self.apply(expected_arg_ty);
                        let (elab_arg, _ty) =
                            self.elab_call_arg_with_predicate_fallback(arg, expected_arg_ty, env)?;
                        new_args.push(elab_arg);
                    }
                    let out = Expr::Call {
                        func: Box::new(func),
                        args: new_args,
                        span,
                    };
                    return Ok((out, result_ty));
                }

                self.subst = base_subst;
                return Err(TypeError {
                    span: expr_span(&func),
                    message: format!("no matching overload for '{}'", name.name),
                    expected: None,
                    found: None,
                });
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
            let (arg, _ty) =
                self.elab_call_arg_with_predicate_fallback(arg, expected_arg_ty, env)?;
            new_args.push(arg);
        }
        let out = Expr::Call {
            func: Box::new(func),
            args: new_args,
            span,
        };
        Ok((out, self.apply(result_ty)))
    }

    fn elab_call_arg_with_predicate_fallback(
        &mut self,
        arg: Expr,
        expected_arg_ty: Type,
        env: &mut TypeEnv,
    ) -> Result<(Expr, Type), TypeError> {
        let expected_applied_inner = self.apply(expected_arg_ty.clone());
        let expected_applied = self.expand_alias(expected_applied_inner);
        let allow_predicate_try = matches!(expected_applied, Type::Func(_, _) | Type::Var(_))
            || matches!(&expected_applied, Type::Con(name, _) if name == "Pred");

        if allow_predicate_try {
            if let Some(rewritten) = lift_predicate_expr(&arg, env, "__pred") {
                let checkpoint = self.subst.clone();
                if let Ok((elab_arg, elab_ty)) =
                    self.elab_expr(rewritten, Some(expected_arg_ty.clone()), env)
                {
                    let elab_applied_inner = self.apply(elab_ty.clone());
                    let elab_applied = self.expand_alias(elab_applied_inner);
                    let is_predicate_fn = if let Type::Func(_, result_ty) = elab_applied {
                        let result_applied_inner = self.apply(*result_ty);
                        let result_applied = self.expand_alias(result_applied_inner);
                        matches!(
                            result_applied,
                            Type::Con(ref name, ref args) if name == "Bool" && args.is_empty()
                        )
                    } else {
                        false
                    };
                    if is_predicate_fn {
                        return Ok((elab_arg, elab_ty));
                    }
                }
                self.subst = checkpoint;
            }
        }

        self.elab_expr(arg, Some(expected_arg_ty), env)
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

        let fields = self.prepend_missing_record_defaults(fields, expected_ty.as_ref(), &span);
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

    fn prepend_missing_record_defaults(
        &mut self,
        fields: Vec<RecordField>,
        expected_ty: Option<&Type>,
        record_span: &Span,
    ) -> Vec<RecordField> {
        let Some(Type::Record {
            fields: expected_fields,
        }) = expected_ty
        else {
            return fields;
        };
        if self.enabled_record_default_types.is_empty() {
            return fields;
        }

        let mut present_top_level = HashSet::new();
        for field in &fields {
            if field.spread {
                continue;
            }
            if let Some(PathSegment::Field(name)) = field.path.first() {
                present_top_level.insert(name.name.clone());
            }
        }

        let mut generated = Vec::new();
        for (field_name, field_ty) in expected_fields.clone() {
            if present_top_level.contains(&field_name) {
                continue;
            }
            let Some(default_value) =
                self.default_expr_for_missing_record_field(&field_ty, record_span)
            else {
                continue;
            };
            generated.push(RecordField {
                spread: false,
                path: vec![PathSegment::Field(SpannedName {
                    name: field_name,
                    span: record_span.clone(),
                })],
                value: default_value,
                span: record_span.clone(),
            });
        }

        if generated.is_empty() {
            fields
        } else {
            let mut merged = generated;
            merged.extend(fields);
            merged
        }
    }

    fn default_expr_for_missing_record_field(&mut self, ty: &Type, span: &Span) -> Option<Expr> {
        let applied = self.apply(ty.clone());
        let expected = self.expand_alias(applied);
        let Type::Con(name, args) = expected else {
            return None;
        };

        match (name.as_str(), args.len()) {
            ("Option", 1) if self.record_default_enabled("Option") => {
                return Some(Expr::Ident(SpannedName {
                    name: "None".into(),
                    span: span.clone(),
                }));
            }
            ("List", 1) if self.record_default_enabled("List") => {
                return Some(Expr::List {
                    items: Vec::new(),
                    span: span.clone(),
                });
            }
            ("Bool", 0) if self.record_default_enabled("Bool") => {
                return Some(Expr::Literal(Literal::Bool {
                    value: false,
                    span: span.clone(),
                }));
            }
            ("Int", 0) if self.record_default_enabled("Int") => {
                return Some(Expr::Literal(Literal::Number {
                    text: "0".into(),
                    span: span.clone(),
                }));
            }
            ("Float", 0) if self.record_default_enabled("Float") => {
                return Some(Expr::Literal(Literal::Number {
                    text: "0.0".into(),
                    span: span.clone(),
                }));
            }
            ("Text", 0) if self.record_default_enabled("Text") => {
                return Some(Expr::Literal(Literal::String {
                    text: String::new(),
                    span: span.clone(),
                }));
            }
            _ => {}
        }

        if (self.record_default_enabled(name.as_str()) || self.record_default_enabled("ToDefault"))
            && self.method_to_classes.contains_key("toDefault")
        {
            return Some(self.to_default_call_expr(span.clone()));
        }
        None
    }

    fn to_default_call_expr(&self, span: Span) -> Expr {
        let func = Expr::Ident(SpannedName {
            name: "toDefault".into(),
            span: span.clone(),
        });
        Expr::Call {
            func: Box::new(func),
            args: Vec::new(),
            span,
        }
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

        if let (Some(expected_ty), Expr::Call { func, args, .. }) = (expected.clone(), &expr) {
            if let Expr::Ident(name) = func.as_ref() {
                if env.get(&name.name).is_none() && self.method_to_classes.contains_key(&name.name)
                {
                    let inferred =
                        self.infer_method_call(name, args, Some(expected_ty.clone()), env)?;
                    return Ok((expr, inferred));
                }
            }
        }

        if let Some(expected_ty) = expected.clone() {
            let expected_applied_inner = self.apply(expected_ty.clone());
            let expected_applied = self.expand_alias(expected_applied_inner);
            let allow_predicate_try =
                matches!(expected_applied, Type::Func(_, _) | Type::Var(_));
            if allow_predicate_try {
                if let Some(rewritten) = lift_predicate_expr(&expr, env, "__pred") {
                    let checkpoint = self.subst.clone();
                    if let Ok(rewritten_ty) = self.infer_expr(&rewritten, env) {
                        let rewritten_applied_inner = self.apply(rewritten_ty.clone());
                        let rewritten_applied = self.expand_alias(rewritten_applied_inner);
                        let rewritten_is_predicate = if let Type::Func(_, result_ty) = rewritten_applied
                        {
                            let result_applied_inner = self.apply(*result_ty);
                            let result_applied = self.expand_alias(result_applied_inner);
                            matches!(
                                result_applied,
                                Type::Con(ref name, ref args) if name == "Bool" && args.is_empty()
                            )
                        } else {
                            false
                        };
                        if rewritten_is_predicate
                            && self
                                .unify_with_span(
                                    rewritten_ty,
                                    expected_ty.clone(),
                                    expr_span(&rewritten),
                                )
                                .is_ok()
                        {
                            return Ok((rewritten, self.apply(expected_ty)));
                        }
                    }
                    self.subst = checkpoint;
                }
            }
        }

        if let (Some(expected_ty), Expr::Record { fields, .. }) = (expected.clone(), &expr) {
            let expected_applied = {
                let applied = self.apply(expected_ty.clone());
                self.expand_alias(applied)
            };
            if matches!(expected_applied, Type::Record { .. }) {
                let mut record_ty = Type::Record {
                    fields: BTreeMap::new(),
                };
                for field in fields {
                    if field.spread {
                        let spread_ty = self.infer_expr(&field.value, env)?;
                        record_ty = self.merge_records(record_ty, spread_ty, field.span.clone())?;
                        continue;
                    }

                    let value_expected = self
                        .record_field_type(
                            expected_applied.clone(),
                            &field.path,
                            field.span.clone(),
                        )
                        .ok();
                    let value_ty = if let Some(value_expected) = value_expected {
                        let (_elab_value, value_ty) =
                            self.check_or_coerce(field.value.clone(), Some(value_expected), env)?;
                        value_ty
                    } else {
                        self.infer_expr(&field.value, env)?
                    };
                    let nested = self.record_from_path(&field.path, value_ty);
                    record_ty = self.merge_records(record_ty, nested, field.span.clone())?;
                }
                self.unify_with_span(record_ty, expected_ty.clone(), expr_span(&expr))?;
                return Ok((expr, self.apply(expected_ty)));
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
                name: "toText".into(),
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
                name: "TextNode".into(),
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
                name: "toText".into(),
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
