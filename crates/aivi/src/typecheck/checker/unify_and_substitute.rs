impl TypeChecker {
    fn unify_with_span(&mut self, left: Type, right: Type, span: Span) -> Result<(), TypeError> {
        self.unify(left, right, span)
    }

    fn unify(&mut self, left: Type, right: Type, span: Span) -> Result<(), TypeError> {
        let left = self.apply(left);
        let left = self.expand_alias(left);
        let right = self.apply(right);
        let right = self.expand_alias(right);
        match (left, right) {
            (Type::Var(a), Type::Var(b)) if a == b => Ok(()),
            (Type::Var(var), ty) | (ty, Type::Var(var)) => self.bind_var(var, ty, span),
            (Type::Con(name_a, args_a), Type::Con(name_b, args_b)) => {
                if name_a != name_b || args_a.len() != args_b.len() {
                    return Err(TypeError {
                        span,
                        message: "type mismatch".to_string(),
                        expected: Some(Box::new(Type::Con(name_a, args_a))),
                        found: Some(Box::new(Type::Con(name_b, args_b))),
                    });
                }
                for (a, b) in args_a.into_iter().zip(args_b.into_iter()) {
                    self.unify(a, b, span.clone())?;
                }
                Ok(())
            }
            (Type::App(base_a, args_a), Type::App(base_b, args_b)) => {
                if args_a.len() != args_b.len() {
                    return Err(TypeError {
                        span,
                        message: "type mismatch".to_string(),
                        expected: Some(Box::new(Type::App(base_a, args_a))),
                        found: Some(Box::new(Type::App(base_b, args_b))),
                    });
                }
                self.unify(*base_a, *base_b, span.clone())?;
                for (a, b) in args_a.into_iter().zip(args_b.into_iter()) {
                    self.unify(a, b, span.clone())?;
                }
                Ok(())
            }
            (Type::App(base_a, args_a), Type::Con(name_b, args_b)) => {
                // Allow unifying a type application with a fully-applied constructor by splitting
                // constructor args into a "prefix" (applied to the base) and a "suffix"
                // corresponding to this application.
                if args_a.len() > args_b.len() {
                    return Err(TypeError {
                        span,
                        message: "type mismatch".to_string(),
                        expected: Some(Box::new(Type::App(base_a, args_a))),
                        found: Some(Box::new(Type::Con(name_b, args_b))),
                    });
                }

                let split = args_b.len() - args_a.len();
                let (prefix, suffix) = args_b.split_at(split);
                self.unify(
                    *base_a,
                    Type::Con(name_b, prefix.to_vec()),
                    span.clone(),
                )?;
                for (a, b) in args_a.into_iter().zip(suffix.iter().cloned()) {
                    self.unify(a, b, span.clone())?;
                }
                Ok(())
            }
            (Type::Con(name_a, args_a), Type::App(base_b, args_b)) => {
                if args_b.len() > args_a.len() {
                    return Err(TypeError {
                        span,
                        message: "type mismatch".to_string(),
                        expected: Some(Box::new(Type::Con(name_a, args_a))),
                        found: Some(Box::new(Type::App(base_b, args_b))),
                    });
                }

                let split = args_a.len() - args_b.len();
                let (prefix, suffix) = args_a.split_at(split);
                self.unify(
                    Type::Con(name_a, prefix.to_vec()),
                    *base_b,
                    span.clone(),
                )?;
                for (a, b) in suffix.iter().cloned().zip(args_b.into_iter()) {
                    self.unify(a, b, span.clone())?;
                }
                Ok(())
            }
            (Type::Func(a1, b1), Type::Func(a2, b2)) => {
                self.unify(*a1, *a2, span.clone())?;
                self.unify(*b1, *b2, span)
            }
            (Type::Tuple(items_a), Type::Tuple(items_b)) => {
                if items_a.len() != items_b.len() {
                    return Err(TypeError {
                        span,
                        message: "tuple length mismatch".to_string(),
                        expected: Some(Box::new(Type::Tuple(items_a))),
                        found: Some(Box::new(Type::Tuple(items_b))),
                    });
                }
                for (a, b) in items_a.into_iter().zip(items_b.into_iter()) {
                    self.unify(a, b, span.clone())?;
                }
                Ok(())
            }
            (
                Type::Record {
                    fields: a,
                    open: open_a,
                },
                Type::Record {
                    fields: b,
                    open: open_b,
                },
            ) => {
                let mut all_fields: HashSet<String> = a.keys().cloned().collect();
                all_fields.extend(b.keys().cloned());

                for field in &all_fields {
                    match (a.get(field), b.get(field)) {
                        (Some(ta), Some(tb)) => {
                            self.unify(ta.clone(), tb.clone(), span.clone())?;
                        }
                        (Some(_), None) => {
                            if !open_b {
                                return Err(TypeError {
                                    span: span.clone(),
                                    message: format!("missing field '{}'", field),
                                    expected: Some(Box::new(Type::Record {
                                        fields: a.clone(),
                                        open: open_a,
                                    })),
                                    found: Some(Box::new(Type::Record {
                                        fields: b.clone(),
                                        open: open_b,
                                    })),
                                });
                            }
                        }
                        (None, Some(_)) => {
                            if !open_a {
                                return Err(TypeError {
                                    span: span.clone(),
                                    message: format!("missing field '{}'", field),
                                    expected: Some(Box::new(Type::Record {
                                        fields: a.clone(),
                                        open: open_a,
                                    })),
                                    found: Some(Box::new(Type::Record {
                                        fields: b.clone(),
                                        open: open_b,
                                    })),
                                });
                            }
                        }
                        (None, None) => {}
                    }
                }
                Ok(())
            }
            (a, b) => Err(TypeError {
                span,
                message: "type mismatch".to_string(),
                expected: Some(Box::new(a)),
                found: Some(Box::new(b)),
            }),
        }
    }

    fn bind_var(&mut self, var: TypeVarId, ty: Type, span: Span) -> Result<(), TypeError> {
        // Normalize through the current substitution before doing the occurs check.
        // Without this, we can falsely report an occurs-check error when `ty` is a var
        // that already resolves to `var` (via substitution), which should be a no-op.
        let ty = self.apply(ty);
        if let Type::Var(other) = &ty {
            if *other == var {
                return Ok(());
            }
        }
        if self.occurs(var, &ty) {
            let mut message = "occurs check failed".to_string();
            if std::env::var("AIVI_DEBUG_TRACE").is_ok_and(|v| v == "1") {
                let ty_str = self.type_to_string(&ty);
                message = format!("occurs check failed (var={:?}, ty={})", var, ty_str);
            }
            return Err(TypeError {
                span,
                message,
                expected: Some(Box::new(Type::Var(var))),
                found: Some(Box::new(ty)),
            });
        }
        self.subst.insert(var, ty);
        Ok(())
    }

    fn occurs(&mut self, var: TypeVarId, ty: &Type) -> bool {
        match self.apply(ty.clone()) {
            Type::Var(id) => id == var,
            Type::Con(_, args) => args.iter().any(|arg| self.occurs(var, arg)),
            Type::App(base, args) => {
                self.occurs(var, &base) || args.iter().any(|arg| self.occurs(var, arg))
            }
            Type::Func(a, b) => self.occurs(var, &a) || self.occurs(var, &b),
            Type::Tuple(items) => items.iter().any(|item| self.occurs(var, item)),
            Type::Record { fields, .. } => fields.values().any(|field| self.occurs(var, field)),
        }
    }

    fn instantiate(&mut self, scheme: &Scheme) -> Type {
        let mut mapping = HashMap::new();
        for var in &scheme.vars {
            mapping.insert(*var, self.fresh_var());
        }
        Self::substitute(&scheme.ty, &mapping)
    }

    fn generalize(&mut self, ty: Type, env: &TypeEnv) -> Scheme {
        let ty = self.apply(ty);
        let env_vars = env.free_vars(self);
        let mut ty_vars = self.free_vars(&ty);
        ty_vars.retain(|var| !env_vars.contains(var));
        Scheme {
            vars: ty_vars.into_iter().collect(),
            ty,
        }
    }

    fn free_vars(&mut self, ty: &Type) -> HashSet<TypeVarId> {
        match self.apply(ty.clone()) {
            Type::Var(id) => vec![id].into_iter().collect(),
            Type::Con(_, args) => args.iter().flat_map(|arg| self.free_vars(arg)).collect(),
            Type::App(base, args) => {
                let mut vars = self.free_vars(&base);
                vars.extend(args.iter().flat_map(|arg| self.free_vars(arg)));
                vars
            }
            Type::Func(a, b) => {
                let mut vars = self.free_vars(&a);
                vars.extend(self.free_vars(&b));
                vars
            }
            Type::Tuple(items) => items.iter().flat_map(|item| self.free_vars(item)).collect(),
            Type::Record { fields, .. } => {
                fields.values().flat_map(|f| self.free_vars(f)).collect()
            }
        }
    }

    pub(super) fn free_vars_scheme(&mut self, scheme: &Scheme) -> HashSet<TypeVarId> {
        let mut vars = self.free_vars(&scheme.ty);
        for var in &scheme.vars {
            vars.remove(var);
        }
        vars
    }

    fn substitute(ty: &Type, mapping: &HashMap<TypeVarId, Type>) -> Type {
        match ty {
            Type::Var(id) => mapping.get(id).cloned().unwrap_or(Type::Var(*id)),
            Type::Con(name, args) => Type::Con(
                name.clone(),
                args.iter()
                    .map(|arg| Self::substitute(arg, mapping))
                    .collect(),
            ),
            Type::App(base, args) => Type::App(
                Box::new(Self::substitute(base, mapping)),
                args.iter()
                    .map(|arg| Self::substitute(arg, mapping))
                    .collect(),
            ),
            Type::Func(a, b) => Type::Func(
                Box::new(Self::substitute(a, mapping)),
                Box::new(Self::substitute(b, mapping)),
            ),
            Type::Tuple(items) => Type::Tuple(
                items
                    .iter()
                    .map(|item| Self::substitute(item, mapping))
                    .collect(),
            ),
            Type::Record { fields, open } => Type::Record {
                fields: fields
                    .iter()
                    .map(|(k, v)| (k.clone(), Self::substitute(v, mapping)))
                    .collect(),
                open: *open,
            },
        }
    }

    fn apply(&mut self, ty: Type) -> Type {
        match ty {
            Type::Var(id) => {
                if let Some(replacement) = self.subst.get(&id).cloned() {
                    let applied = self.apply(replacement);
                    self.subst.insert(id, applied.clone());
                    applied
                } else {
                    Type::Var(id)
                }
            }
            Type::Con(name, args) => {
                Type::Con(name, args.into_iter().map(|arg| self.apply(arg)).collect())
            }
            Type::App(base, args) => Type::App(
                Box::new(self.apply(*base)),
                args.into_iter().map(|arg| self.apply(arg)).collect(),
            ),
            Type::Func(a, b) => Type::Func(Box::new(self.apply(*a)), Box::new(self.apply(*b))),
            Type::Tuple(items) => {
                Type::Tuple(items.into_iter().map(|item| self.apply(item)).collect())
            }
            Type::Record { fields, open } => Type::Record {
                fields: fields
                    .into_iter()
                    .map(|(k, v)| (k, self.apply(v)))
                    .collect(),
                open,
            },
        }
    }

    fn expand_alias(&mut self, ty: Type) -> Type {
        if let Type::Con(name, args) = &ty {
            if let Some(alias) = self.aliases.get(name).cloned() {
                let mut mapping = HashMap::new();
                for (param, arg) in alias.params.iter().zip(args.iter()) {
                    mapping.insert(*param, arg.clone());
                }
                return Self::substitute(&alias.body, &mapping);
            }
        }
        ty
    }
}
