impl TypeChecker {
    fn check_instance_decl(
        &mut self,
        instance: &crate::surface::InstanceDecl,
        env: &TypeEnv,
        module: &Module,
        diagnostics: &mut Vec<FileDiagnostic>,
    ) {
        let Some(class_info) = self.classes.get(&instance.name.name).cloned() else {
            diagnostics.push(self.error_to_diag(
                module,
                TypeError {
                    span: instance.span.clone(),
                    message: format!("unknown class '{}'", instance.name.name),
                    expected: None,
                    found: None,
                },
            ));
            return;
        };

        if instance.params.len() != class_info.params.len() {
            diagnostics.push(self.error_to_diag(
                module,
                TypeError {
                    span: instance.span.clone(),
                    message: format!(
                        "instance '{}' expects {} parameter(s), found {}",
                        instance.name.name,
                        class_info.params.len(),
                        instance.params.len()
                    ),
                    expected: None,
                    found: None,
                },
            ));
            return;
        }

        let mut defs_by_name: HashMap<String, &Def> = HashMap::new();
        for def in &instance.defs {
            if defs_by_name.insert(def.name.name.clone(), def).is_some() {
                diagnostics.push(self.error_to_diag(
                    module,
                    TypeError {
                        span: def.span.clone(),
                        message: format!("duplicate instance method '{}'", def.name.name),
                        expected: None,
                        found: None,
                    },
                ));
            }
        }

        for (member_name, member_sig) in class_info.members.iter() {
            let Some(def) = defs_by_name.get(member_name).copied() else {
                // If the member is inherited via a superclass constraint, allow the instance
                // to omit it as long as a matching superclass instance is in scope.
                if self.instance_method_satisfied_by_super(instance, &class_info, member_name) {
                    continue;
                }

                diagnostics.push(self.error_to_diag(
                    module,
                    TypeError {
                        span: instance.span.clone(),
                        message: format!("missing instance method '{}'", member_name),
                        expected: None,
                        found: None,
                    },
                ));
                continue;
            };

            let base_subst = self.subst.clone();
            let mut ctx = TypeContext::new(&self.type_constructors);
            for (class_param, inst_param) in class_info.params.iter().zip(instance.params.iter()) {
                let class_ty = self.type_from_expr(class_param, &mut ctx);
                let inst_ty = self.type_from_expr(inst_param, &mut ctx);
                if let Err(err) = self.unify_with_span(class_ty, inst_ty, instance.span.clone()) {
                    diagnostics.push(self.error_to_diag(module, err));
                    self.subst = base_subst;
                    return;
                }
            }
            let expected = self.type_from_expr(member_sig, &mut ctx);

            let expr = crate::surface::desugar_effect_sugars(desugar_holes(def.expr.clone()));
            let mut local_env = env.clone();
            local_env.insert(def.name.name.clone(), Scheme::mono(expected.clone()));

            let assumed_constraints: Vec<(String, TypeVarId)> = class_info
                .constraints
                .iter()
                .filter_map(|(var_name, class_name)| {
                    ctx.type_vars
                        .get(var_name)
                        .map(|id| (class_name.clone(), *id))
                })
                .collect();
            let old_assumed =
                std::mem::replace(&mut self.assumed_class_constraints, assumed_constraints);

            let result: Result<(), TypeError> = (|| {
                // Instance methods are often written as `name: x y => ...` (a lambda expression).
                // When an expected member type exists, unify parameter patterns against it so
                // member-level type-variable constraints can participate in overload checking.
                if !def.params.is_empty() {
                    let mut remaining = expected.clone();
                    for param in &def.params {
                        let remaining_applied = self.apply(remaining);
                        let remaining_norm = self.expand_alias(remaining_applied);
                        let Type::Func(expected_param, expected_rest) = remaining_norm else {
                            return Err(TypeError {
                                span: def.span.clone(),
                                message: format!(
                                    "expected function type for instance method '{}'",
                                    def.name.name
                                ),
                                expected: None,
                                found: None,
                            });
                        };
                        let pat_ty = self.infer_pattern(param, &mut local_env)?;
                        self.unify_with_span(pat_ty, *expected_param, pattern_span(param))?;
                        remaining = *expected_rest;
                    }
                    let body_ty = self.infer_expr(&expr, &mut local_env)?;
                    self.unify_with_span(body_ty, remaining, expr_span(&expr))?;
                    return Ok(());
                }

                if let Expr::Lambda { params, body, .. } = &expr {
                    let mut remaining = expected.clone();
                    for param in params {
                        let remaining_applied = self.apply(remaining);
                        let remaining_norm = self.expand_alias(remaining_applied);
                        let Type::Func(expected_param, expected_rest) = remaining_norm else {
                            return Err(TypeError {
                                span: def.span.clone(),
                                message: format!(
                                    "expected function type for instance method '{}'",
                                    def.name.name
                                ),
                                expected: None,
                                found: None,
                            });
                        };
                        let pat_ty = self.infer_pattern(param, &mut local_env)?;
                        self.unify_with_span(pat_ty, *expected_param, pattern_span(param))?;
                        remaining = *expected_rest;
                    }
                    let body_ty = self.infer_expr(body, &mut local_env)?;
                    self.unify_with_span(body_ty, remaining, expr_span(body))?;
                    return Ok(());
                }

                let inferred = self.infer_expr(&expr, &mut local_env)?;
                self.unify_with_span(inferred, expected, def.span.clone())?;
                Ok(())
            })();

            self.assumed_class_constraints = old_assumed;
            if let Err(err) = result {
                diagnostics.push(self.error_to_diag(module, err));
            }

            self.subst = base_subst;
        }

        for method_name in defs_by_name.keys() {
            if !class_info.members.contains_key(method_name) {
                diagnostics.push(self.error_to_diag(
                    module,
                    TypeError {
                        span: instance.span.clone(),
                        message: format!("unknown instance method '{}'", method_name),
                        expected: None,
                        found: None,
                    },
                ));
            }
        }
    }

    fn instance_method_satisfied_by_super(
        &mut self,
        instance: &crate::surface::InstanceDecl,
        class_info: &ClassDeclInfo,
        missing_member: &str,
    ) -> bool {
        // Only methods provided by superclass constraints may be delegated.
        let direct_supers = self.flatten_type_and_list(&class_info.supers);
        for super_expr in direct_supers {
            let Some((super_name, super_params)) = self.class_ref_from_type_expr(&super_expr)
            else {
                continue;
            };
            let Some(super_info) = self.classes.get(super_name) else {
                continue;
            };
            if !super_info.members.contains_key(missing_member) {
                continue;
            }
            // Instantiate the superclass parameters by unifying the class parameters with the
            // concrete instance parameters, then applying the resulting substitution.
            let base_subst = self.subst.clone();
            let mut ctx = TypeContext::new(&self.type_constructors);
            for (class_param, inst_param) in class_info.params.iter().zip(instance.params.iter()) {
                let class_ty = self.type_from_expr(class_param, &mut ctx);
                let inst_ty = self.type_from_expr(inst_param, &mut ctx);
                if self
                    .unify(class_ty, inst_ty, instance.span.clone())
                    .is_err()
                {
                    self.subst = base_subst;
                    return false;
                }
            }

            let mut instantiated_params = Vec::with_capacity(super_params.len());
            for p in &super_params {
                let ty = self.type_from_expr(p, &mut ctx);
                instantiated_params.push(self.apply(ty));
            }

            self.subst = base_subst;

            if self.find_instance_types(super_name, &instantiated_params, instance.span.clone()) {
                return true;
            }
        }
        false
    }

    fn find_instance_types(&mut self, class_name: &str, params: &[Type], span: Span) -> bool {
        let candidates: Vec<InstanceDeclInfo> = self
            .instances
            .iter()
            .filter(|inst| inst.class_name == class_name && inst.params.len() == params.len())
            .cloned()
            .collect();

        for candidate in candidates {
            let base_subst = self.subst.clone();
            let mut ctx = TypeContext::new(&self.type_constructors);
            let mut ok = true;
            for (expected_ty, candidate_param) in params.iter().zip(candidate.params.iter()) {
                let candidate_ty = self.type_from_expr(candidate_param, &mut ctx);
                if self
                    .unify(expected_ty.clone(), candidate_ty, span.clone())
                    .is_err()
                {
                    ok = false;
                    break;
                }
            }
            self.subst = base_subst;
            if ok {
                return true;
            }
        }
        false
    }

    fn flatten_type_and_list(&self, items: &[TypeExpr]) -> Vec<TypeExpr> {
        let mut out = Vec::new();
        for item in items {
            Self::flatten_type_and_into(item, &mut out);
        }
        out
    }

    fn flatten_type_and_into(item: &TypeExpr, out: &mut Vec<TypeExpr>) {
        match item {
            TypeExpr::And { items, .. } => {
                for inner in items {
                    Self::flatten_type_and_into(inner, out);
                }
            }
            other => out.push(other.clone()),
        }
    }

    fn class_ref_from_type_expr<'a>(&self, ty: &'a TypeExpr) -> Option<(&'a str, Vec<TypeExpr>)> {
        match ty {
            TypeExpr::Name(name) => Some((name.name.as_str(), Vec::new())),
            TypeExpr::Apply { base, args, .. } => match base.as_ref() {
                TypeExpr::Name(name) => Some((name.name.as_str(), args.clone())),
                _ => None,
            },
            _ => None,
        }
    }

    fn check_def(
        &mut self,
        def: &Def,
        sigs: &HashMap<String, Vec<Scheme>>,
        env: &mut TypeEnv,
        module: &Module,
        diagnostics: &mut Vec<FileDiagnostic>,
    ) {
        let name = def.name.name.clone();
        let expr = crate::surface::desugar_effect_sugars(desugar_holes(def.expr.clone()));
        if let Some(sig) = sigs.get(&name).and_then(|items| (items.len() == 1).then(|| &items[0]))
        {
            let mut local_env = env.clone();
            let expected = self.instantiate(sig);
            local_env.insert(name.clone(), Scheme::mono(expected.clone()));

            let result: Result<(), TypeError> = (|| {
                if def.params.is_empty() {
                    // If the surface syntax used `name = x y => ...`, the parameters live in a
                    // top-level lambda expression instead of `def.params`. Peel that lambda so we
                    // can use the signature to constrain parameter types early. This avoids
                    // incorrectly selecting domain operators when operands start as unconstrained
                    // type variables (e.g. `a.x + b.x` inside a domain `(+)=...` implementation).
                    if let Expr::Lambda { params, body, .. } = expr.clone() {
                        let mut remaining = expected;
                        for param in &params {
                            let remaining_applied = self.apply(remaining);
                            let remaining_norm = self.expand_alias(remaining_applied);
                            let Type::Func(expected_param, expected_rest) = remaining_norm else {
                                return Err(TypeError {
                                    span: def.span.clone(),
                                    message: format!("expected function type for '{name}'"),
                                    expected: None,
                                    found: None,
                                });
                            };
                            let pat_ty = self.infer_pattern(param, &mut local_env)?;
                            self.unify_with_span(pat_ty, *expected_param, pattern_span(param))?;
                            remaining = *expected_rest;
                        }
                        // Elaborate against the remaining expected return type so the typechecker
                        // can apply expected-type coercions in the body.
                        let (_elab, _ty) = self.elab_expr(*body, Some(remaining), &mut local_env)?;
                        return Ok(());
                    }

                    // Use expected-type elaboration so mismatches inside the expression (e.g. a
                    // record field) get a precise span instead of underlining the entire def.
                    let (_elab, _ty) =
                        self.elab_expr(expr.clone(), Some(expected), &mut local_env)?;
                    return Ok(());
                }

                let mut remaining = expected;
                for param in &def.params {
                    let remaining_applied = self.apply(remaining);
                    let remaining_norm = self.expand_alias(remaining_applied);
                    let Type::Func(expected_param, expected_rest) = remaining_norm else {
                        return Err(TypeError {
                            span: def.span.clone(),
                            message: format!("expected function type for '{name}'"),
                            expected: None,
                            found: None,
                        });
                    };
                    let pat_ty = self.infer_pattern(param, &mut local_env)?;
                    self.unify_with_span(pat_ty, *expected_param, pattern_span(param))?;
                    remaining = *expected_rest;
                }
                // Elaborate against the remaining expected return type so the typechecker can
                // apply expected-type coercions (e.g. `Text` -> `VNode` via `TextNode`).
                let (_elab, _ty) = self.elab_expr(expr.clone(), Some(remaining), &mut local_env)?;
                Ok(())
            })();

            if let Err(err) = result {
                diagnostics.push(self.error_to_diag(module, err));
                return;
            }
            env.insert(name.clone(), sig.clone());
        } else {
            let prior_scheme = env.get(&name).cloned();
            let is_repeat = self.checked_defs.contains(&name);
            let mut local_env = env.clone();
            let placeholder = self.fresh_var();
            local_env.insert(name.clone(), Scheme::mono(placeholder.clone()));
            // Even without an explicit signature, run expected-type elaboration so argument/field
            // positions can insert coercions (e.g. spliced `Text` in `~<html>` children).
            let inferred = if def.params.is_empty() {
                self.elab_expr(expr.clone(), None, &mut local_env)
                    .map(|(_elab, ty)| ty)
            } else {
                let lambda = Expr::Lambda {
                    params: def.params.clone(),
                    body: Box::new(expr.clone()),
                    span: def.span.clone(),
                };
                self.elab_expr(lambda, None, &mut local_env)
                    .map(|(_elab, ty)| ty)
            };
            let inferred = match inferred {
                Ok(ty) => ty,
                Err(err) => {
                    diagnostics.push(self.error_to_diag(module, err));
                    return;
                }
            };
            if let Err(err) = self.unify_with_span(placeholder, inferred.clone(), def.span.clone())
            {
                diagnostics.push(self.error_to_diag(module, err));
                return;
            }
            let inferred = self.apply(inferred);

            if is_repeat {
                if let Some(sig) = prior_scheme {
                    let expected = self.instantiate(&sig);
                    if let Err(err) =
                        self.unify_with_span(inferred.clone(), expected.clone(), def.span.clone())
                    {
                        diagnostics.push(self.error_to_diag(module, err));
                        return;
                    }
                    env.insert(name.clone(), sig);
                }
            } else {
                let scheme = self.generalize(inferred, env);
                env.insert(name.clone(), scheme);
            }
        }
        self.checked_defs.insert(name);
    }
}
