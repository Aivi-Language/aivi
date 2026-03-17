impl TypeChecker {
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
                            let Some(rewritten) = lift_predicate_expr(&arg, env, &self.method_to_classes, "__pred") else {
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

        let prev_in_call_arg = self.in_call_arg;
        self.in_call_arg = true;

        if allow_predicate_try {
            if let Some(rewritten) = lift_predicate_expr(&arg, env, &self.method_to_classes, "__pred") {
                let checkpoint = self.subst.clone();
                if let Ok((elab_arg, elab_ty)) =
                    self.elab_expr(rewritten, Some(expected_arg_ty.clone()), env)
                {
                    let elab_applied_inner = self.apply(elab_ty.clone());
                    let elab_applied = self.expand_alias(elab_applied_inner);
                    if matches!(elab_applied, Type::Func(_, _)) {
                        self.in_call_arg = prev_in_call_arg;
                        return Ok((elab_arg, elab_ty));
                    }
                }
                self.subst = checkpoint;
            }
        }

        let result = self.elab_expr(arg, Some(expected_arg_ty), env);
        self.in_call_arg = prev_in_call_arg;
        result
    }

    fn expected_patch_target_ty(&mut self, expected: Option<&Type>) -> Option<Type> {
        let applied = self.apply(expected?.clone());
        match applied {
            Type::Con(name, args) if self.type_name_matches(&name, "Patch") && args.len() == 1 => {
                args.into_iter().next()
            }
            _ => None,
        }
    }

    fn elab_record(
        &mut self,
        fields: Vec<RecordField>,
        span: Span,
        expected: Option<Type>,
        env: &mut TypeEnv,
    ) -> Result<(Expr, Type), TypeError> {
        if let Some(target_ty) = self.expected_patch_target_ty(expected.as_ref()) {
            self.infer_patch(target_ty, &fields, env)?;
            return self.check_or_coerce(Expr::PatchLit { fields, span }, expected, env);
        }

        let expected_ty = if let Some(ty) = expected.as_ref() {
            let applied = self.apply(ty.clone());
            Some(self.expand_alias(applied))
        } else {
            None
        };

        let fields = self.prepend_missing_record_defaults(fields, expected_ty.as_ref(), &span);

        // When elaborating a function call argument (in_call_arg) OR when record
        // defaults are enabled (`use aivi.defaults`), check for fields that are STILL
        // missing after default synthesis.  These have no valid default and will crash
        // at runtime (e.g. function-typed props like `onChange`, or any field when
        // defaults are not imported).
        // Skip the check when a spread is present — it may supply the missing fields.
        if self.in_call_arg || !self.enabled_record_default_types.is_empty() {
            if let Some(Type::Record {
                fields: ref expected_fields,
            }) = expected_ty
            {
                let has_spread = fields.iter().any(|f| f.spread);
                if !has_spread {
                    let present: HashSet<&str> = fields
                        .iter()
                        .filter_map(|f| match f.path.first() {
                            Some(PathSegment::Field(name)) => Some(name.name.as_str()),
                            _ => None,
                        })
                        .collect();
                    let missing: Vec<&str> = expected_fields
                        .keys()
                        .filter(|k| !present.contains(k.as_str()))
                        .map(|k| k.as_str())
                        .collect();
                    if !missing.is_empty() {
                        return Err(TypeError {
                            span: span.clone(),
                            message: format!(
                                "missing required field{} {} in record",
                                if missing.len() > 1 { "s" } else { "" },
                                missing
                                    .iter()
                                    .map(|n| format!("'{n}'"))
                                    .collect::<Vec<_>>()
                                    .join(", "),
                            ),
                            expected: None,
                            found: None,
                        });
                    }
                }
            }
        }

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

        // Bare class-member identifiers (zero args) — use the expected type to
        // pick the right instance.  This is the bidirectional path that makes
        // `empty` (Monoid), `id` (Category), etc. usable without explicit type
        // application: the expected type flows down from annotations, argument
        // positions, or constraint propagation and disambiguates the instance.
        //
        // After type-checking succeeds, we inline the matching instance's body
        // expression so the runtime gets the concrete value (e.g. `[]` for
        // Monoid List) instead of a bare `Ident("empty")` that can't dispatch.
        if let (Some(expected_ty), Expr::Ident(name)) = (expected.clone(), &expr) {
            if env.get(&name.name).is_none()
                && env
                    .get_all(&name.name)
                    .is_none_or(|items| items.len() <= 1)
                && self.method_to_classes.contains_key(&name.name)
            {
                let checkpoint = self.subst.clone();
                match self.infer_method_call(name, &[], Some(expected_ty.clone()), env) {
                    Ok(inferred) => {
                        // Find the matching instance and inline its body so the
                        // runtime gets the concrete value instead of the ambiguous
                        // bare identifier.
                        if let Some(body) =
                            self.find_instance_member_body(&name.name, &inferred)
                        {
                            return self.elab_expr(body, Some(inferred), env);
                        }
                        return Ok((expr, inferred));
                    }
                    Err(_) => self.subst = checkpoint,
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
                if let Some(rewritten) = lift_predicate_expr(&expr, env, &self.method_to_classes, "__pred") {
                    let checkpoint = self.subst.clone();
                    if let Ok(rewritten_ty) = self.infer_expr(&rewritten, env) {
                        if self
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

        if let (Some(target_ty), Expr::Record { fields, span }) =
            (self.expected_patch_target_ty(expected.as_ref()), &expr)
        {
            self.infer_patch(target_ty, fields, env)?;
            return self.check_or_coerce(
                Expr::PatchLit {
                    fields: fields.clone(),
                    span: span.clone(),
                },
                expected,
                env,
            );
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
            Type::Con(ref name, ref args) if self.type_name_matches(name, "VNode") && args.len() == 1
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

        // Coerce into `Body`: record → Json (toJson record), Text → Plain, JsonValue → Json.
        let is_body = matches!(
            expected_applied,
            Type::Con(ref name, ref args) if name == "Body" && args.is_empty()
        );
        if is_body {
            let inferred_applied = self.apply(inferred.clone());
            let inferred_expanded = self.expand_alias(inferred_applied);

            // Record literal → Json (toJson record)
            if matches!(expr, Expr::Record { .. }) {
                let to_json = Expr::Ident(SpannedName {
                    name: "toJson".into(),
                    span: expr_span(&expr),
                });
                let to_json_call = Expr::Call {
                    func: Box::new(to_json),
                    args: vec![expr.clone()],
                    span: expr_span(&expr),
                };
                let json_ctor = Expr::Ident(SpannedName {
                    name: "Json".into(),
                    span: expr_span(&expr),
                });
                let wrapped = Expr::Call {
                    func: Box::new(json_ctor),
                    args: vec![to_json_call],
                    span: expr_span(&expr),
                };
                let wrapped_ty = self.infer_expr(&wrapped, env)?;
                let checkpoint = self.subst.clone();
                if self
                    .unify_with_span(wrapped_ty, expected.clone(), expr_span(&wrapped))
                    .is_ok()
                {
                    return Ok((wrapped, self.apply(expected)));
                }
                self.subst = checkpoint;
            }

            // Text → Plain text
            if matches!(inferred_expanded, Type::Con(ref n, ref a) if n == "Text" && a.is_empty())
            {
                let plain_ctor = Expr::Ident(SpannedName {
                    name: "Plain".into(),
                    span: expr_span(&expr),
                });
                let wrapped = Expr::Call {
                    func: Box::new(plain_ctor),
                    args: vec![expr.clone()],
                    span: expr_span(&expr),
                };
                let wrapped_ty = self.infer_expr(&wrapped, env)?;
                let checkpoint = self.subst.clone();
                if self
                    .unify_with_span(wrapped_ty, expected.clone(), expr_span(&wrapped))
                    .is_ok()
                {
                    return Ok((wrapped, self.apply(expected)));
                }
                self.subst = checkpoint;
            }

            // JsonValue → Json jv
            if matches!(inferred_expanded, Type::Con(ref n, ref a) if n == "JsonValue" && a.is_empty())
            {
                let json_ctor = Expr::Ident(SpannedName {
                    name: "Json".into(),
                    span: expr_span(&expr),
                });
                let wrapped = Expr::Call {
                    func: Box::new(json_ctor),
                    args: vec![expr.clone()],
                    span: expr_span(&expr),
                };
                let wrapped_ty = self.infer_expr(&wrapped, env)?;
                let checkpoint = self.subst.clone();
                if self
                    .unify_with_span(wrapped_ty, expected.clone(), expr_span(&wrapped))
                    .is_ok()
                {
                    return Ok((wrapped, self.apply(expected)));
                }
                self.subst = checkpoint;
            }
        }

        // Option coercion: when expected is `Option A`, try coercing the expression
        // to `A` and wrap in `Some`. This enables e.g. `body: { ... }` where
        // `Option Body` is expected.
        if let Type::Con(ref opt_name, ref opt_args) = expected_applied {
            if opt_name == "Option" && opt_args.len() == 1 {
                let inner_expected = opt_args[0].clone();
                let option_checkpoint = self.subst.clone();
                if let Ok((coerced_expr, _)) =
                    self.check_or_coerce(expr.clone(), Some(inner_expected), env)
                {
                    let some_ctor = Expr::Ident(SpannedName {
                        name: "Some".into(),
                        span: expr_span(&coerced_expr),
                    });
                    let wrapped = Expr::Call {
                        func: Box::new(some_ctor),
                        args: vec![coerced_expr],
                        span: expr_span(&expr),
                    };
                    let wrapped_ty = self.infer_expr(&wrapped, env)?;
                    let checkpoint = self.subst.clone();
                    if self
                        .unify_with_span(wrapped_ty, expected.clone(), expr_span(&wrapped))
                        .is_ok()
                    {
                        return Ok((wrapped, self.apply(expected)));
                    }
                    self.subst = checkpoint;
                }
                self.subst = option_checkpoint;
            }
        }

        // Fall back to the original mismatch (without keeping any partial unifications).
        self.subst = base_subst;
        self.unify_with_span(inferred, expected.clone(), expr_span(&expr))?;
        Ok((expr, self.apply(expected)))
    }
}
