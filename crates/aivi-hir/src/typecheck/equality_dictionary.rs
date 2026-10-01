pub(crate) struct DerivedEqualityDictionary {
    pub shape: Option<std::sync::Arc<crate::EqualityShape>>,
    pub evidence: Vec<DerivedEqualityEvidence>,
}

pub(crate) struct DerivedEqualityEvidence {
    pub member: ClassMemberResolution,
    pub subject: TypeBinding,
    pub ty: GateType,
}

/// The root dictionary has already been selected. Only its payloads perform
/// lexical comparison search; selecting the root again could change identity.
pub(crate) fn derived_equality_dictionary(
    module: &Module,
    env: &GateExprEnv,
    subject: &GateType,
) -> Option<DerivedEqualityDictionary> {
    if matches!(subject, GateType::Primitive(_)) {
        return Some(DerivedEqualityDictionary {
            shape: None,
            evidence: Vec::new(),
        });
    }
    let mut checker = TypeChecker::new(module);
    checker.with_class_constraint_scope(evidence_scope_constraints(env), |checker| {
        checker.build_equality_dictionary(subject).ok()
    })
}

impl TypeChecker<'_> {
    fn imported_equality_definition(
        &self,
        ty: &GateType,
        path: &EqualityProofPath,
    ) -> Option<crate::ImportTypeDefinition> {
        let GateType::OpaqueImport {
            origin,
            import,
            definition,
            ..
        } = ty
        else {
            return None;
        };
        if let Some(definition) = definition {
            return Some(definition.as_ref().clone());
        }
        let registered = self.module.imports().iter().find_map(|(id, binding)| {
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
                ImportBindingMetadata::TypeConstructor { definition, .. } => definition.clone(),
                ImportBindingMetadata::Domain {
                    carrier: Some(carrier),
                    ..
                } => Some(crate::ImportTypeDefinition::Domain(carrier.clone())),
                _ => None,
            }
        });
        registered.or_else(|| {
            let key = EqualityProofKey::of(self.module, ty)?;
            path.active.iter().rev().find_map(|frame| {
                (frame.key.identity == key.identity)
                    .then(|| frame.definition.as_deref().cloned())
                    .flatten()
            })
        })
    }

    fn build_equality_dictionary(
        &mut self,
        subject: &GateType,
    ) -> Result<DerivedEqualityDictionary, ComparisonError> {
        use crate::{
            EqualityEvidenceId, EqualityNodeId, EqualityRecordField, EqualityShape,
            EqualityShapeNode as Node, EqualitySumVariant,
        };
        let mut nodes = vec![Node::Structural];
        let root = EqualityNodeId::from_raw(0);
        let mut pending = vec![(root, subject.clone(), 0usize, true)];
        let mut nominal: Vec<(EqualityProofKey, EqualityNodeId)> = Vec::new();
        let mut evidence: Vec<DerivedEqualityEvidence> = Vec::new();
        let mut path = EqualityProofPath::default();
        while let Some((id, ty, depth, root)) = pending.pop() {
            if nodes.len() > 4096 {
                return Err(ComparisonError::Complexity);
            }
            if !root {
                let matched =
                    self.select_comparison_member(ComparisonKind::Equality, &ty, &mut path)?;
                let implementation =
                    self.class_member_implementation(matched.resolution, &matched.evidence.subject);
                let scoped = self.in_scope_class_constraints.contains(&matched.evidence);
                if scoped || !matches!(implementation, Some(ClassMemberImplementation::Builtin)) {
                    let slot = evidence
                        .iter()
                        .position(|leaf| {
                            leaf.member == matched.resolution
                                && leaf.subject == matched.evidence.subject
                        })
                        .unwrap_or_else(|| {
                            let index = evidence.len();
                            evidence.push(DerivedEqualityEvidence {
                                member: matched.resolution,
                                subject: matched.evidence.subject,
                                ty: matched.parameters.into_iter().rev().fold(
                                    matched.result,
                                    |result, parameter| GateType::Arrow {
                                        parameter: Box::new(parameter),
                                        result: Box::new(result),
                                    },
                                ),
                            });
                            index
                        });
                    nodes[id.as_raw() as usize] =
                        Node::Evidence(EqualityEvidenceId::from_raw(slot as u32));
                    continue;
                }
            }
            if depth >= 128 {
                return Err(ComparisonError::Complexity);
            }
            if let Some(key) = EqualityProofKey::of(self.module, &ty) {
                if let Some((_, existing)) = nominal.iter().find(|(candidate, _)| candidate == &key)
                {
                    nodes[id.as_raw() as usize] = Node::Carrier(*existing);
                    continue;
                }
                let definition = self.imported_equality_definition(&ty, &path).map(Box::new);
                path.active.push(EqualityProofFrame {
                    key: key.clone(),
                    definition,
                });
                nominal.push((key, id));
            }
            let mut child = |ty: GateType| {
                let id = EqualityNodeId::from_raw(nodes.len() as u32);
                nodes.push(Node::Structural);
                pending.push((id, ty, depth + 1, false));
                id
            };
            let node = match &ty {
                GateType::Primitive(_) => Node::Structural,
                GateType::Tuple(fields) => Node::Tuple(fields.iter().cloned().map(&mut child).collect()),
                GateType::Record(fields) => Node::Record(fields.iter().map(|field| EqualityRecordField { name: field.name.clone().into(), node: child(field.ty.clone()) }).collect()),
                GateType::List(element) => Node::List(child(element.as_ref().clone())),
                GateType::Option(element) => Node::Option(child(element.as_ref().clone())),
                GateType::Result { error, value } => Node::Result { error: child(error.as_ref().clone()), value: child(value.as_ref().clone()) },
                GateType::Validation { error, value } => Node::Validation { error: child(error.as_ref().clone()), value: child(value.as_ref().clone()) },
                GateType::Domain { .. } => Node::Carrier(child(self.derived_equality_payloads(&ty, &path)?.into_iter().next().ok_or_else(|| ComparisonError::Missing("equality carrier is unavailable".into()))?)),
                GateType::OpaqueItem { item, .. } => match &self.module.items()[*item] {
                    Item::Type(item) => match &item.body {
                        TypeItemBody::Alias(_) => Node::Carrier(child(self.derived_equality_payloads(&ty, &path)?.into_iter().next().ok_or_else(|| ComparisonError::Missing("equality alias is unavailable".into()))?)),
                        TypeItemBody::Sum(_) => {
                            let variants = crate::typecheck_context::opaque_type_variants(self.module, &ty).ok_or_else(|| ComparisonError::Missing("equality constructors are unavailable".into()))?;
                            Node::Sum(variants.into_iter().map(|variant| EqualitySumVariant { name: variant.name, fields: variant.fields.into_iter().map(&mut child).collect() }).collect())
                        }
                    },
                    _ => return Err(ComparisonError::Missing("invalid equality declaration".into())),
                },
                GateType::OpaqueImport { arguments, .. } => match self.imported_equality_definition(&ty, &path) {
                    Some(crate::ImportTypeDefinition::Sum(variants)) => Node::Sum(variants.into_iter().map(|variant| EqualitySumVariant { name: variant.name, fields: variant.fields.iter().map(|field| child(crate::typecheck_context::lower_import_value_type_with_substitutions(self.module, field, arguments))).collect() }).collect()),
                    Some(crate::ImportTypeDefinition::Alias(carrier) | crate::ImportTypeDefinition::Domain(carrier)) => Node::Carrier(child(crate::typecheck_context::lower_import_value_type_with_substitutions(self.module, &carrier, arguments))),
                    None => return Err(ComparisonError::Missing("imported equality representation is unavailable".into())),
                },
                _ => return Err(ComparisonError::Missing(format!("cannot derive equality for `{ty}`"))),
            };
            nodes[id.as_raw() as usize] = node;
        }
        let shape = if evidence.is_empty() {
            None
        } else {
            Some(std::sync::Arc::new(
                EqualityShape::new(root, nodes, evidence.len())
                    .map_err(|error| ComparisonError::Missing(error.to_string()))?,
            ))
        };
        Ok(DerivedEqualityDictionary { shape, evidence })
    }
}
