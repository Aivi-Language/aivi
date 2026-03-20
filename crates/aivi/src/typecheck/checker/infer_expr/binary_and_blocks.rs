#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SignalWriteKind {
    Set,
    Update,
}

impl TypeChecker {
    fn extract_signal_item_type(&mut self, ty: Type) -> Option<Type> {
        let applied = self.apply(ty);
        let resolved = self.expand_alias(applied);
        match resolved {
            Type::Con(name, args) if self.type_name_matches(&name, "Signal") && args.len() == 1 => {
                Some(args[0].clone())
            }
            Type::App(base, args) => match &*base {
                Type::Con(name, existing)
                    if self.type_name_matches(name, "Signal")
                        && existing.len() + args.len() == 1 =>
                {
                    args.last().cloned().or_else(|| existing.last().cloned())
                }
                _ => None,
            },
            _ => None,
        }
    }

    fn extract_table_row_type(&mut self, ty: Type) -> Option<Type> {
        let applied = self.apply(ty);
        if let Some(row_ty) = Self::extract_named_type_arg(&applied, "Relation") {
            let applied_row = self.apply(row_ty);
            return Some(self.expand_alias(applied_row));
        }

        let resolved = self.expand_alias(applied);
        let Type::Record { fields } = resolved else {
            return None;
        };
        let rows_ty = fields.get("rows")?.clone();
        let applied_rows = self.apply(rows_ty);
        let list_row = Self::extract_named_type_arg(&applied_rows, "List")?;
        let applied_row = self.apply(list_row);
        Some(self.expand_alias(applied_row))
    }

    fn extract_db_selection_row_type(&mut self, ty: Type) -> Option<Type> {
        let applied = self.apply(ty);
        if let Some(row_ty) = Self::extract_named_type_arg(&applied, "Query") {
            let applied_row = self.apply(row_ty);
            return Some(self.expand_alias(applied_row));
        }

        let resolved = self.expand_alias(applied);
        let Type::Record { fields } = resolved else {
            return None;
        };
        let run_ty = fields.get("run")?.clone();
        let applied_run = self.apply(run_ty);
        let expanded_run = self.expand_alias(applied_run);
        let Type::Func(_, result_ty) = expanded_run else {
            return None;
        };
        let effect_result = Self::extract_effect_result_type(&result_ty)?;
        let row_ty = Self::extract_named_type_arg(&effect_result, "List")?;
        let applied_row = self.apply(row_ty);
        Some(self.expand_alias(applied_row))
    }

    fn db_effect_table_type(&self, row_ty: Type) -> Type {
        self.named_type("Effect").app(vec![
            self.named_type("DbError"),
            self.named_type("Relation").app(vec![row_ty]),
        ])
    }

    fn infer_signal_pipe_result(
        &mut self,
        left: &Expr,
        right: &Expr,
        signal_item_ty: Type,
        env: &mut TypeEnv,
    ) -> Result<Type, TypeError> {
        let param = SpannedName {
            name: "__signalValue".to_string(),
            span: expr_span(left),
        };
        let piped_body = Expr::Binary {
            op: "|>".to_string(),
            left: Box::new(Expr::Ident(param.clone())),
            right: Box::new(right.clone()),
            span: expr_span(right),
        };
        let mut body_env = env.clone();
        body_env.insert(param.name.clone(), Scheme::mono(signal_item_ty));
        let result_ty = self.infer_expr(&piped_body, &mut body_env)?;
        Ok(self.named_type("Signal").app(vec![result_ty]))
    }

    fn infer_pipe_partial_call(
        &mut self,
        left: &Expr,
        right: &Expr,
        env: &mut TypeEnv,
        debug_pipe: bool,
    ) -> Option<Result<Type, TypeError>> {
        let Expr::Call {
            func,
            args: partial_args,
            ..
        } = right
        else {
            return None;
        };
        let Expr::Ident(name) = func.as_ref() else {
            return None;
        };

        if debug_pipe {
            eprintln!(
                "[PIPE_DEBUG] |> right=Call({}) in_env={} overloads={} in_methods={}",
                name.name,
                env.get(&name.name).is_some(),
                env.get_all(&name.name).map(|items| items.len()).unwrap_or(0),
                self.method_to_classes.contains_key(&name.name)
            );
        }

        if env.get(&name.name).is_none() && self.method_to_classes.contains_key(&name.name) {
            let mut all_args: Vec<Expr> = partial_args.clone();
            all_args.push(left.clone());
            return Some(self.infer_method_call(name, &all_args, None, env));
        }

        if env.get_all(&name.name).is_some_and(|items| items.len() > 1) {
            let mut all_args: Vec<Expr> = partial_args.clone();
            all_args.push(left.clone());
            return Some(self.infer_call(func, &all_args, env));
        }

        None
    }

    fn infer_signal_write_kind(
        &mut self,
        right: &Expr,
        signal_item_ty: &Type,
        env: &mut TypeEnv,
    ) -> Result<SignalWriteKind, TypeError> {
        if matches!(right, Expr::Record { .. }) {
            return Ok(SignalWriteKind::Update);
        }

        let right_ty = self.infer_expr(right, env)?;
        let checkpoint = self.subst.clone();

        if self
            .unify_with_span(right_ty.clone(), signal_item_ty.clone(), expr_span(right))
            .is_ok()
        {
            return Ok(SignalWriteKind::Set);
        }

        self.subst = checkpoint.clone();
        let updater_ty = Type::Func(
            Box::new(signal_item_ty.clone()),
            Box::new(signal_item_ty.clone()),
        );
        if self
            .unify_with_span(right_ty, updater_ty, expr_span(right))
            .is_ok()
        {
            return Ok(SignalWriteKind::Update);
        }

        self.subst = checkpoint;
        let item_text = self.type_to_string(signal_item_ty);
        Err(TypeError {
            span: expr_span(right),
            message: format!(
                "signal update expects a replacement value of type {item_text}, an updater function {item_text} -> {item_text}, or a record patch"
            ),
            expected: None,
            found: None,
        })
    }

    fn infer_binary(
        &mut self,
        op: &str,
        left: &Expr,
        right: &Expr,
        env: &mut TypeEnv,
    ) -> Result<Type, TypeError> {
        let debug_pipe = std::env::var("AIVI_DEBUG_PIPE").is_ok_and(|v| v == "1");
        if debug_pipe {
            eprintln!("[PIPE_DEBUG] infer_binary op={}", op);
        }
        if op == "|>" {
            let right = self.normalize_pipe_transformer(right, env);
            let needs_syntactic_query_fallback = !matches!(left, Expr::Ident(_))
                && self.looks_like_relation_query_expr(left)
                && self.looks_like_relation_query_stage(&right);
            if needs_syntactic_query_fallback {
                let checkpoint = self.subst.clone();
                if let Ok(arg_ty) = self.infer_expr(left, env) {
                    if self.extract_query_row_type(arg_ty.clone()).is_some() {
                        self.validate_query_pipe_transformer(&right, &arg_ty, env)?;
                        if let Some(result) =
                            self.infer_pipe_partial_call(left, &right, env, debug_pipe)
                        {
                            return result;
                        }
                        let func_ty = self.infer_expr(&right, env)?;
                        let result_ty = self.fresh_var();
                        self.unify_with_span(
                            func_ty,
                            Type::Func(Box::new(arg_ty), Box::new(result_ty.clone())),
                            expr_span(&right),
                        )?;
                        return Ok(result_ty);
                    }
                }
                self.subst = checkpoint;
                return Ok(self.fresh_var());
            }
            let arg_ty = self.infer_expr(left, env)?;
            if std::env::var("AIVI_DEBUG_SIG").is_ok_and(|v| v == "1") {
                eprintln!(
                    "[PIPE_INNER] |> right after normalize: disc={:?}",
                    std::mem::discriminant(&right)
                );
            }
            if let Some(result) = self.infer_pipe_partial_call(left, &right, env, debug_pipe) {
                self.validate_query_pipe_transformer(&right, &arg_ty, env)?;
                return result;
            }
            let func_ty = self.infer_expr(&right, env)?;
            self.validate_query_pipe_transformer(&right, &arg_ty, env)?;
            let result_ty = self.fresh_var();
            self.unify_with_span(
                func_ty,
                Type::Func(Box::new(arg_ty), Box::new(result_ty.clone())),
                expr_span(&right),
            )?;
            return Ok(result_ty);
        }
        if op == "->>" {
            let arg_ty = self.infer_expr(left, env)?;
            let right = self.normalize_pipe_transformer(right, env);
            if std::env::var("AIVI_DEBUG_SIG").is_ok_and(|v| v == "1") {
                eprintln!(
                    "[SIG_PIPE] right after normalize: disc={:?}",
                    std::mem::discriminant(&right)
                );
            }
            if let Some(signal_item_ty) = self.extract_signal_item_type(arg_ty.clone()) {
                return self.infer_signal_pipe_result(left, &right, signal_item_ty, env);
            }
            return Err(TypeError {
                span: expr_span(left),
                message: format!(
                    "the `->>` operator requires a `Signal` on the left-hand side, but found `{}`",
                    self.type_to_string(&arg_ty)
                ),
                expected: None,
                found: None,
            });
        }
        if op == "<|" {
            let target_ty = self.infer_expr(left, env)?;
            if let Some(row_ty) = self.extract_db_selection_row_type(target_ty.clone()) {
                let effect_table_ty = self.db_effect_table_type(row_ty.clone());
                return match right {
                    Expr::Record { fields, .. } | Expr::PatchLit { fields, .. } => {
                        self.infer_patch(row_ty, fields, env)?;
                        Ok(effect_table_ty)
                    }
                    _ => Err(TypeError {
                        span: expr_span(right),
                        message: "database query updates accept only a patch block like `{ ... }`"
                            .to_string(),
                        expected: None,
                        found: None,
                    }),
                };
            }
            let applied_target_ty = self.apply(target_ty.clone());
            let resolved = self.expand_alias(applied_target_ty);
            if let Some(type_name) = self.opaque_con_name(&resolved) {
                if let Some(defining_module) = self.is_opaque_from_here(&type_name).cloned() {
                    return Err(TypeError {
                        span: expr_span(right),
                        message: format!(
                            "cannot update opaque type `{}` outside module `{}`",
                            type_name, defining_module
                        ),
                        expected: None,
                        found: None,
                    });
                }
            }
            if let Expr::Record { fields, .. } | Expr::PatchLit { fields, .. } = right {
                return self.infer_patch(target_ty, fields, env);
            }
        }
        if op == "<<-" {
            let target_ty = self.infer_expr(left, env)?;
            if let Some(signal_item_ty) = self.extract_signal_item_type(target_ty.clone()) {
                if let Expr::Record { fields, .. } = right {
                    self.infer_patch(signal_item_ty, fields, env)?;
                    return Ok(Type::con("Unit"));
                }
                match self.infer_signal_write_kind(right, &signal_item_ty, env)? {
                    SignalWriteKind::Set | SignalWriteKind::Update => {}
                }
                return Ok(Type::con("Unit"));
            }
            return Err(TypeError {
                span: expr_span(left),
                message: format!(
                    "the `<<-` operator requires a `Signal` on the left-hand side, but found `{}`",
                    self.type_to_string(&target_ty)
                ),
                expected: None,
                found: None,
            });
        }

        let _subst_before_operands = self.subst.clone();
        let left_ty = self.infer_expr(left, env)?;
        let right_ty = self.infer_expr(right, env)?;
        let subst_after_operands = self.subst.clone();
        match op {
            "&&" | "||" => {
                self.unify_with_span(left_ty, Type::con("Bool"), expr_span(left))?;
                self.unify_with_span(right_ty, Type::con("Bool"), expr_span(right))?;
                Ok(Type::con("Bool"))
            }
            "==" | "!=" => {
                self.unify_with_span(left_ty, right_ty, expr_span(right))?;
                Ok(Type::con("Bool"))
            }
            "<" | ">" | "<=" | ">=" => {
                let op_name = format!("({})", op);
                let left_applied = self.apply(left_ty.clone());
                let left_applied = self.expand_alias(left_applied);
                let right_applied = self.apply(right_ty.clone());
                let right_applied = self.expand_alias(right_applied);
                let both_int = matches!(left_applied, Type::Con(ref name, _) if name == "Int")
                    && matches!(right_applied, Type::Con(ref name, _) if name == "Int");
                let both_float = matches!(left_applied, Type::Con(ref name, _) if name == "Float")
                    && matches!(right_applied, Type::Con(ref name, _) if name == "Float");
                let both_bigint = matches!(left_applied, Type::Con(ref name, _) if name == "BigInt")
                    && matches!(right_applied, Type::Con(ref name, _) if name == "BigInt");
                let both_text = matches!(left_applied, Type::Con(ref name, _) if name == "Text")
                    && matches!(right_applied, Type::Con(ref name, _) if name == "Text");
                // Check if either operand is Float (the other might be a type variable)
                let left_is_float =
                    matches!(left_applied, Type::Con(ref name, _) if name == "Float");
                let right_is_float =
                    matches!(right_applied, Type::Con(ref name, _) if name == "Float");
                let either_float = left_is_float || right_is_float;

                // Text comparison is built-in (lexicographic / Unicode codepoint order)
                if both_text {
                    return Ok(Type::con("Bool"));
                }

                // Float comparison is built-in like Int
                if both_float {
                    return Ok(Type::con("Bool"));
                }

                if both_bigint {
                    return Ok(Type::con("Bool"));
                }

                // If one operand is Float and the other is a type variable, unify with Float
                if either_float && !both_int {
                    self.unify_with_span(left_ty, Type::con("Float"), expr_span(left))?;
                    self.unify_with_span(right_ty, Type::con("Float"), expr_span(right))?;
                    return Ok(Type::con("Bool"));
                }

                if !both_int {
                    let any_var = matches!(left_applied, Type::Var(_))
                        || matches!(right_applied, Type::Var(_));
                    let concrete_non_int = matches!(left_applied, Type::Con(ref name, _) if name != "Int")
                        || matches!(right_applied, Type::Con(ref name, _) if name != "Int");
                    if let Some(candidates) = env.get_all(&op_name) {
                        let base_subst = self.subst.clone();
                        let mut selected: Option<(
                            String,
                            String,
                            String,
                            std::collections::HashMap<TypeVarId, Type>,
                        )> = None;
                        for scheme in candidates {
                            self.subst = base_subst.clone();
                            let op_ty = self.instantiate(scheme);
                            let origin = scheme
                                .origin
                                .as_ref()
                                .map(|o| o.render())
                                .unwrap_or_else(|| "<unknown>".to_string());
                            let sig = self.type_to_string(&scheme.ty);
                            let rest_ty = self.fresh_var();
                            // Use applied types for better disambiguation
                            let left_ty_for_unify = self.apply(left_ty.clone());
                            if self
                                .unify_with_span(
                                    op_ty,
                                    Type::Func(
                                        Box::new(left_ty_for_unify),
                                        Box::new(rest_ty.clone()),
                                    ),
                                    expr_span(left),
                                )
                                .is_err()
                            {
                                continue;
                            }
                            let result_ty = self.fresh_var();
                            let right_ty_for_unify = self.apply(right_ty.clone());
                            if self
                                .unify_with_span(
                                    rest_ty,
                                    Type::Func(
                                        Box::new(right_ty_for_unify.clone()),
                                        Box::new(result_ty.clone()),
                                    ),
                                    expr_span(right),
                                )
                                .is_err()
                            {
                                continue;
                            }
                            if self
                                .unify_with_span(result_ty, Type::con("Bool"), expr_span(left))
                                .is_err()
                            {
                                continue;
                            }

                            let key_ty = Type::Func(
                                Box::new(right_ty_for_unify),
                                Box::new(Type::con("Bool")),
                            );
                            let key = self.type_to_string(&key_ty);
                            if let Some((existing_key, existing_origin, existing_sig, _)) =
                                &selected
                            {
                                if *existing_key != key {
                                    // Check if the operand is a type variable - if so, suggest adding a type annotation
                                    let left_ty_resolved = self.apply(left_ty.clone());
                                    let is_type_var = matches!(left_ty_resolved, Type::Var(_));
                                    let message = if is_type_var {
                                        format!(
                                            "cannot determine which domain operator '{}' to use; candidates: {} ({}) vs {} ({}); add a type annotation to disambiguate",
                                            op, existing_origin, existing_sig, origin, sig
                                        )
                                    } else {
                                        format!(
                                            "ambiguous domain operator '{}' for these operand types; candidates: {} ({}) vs {} ({})",
                                            op, existing_origin, existing_sig, origin, sig
                                        )
                                    };
                                    self.subst = base_subst;
                                    return Err(TypeError {
                                        span: expr_span(left),
                                        message,
                                        expected: None,
                                        found: None,
                                    });
                                }
                                // Duplicate overload (typically from repeated imports); ignore.
                                continue;
                            }
                            selected = Some((key, origin, sig, self.subst.clone()));
                        }
                        if let Some((_, _, _, subst)) = selected {
                            self.subst = subst;
                            return Ok(Type::con("Bool"));
                        }
                        self.subst = base_subst;
                    }
                    if concrete_non_int && !any_var {
                        return Err(TypeError {
                            span: expr_span(left),
                            message: format!("no domain operator '{}' for these operand types", op),
                            expected: None,
                            found: None,
                        });
                    }
                }

                self.unify_with_span(left_ty, Type::con("Int"), expr_span(left))?;
                self.unify_with_span(right_ty, Type::con("Int"), expr_span(right))?;
                Ok(Type::con("Bool"))
            }
            "++" | "+" | "-" | "*" | "×" | "/" | "%" => {
                let op_name = format!("({})", op);
                let left_applied = self.apply(left_ty.clone());
                let left_applied = self.expand_alias(left_applied);
                let right_applied = self.apply(right_ty.clone());
                let right_applied = self.expand_alias(right_applied);

                let allow_int_fallback = op != "++";
                let both_int = allow_int_fallback
                    && matches!(left_applied, Type::Con(ref name, _) if name == "Int")
                    && matches!(right_applied, Type::Con(ref name, _) if name == "Int");
                let both_float = allow_int_fallback
                    && matches!(left_applied, Type::Con(ref name, _) if name == "Float")
                    && matches!(right_applied, Type::Con(ref name, _) if name == "Float");
                let both_bigint = allow_int_fallback
                    && matches!(left_applied, Type::Con(ref name, _) if name == "BigInt")
                    && matches!(right_applied, Type::Con(ref name, _) if name == "BigInt");
                // Check if either operand is Float (the other might be a type variable)
                let left_is_float =
                    matches!(left_applied, Type::Con(ref name, _) if name == "Float");
                let right_is_float =
                    matches!(right_applied, Type::Con(ref name, _) if name == "Float");
                let either_float = allow_int_fallback && (left_is_float || right_is_float);
                // Float shortcut only applies when the non-Float operand is a type variable or also
                // Float/Int. If the non-Float operand is a concrete domain type (e.g. Vec4), domain
                // operators must be checked first.
                let non_float_side_is_domain_type = if left_is_float {
                    matches!(left_applied, Type::Con(ref name, _) if name == "Float")
                        && matches!(right_applied, Type::Record { .. } | Type::Con(_, _))
                        && !right_is_float
                        && !matches!(right_applied, Type::Var(_))
                        && !matches!(right_applied, Type::Con(ref name, _) if name == "Int")
                } else if right_is_float {
                    matches!(right_applied, Type::Con(ref name, _) if name == "Float")
                        && matches!(left_applied, Type::Record { .. } | Type::Con(_, _))
                        && !left_is_float
                        && !matches!(left_applied, Type::Var(_))
                        && !matches!(left_applied, Type::Con(ref name, _) if name == "Int")
                } else {
                    false
                };

                // Float arithmetic is built-in like Int
                if both_float {
                    return Ok(Type::con("Float"));
                }

                if both_bigint && matches!(op, "+" | "-" | "*") {
                    return Ok(Type::con("BigInt"));
                }

                // If one operand is Float and the other is a type variable, unify with Float.
                // Skip if the non-Float side is a concrete domain type — domain operators must
                // be resolved first.
                if either_float && !both_int && !non_float_side_is_domain_type {
                    self.unify_with_span(left_ty, Type::con("Float"), expr_span(left))?;
                    self.unify_with_span(right_ty, Type::con("Float"), expr_span(right))?;
                    return Ok(Type::con("Float"));
                }

                if !both_int {
                    let any_var = matches!(left_applied, Type::Var(_))
                        || matches!(right_applied, Type::Var(_));
                    let concrete_non_int = matches!(left_applied, Type::Con(ref name, _) if name != "Int")
                        || matches!(right_applied, Type::Con(ref name, _) if name != "Int");
                    if let Some(candidates) = env.get_all(&op_name) {
                        let candidates: Vec<Scheme> = candidates.to_vec();

                        // Use subst_after_operands as base so operand types are already constrained
                        let base_subst = subst_after_operands.clone();
                        #[allow(clippy::type_complexity)]
                        let mut selected: Option<(
                            String,
                            String,
                            String,
                            Type,
                            std::collections::HashMap<TypeVarId, Type>,
                        )> = None;
                        let use_expected_rhs = candidates.len() > 1;

                        for scheme in &candidates {
                            self.subst = base_subst.clone();

                            let op_ty = self.instantiate(scheme);
                            let origin = scheme
                                .origin
                                .as_ref()
                                .map(|o| o.render())
                                .unwrap_or_else(|| "<unknown>".to_string());
                            let sig = self.type_to_string(&scheme.ty);
                            // Use already-inferred left_ty instead of re-inferring
                            let left_ty_applied = self.apply(left_ty.clone());
                            let left_ty_expanded = self.expand_alias(left_ty_applied.clone());

                            // Debug: show type variable info before resolution
                            if std::env::var("AIVI_DEBUG_DOMAIN").is_ok() {
                                let span = expr_span(left);
                                if span.start.line == 19 && span.start.column == 47 {
                                    eprintln!("DEBUG: left_ty (raw) = {:?}", left_ty);
                                    eprintln!("DEBUG: subst has {} mappings", self.subst.len());
                                    if let Type::Var(v) = &left_ty {
                                        let mapped = self.subst.get(v).cloned();
                                        eprintln!(
                                            "DEBUG: left_ty var {:?} maps to {:?}",
                                            v, mapped
                                        );
                                    }
                                }
                            }

                            // Extract the expected left operand type from the operator
                            let op_ty_expanded = self.expand_alias(op_ty.clone());
                            if let Type::Func(op_param, _) = &op_ty_expanded {
                                let op_param_expanded = self.expand_alias((**op_param).clone());
                                // Check if operator expects more fields than operand has
                                if let (
                                    Type::Record {
                                        fields: op_fields, ..
                                    },
                                    Type::Record {
                                        fields: val_fields, ..
                                    },
                                ) = (&op_param_expanded, &left_ty_expanded)
                                {
                                    // If operator expects fields that operand doesn't have, skip
                                    let has_extra_fields =
                                        op_fields.keys().any(|k| !val_fields.contains_key(k));
                                    if has_extra_fields {
                                        continue;
                                    }
                                    // If operand has fields not required by the operator, this
                                    // overload is for a different carrier type — skip it.
                                    // This prevents Mat4 from spuriously matching Mat2 overloads
                                    // because open record unification would otherwise succeed.
                                    let val_has_extra =
                                        val_fields.keys().any(|k| !op_fields.contains_key(k));
                                    if val_has_extra {
                                        continue;
                                    }
                                }
                            }

                            let rest_ty = self.fresh_var();
                            if self
                                .unify_with_span(
                                    op_ty,
                                    Type::Func(
                                        Box::new(left_ty_applied),
                                        Box::new(rest_ty.clone()),
                                    ),
                                    expr_span(left),
                                )
                                .is_err()
                            {
                                continue;
                            }

                            let (match_key, result_ty) = if use_expected_rhs {
                                let rest_applied = self.apply(rest_ty);
                                let rest_norm = self.expand_alias(rest_applied);
                                let Type::Func(expected_rhs, expected_result) = rest_norm else {
                                    continue;
                                };
                                let expected_rhs_ty = *expected_rhs;
                                let expected_result_ty = *expected_result;

                                // Before calling elab_expr, check structural compatibility of the
                                // expected RHS type against the already-inferred right_ty. Two
                                // record types with completely different field names (e.g. Vec4 vs
                                // Mat4) should not unify — but open-record unification would
                                // accept them both. Apply exact-field-set matching to skip the
                                // structurally incompatible overloads.
                                let expected_rhs_expanded =
                                    self.expand_alias(expected_rhs_ty.clone());
                                let right_ty_raw = self.apply(right_ty.clone());
                                let right_ty_expanded = self.expand_alias(right_ty_raw);
                                if let (
                                    Type::Record {
                                        fields: exp_fields, ..
                                    },
                                    Type::Record {
                                        fields: actual_fields,
                                        ..
                                    },
                                ) = (&expected_rhs_expanded, &right_ty_expanded)
                                {
                                    let exp_has_extra =
                                        exp_fields.keys().any(|k| !actual_fields.contains_key(k));
                                    let actual_has_extra =
                                        actual_fields.keys().any(|k| !exp_fields.contains_key(k));
                                    if exp_has_extra || actual_has_extra {
                                        continue;
                                    }
                                }

                                if self
                                    .elab_expr(right.clone(), Some(expected_rhs_ty.clone()), env)
                                    .is_err()
                                {
                                    continue;
                                }
                                let res_ty = self.apply(expected_result_ty);
                                let key_ty =
                                    Type::Func(Box::new(expected_rhs_ty), Box::new(res_ty.clone()));
                                (self.type_to_string(&key_ty), res_ty)
                            } else {
                                // Use already-inferred right_ty instead of re-inferring
                                let right_ty_applied = self.apply(right_ty.clone());
                                let result_ty = self.fresh_var();
                                if self
                                    .unify_with_span(
                                        rest_ty.clone(),
                                        Type::Func(
                                            Box::new(right_ty_applied),
                                            Box::new(result_ty.clone()),
                                        ),
                                        expr_span(right),
                                    )
                                    .is_err()
                                {
                                    continue;
                                }
                                let res_ty = self.apply(result_ty);
                                let rest_applied = self.apply(rest_ty);
                                let rest_norm = self.expand_alias(rest_applied);
                                let Type::Func(rhs_ty, _) = rest_norm else {
                                    continue;
                                };
                                let key_ty =
                                    Type::Func(Box::new(*rhs_ty), Box::new(res_ty.clone()));
                                (self.type_to_string(&key_ty), res_ty)
                            };

                            if let Some((existing_key, existing_origin, existing_sig, _, _)) =
                                &selected
                            {
                                if *existing_key != match_key {
                                    // Check if the operand is a type variable - if so, suggest adding a type annotation
                                    let left_ty_resolved = self.apply(left_ty.clone());
                                    let is_type_var = matches!(left_ty_resolved, Type::Var(_));
                                    let message = if is_type_var {
                                        format!(
                                            "cannot determine which domain operator '{}' to use; candidates: {} ({}) vs {} ({}); add a type annotation to disambiguate",
                                            op, existing_origin, existing_sig, origin, sig
                                        )
                                    } else {
                                        format!(
                                            "ambiguous domain operator '{}' for these operand types; candidates: {} ({}) vs {} ({})",
                                            op, existing_origin, existing_sig, origin, sig
                                        )
                                    };
                                    self.subst = subst_after_operands.clone();
                                    return Err(TypeError {
                                        span: expr_span(left),
                                        message,
                                        expected: None,
                                        found: None,
                                    });
                                }
                                // Duplicate overload (typically from repeated imports); ignore.
                                continue;
                            }
                            selected =
                                Some((match_key, origin, sig, result_ty, self.subst.clone()));
                        }

                        if let Some((_, _, _, result, subst)) = selected {
                            self.subst = subst;
                            return Ok(result);
                        }

                        self.subst = subst_after_operands.clone();
                    }
                    if !allow_int_fallback {
                        return Err(TypeError {
                            span: expr_span(left),
                            message: format!("no domain operator '{}' for these operand types", op),
                            expected: None,
                            found: None,
                        });
                    }
                    if concrete_non_int && !any_var {
                        return Err(TypeError {
                            span: expr_span(left),
                            message: format!("no domain operator '{}' for these operand types", op),
                            expected: None,
                            found: None,
                        });
                    }
                }

                if !allow_int_fallback {
                    return Err(TypeError {
                        span: expr_span(left),
                        message: format!("no domain operator '{}' for these operand types", op),
                        expected: None,
                        found: None,
                    });
                }

                self.unify_with_span(left_ty, Type::con("Int"), expr_span(left))?;
                self.unify_with_span(right_ty, Type::con("Int"), expr_span(right))?;
                Ok(Type::con("Int"))
            }
            ".." => {
                self.unify_with_span(left_ty, Type::con("Int"), expr_span(left))?;
                self.unify_with_span(right_ty, Type::con("Int"), expr_span(right))?;
                Ok(Type::con("List").app(vec![Type::con("Int")]))
            }
            "??" => {
                // lhs ?? rhs  ⟹  lhs : Option A, rhs : A, result : A
                let inner = self.fresh_var();
                self.unify_with_span(
                    left_ty,
                    Type::con("Option").app(vec![inner.clone()]),
                    expr_span(left),
                )?;
                self.unify_with_span(right_ty, inner.clone(), expr_span(right))?;
                Ok(inner)
            }
            _ => Ok(self.fresh_var()),
        }
    }

    fn infer_block(
        &mut self,
        kind: &BlockKind,
        items: &[BlockItem],
        env: &mut TypeEnv,
    ) -> Result<Type, TypeError> {
        match kind {
            BlockKind::Plain => self.infer_plain_block(items, env),
            BlockKind::Do { monad } if monad.name == "Effect" => {
                self.infer_effect_block(items, env)
            }
            BlockKind::Do { monad } if monad.name == "Event" => self.infer_event_block(items, env),
            BlockKind::Do { monad } if monad.name == "Applicative" => {
                self.infer_applicative_do_block(&monad.span, items, env)
            }
            BlockKind::Do { monad } => {
                self.infer_generic_do_block(&monad.name, &monad.span, items, env)
            }
            BlockKind::Generate => self.infer_generate_block(items, env),
            BlockKind::Managed => self.infer_managed_block(items, env),
        }
    }

    fn infer_plain_block(
        &mut self,
        items: &[BlockItem],
        env: &mut TypeEnv,
    ) -> Result<Type, TypeError> {
        let mut local_env = env.clone();
        let mut last_ty = Type::con("Unit");
        for item in items {
            match item {
                BlockItem::Bind { pattern, expr, .. } => {
                    let expr_ty = self.infer_expr(expr, &mut local_env)?;
                    let pat_ty = self.infer_pattern(pattern, &mut local_env)?;
                    let pat_ty_for_span = pat_ty.clone();
                    self.unify_with_span(pat_ty, expr_ty, pattern_span(pattern))?;
                    self.span_types
                        .push((pattern_span(pattern), pat_ty_for_span));
                }
                BlockItem::Let { pattern, expr, .. } => {
                    // Compiler-generated let bindings (e.g. __loop from
                    // loop/recurse desugaring) may be self-referential.
                    // Pre-add a fresh type var so the recursive reference
                    // inside the lambda body can be inferred.
                    if matches!(pattern, Pattern::Ident(n) if n.name.starts_with("__")) {
                        self.infer_pattern(pattern, &mut local_env)?;
                    }
                    let expr_ty = self.infer_expr(expr, &mut local_env)?;
                    let pat_ty = self.infer_pattern(pattern, &mut local_env)?;
                    let pat_ty_for_span = pat_ty.clone();
                    self.unify_with_span(pat_ty, expr_ty, pattern_span(pattern))?;
                    self.span_types
                        .push((pattern_span(pattern), pat_ty_for_span));
                }
                BlockItem::Filter { expr, .. }
                | BlockItem::Yield { expr, .. }
                | BlockItem::Recurse { expr, .. }
                | BlockItem::Expr { expr, .. } => {
                    last_ty = self.infer_expr(expr, &mut local_env)?;
                }
                BlockItem::When { cond, effect, .. } | BlockItem::Unless { cond, effect, .. } => {
                    self.infer_expr(cond, &mut local_env)?;
                    self.infer_expr(effect, &mut local_env)?;
                }
                BlockItem::Given {
                    cond, fail_expr, ..
                } => {
                    self.infer_expr(cond, &mut local_env)?;
                    self.infer_expr(fail_expr, &mut local_env)?;
                }
            }
        }
        Ok(last_ty)
    }
}
