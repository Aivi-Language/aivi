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
        } else if self.method_to_classes.contains_key(&name.name) {
            // Zero-argument class member used without enough type context.
            // Bidirectional resolution (via check_or_coerce) handles this when
            // an expected type is available. Without context, report a helpful error.
            let classes = self.method_to_classes[&name.name].join(", ");
            Err(TypeError {
                span: name.span.clone(),
                message: format!(
                    "cannot resolve class member '{}' (from {}) without type context — add a type annotation or use a qualified form (e.g. List.{})",
                    name.name, classes, name.name
                ),
                expected: None,
                found: None,
            })
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
                "tz" => Type::con("TimeZone"),
                "zdt" => Type::con("ZonedDateTime"),
                "k" => Type::con("Key"),
                "m" => Type::con("Message"),
                "raw" => Type::con("Text"),
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
        };

        fn closed_record_from_path(path: &[PathSegment], value: Type) -> Type {
            let mut current = value;
            for segment in path.iter().rev() {
                match segment {
                    PathSegment::Field(name) => {
                        let mut fields = BTreeMap::new();
                        fields.insert(name.name.clone(), current);
                        current = Type::Record {
                            fields,
                        };
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
        let resolved = self.apply(base_ty.clone());
        if let Some(type_name) = self.opaque_con_name(&resolved) {
            if let Some(defining_module) = self.is_opaque_from_here(&type_name).cloned() {
                return Err(TypeError {
                    span: field.span.clone(),
                    message: format!(
                        "cannot access field `{}` on opaque type `{}` outside module `{}`",
                        field.name, type_name, defining_module
                    ),
                    expected: None,
                    found: None,
                });
            }
        }
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
                return self.infer_method_call(name, args, None, env);
            }

            if env
                .get_all(&name.name)
                .is_some_and(|items| items.len() > 1)
            {
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

                let base_subst = self.subst.clone();
                let mut selected: Option<(Type, std::collections::HashMap<TypeVarId, Type>)> =
                    None;

                for scheme in candidates {
                    self.subst = base_subst.clone();
                    let mut func_ty = self.instantiate(scheme);
                    let mut ok = true;
                    for (arg_ty, arg_expr) in arg_tys.iter().zip(args.iter()) {
                        // Structurally check record field sets before unification.
                        // An open record `{ x, y, .. }` should NOT match a candidate
                        // expecting `{ x, y, z }` — the extra field disqualifies it.
                        let func_ty_applied = self.apply(func_ty.clone());
                        let func_ty_expanded = self.expand_alias(func_ty_applied);
                        if let Type::Func(ref param, _) = func_ty_expanded {
                            let param_expanded = self.expand_alias((**param).clone());
                            let arg_applied = self.apply(arg_ty.clone());
                            let arg_expanded = self.expand_alias(arg_applied);
                            if let (
                                Type::Record { fields: param_fields, .. },
                                Type::Record { fields: arg_fields, .. },
                            ) = (&param_expanded, &arg_expanded)
                            {
                                let param_has_extra = param_fields.keys().any(|k| !arg_fields.contains_key(k));
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

        // Detect polymorphic identifier for call-site type recording (monomorphization)
        let poly_call_info: Option<String> = if let Expr::Ident(name) = func {
            env.get(&name.name)
                .filter(|s| !s.vars.is_empty())
                .map(|s| {
                    let prefix = s
                        .origin
                        .as_ref()
                        .map(|o| o.render())
                        .unwrap_or_else(|| self.current_module_name.clone());
                    format!("{}.{}", prefix, name.name)
                })
        } else {
            None
        };

        let mut func_ty = self.infer_expr(func, env)?;
        let original_func_ty = if poly_call_info.is_some() {
            Some(func_ty.clone())
        } else {
            None
        };
        let mut resolved_arg_tys = Vec::with_capacity(args.len());
        for arg in args {
            let param_ty = self.fresh_var();
            let result_ty = self.fresh_var();
            self.unify_with_span(
                func_ty,
                Type::Func(Box::new(param_ty.clone()), Box::new(result_ty.clone())),
                expr_span(arg),
            )?;
            let expected_arg_ty = self.apply(param_ty.clone());
            let arg_checkpoint = self.subst.clone();
            let arg_ty = match self.check_or_coerce(arg.clone(), Some(expected_arg_ty), env) {
                Ok((_arg_expr, arg_ty)) => arg_ty,
                Err(original_err) if original_err.message.starts_with("unknown name '") => {
                    self.subst = arg_checkpoint.clone();
                    match self.infer_arg_with_predicate_fallback(arg, env) {
                        Ok(arg_ty) => arg_ty,
                        Err(_) => {
                            self.subst = arg_checkpoint;
                            return Err(original_err);
                        }
                    }
                }
                Err(err) => return Err(err),
            };
            self.unify_with_span(arg_ty, param_ty.clone(), expr_span(arg))?;
            resolved_arg_tys.push(self.apply(param_ty));
            func_ty = result_ty;
        }
        self.validate_query_call_args(func, args, &resolved_arg_tys, env)?;
        if let (Some(qname), Some(orig_ty)) = (poly_call_info, original_func_ty) {
            let resolved = self.apply(orig_ty.clone());
            // Record source schemas for `load` calls: extract the inner type `A`
            // from `Source K A → Effect (SourceError K) A`.
            if let Expr::Ident(ref ident) = func {
                if ident.name == "load" {
                    if let Type::Func(ref param, _) = resolved {
                        if let Type::Con(ref name, ref sargs) = **param {
                            if name == "Source" && sargs.len() == 2 {
                                let env_ref: &crate::typecheck::types::TypeEnv = env;
                                let inner_cg = self.type_to_cg_type(&sargs[1], env_ref);
                                if inner_cg != CgType::Dynamic && inner_cg.is_closed() {
                                    self.load_source_schemas.push((
                                        self.current_module_name.clone(),
                                        self.current_def_name.clone(),
                                        inner_cg,
                                    ));
                                }
                            }
                        }
                    }
                }
            }
            self.poly_instantiations.push((qname, resolved));
        }
        Ok(func_ty)
    }

    fn infer_arg_with_predicate_fallback(
        &mut self,
        arg: &Expr,
        env: &mut TypeEnv,
    ) -> Result<Type, TypeError> {
        let checkpoint = self.subst.clone();
        match self.infer_expr(arg, env) {
            Ok(ty) => Ok(ty),
            Err(original_err) => {
                self.subst = checkpoint.clone();
                let Some(rewritten) = lift_predicate_expr(arg, env, "__pred") else {
                    return Err(original_err);
                };
                let Ok(rewritten_ty) = self.infer_expr(&rewritten, env) else {
                    self.subst = checkpoint;
                    return Err(original_err);
                };
                Ok(rewritten_ty)
            }
        }
    }

    fn infer_method_call(
        &mut self,
        method: &SpannedName,
        args: &[Expr],
        expected_result: Option<Type>,
        env: &mut TypeEnv,
    ) -> Result<Type, TypeError> {
        let mut arg_tys = Vec::new();
        for arg in args {
            arg_tys.push(self.infer_arg_with_predicate_fallback(arg, env)?);
        }

        let debug = std::env::var("AIVI_DEBUG_METHOD").is_ok_and(|v| v == method.name);
        if debug {
            let arg_strs: Vec<String> = arg_tys.iter().map(|t| self.type_to_string(t)).collect();
            eprintln!("[METHOD_DEBUG] infer_method_call({}) args=[{}] expected={:?}",
                method.name, arg_strs.join(", "),
                expected_result.as_ref().map(|t| self.type_to_string(t)));
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

                // Use separate TypeContexts for class params and instance params.
                // Sharing a single context causes occurs-check failures for non-HKT
                // classes (e.g. `Semigroup A` with instance `Semigroup (List A)`):
                // the class-param var `a` would be unified with `Con("List", [Var(a)])`,
                // triggering an occurs check. By keeping distinct vars per side, we
                // unify `Var(a_class)` with `Con("List", [Var(a_inst)])` cleanly.
                // The member-type context is built from the class-param vars so that
                // substituting after unification yields the correct specialised type.
                let mut ctx_class = TypeContext::new(&self.type_constructors);
                let mut ctx_inst = TypeContext::new(&self.type_constructors);
                let mut ok = true;
                for (class_param, inst_param) in
                    class_info.params.iter().zip(instance.params.iter())
                {
                    let class_ty = self.type_from_expr(class_param, &mut ctx_class);
                    let inst_ty = self.type_from_expr(inst_param, &mut ctx_inst);
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

                let member_ty = self.type_from_expr(&member_ty_expr, &mut ctx_class);
                let result_ty = self.fresh_var();
                let mut expected = result_ty.clone();
                for arg_ty in arg_tys.iter().rev() {
                    expected = Type::Func(Box::new(arg_ty.clone()), Box::new(expected));
                }

                if self
                    .unify_with_span(member_ty, expected, method.span.clone())
                    .is_ok()
                {
                    let mut resolved_result = self.apply(result_ty);
                    if let Some(expected_ty) = expected_result.clone() {
                        if self
                            .unify_with_span(
                                resolved_result.clone(),
                                expected_ty.clone(),
                                method.span.clone(),
                            )
                            .is_err()
                        {
                            continue;
                        }
                        resolved_result = self.apply(expected_ty);
                    }
                    if debug {
                        eprintln!("[METHOD_DEBUG]   CANDIDATE from instance {:?} result={}", instance.params.iter().map(|p| format!("{p:?}")).collect::<Vec<_>>(), self.type_to_string(&resolved_result));
                    }
                    candidates.push((self.subst.clone(), resolved_result));
                }
            }
        }

        self.subst = base_subst;
        if debug {
            eprintln!("[METHOD_DEBUG] candidates.len()={}", candidates.len());
        }
        if candidates.len() == 1 {
            let (subst, result) = candidates.remove(0);
            self.subst = subst;
            return Ok(result);
        }

        // If instance selection fails due to polymorphism, allow the call when a matching class
        // constraint is in scope for one of the argument type variables.
        //
        // This supports class members that require constraints like `given (A: Eq)` where method
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
                let mut resolved_result = self.apply(result_ty);
                if let Some(expected_ty) = expected_result.clone() {
                    if self
                        .unify_with_span(
                            resolved_result.clone(),
                            expected_ty.clone(),
                            method.span.clone(),
                        )
                        .is_err()
                    {
                        self.subst = base_subst;
                        continue;
                    }
                    resolved_result = self.apply(expected_ty);
                }
                constrained_candidates.push((self.subst.clone(), resolved_result));
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
        // Check for opaque type pattern matching violations.
        let resolved_scrutinee = self.apply(scrutinee_ty.clone());
        if let Some(type_name) = self.opaque_con_name(&resolved_scrutinee) {
            if let Some(defining_module) = self.is_opaque_from_here(&type_name).cloned() {
                for arm in arms {
                    if self.pattern_destructures(&arm.pattern) {
                        return Err(TypeError {
                            span: arm.span.clone(),
                            message: format!(
                                "cannot pattern match on opaque type `{}` outside module `{}`",
                                type_name, defining_module
                            ),
                            expected: None,
                            found: None,
                        });
                    }
                }
            }
        }
        let result_ty = self.fresh_var();
        for arm in arms {
            let mut arm_env = env.clone();
            let pat_ty = self.infer_pattern(&arm.pattern, &mut arm_env)?;
            self.unify_with_span(pat_ty, scrutinee_ty.clone(), arm.span.clone())?;
            self.check_pattern_linear(&arm.pattern);
            self.check_record_pattern_extra_fields(&arm.pattern, &scrutinee_ty);
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

    fn check_pattern_linear(&mut self, pattern: &Pattern) {
        fn collect_names<'a>(p: &'a Pattern, out: &mut Vec<(&'a str, Span)>) {
            match p {
                Pattern::Ident(name) | Pattern::SubjectIdent(name) => {
                    out.push((&name.name, name.span.clone()));
                }
                Pattern::At { name, pattern, .. } => {
                    out.push((&name.name, name.span.clone()));
                    collect_names(pattern, out);
                }
                Pattern::Constructor { args, .. } => {
                    for arg in args {
                        collect_names(arg, out);
                    }
                }
                Pattern::Tuple { items, .. } => {
                    for item in items {
                        collect_names(item, out);
                    }
                }
                Pattern::List { items, rest, .. } => {
                    for item in items {
                        collect_names(item, out);
                    }
                    if let Some(rest) = rest {
                        collect_names(rest, out);
                    }
                }
                Pattern::Record { fields, .. } => {
                    for field in fields {
                        collect_names(&field.pattern, out);
                    }
                }
                Pattern::Wildcard(_) | Pattern::Literal(_) => {}
            }
        }

        let mut names: Vec<(&str, Span)> = Vec::new();
        collect_names(pattern, &mut names);

        let mut seen: HashSet<&str> = HashSet::new();
        for (name, span) in &names {
            if !seen.insert(name) {
                self.emit_extra_diag(
                    "E3102",
                    crate::diagnostics::DiagnosticSeverity::Error,
                    format!("non-linear pattern: '{}' is bound more than once", name),
                    span.clone(),
                );
            }
        }
    }

    fn check_record_pattern_extra_fields(&mut self, pattern: &Pattern, scrutinee_ty: &Type) {
        let expanded = self.expand_alias(scrutinee_ty.clone());
        let resolved = self.apply(expanded);
        let Type::Record { fields: scr_fields } = &resolved else {
            return;
        };
        if scr_fields.is_empty() {
            return;
        }
        let Pattern::Record { fields: pat_fields, .. } = pattern else {
            return;
        };
        for field in pat_fields {
            if let Some(first) = field.path.first() {
                if !scr_fields.contains_key(&first.name) {
                    self.emit_extra_diag(
                        "E3000",
                        crate::diagnostics::DiagnosticSeverity::Error,
                        format!("record has no field '{}'", first.name),
                        first.span.clone(),
                    );
                }
            }
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

        // List pattern tracking: exact-length patterns (e.g. `[]`, `[x]`) and
        // the lowest N for a `[_, ..., _, ...rest]` pattern (covers length N+).
        let mut list_exact_lengths: HashSet<usize> = HashSet::new();
        let mut list_min_rest: Option<usize> = None;

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

            if !guarded {
                if let Pattern::List { items, rest, .. } = &arm.pattern {
                    if rest.is_some() {
                        let min = items.len();
                        list_min_rest = Some(match list_min_rest {
                            Some(prev) => prev.min(min),
                            None => min,
                        });
                    } else {
                        list_exact_lengths.insert(items.len());
                    }
                }
            }
        }

        // Non-exhaustive matches are errors unless there is a catch-all arm.
        if has_catch_all.is_some() {
            return;
        }

        let scrutinee_ty = self.apply(scrutinee_ty);
        let scrutinee_ty = self.expand_alias(scrutinee_ty);

        // List exhaustiveness: `[]` + `[x, ...rest]` covers all lists.
        if let Type::Con(ref name, _) = scrutinee_ty {
            if name == "List" && (list_min_rest.is_some() || !list_exact_lengths.is_empty()) {
                if let Some(min_rest) = list_min_rest {
                    let all_covered =
                        (0..min_rest).all(|len| list_exact_lengths.contains(&len));
                    if all_covered {
                        return;
                    }
                    let missing: Vec<String> = (0..min_rest)
                        .filter(|len| !list_exact_lengths.contains(len))
                        .map(|len| {
                            if len == 0 {
                                "[]".to_string()
                            } else {
                                format!("list of exactly {} element(s)", len)
                            }
                        })
                        .collect();
                    self.emit_extra_diag(
                        "E3100",
                        crate::diagnostics::DiagnosticSeverity::Error,
                        format!(
                            "non-exhaustive list match (missing: {})",
                            missing.join(", ")
                        ),
                        match_span.clone(),
                    );
                    return;
                }
                // Only exact-length list patterns, no `...rest` — never exhaustive.
                self.emit_extra_diag(
                    "W3102",
                    crate::diagnostics::DiagnosticSeverity::Warning,
                    "non-exhaustive list match: add a `[..., ...rest]` or `_` arm to cover all lengths"
                        .to_string(),
                    match_span.clone(),
                );
                return;
            }
        }

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
            // Non-enum scrutinee (e.g. Text, Int) with pattern arms but no catch-all:
            // this match will fail at runtime for any unmatched value.
            if !arms.is_empty() {
                self.emit_extra_diag(
                    "W3102",
                    crate::diagnostics::DiagnosticSeverity::Warning,
                    "match without catch-all `_` arm on a non-enum type may fail at runtime"
                        .to_string(),
                    match_span.clone(),
                );
            }
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
        let snapshot = self.subst.clone();
        if self
            .unify_with_span(then_ty.clone(), else_ty.clone(), expr_span(else_branch))
            .is_err()
        {
            // If one branch is `Effect E Unit` and the other is bare `Unit`, coerce the `Unit`
            // branch to `Effect E Unit` so that `if cond then someEffect else Unit` type-checks.
            self.subst = snapshot;
            let then_applied = self.apply(then_ty.clone());
            let else_applied = self.apply(else_ty.clone());
            let then_is_unit = matches!(&then_applied, Type::Con(n, a) if n == "Unit" && a.is_empty());
            let else_is_unit = matches!(&else_applied, Type::Con(n, a) if n == "Unit" && a.is_empty());
            let effect_ty = if else_is_unit && Self::is_fully_applied_effect(&then_applied) {
                Some(then_applied)
            } else if then_is_unit && Self::is_fully_applied_effect(&else_applied) {
                Some(else_applied)
            } else {
                None
            };
            if let Some(eff) = effect_ty {
                return Ok(eff);
            }
            // No coercion possible — re-run to produce the diagnostic.
            self.unify_with_span(then_ty.clone(), else_ty, expr_span(else_branch))?;
        }
        Ok(then_ty)
    }

    fn is_fully_applied_effect(ty: &Type) -> bool {
        match ty {
            Type::Con(name, args) => name == "Effect" && args.len() == 2,
            Type::App(base, args) => {
                if let Type::Con(name, existing) = &**base {
                    if name == "Effect" {
                        return existing.len() + args.len() == 2;
                    }
                }
                false
            }
            _ => false,
        }
    }
}

include!("infer_expr/binary_and_blocks.rs");
