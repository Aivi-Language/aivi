impl GateTypeContext<'_> {
    fn collect_import_constructor_parameters(
        template: &GateType,
        constructor_parameters: &mut HashMap<TypeParameterId, usize>,
    ) {
        let mut pending = vec![template];
        while let Some(ty) = pending.pop() {
            match ty {
                GateType::TypeApplication {
                    parameter,
                    arguments,
                    ..
                } => {
                    constructor_parameters.insert(*parameter, arguments.len());
                    pending.extend(arguments);
                }
                GateType::Tuple(elements) => pending.extend(elements),
                GateType::Record(fields) => pending.extend(fields.iter().map(|field| &field.ty)),
                GateType::Arrow { parameter, result } => {
                    pending.extend([parameter.as_ref(), result.as_ref()])
                }
                GateType::List(element)
                | GateType::Set(element)
                | GateType::Option(element)
                | GateType::Signal(element) => pending.push(element),
                GateType::Map { key, value } => pending.extend([key.as_ref(), value.as_ref()]),
                GateType::Result { error, value }
                | GateType::Validation { error, value }
                | GateType::Task { error, value } => {
                    pending.extend([error.as_ref(), value.as_ref()])
                }
                GateType::Domain { arguments, .. }
                | GateType::OpaqueItem { arguments, .. }
                | GateType::OpaqueImport { arguments, .. } => pending.extend(arguments),
                GateType::Primitive(_) | GateType::TypeParameter { .. } => {}
            }
        }
    }

    fn imported_constructor_head(
        &self,
        head: &crate::ImportedTypeConstructor,
    ) -> Option<TypeConstructorHead> {
        Some(match head {
            crate::ImportedTypeConstructor::Builtin(builtin) => {
                TypeConstructorHead::Builtin(*builtin)
            }
            crate::ImportedTypeConstructor::Parameter { index, arity, .. } => {
                TypeConstructorHead::Parameter {
                    parameter: TypeParameterId::from_raw(u32::MAX - *index as u32),
                    arity: *arity,
                }
            }
            crate::ImportedTypeConstructor::Named { name, .. } => {
                let (id, _) = self.module.imports().iter().find(|(_, binding)| {
                    binding.imported_name.text() == name.as_ref()
                        && matches!(
                            binding.metadata,
                            ImportBindingMetadata::TypeConstructor { .. }
                                | ImportBindingMetadata::Domain { .. }
                                | ImportBindingMetadata::AmbientType
                                | ImportBindingMetadata::BuiltinType(_)
                        )
                })?;
                if let ImportBindingMetadata::BuiltinType(builtin) =
                    self.module.imports()[id].metadata
                {
                    return Some(TypeConstructorHead::Builtin(builtin));
                }
                if matches!(
                    self.module.imports()[id].metadata,
                    ImportBindingMetadata::AmbientType
                ) {
                    let item =
                        self.module.ambient_items().iter().copied().find(|item| {
                            match &self.module.items()[*item] {
                                Item::Type(ty) => ty.name.text() == name.as_ref(),
                                Item::Domain(domain) => domain.name.text() == name.as_ref(),
                                _ => false,
                            }
                        })?;
                    return Some(TypeConstructorHead::Item(item));
                }
                TypeConstructorHead::Import(id)
            }
        })
    }

    pub(crate) fn lower_import_type_binding(
        &self,
        template: &crate::ImportedTypeBinding,
    ) -> Option<TypeBinding> {
        Some(match template {
            crate::ImportedTypeBinding::Type(ty) => {
                TypeBinding::Type(self.lower_import_value_type(ty))
            }
            crate::ImportedTypeBinding::Constructor { head, arguments } => {
                TypeBinding::Constructor(TypeConstructorBinding::new(
                    self.imported_constructor_head(head)?,
                    arguments
                        .iter()
                        .map(|ty| self.lower_import_value_type(ty))
                        .collect(),
                ))
            }
        })
    }

    /// Infer one shared substitution environment from a portable instance head.
    pub(crate) fn match_import_type_binding(
        &self,
        template: &crate::ImportedTypeBinding,
        actual: &TypeBinding,
        bindings: &mut PolyTypeBindings,
    ) -> bool {
        let mut substitutions = HashMap::new();
        let mut constructor_parameters = HashMap::new();
        let matches = match (template, actual) {
            (crate::ImportedTypeBinding::Type(template), TypeBinding::Type(actual)) => {
                let template = self.lower_import_value_type(template);
                Self::collect_import_constructor_parameters(&template, &mut constructor_parameters);
                self.match_gate_type_template(&template, actual, &mut substitutions)
            }
            (
                crate::ImportedTypeBinding::Constructor { head, arguments },
                TypeBinding::Constructor(actual),
            ) => {
                let Some(head) = self.imported_constructor_head(head) else {
                    return false;
                };
                let prefix_len = match head {
                    TypeConstructorHead::Parameter { parameter, .. } => {
                        let Some(prefix_len) = actual.arguments.len().checked_sub(arguments.len())
                        else {
                            return false;
                        };
                        let candidate = TypeBinding::Constructor(TypeConstructorBinding::new(
                            actual.head,
                            actual.arguments[..prefix_len].to_vec(),
                        ));
                        if let Some(previous) = bindings.get(&parameter) {
                            if !self.type_bindings_match(previous, &candidate) {
                                return false;
                            }
                        } else {
                            bindings.insert(parameter, candidate);
                        }
                        prefix_len
                    }
                    head if self.constructor_heads_match(head, actual.head)
                        && arguments.len() == actual.arguments.len() =>
                    {
                        0
                    }
                    _ => return false,
                };
                arguments
                    .iter()
                    .zip(&actual.arguments[prefix_len..])
                    .all(|(template, actual)| {
                        let template = self.lower_import_value_type(template);
                        Self::collect_import_constructor_parameters(
                            &template,
                            &mut constructor_parameters,
                        );
                        self.match_gate_type_template(&template, actual, &mut substitutions)
                    })
            }
            _ => false,
        };
        if !matches {
            return false;
        }
        for (parameter, ty) in substitutions {
            let candidate = if let Some(applied) = constructor_parameters.get(&parameter) {
                let Some((head, arguments)) = ty.constructor_view() else {
                    return false;
                };
                let Some(prefix_len) = arguments.len().checked_sub(*applied) else {
                    return false;
                };
                TypeBinding::Constructor(TypeConstructorBinding::new(
                    head,
                    arguments[..prefix_len].to_vec(),
                ))
            } else {
                TypeBinding::Type(ty)
            };
            if let Some(previous) = bindings.get(&parameter) {
                if !self.type_bindings_match(previous, &candidate) {
                    return false;
                }
            } else {
                bindings.insert(parameter, candidate);
            }
        }
        true
    }

    pub(crate) fn instantiate_import_type_binding(
        &mut self,
        template: &crate::ImportedTypeBinding,
        bindings: &PolyTypeBindings,
    ) -> Option<TypeBinding> {
        Some(match template {
            crate::ImportedTypeBinding::Type(ty) => {
                TypeBinding::Type(self.instantiate_import_value_type(ty, bindings)?)
            }
            crate::ImportedTypeBinding::Constructor { head, arguments } => {
                let head = self.imported_constructor_head(head)?;
                let (head, mut fixed) =
                    if let TypeConstructorHead::Parameter { parameter, .. } = head {
                        let TypeBinding::Constructor(binding) = bindings.get(&parameter)? else {
                            return None;
                        };
                        (binding.head, binding.arguments.clone())
                    } else {
                        (head, Vec::new())
                    };
                for ty in arguments {
                    fixed.push(self.instantiate_import_value_type(ty, bindings)?);
                }
                TypeBinding::Constructor(TypeConstructorBinding::new(head, fixed))
            }
        })
    }

    /// Instantiate the instance-head quantifiers while keeping member-local
    /// quantifiers open for comparison with the receiving class contract.
    pub(crate) fn instantiate_import_member_type(
        &mut self,
        ty: &ImportValueType,
        head_bindings: &PolyTypeBindings,
    ) -> Option<GateType> {
        let mut bindings = head_bindings.clone();
        let mut pending = vec![ty];
        while let Some(ty) = pending.pop() {
            match ty {
                ImportValueType::TypeVariable { index, name } => {
                    let parameter = TypeParameterId::from_raw(u32::MAX - *index as u32);
                    bindings.entry(parameter).or_insert_with(|| {
                        TypeBinding::Type(GateType::TypeParameter {
                            parameter,
                            name: name.clone(),
                        })
                    });
                }
                ImportValueType::TypeApplication {
                    index, arguments, ..
                } => {
                    let parameter = TypeParameterId::from_raw(u32::MAX - *index as u32);
                    bindings.entry(parameter).or_insert_with(|| {
                        TypeBinding::Constructor(TypeConstructorBinding::new(
                            TypeConstructorHead::Parameter {
                                parameter,
                                arity: arguments.len(),
                            },
                            Vec::new(),
                        ))
                    });
                    pending.extend(arguments);
                }
                ImportValueType::Arrow { parameter, result } => {
                    pending.extend([parameter.as_ref(), result.as_ref()])
                }
                ImportValueType::Tuple(elements) => pending.extend(elements),
                ImportValueType::Record(fields) => {
                    pending.extend(fields.iter().map(|field| &field.ty))
                }
                ImportValueType::List(element)
                | ImportValueType::Set(element)
                | ImportValueType::Option(element)
                | ImportValueType::Signal(element) => pending.push(element),
                ImportValueType::Map { key, value } => {
                    pending.extend([key.as_ref(), value.as_ref()])
                }
                ImportValueType::Result { error, value }
                | ImportValueType::Validation { error, value }
                | ImportValueType::Task { error, value } => {
                    pending.extend([error.as_ref(), value.as_ref()])
                }
                ImportValueType::Named { arguments, .. } => pending.extend(arguments),
                ImportValueType::Primitive(_) => {}
            }
        }
        self.instantiate_import_value_type(ty, &bindings)
    }

    fn instantiate_import_value_type(
        &mut self,
        ty: &ImportValueType,
        bindings: &PolyTypeBindings,
    ) -> Option<GateType> {
        Some(match ty {
            ImportValueType::TypeVariable { index, .. } => {
                let TypeBinding::Type(ty) =
                    bindings.get(&TypeParameterId::from_raw(u32::MAX - *index as u32))?
                else {
                    return None;
                };
                ty.clone()
            }
            ImportValueType::TypeApplication {
                index, arguments, ..
            } => {
                let TypeBinding::Constructor(binding) =
                    bindings.get(&TypeParameterId::from_raw(u32::MAX - *index as u32))?
                else {
                    return None;
                };
                let head = binding.head;
                let mut fixed = binding.arguments.clone();
                for ty in arguments {
                    fixed.push(self.instantiate_import_value_type(ty, bindings)?);
                }
                if fixed.len() != type_constructor_arity(head, self.module) {
                    return None;
                }
                self.apply_type_constructor(head, &fixed, &mut Vec::new())?
            }
            ImportValueType::Primitive(builtin) => GateType::Primitive(*builtin),
            ImportValueType::Tuple(elements) => GateType::Tuple(
                elements
                    .iter()
                    .map(|ty| self.instantiate_import_value_type(ty, bindings))
                    .collect::<Option<Vec<_>>>()?,
            ),
            ImportValueType::Record(fields) => GateType::Record(
                fields
                    .iter()
                    .map(|field| {
                        Some(GateRecordField {
                            name: field.name.to_string(),
                            ty: self.instantiate_import_value_type(&field.ty, bindings)?,
                        })
                    })
                    .collect::<Option<Vec<_>>>()?,
            ),
            ImportValueType::Arrow { parameter, result } => GateType::Arrow {
                parameter: Box::new(self.instantiate_import_value_type(parameter, bindings)?),
                result: Box::new(self.instantiate_import_value_type(result, bindings)?),
            },
            ImportValueType::List(ty) => {
                GateType::List(Box::new(self.instantiate_import_value_type(ty, bindings)?))
            }
            ImportValueType::Option(ty) => {
                GateType::Option(Box::new(self.instantiate_import_value_type(ty, bindings)?))
            }
            ImportValueType::Set(ty) => {
                GateType::Set(Box::new(self.instantiate_import_value_type(ty, bindings)?))
            }
            ImportValueType::Signal(ty) => {
                GateType::Signal(Box::new(self.instantiate_import_value_type(ty, bindings)?))
            }
            ImportValueType::Map { key, value } => GateType::Map {
                key: Box::new(self.instantiate_import_value_type(key, bindings)?),
                value: Box::new(self.instantiate_import_value_type(value, bindings)?),
            },
            ImportValueType::Result { error, value } => GateType::Result {
                error: Box::new(self.instantiate_import_value_type(error, bindings)?),
                value: Box::new(self.instantiate_import_value_type(value, bindings)?),
            },
            ImportValueType::Validation { error, value } => GateType::Validation {
                error: Box::new(self.instantiate_import_value_type(error, bindings)?),
                value: Box::new(self.instantiate_import_value_type(value, bindings)?),
            },
            ImportValueType::Task { error, value } => GateType::Task {
                error: Box::new(self.instantiate_import_value_type(error, bindings)?),
                value: Box::new(self.instantiate_import_value_type(value, bindings)?),
            },
            ImportValueType::Named { arguments, .. } => {
                let arguments = arguments
                    .iter()
                    .map(|ty| self.instantiate_import_value_type(ty, bindings))
                    .collect::<Option<Vec<_>>>()?;
                self.lower_import_value_type(ty)
                    .with_applied_arguments(&arguments)?
            }
        })
    }
}
