/// A nominal recursive equality equation includes its instantiated arguments.
/// Portable definitions and import aliases are representations, not identity.
#[derive(Clone, Debug, PartialEq, Eq)]
struct EqualityProofKey {
    identity: EqualityProofIdentity,
    arguments: Vec<GateType>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum EqualityProofIdentity {
    Declaration(crate::TypeIdentity),
    Import(ImportId),
}

struct EqualityProofFrame {
    key: EqualityProofKey,
    definition: Option<Box<crate::ImportTypeDefinition>>,
}

#[derive(Default)]
struct EqualityProofPath {
    active: Vec<EqualityProofFrame>,
    steps: usize,
}

impl EqualityProofKey {
    fn of(module: &Module, ty: &GateType) -> Option<Self> {
        let (identity, arguments) = match ty {
            GateType::Domain {
                item, arguments, ..
            }
            | GateType::OpaqueItem {
                item, arguments, ..
            } => (
                EqualityProofIdentity::Declaration(module.type_origin(*item)?.identity),
                arguments,
            ),
            GateType::OpaqueImport {
                origin,
                import,
                arguments,
                ..
            } => (
                match origin.as_deref().or_else(|| {
                    module
                        .imports()
                        .get(*import)
                        .and_then(|binding| binding.metadata.type_origin())
                        .map(|origin| &origin.identity)
                }) {
                    Some(identity) => EqualityProofIdentity::Declaration(identity.clone()),
                    None => {
                        module.imports().get(*import)?;
                        EqualityProofIdentity::Import(*import)
                    }
                },
                arguments,
            ),
            _ => return None,
        };
        Some(Self {
            identity,
            arguments: arguments.clone(),
        })
    }
}

impl TypeChecker<'_> {
    fn require_eq_with_scope(
        &mut self,
        ty: &GateType,
        scope: &EqConstraintScope,
        path: &mut EqualityProofPath,
    ) -> Result<(), ComparisonError> {
        self.with_class_constraint_scope(scope.class_constraints.clone(), |this| {
            this.select_comparison_member(ComparisonKind::Equality, ty, path)
                .map(|_| ())
        })
    }

    fn require_compiler_derived_eq(
        &mut self,
        ty: &GateType,
        path: &mut EqualityProofPath,
    ) -> Result<(), ComparisonError> {
        let scope = self.current_eq_constraint_scope();
        self.require_compiler_derived_eq_with_scope(ty, &scope, path)
    }

    fn require_compiler_derived_eq_with_scope(
        &mut self,
        ty: &GateType,
        scope: &EqConstraintScope,
        path: &mut EqualityProofPath,
    ) -> Result<(), ComparisonError> {
        // Bounds apply across re-entrant class prerequisites, not just nominal
        // cycles. Failed candidates restore the entire caller-owned active path.
        const MAX_EQUALITY_DEPTH: usize = 128;
        const MAX_EQUALITY_STEPS: usize = 4096;
        if self.equality_proof_depth >= MAX_EQUALITY_DEPTH || path.steps >= MAX_EQUALITY_STEPS {
            return Err(ComparisonError::Complexity);
        }
        path.steps += 1;
        let key = EqualityProofKey::of(self.module, ty);
        if key
            .as_ref()
            .is_some_and(|key| path.active.iter().any(|frame| &frame.key == key))
        {
            return Ok(());
        }
        let previous = path.active.len();
        if let Some(key) = key {
            let definition = match ty {
                GateType::OpaqueImport { definition, .. } => definition.clone(),
                _ => None,
            };
            path.active.push(EqualityProofFrame { key, definition });
        }
        self.equality_proof_depth += 1;
        let result = (|| {
            for payload in self.derived_equality_payloads(ty, path)? {
                self.require_eq_with_scope(&payload, scope, path)?;
            }
            Ok(())
        })();
        self.equality_proof_depth -= 1;
        path.active.truncate(previous);
        result
    }

    fn derived_equality_payloads(
        &mut self,
        ty: &GateType,
        path: &EqualityProofPath,
    ) -> Result<Vec<GateType>, ComparisonError> {
        let unsupported = || {
            ComparisonError::Missing(format!(
                "`{ty}` does not have a compiler-derived `Eq` instance in v1"
            ))
        };
        match ty {
            GateType::Primitive(BuiltinType::Bytes) => Err(unsupported()),
            GateType::Primitive(_) => Ok(Vec::new()),
            GateType::TypeParameter { name, .. } => Err(ComparisonError::Missing(format!(
                "open type parameter `{name}` requires explicit equality evidence; add `Eq {name} =>` to the function annotation"
            ))),
            GateType::TypeApplication { .. } => Err(unsupported()),
            GateType::Tuple(elements) => Ok(elements.clone()),
            GateType::Record(fields) => Ok(fields.iter().map(|field| field.ty.clone()).collect()),
            GateType::List(element) | GateType::Option(element) => {
                Ok(vec![element.as_ref().clone()])
            }
            GateType::Result { error, value } | GateType::Validation { error, value } => {
                Ok(vec![error.as_ref().clone(), value.as_ref().clone()])
            }
            GateType::Domain {
                item, arguments, ..
            }
            | GateType::OpaqueItem {
                item, arguments, ..
            } => {
                let (parameters, fields) = match &self.module.items()[*item] {
                    Item::Domain(domain) => (&domain.parameters, vec![domain.carrier]),
                    Item::Type(item) => (
                        &item.parameters,
                        match &item.body {
                            TypeItemBody::Alias(alias) => vec![*alias],
                            TypeItemBody::Sum(variants) => variants
                                .iter()
                                .flat_map(|variant| variant.fields.iter().map(|field| field.ty))
                                .collect(),
                        },
                    ),
                    _ => return Err(unsupported()),
                };
                if parameters.len() != arguments.len() {
                    return Err(ComparisonError::Missing(format!(
                        "`{ty}` has incomplete type arguments for equality"
                    )));
                }
                let substitutions = parameters
                    .iter()
                    .copied()
                    .zip(arguments.iter().cloned())
                    .collect();
                fields
                    .into_iter()
                    .map(|field| {
                        self.typing
                            .lower_hir_type(field, &substitutions)
                            .ok_or_else(|| {
                                ComparisonError::Missing(format!(
                                    "the payload type for `{ty}` could not be lowered for equality"
                                ))
                            })
                    })
                    .collect()
            }
            GateType::OpaqueImport {
                origin,
                import,
                arguments,
                definition,
                ..
            } => {
                let key = EqualityProofKey::of(self.module, ty);
                let declaration = definition
                    .as_deref()
                    .or_else(|| {
                        self.module.imports().iter().find_map(|(id, binding)| {
                            let same = origin.as_deref().map_or(id == *import, |origin| {
                                binding
                                    .metadata
                                    .type_origin()
                                    .is_some_and(|candidate| &candidate.identity == origin)
                            });
                            if !same {
                                return None;
                            }
                            match &binding.metadata {
                                ImportBindingMetadata::TypeConstructor { definition, .. } => {
                                    definition.as_ref()
                                }
                                _ => None,
                            }
                        })
                    })
                    .or_else(|| {
                        let identity = &key.as_ref()?.identity;
                        path.active.iter().rev().find_map(|frame| {
                            (&frame.key.identity == identity)
                                .then_some(frame.definition.as_deref())
                                .flatten()
                        })
                    });
                let fields = match declaration {
                    Some(
                        crate::ImportTypeDefinition::Alias(carrier)
                        | crate::ImportTypeDefinition::Domain(carrier),
                    ) => vec![carrier],
                    Some(crate::ImportTypeDefinition::Sum(variants)) => variants
                        .iter()
                        .flat_map(|variant| variant.fields.iter())
                        .collect(),
                    None => {
                        if let Some(carrier) =
                            self.module.imports().get(*import).and_then(|binding| {
                                match &binding.metadata {
                                    ImportBindingMetadata::Domain { carrier, .. } => {
                                        carrier.as_ref()
                                    }
                                    _ => None,
                                }
                            })
                        {
                            vec![carrier]
                        } else {
                            return Err(ComparisonError::Missing(format!(
                                "the closed representation of imported `{ty}` is unavailable; explicit equality evidence is required"
                            )));
                        }
                    }
                };
                Ok(fields
                    .into_iter()
                    .map(|field| {
                        crate::typecheck_context::lower_import_value_type_with_substitutions(
                            self.module,
                            field,
                            arguments,
                        )
                    })
                    .collect())
            }
            GateType::Arrow { .. }
            | GateType::Map { .. }
            | GateType::Set(_)
            | GateType::Signal(_)
            | GateType::Task { .. } => Err(unsupported()),
        }
    }
}
