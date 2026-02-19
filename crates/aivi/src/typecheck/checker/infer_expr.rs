impl TypeChecker {
    fn infer_ident(&mut self, name: &SpannedName, env: &mut TypeEnv) -> Result<Type, TypeError> {
        if let Some(scheme) = env.get(&name.name) {
            Ok(self.instantiate(scheme))
        } else if env
            .get_all(&name.name)
            .is_some_and(|items| items.len() > 1)
        {
            Err(TypeError {
                span: name.span.clone(),
                message: format!(
                    "ambiguous name '{}' (multiple type signatures in scope; add a type annotation or call it with enough arguments to disambiguate)",
                    name.name
                ),
                expected: None,
                found: None,
            })
        } else if name.name == "_" {
            Ok(self.fresh_var())
        } else {
            Err(TypeError {
                span: name.span.clone(),
                message: format!("unknown name '{}'", name.name),
                expected: None,
                found: None,
            })
        }
    }

    fn literal_type(&mut self, literal: &Literal) -> Type {
        match literal {
            Literal::Number { text, .. } => match number_kind(text) {
                Some(NumberKind::Float) => Type::con("Float"),
                Some(NumberKind::Int) => Type::con("Int"),
                None => self.fresh_var(),
            },
            Literal::String { .. } => Type::con("Text"),
            Literal::Sigil { tag, .. } => match tag.as_str() {
                "r" => Type::con("Regex"),
                "u" | "url" => Type::con("Url"),
                "p" | "path" => Type::con("Path"),
                "d" => Type::con("Date"),
                "t" | "dt" => Type::con("DateTime"),
                "k" => Type::con("Key"),
                "m" => Type::con("Message"),
                _ => Type::con("Text"),
            },
            Literal::Bool { .. } => Type::con("Bool"),
            Literal::DateTime { .. } => Type::con("DateTime"),
        }
    }

    fn infer_list(
        &mut self,
        items: &[crate::surface::ListItem],
        env: &mut TypeEnv,
    ) -> Result<Type, TypeError> {
        let elem = self.fresh_var();
        for item in items {
            let item_ty = self.infer_expr(&item.expr, env)?;
            if item.spread || is_range_expr(&item.expr) {
                let expected = Type::con("List").app(vec![elem.clone()]);
                self.unify_with_span(item_ty, expected, expr_span(&item.expr))?;
            } else {
                self.unify_with_span(item_ty, elem.clone(), expr_span(&item.expr))?;
            }
        }
        Ok(Type::con("List").app(vec![elem]))
    }

    fn infer_tuple(&mut self, items: &[Expr], env: &mut TypeEnv) -> Result<Type, TypeError> {
        let mut tys = Vec::new();
        for item in items {
            tys.push(self.infer_expr(item, env)?);
        }
        Ok(Type::Tuple(tys))
    }

    fn infer_record(
        &mut self,
        fields: &[RecordField],
        env: &mut TypeEnv,
    ) -> Result<Type, TypeError> {
        // A record literal without spreads is concrete and cannot have unknown extra fields.
        // This is important for catching missing required fields when checking against a known
        // record type.
        let mut record_ty = Type::Record {
            fields: BTreeMap::new(),
            open: false,
        };

        fn closed_record_from_path(path: &[PathSegment], value: Type) -> Type {
            let mut current = value;
            for segment in path.iter().rev() {
                match segment {
                    PathSegment::Field(name) => {
                        let mut fields = BTreeMap::new();
                        fields.insert(name.name.clone(), current);
                        current = Type::Record { fields, open: false };
                    }
                    PathSegment::Index(_, _) | PathSegment::All(_) => {
                        current = Type::con("List").app(vec![current]);
                    }
                }
            }
            current
        }
        for field in fields {
            let value_ty = self.infer_expr(&field.value, env)?;
            if field.spread {
                // `{ ...base, field: value }` composes record types.
                record_ty = self.merge_records(record_ty, value_ty, field.span.clone())?;
            } else {
                let field_ty = closed_record_from_path(&field.path, value_ty);
                record_ty = self.merge_records(record_ty, field_ty, field.span.clone())?;
            }
        }
        Ok(record_ty)
    }

    fn infer_field_access(
        &mut self,
        base: &Expr,
        field: &SpannedName,
        env: &mut TypeEnv,
    ) -> Result<Type, TypeError> {
        let base_ty = self.infer_expr(base, env)?;
        self.record_field_type(
            base_ty,
            &[PathSegment::Field(field.clone())],
            field.span.clone(),
        )
    }

    fn infer_index(
        &mut self,
        base: &Expr,
        index: &Expr,
        env: &mut TypeEnv,
    ) -> Result<Type, TypeError> {
        let base_ty = self.infer_expr(base, env)?;
        let index_ty = self.infer_expr(index, env)?;

        // `x[i]` is overloaded for a few container types.
        // Try `List[Int]` first, then fall back to `Map[key]`.
        let base_subst = self.subst.clone();

        // List indexing: `List A` + `Int` -> `A`
        let list_elem_ty = self.fresh_var();
        if self
            .unify_with_span(index_ty.clone(), Type::con("Int"), expr_span(index))
            .is_ok()
            && self
                .unify_with_span(
                    base_ty.clone(),
                    Type::con("List").app(vec![list_elem_ty.clone()]),
                    expr_span(base),
                )
                .is_ok()
        {
            return Ok(self.apply(list_elem_ty));
        }

        // Reset any constraints added by the failed list attempt.
        self.subst = base_subst;

        // Map indexing: `Map K V` + `K` -> `V`
        let key_ty = self.fresh_var();
        let value_ty = self.fresh_var();
        self.unify_with_span(
            base_ty,
            Type::con("Map").app(vec![key_ty.clone(), value_ty.clone()]),
            expr_span(base),
        )?;
        self.unify_with_span(index_ty, key_ty, expr_span(index))?;
        Ok(self.apply(value_ty))
    }

    fn infer_call(
        &mut self,
        func: &Expr,
        args: &[Expr],
        env: &mut TypeEnv,
    ) -> Result<Type, TypeError> {
        if let Expr::Ident(name) = func {
            if env.get(&name.name).is_none() && self.method_to_classes.contains_key(&name.name) {
                return self.infer_method_call(name, args, env);
            }

            if env
                .get_all(&name.name)
                .is_some_and(|items| items.len() > 1)
            {
                let arg_tys: Vec<Type> = args
                    .iter()
                    .map(|arg| self.infer_expr(arg, env))
                    .collect::<Result<_, _>>()?;

                let Some(candidates) = env.get_all(&name.name) else {
                    return Err(TypeError {
                        span: name.span.clone(),
                        message: format!("unknown name '{}'", name.name),
                        expected: None,
                        found: None,
                    });
                };

                let base_subst = self.subst.clone();
                let mut selected: Option<(Type, std::collections::HashMap<TypeVarId, Type>)> =
                    None;

                for scheme in candidates {
                    self.subst = base_subst.clone();
                    let mut func_ty = self.instantiate(scheme);
                    let mut ok = true;
                    for (arg_ty, arg_expr) in arg_tys.iter().zip(args.iter()) {
                        let result_ty = self.fresh_var();
                        if self
                            .unify_with_span(
                                func_ty.clone(),
                                Type::Func(
                                    Box::new(arg_ty.clone()),
                                    Box::new(result_ty.clone()),
                                ),
                                expr_span(arg_expr),
                            )
                            .is_err()
                        {
                            ok = false;
                            break;
                        }
                        func_ty = result_ty;
                    }
                    if !ok {
                        continue;
                    }
                    let applied = self.apply(func_ty.clone());
                    if selected.is_some() {
                        self.subst = base_subst;
                        return Err(TypeError {
                            span: expr_span(func),
                            message: format!(
                                "ambiguous call to '{}' (multiple overloads match)",
                                name.name
                            ),
                            expected: None,
                            found: None,
                        });
                    }
                    selected = Some((applied, self.subst.clone()));
                }

                if let Some((ty, subst)) = selected {
                    self.subst = subst;
                    return Ok(ty);
                }

                self.subst = base_subst;
                return Err(TypeError {
                    span: expr_span(func),
                    message: format!("no matching overload for '{}'", name.name),
                    expected: None,
                    found: None,
                });
            }
        }

        let mut func_ty = self.infer_expr(func, env)?;
        for arg in args {
            let arg_ty = self.infer_expr(arg, env)?;
            let result_ty = self.fresh_var();
            self.unify_with_span(
                func_ty,
                Type::Func(Box::new(arg_ty), Box::new(result_ty.clone())),
                expr_span(arg),
            )?;
            func_ty = result_ty;
        }
        Ok(func_ty)
    }

    fn infer_method_call(
        &mut self,
        method: &SpannedName,
        args: &[Expr],
        env: &mut TypeEnv,
    ) -> Result<Type, TypeError> {
        let mut arg_tys = Vec::new();
        for arg in args {
            arg_tys.push(self.infer_expr(arg, env)?);
        }

        let Some(classes) = self.method_to_classes.get(&method.name).cloned() else {
            return Err(TypeError {
                span: method.span.clone(),
                message: format!("unknown method '{}'", method.name),
                expected: None,
                found: None,
            });
        };

        let base_subst = self.subst.clone();
        let mut candidates: Vec<(HashMap<TypeVarId, Type>, Type)> = Vec::new();

        for class_name in classes.iter().cloned() {
            let Some(class_info) = self.classes.get(&class_name).cloned() else {
                continue;
            };
            let Some(member_ty_expr) = class_info.members.get(&method.name).cloned() else {
                continue;
            };

            let instances: Vec<InstanceDeclInfo> = self
                .instances
                .iter()
                .filter(|instance| instance.class_name == class_name)
                .cloned()
                .collect();

            for instance in instances {
                if instance.params.len() != class_info.params.len() {
                    continue;
                }

                self.subst = base_subst.clone();

                let mut ctx = TypeContext::new(&self.type_constructors);
                let mut ok = true;
                for (class_param, inst_param) in
                    class_info.params.iter().zip(instance.params.iter())
                {
                    let class_ty = self.type_from_expr(class_param, &mut ctx);
                    let inst_ty = self.type_from_expr(inst_param, &mut ctx);
                    if self
                        .unify_with_span(class_ty, inst_ty, method.span.clone())
                        .is_err()
                    {
                        ok = false;
                        break;
                    }
                }
                if !ok {
                    continue;
                }

                let member_ty = self.type_from_expr(&member_ty_expr, &mut ctx);
                let result_ty = self.fresh_var();
                let mut expected = result_ty.clone();
                for arg_ty in arg_tys.iter().rev() {
                    expected = Type::Func(Box::new(arg_ty.clone()), Box::new(expected));
                }

                if self
                    .unify_with_span(member_ty, expected, method.span.clone())
                    .is_ok()
                {
                    candidates.push((self.subst.clone(), self.apply(result_ty)));
                }
            }
        }

        self.subst = base_subst;
        if candidates.len() == 1 {
            let (subst, result) = candidates.remove(0);
            self.subst = subst;
            return Ok(result);
        }

        // If instance selection fails due to polymorphism, allow the call when a matching class
        // constraint is in scope for one of the argument type variables.
        //
        // This supports class members that require constraints like `with (A: Eq)` where method
        // bodies can call `eq` on `A` without committing to a particular instance upfront.
        let arg_tys_applied: Vec<Type> = arg_tys.into_iter().map(|ty| self.apply(ty)).collect();
        let arg_var_ids: HashSet<TypeVarId> = arg_tys_applied
            .iter()
            .filter_map(|ty| match ty {
                Type::Var(id) => Some(*id),
                _ => None,
            })
            .collect();

        let mut constrained_candidates: Vec<(HashMap<TypeVarId, Type>, Type)> = Vec::new();
        for class_name in classes.iter() {
            if !self
                .assumed_class_constraints
                .iter()
                .any(|(constraint_class, constraint_var)| {
                    constraint_class == class_name && arg_var_ids.contains(constraint_var)
                })
            {
                continue;
            }
            let Some(class_info) = self.classes.get(class_name).cloned() else {
                continue;
            };
            let Some(member_ty_expr) = class_info.members.get(&method.name).cloned() else {
                continue;
            };

            let base_subst = self.subst.clone();
            let mut ctx = TypeContext::new(&self.type_constructors);
            let member_ty = self.type_from_expr(&member_ty_expr, &mut ctx);

            let result_ty = self.fresh_var();
            let mut expected = result_ty.clone();
            for arg_ty in arg_tys_applied.iter().rev() {
                expected = Type::Func(Box::new(arg_ty.clone()), Box::new(expected));
            }

            if self
                .unify_with_span(member_ty, expected, method.span.clone())
                .is_ok()
            {
                constrained_candidates.push((self.subst.clone(), self.apply(result_ty)));
            }
            self.subst = base_subst;
        }

        match (candidates.len(), constrained_candidates.len()) {
            (_, 1) => {
                let (subst, result) = constrained_candidates.remove(0);
                self.subst = subst;
                Ok(result)
            }
            (0, 0) => Err(TypeError {
                span: method.span.clone(),
                message: format!("no instance found for method '{}'", method.name),
                expected: None,
                found: None,
            }),
            (_, 0) => Err(TypeError {
                span: method.span.clone(),
                message: format!("ambiguous instance for method '{}'", method.name),
                expected: None,
                found: None,
            }),
            _ => Err(TypeError {
                span: method.span.clone(),
                message: format!("ambiguous constrained call for method '{}'", method.name),
                expected: None,
                found: None,
            }),
        }
    }

    fn infer_lambda(
        &mut self,
        params: &[Pattern],
        body: &Expr,
        env: &mut TypeEnv,
    ) -> Result<Type, TypeError> {
        let mut local_env = env.clone();
        let mut param_tys = Vec::new();
        for pattern in params {
            let param_ty = self.infer_pattern(pattern, &mut local_env)?;
            param_tys.push(param_ty);
        }
        let mut body_ty = self.infer_expr(body, &mut local_env)?;
        for param_ty in param_tys.into_iter().rev() {
            body_ty = Type::Func(Box::new(param_ty), Box::new(body_ty));
        }
        Ok(body_ty)
    }

    fn infer_match(
        &mut self,
        scrutinee: &Option<Box<Expr>>,
        arms: &[crate::surface::MatchArm],
        match_span: &Span,
        env: &mut TypeEnv,
    ) -> Result<Type, TypeError> {
        let scrutinee_ty = if let Some(scrutinee) = scrutinee {
            self.infer_expr(scrutinee, env)?
        } else {
            self.fresh_var()
        };
        let result_ty = self.fresh_var();
        for arm in arms {
            let mut arm_env = env.clone();
            let pat_ty = self.infer_pattern(&arm.pattern, &mut arm_env)?;
            self.unify_with_span(pat_ty, scrutinee_ty.clone(), arm.span.clone())?;
            if let Some(guard) = &arm.guard {
                let guard_ty = self.infer_expr(guard, &mut arm_env)?;
                self.unify_with_span(guard_ty, Type::con("Bool"), expr_span(guard))?;
            }
            let body_ty = self.infer_expr(&arm.body, &mut arm_env)?;
            self.unify_with_span(body_ty, result_ty.clone(), arm.span.clone())?;
        }
        self.check_match_arms(scrutinee_ty.clone(), arms, match_span);
        // A match without an explicit scrutinee is the multi-clause unary-function sugar:
        //   f =
        //     | Pat1 => expr1
        //     | Pat2 => expr2
        if scrutinee.is_none() {
            Ok(Type::Func(Box::new(scrutinee_ty), Box::new(result_ty)))
        } else {
            Ok(result_ty)
        }
    }

    fn check_match_arms(
        &mut self,
        scrutinee_ty: Type,
        arms: &[crate::surface::MatchArm],
        match_span: &Span,
    ) {
        fn is_catch_all_pattern(pattern: &Pattern) -> bool {
            match pattern {
                Pattern::Wildcard(_) | Pattern::Ident(_) | Pattern::SubjectIdent(_) => true,
                Pattern::At { pattern, .. } => is_catch_all_pattern(pattern),
                _ => false,
            }
        }

        // Unreachable arms: catch-all without a guard makes later arms unreachable.
        let mut has_catch_all: Option<Span> = None;
        // `covered_ctors` tracks constructors that are fully covered by a previous arm
        // (i.e. a constructor arm whose arguments are all wildcards/idents).
        let mut covered_ctors: HashSet<String> = HashSet::new();
        // `seen_ctors` tracks constructors that appear anywhere in the match, regardless of
        // argument patterns, for basic exhaustiveness checking.
        let mut seen_ctors: HashSet<String> = HashSet::new();

        for arm in arms {
            if has_catch_all.is_some() {
                self.emit_extra_diag(
                    "W3101",
                    crate::diagnostics::DiagnosticSeverity::Warning,
                    "unreachable match arm (previous arm matches everything)".to_string(),
                    arm.span.clone(),
                );
                continue;
            }

            let guarded = arm.guard.is_some();
            if is_catch_all_pattern(&arm.pattern) && !guarded {
                has_catch_all = Some(arm.span.clone());
                continue;
            }

            if let Pattern::Constructor { name, ref args, .. } = &arm.pattern {
                let ctor_name = name.name.clone();
                seen_ctors.insert(ctor_name.clone());

                if guarded {
                    continue;
                }

                let ctor_catch_all = args
                    .iter()
                    .all(is_catch_all_pattern);
                if !ctor_catch_all {
                    continue;
                }

                // If a previous arm already fully covered this constructor, this arm is unreachable.
                if covered_ctors.contains(&ctor_name) {
                    self.emit_extra_diag(
                        "W3101",
                        crate::diagnostics::DiagnosticSeverity::Warning,
                        format!(
                            "unreachable match arm (constructor '{}' already matched by a previous arm)",
                            ctor_name
                        ),
                        arm.span.clone(),
                    );
                } else {
                    covered_ctors.insert(ctor_name);
                }
            }
        }

        // Non-exhaustive matches are errors unless there is a catch-all arm.
        if has_catch_all.is_some() {
            return;
        }

        let scrutinee_ty = self.apply(scrutinee_ty);
        let scrutinee_ty = self.expand_alias(scrutinee_ty);
        let expected_ctors: Option<Vec<String>> = match scrutinee_ty {
            Type::Con(ref name, _) if name == "Bool" => {
                Some(vec!["True".to_string(), "False".to_string()])
            }
            Type::Con(ref name, _) if name == "Option" => {
                Some(vec!["None".to_string(), "Some".to_string()])
            }
            Type::Con(ref name, _) if name == "Result" => {
                Some(vec!["Ok".to_string(), "Err".to_string()])
            }
            Type::Con(ref name, _) => self.adt_constructors.get(name).cloned(),
            _ => None,
        };

        let Some(expected_ctors) = expected_ctors else {
            return;
        };

        let mut missing = Vec::new();
        for ctor in expected_ctors {
            if !seen_ctors.contains(&ctor) {
                missing.push(ctor);
            }
        }
        if !missing.is_empty() {
            self.emit_extra_diag(
                "E3100",
                crate::diagnostics::DiagnosticSeverity::Error,
                format!("non-exhaustive match (missing: {})", missing.join(", ")),
                match_span.clone(),
            );
        }
    }

    fn infer_if(
        &mut self,
        cond: &Expr,
        then_branch: &Expr,
        else_branch: &Expr,
        env: &mut TypeEnv,
    ) -> Result<Type, TypeError> {
        let cond_ty = self.infer_expr(cond, env)?;
        self.unify_with_span(cond_ty, Type::con("Bool"), expr_span(cond))?;
        let then_ty = self.infer_expr(then_branch, env)?;
        let else_ty = self.infer_expr(else_branch, env)?;
        self.unify_with_span(then_ty.clone(), else_ty.clone(), expr_span(else_branch))?;
        Ok(then_ty)
    }

    fn infer_binary(
        &mut self,
        op: &str,
        left: &Expr,
        right: &Expr,
        env: &mut TypeEnv,
    ) -> Result<Type, TypeError> {
        if op == "|>" {
            let arg_ty = self.infer_expr(left, env)?;
            let func_ty = self.infer_expr(right, env)?;
            let result_ty = self.fresh_var();
            self.unify_with_span(
                func_ty,
                Type::Func(Box::new(arg_ty), Box::new(result_ty.clone())),
                expr_span(right),
            )?;
            return Ok(result_ty);
        }
        if op == "<|" {
            let target_ty = self.infer_expr(left, env)?;
            if let Expr::Record { fields, .. } | Expr::PatchLit { fields, .. } = right {
                return self.infer_patch(target_ty, fields, env);
            }
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
                // Check if either operand is Float (the other might be a type variable)
                let left_is_float = matches!(left_applied, Type::Con(ref name, _) if name == "Float");
                let right_is_float = matches!(right_applied, Type::Con(ref name, _) if name == "Float");
                let either_float = left_is_float || right_is_float;

                // Float comparison is built-in like Int
                if both_float {
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

                            let key_ty = Type::Func(Box::new(right_ty_for_unify), Box::new(Type::con("Bool")));
                            let key = self.type_to_string(&key_ty);
                            if let Some((existing_key, existing_origin, existing_sig, _)) = &selected {
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
                // Check if either operand is Float (the other might be a type variable)
                let left_is_float = matches!(left_applied, Type::Con(ref name, _) if name == "Float");
                let right_is_float = matches!(right_applied, Type::Con(ref name, _) if name == "Float");
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

                        // Debug: show all candidates
                        if std::env::var("AIVI_DEBUG_DOMAIN").is_ok() {
                            eprintln!("DEBUG: {} candidates for '{}' operator", candidates.len(), op);
                        }

                        // Use subst_after_operands as base so operand types are already constrained
                        let base_subst = subst_after_operands.clone();
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
                                        eprintln!("DEBUG: left_ty var {:?} maps to {:?}", v, mapped);
                                    }
                                }
                            }

                            // Extract the expected left operand type from the operator
                            let op_ty_expanded = self.expand_alias(op_ty.clone());
                            if let Type::Func(op_param, _) = &op_ty_expanded {
                                let op_param_expanded = self.expand_alias((**op_param).clone());
                                // Check if operator expects more fields than operand has
                                if let (
                                    Type::Record { fields: op_fields, .. },
                                    Type::Record { fields: val_fields, .. }
                                ) = (&op_param_expanded, &left_ty_expanded) {
                                    // If operator expects fields that operand doesn't have, skip
                                    let has_extra_fields = op_fields.keys().any(|k| !val_fields.contains_key(k));
                                    if has_extra_fields {
                                        continue;
                                    }
                                    // If operand has fields not required by the operator, this
                                    // overload is for a different carrier type — skip it.
                                    // This prevents Mat4 from spuriously matching Mat2 overloads
                                    // because open record unification would otherwise succeed.
                                    let val_has_extra = val_fields.keys().any(|k| !op_fields.contains_key(k));
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
                                let expected_rhs_expanded = self.expand_alias(expected_rhs_ty.clone());
                                let right_ty_raw = self.apply(right_ty.clone());
                                let right_ty_expanded = self.expand_alias(right_ty_raw);
                                if let (
                                    Type::Record { fields: exp_fields, .. },
                                    Type::Record { fields: actual_fields, .. },
                                ) = (&expected_rhs_expanded, &right_ty_expanded)
                                {
                                    let exp_has_extra = exp_fields.keys().any(|k| !actual_fields.contains_key(k));
                                    let actual_has_extra = actual_fields.keys().any(|k| !exp_fields.contains_key(k));
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
                            selected = Some((match_key, origin, sig, result_ty, self.subst.clone()));
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
            BlockKind::Do { monad } => self.infer_generic_do_block(&monad.name, &monad.span, items, env),
            BlockKind::Generate => self.infer_generate_block(items, env),
            BlockKind::Resource => self.infer_resource_block(items, env),
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
                    self.unify_with_span(pat_ty, expr_ty, pattern_span(pattern))?;
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
                    self.unify_with_span(pat_ty, expr_ty, pattern_span(pattern))?;
                }
                BlockItem::Filter { expr, .. }
                | BlockItem::Yield { expr, .. }
                | BlockItem::Recurse { expr, .. }
                | BlockItem::Expr { expr, .. } => {
                    last_ty = self.infer_expr(expr, &mut local_env)?;
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
                    last_ty = self.infer_expr(handler, &mut local_env)?;
                }
            }
        }
        Ok(last_ty)
    }
}
