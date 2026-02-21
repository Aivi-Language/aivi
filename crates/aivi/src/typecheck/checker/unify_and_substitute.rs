impl TypeChecker {
    fn unify_with_span(&mut self, found: Type, expected: Type, span: Span) -> Result<(), TypeError> {
        self.unify(found, expected, span)
    }

    fn unify(&mut self, found: Type, expected: Type, span: Span) -> Result<(), TypeError> {
        let found = self.apply(found);
        let found = self.expand_alias(found);
        let expected = self.apply(expected);
        let expected = self.expand_alias(expected);
        match (found, expected) {
            (Type::Var(a), Type::Var(b)) if a == b => Ok(()),
            (Type::Var(var), ty) => self.bind_var(var, ty, span, true),
            (ty, Type::Var(var)) => self.bind_var(var, ty, span, false),
            (Type::Con(name_f, args_f), Type::Con(name_e, args_e)) => {
                if name_f != name_e || args_f.len() != args_e.len() {
                    return Err(TypeError {
                        span,
                        message: "type mismatch".to_string(),
                        expected: Some(Box::new(Type::Con(name_e, args_e))),
                        found: Some(Box::new(Type::Con(name_f, args_f))),
                    });
                }
                for (f, e) in args_f.into_iter().zip(args_e.into_iter()) {
                    self.unify(f, e, span.clone())?;
                }
                Ok(())
            }
            (Type::App(base_f, args_f), Type::App(base_e, args_e)) => {
                if args_f.len() != args_e.len() {
                    return Err(TypeError {
                        span,
                        message: "type mismatch".to_string(),
                        expected: Some(Box::new(Type::App(base_e, args_e))),
                        found: Some(Box::new(Type::App(base_f, args_f))),
                    });
                }
                self.unify(*base_f, *base_e, span.clone())?;
                for (f, e) in args_f.into_iter().zip(args_e.into_iter()) {
                    self.unify(f, e, span.clone())?;
                }
                Ok(())
            }
            (Type::App(base_f, args_f), Type::Con(name_e, args_e)) => {
                // Allow unifying a type application with a fully-applied constructor by splitting
                // constructor args into a "prefix" (applied to the base) and a "suffix"
                // corresponding to this application.
                if args_f.len() > args_e.len() {
                    return Err(TypeError {
                        span,
                        message: "type mismatch".to_string(),
                        expected: Some(Box::new(Type::Con(name_e, args_e))),
                        found: Some(Box::new(Type::App(base_f, args_f))),
                    });
                }

                let split = args_e.len() - args_f.len();
                let (prefix, suffix) = args_e.split_at(split);
                self.unify(
                    *base_f,
                    Type::Con(name_e, prefix.to_vec()),
                    span.clone(),
                )?;
                for (f, e) in args_f.into_iter().zip(suffix.iter().cloned()) {
                    self.unify(f, e, span.clone())?;
                }
                Ok(())
            }
            (Type::Con(name_f, args_f), Type::App(base_e, args_e)) => {
                if args_e.len() > args_f.len() {
                    return Err(TypeError {
                        span,
                        message: "type mismatch".to_string(),
                        expected: Some(Box::new(Type::App(base_e, args_e))),
                        found: Some(Box::new(Type::Con(name_f, args_f))),
                    });
                }

                let split = args_f.len() - args_e.len();
                let (prefix, suffix) = args_f.split_at(split);
                self.unify(
                    Type::Con(name_f, prefix.to_vec()),
                    *base_e,
                    span.clone(),
                )?;
                for (f, e) in suffix.iter().cloned().zip(args_e.into_iter()) {
                    self.unify(f, e, span.clone())?;
                }
                Ok(())
            }
            (Type::Func(a1, b1), Type::Func(a2, b2)) => {
                self.unify(*a1, *a2, span.clone())?;
                self.unify(*b1, *b2, span)
            }
            (Type::Tuple(items_f), Type::Tuple(items_e)) => {
                if items_f.len() != items_e.len() {
                    return Err(TypeError {
                        span,
                        message: "tuple length mismatch".to_string(),
                        expected: Some(Box::new(Type::Tuple(items_e))),
                        found: Some(Box::new(Type::Tuple(items_f))),
                    });
                }
                for (f, e) in items_f.into_iter().zip(items_e.into_iter()) {
                    self.unify(f, e, span.clone())?;
                }
                Ok(())
            }
            (
                Type::Record {
                    fields: f_fields,
                    open: open_f,
                },
                Type::Record {
                    fields: e_fields,
                    open: open_e,
                },
            ) => {
                let mut all_fields: HashSet<String> = f_fields.keys().cloned().collect();
                all_fields.extend(e_fields.keys().cloned());

                for field in &all_fields {
                    match (f_fields.get(field), e_fields.get(field)) {
                        (Some(tf), Some(te)) => {
                            self.unify(tf.clone(), te.clone(), span.clone())?;
                        }
                        (Some(_), None) => {
                            if !open_e {
                                return Err(TypeError {
                                    span: span.clone(),
                                    message: format!("missing field '{}'", field),
                                    expected: Some(Box::new(Type::Record {
                                        fields: e_fields.clone(),
                                        open: open_e,
                                    })),
                                    found: Some(Box::new(Type::Record {
                                        fields: f_fields.clone(),
                                        open: open_f,
                                    })),
                                });
                            }
                        }
                        (None, Some(_)) => {
                            if !open_f {
                                return Err(TypeError {
                                    span: span.clone(),
                                    message: format!("missing field '{}'", field),
                                    expected: Some(Box::new(Type::Record {
                                        fields: e_fields.clone(),
                                        open: open_e,
                                    })),
                                    found: Some(Box::new(Type::Record {
                                        fields: f_fields.clone(),
                                        open: open_f,
                                    })),
                                });
                            }
                        }
                        (None, None) => {}
                    }
                }
                Ok(())
            }
            (f, e) => Err(TypeError {
                span,
                message: "type mismatch".to_string(),
                expected: Some(Box::new(e)),
                found: Some(Box::new(f)),
            }),
        }
    }

    fn bind_var(
        &mut self,
        var: TypeVarId,
        ty: Type,
        span: Span,
        var_is_found: bool,
    ) -> Result<(), TypeError> {
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
                expected: Some(Box::new(if var_is_found {
                    ty.clone()
                } else {
                    Type::Var(var)
                })),
                found: Some(Box::new(if var_is_found {
                    Type::Var(var)
                } else {
                    ty
                })),
            });
        }
        self.subst.insert(var, ty);
        Ok(())
    }

    fn occurs(&mut self, var: TypeVarId, ty: &Type) -> bool {
        // Cyclic substitutions should never be introduced (occurs check), but if they appear,
        // we must not recurse indefinitely while detecting them.
        let mut visiting = HashSet::new();
        self.occurs_with_visiting(var, ty, &mut visiting)
    }

    fn occurs_with_visiting(
        &mut self,
        needle: TypeVarId,
        ty: &Type,
        visiting: &mut HashSet<TypeVarId>,
    ) -> bool {
        match ty {
            Type::Var(id) => {
                if *id == needle {
                    return true;
                }
                // Follow substitutions, but guard against cycles like a ~ b, b ~ a.
                if !visiting.insert(*id) {
                    return false;
                }
                if let Some(next) = self.subst.get(id).cloned() {
                    self.occurs_with_visiting(needle, &next, visiting)
                } else {
                    false
                }
            }
            Type::Con(_, args) => args
                .iter()
                .any(|arg| self.occurs_with_visiting(needle, arg, visiting)),
            Type::App(base, args) => {
                self.occurs_with_visiting(needle, base, visiting)
                    || args
                        .iter()
                        .any(|arg| self.occurs_with_visiting(needle, arg, visiting))
            }
            Type::Func(a, b) => {
                self.occurs_with_visiting(needle, a, visiting)
                    || self.occurs_with_visiting(needle, b, visiting)
            }
            Type::Tuple(items) => items
                .iter()
                .any(|item| self.occurs_with_visiting(needle, item, visiting)),
            Type::Record { fields, .. } => fields
                .values()
                .any(|field| self.occurs_with_visiting(needle, field, visiting)),
        }
    }

    fn instantiate(&mut self, scheme: &Scheme) -> Type {
        let mut mapping = HashMap::new();
        for var in &scheme.vars {
            let fresh = self.fresh_var_id();
            if let Some(name) = self.var_names.get(var).cloned() {
                self.var_names.insert(fresh, name);
            }
            mapping.insert(*var, Type::Var(fresh));
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
            origin: None,
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
        // Substitution application must be cycle-safe. Even with an occurs check, inference bugs or
        // recursive aliases can temporarily create substitution cycles. Guard to avoid Rust stack
        // overflow and keep typechecking deterministic.
        let mut visiting = HashSet::new();
        self.apply_with_visiting(ty, &mut visiting)
    }

    fn apply_with_visiting(&mut self, ty: Type, visiting: &mut HashSet<TypeVarId>) -> Type {
        match ty {
            Type::Var(id) => {
                if !visiting.insert(id) {
                    // Cycle: stop expanding.
                    return Type::Var(id);
                }
                if let Some(replacement) = self.subst.get(&id).cloned() {
                    let applied = self.apply_with_visiting(replacement, visiting);
                    self.subst.insert(id, applied.clone());
                    visiting.remove(&id);
                    applied
                } else {
                    visiting.remove(&id);
                    Type::Var(id)
                }
            }
            Type::Con(name, args) => Type::Con(
                name,
                args.into_iter()
                    .map(|arg| self.apply_with_visiting(arg, visiting))
                    .collect(),
            ),
            Type::App(base, args) => Type::App(
                Box::new(self.apply_with_visiting(*base, visiting)),
                args.into_iter()
                    .map(|arg| self.apply_with_visiting(arg, visiting))
                    .collect(),
            ),
            Type::Func(a, b) => Type::Func(
                Box::new(self.apply_with_visiting(*a, visiting)),
                Box::new(self.apply_with_visiting(*b, visiting)),
            ),
            Type::Tuple(items) => Type::Tuple(
                items
                    .into_iter()
                    .map(|item| self.apply_with_visiting(item, visiting))
                    .collect(),
            ),
            Type::Record { fields, open } => Type::Record {
                fields: fields
                    .into_iter()
                    .map(|(k, v)| (k, self.apply_with_visiting(v, visiting)))
                    .collect(),
                open,
            },
        }
    }

    fn expand_alias(&mut self, ty: Type) -> Type {
        // Expands type aliases while guarding against recursive (or mutually-recursive) aliases.
        // A recursive alias should behave like an opaque constructor during unification,
        // otherwise we can infinitely unfold it and blow the Rust stack.
        let mut visiting = HashSet::new();
        self.expand_alias_with_visiting(ty, &mut visiting)
    }

    fn expand_alias_with_visiting(&mut self, ty: Type, visiting: &mut HashSet<String>) -> Type {
        match ty {
            Type::Var(id) => Type::Var(id),
            Type::Con(name, args) => {
                let args = args
                    .into_iter()
                    .map(|arg| self.expand_alias_with_visiting(arg, visiting))
                    .collect::<Vec<_>>();

                let Some(alias) = self.aliases.get(&name).cloned() else {
                    return Type::Con(name, args);
                };

                if visiting.contains(&name) {
                    // Recursive reference; stop expanding and treat as nominal.
                    return Type::Con(name, args);
                }

                visiting.insert(name.clone());

                let mut mapping = HashMap::new();
                for (param, arg) in alias.params.iter().zip(args.iter()) {
                    mapping.insert(*param, arg.clone());
                }
                let body = Self::substitute(&alias.body, &mapping);
                let expanded = self.expand_alias_with_visiting(body, visiting);

                visiting.remove(&name);
                expanded
            }
            Type::App(base, args) => Type::App(
                Box::new(self.expand_alias_with_visiting(*base, visiting)),
                args.into_iter()
                    .map(|arg| self.expand_alias_with_visiting(arg, visiting))
                    .collect(),
            ),
            Type::Func(a, b) => Type::Func(
                Box::new(self.expand_alias_with_visiting(*a, visiting)),
                Box::new(self.expand_alias_with_visiting(*b, visiting)),
            ),
            Type::Tuple(items) => Type::Tuple(
                items
                    .into_iter()
                    .map(|item| self.expand_alias_with_visiting(item, visiting))
                    .collect(),
            ),
            Type::Record { fields, open } => Type::Record {
                fields: fields
                    .into_iter()
                    .map(|(k, v)| (k, self.expand_alias_with_visiting(v, visiting)))
                    .collect(),
                open,
            },
        }
    }
}
