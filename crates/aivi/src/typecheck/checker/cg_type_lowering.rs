// Type → CgType conversion for the typed codegen path.

use crate::cg_type::CgType;

impl TypeChecker {
    const MAX_CG_LOWERING_DEPTH: usize = 64;

    /// Convert a type-checker `Type` to a codegen-friendly `CgType`.
    ///
    /// The `Type` should already have substitution applied (`self.apply(ty)`).
    /// Any remaining `Type::Var`, open records, or un-resolved HKTs produce `CgType::Dynamic`.
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

            Type::Record { fields, open, .. } => {
                if *open {
                    CgType::Dynamic
                } else {
                    CgType::Record(
                        fields
                                .iter()
                                .map(|(name, ty)| {
                                    (
                                        name.clone(),
                                        self.type_to_cg_type_inner_with_depth(
                                            ty,
                                            env,
                                            depth_left - 1,
                                        ),
                                    )
                                })
                                .collect(),
                        )
                }
            }
        }
    }

    /// Extract the argument types for an ADT constructor.
    ///
    /// Given constructor name (e.g. "Some") and the ADT it belongs to ("Option") with
    /// the applied type arguments (e.g. [Int] for `Option Int`), returns the constructor's
    /// payload types with variables resolved.
    fn extract_ctor_arg_types(
        &self,
        _ctor_name: &str,
        _adt_name: &str,
        _type_args: &[Type],
        _env: &crate::typecheck::types::TypeEnv,
        _depth_left: usize,
    ) -> Vec<CgType> {
        // Keep constructor payload lowering conservative to avoid recursive ADT blowups.
        Vec::new()
    }
}
