// Type → CgType conversion for the typed codegen path.

use crate::cg_type::CgType;

impl TypeChecker {
    /// Convert a type-checker `Type` to a codegen-friendly `CgType`.
    ///
    /// The `Type` should already have substitution applied (`self.apply(ty)`).
    /// Any remaining `Type::Var`, open records, or un-resolved HKTs produce `CgType::Dynamic`.
    pub(super) fn type_to_cg_type(&mut self, ty: &Type) -> CgType {
        let resolved = self.apply(ty.clone());
        self.type_to_cg_type_inner(&resolved)
    }

    fn type_to_cg_type_inner(&self, ty: &Type) -> CgType {
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
                    CgType::ListOf(Box::new(self.type_to_cg_type_inner(&args[0])))
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
                            let ctor_arg_types = self.extract_ctor_arg_types(ctor_name, name, args);
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
                Box::new(self.type_to_cg_type_inner(a)),
                Box::new(self.type_to_cg_type_inner(b)),
            ),

            Type::Tuple(items) => {
                CgType::Tuple(items.iter().map(|t| self.type_to_cg_type_inner(t)).collect())
            }

            Type::Record { fields, open } => {
                if *open {
                    CgType::Dynamic
                } else {
                    CgType::Record(
                        fields
                            .iter()
                            .map(|(name, ty)| (name.clone(), self.type_to_cg_type_inner(ty)))
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
    fn extract_ctor_arg_types(&self, _ctor_name: &str, _adt_name: &str, _type_args: &[Type]) -> Vec<CgType> {
        // For v0.1 we emit ADTs as Value anyway, so this is a placeholder.
        // A full implementation would:
        // 1. Look up the constructor's Scheme from the environment
        // 2. Instantiate type variables with _type_args
        // 3. Walk the Func chain to extract argument types
        // 4. Convert each to CgType
        Vec::new()
    }
}
