// Type → CgType conversion for the typed codegen path.

use crate::cg_type::CgType;

impl TypeChecker {
    const MAX_CG_LOWERING_DEPTH: usize = 64;

    /// Convert a type-checker `Type` to a codegen-friendly `CgType`.
    ///
    /// The `Type` should already have substitution applied (`self.apply(ty)`).
    /// Any remaining `Type::Var` or un-resolved HKTs produce `CgType::Dynamic`.
    pub(super) fn type_to_cg_type(&mut self, ty: &Type, env: &crate::typecheck::types::TypeEnv) -> CgType {
        let resolved = self.apply(ty.clone());
        self.type_to_cg_type_inner_with_depth(&resolved, env, Self::MAX_CG_LOWERING_DEPTH)
    }

    fn type_to_cg_type_inner_with_depth(
        &self,
        ty: &Type,
        env: &crate::typecheck::types::TypeEnv,
        depth_left: usize,
    ) -> CgType {
        if depth_left == 0 {
            return CgType::Dynamic;
        }
        match ty {
            Type::Var(_) => CgType::Dynamic,

            Type::Con(name, args) => match name.as_str() {
                "Int" if args.is_empty() => CgType::Int,
                "Float" if args.is_empty() => CgType::Float,
                "Bool" if args.is_empty() => CgType::Bool,
                "Text" if args.is_empty() => CgType::Text,
                "Unit" if args.is_empty() => CgType::Unit,
                "DateTime" if args.is_empty() => CgType::DateTime,
                "List" if args.len() == 1 => {
                    CgType::ListOf(Box::new(
                        self.type_to_cg_type_inner_with_depth(&args[0], env, depth_left - 1),
                    ))
                }
                _ => {
                    // Check if this is a known ADT
                    if let Some(ctor_names) = self.adt_constructors.get(name) {
                        let mut constructors = Vec::new();
                        for ctor_name in ctor_names {
                            // For each constructor, figure out its payload types from the
                            // environment. Constructors are registered as functions:
                            // `Ctor : Arg1 -> Arg2 -> ... -> ResultType`
                            // We need to collect the argument types.
                            let ctor_arg_types = self.extract_ctor_arg_types(
                                ctor_name,
                                name,
                                args,
                                env,
                                depth_left - 1,
                            );
                            constructors.push((ctor_name.clone(), ctor_arg_types));
                        }
                        CgType::Adt {
                            name: name.clone(),
                            constructors,
                        }
                    } else {
                        // Unknown type constructor — Dynamic
                        CgType::Dynamic
                    }
                }
            },

            Type::App(_, _) => CgType::Dynamic,

            Type::Func(a, b) => CgType::Func(
                Box::new(self.type_to_cg_type_inner_with_depth(a, env, depth_left - 1)),
                Box::new(self.type_to_cg_type_inner_with_depth(b, env, depth_left - 1)),
            ),

            Type::Tuple(items) => {
                CgType::Tuple(
                    items
                        .iter()
                        .map(|t| self.type_to_cg_type_inner_with_depth(t, env, depth_left - 1))
                        .collect(),
                )
            }

            Type::Record { fields } => CgType::Record(
                fields
                    .iter()
                    .map(|(name, ty)| {
                        (
                            name.clone(),
                            self.type_to_cg_type_inner_with_depth(ty, env, depth_left - 1),
                        )
                    })
                    .collect(),
            ),
        }
    }

    /// Extract the argument types for an ADT constructor.
    ///
    /// Given constructor name (e.g. "Some") and the ADT it belongs to ("Option") with
    /// the applied type arguments (e.g. [Int] for `Option Int`), returns the constructor's
    /// payload types with variables resolved.
    fn extract_ctor_arg_types(
        &self,
        ctor_name: &str,
        adt_name: &str,
        type_args: &[Type],
        env: &crate::typecheck::types::TypeEnv,
        depth_left: usize,
    ) -> Vec<CgType> {
        if depth_left == 0 {
            return Vec::new();
        }

        let Some(scheme) = env.get(ctor_name) else {
            return Vec::new();
        };

        let mut cur = &scheme.ty;
        let mut arg_types = Vec::new();
        while let Type::Func(arg, next) = cur {
            arg_types.push(arg.as_ref().clone());
            cur = next.as_ref();
        }

        let subst_map = match cur {
            Type::Con(name, ctor_vars) if name == adt_name && ctor_vars.len() == type_args.len() => {
                let mut map = std::collections::HashMap::new();
                for (ctor_var, actual_arg) in ctor_vars.iter().zip(type_args.iter()) {
                    if let Type::Var(v) = ctor_var {
                        map.insert(*v, actual_arg.clone());
                    }
                }
                map
            }
            Type::App(base, args) => {
                let mut base_ptr = base.as_ref();
                let mut all_args = args.clone();
                while let Type::App(inner_base, inner_args) = base_ptr {
                    all_args.splice(0..0, inner_args.iter().cloned());
                    base_ptr = inner_base.as_ref();
                }
                if let Type::Con(name, existing_args) = base_ptr {
                    let mut combined = existing_args.clone();
                    combined.extend(all_args);
                    if name == adt_name && combined.len() == type_args.len() {
                        let mut map = std::collections::HashMap::new();
                        for (ctor_var, actual_arg) in combined.iter().zip(type_args.iter()) {
                            if let Type::Var(v) = ctor_var {
                                map.insert(*v, actual_arg.clone());
                            }
                        }
                        map
                    } else {
                        return Vec::new();
                    }
                } else {
                    return Vec::new();
                }
            }
            _ => {
                if type_args.is_empty() {
                    std::collections::HashMap::new()
                } else {
                    return Vec::new();
                }
            }
        };

        arg_types
            .into_iter()
            .map(|arg| {
                let subst_t = self.apply_local_subst(arg, &subst_map);
                if contains_adt_name(&subst_t, adt_name) {
                    CgType::Dynamic
                } else {
                    self.type_to_cg_type_inner_with_depth(&subst_t, env, depth_left - 1)
                }
            })
            .collect()
    }

    #[allow(clippy::only_used_in_recursion)]
    fn apply_local_subst(
        &self,
        ty: Type,
        subst: &std::collections::HashMap<crate::typecheck::types::TypeVarId, Type>,
    ) -> Type {
        match ty {
            Type::Var(v) => subst.get(&v).cloned().unwrap_or(Type::Var(v)),
            Type::Con(name, args) => Type::Con(
                name,
                args.into_iter()
                    .map(|a| self.apply_local_subst(a, subst))
                    .collect(),
            ),
            Type::App(base, args) => Type::App(
                Box::new(self.apply_local_subst(*base, subst)),
                args.into_iter()
                    .map(|a| self.apply_local_subst(a, subst))
                    .collect(),
            ),
            Type::Func(a, b) => Type::Func(
                Box::new(self.apply_local_subst(*a, subst)),
                Box::new(self.apply_local_subst(*b, subst)),
            ),
            Type::Tuple(items) => Type::Tuple(
                items
                    .into_iter()
                    .map(|i| self.apply_local_subst(i, subst))
                    .collect(),
            ),
            Type::Record { fields } => Type::Record {
                fields: fields
                    .into_iter()
                    .map(|(k, v)| (k, self.apply_local_subst(v, subst)))
                    .collect(),
            },
        }
    }
}

fn contains_adt_name(ty: &Type, adt_name: &str) -> bool {
    match ty {
        Type::Var(_) => false,
        Type::Con(name, args) => name == adt_name || args.iter().any(|arg| contains_adt_name(arg, adt_name)),
        Type::App(base, args) => {
            contains_adt_name(base, adt_name)
                || args.iter().any(|arg| contains_adt_name(arg, adt_name))
        }
        Type::Func(a, b) => contains_adt_name(a, adt_name) || contains_adt_name(b, adt_name),
        Type::Tuple(items) => items.iter().any(|item| contains_adt_name(item, adt_name)),
        Type::Record { fields, .. } => fields.values().any(|field| contains_adt_name(field, adt_name)),
    }
}
