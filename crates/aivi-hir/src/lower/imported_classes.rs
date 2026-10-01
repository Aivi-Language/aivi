impl Lowerer<'_> {
    /// Signature projections belong to the consumer arena but keep the original
    /// declaration identity. They are private until an explicit class import
    /// opens their names; prerequisite classes do not leak into the namespace.
    fn project_imported_classes(&mut self) {
        let definitions = self.module.imported_class_definitions.clone();
        let span = SourceSpan::new(self.module.file(), aivi_base::Span::from(0..0));
        let mut pending = Vec::new();
        for definition in definitions {
            if self.class_projection(&definition.identity).is_some() {
                continue;
            }
            let parameters = definition
                .parameters
                .iter()
                .map(|name| {
                    self.alloc_type_parameter(TypeParameter {
                        span,
                        name: self.make_name(name, span),
                    })
                })
                .collect::<Vec<_>>();
            let Ok(parameters) = NonEmpty::from_vec(parameters) else {
                self.emit_error(
                    span,
                    "imported class has no parameters",
                    code("invalid-imported-class"),
                );
                continue;
            };
            let class = ClassItem {
                header: ItemHeader {
                    span,
                    decorators: Vec::new(),
                },
                identity: definition.identity.clone(),
                name: self.make_name(&definition.name, span),
                parameters,
                superclasses: Vec::new(),
                param_constraints: Vec::new(),
                members: Vec::new(),
            };
            let id = self
                .module
                .arenas
                .items
                .alloc(Item::Class(class))
                .unwrap_or_else(|_| {
                    self.emit_arena_overflow("HIR class projection arena");
                    std::process::exit(1);
                });
            pending.push((id, definition));
        }
        // Allocate every class first so mutually referenced signature catalogs
        // can resolve constraints without recursively projecting declarations.
        for (id, definition) in pending {
            let Item::Class(mut class) = self.module.items()[id].clone() else {
                unreachable!()
            };
            let parameters = class.parameters.iter().copied().collect::<Vec<_>>();
            let projected = (|| -> Result<(), String> {
                class.superclasses = self.project_class_constraints(
                    &definition.superclasses,
                    &parameters,
                    definition.source_module.as_deref(),
                    span,
                )?;
                class.param_constraints = self.project_class_constraints(
                    &definition.param_constraints,
                    &parameters,
                    definition.source_module.as_deref(),
                    span,
                )?;
                for member in &definition.members {
                    let local = member
                        .type_parameters
                        .iter()
                        .map(|name| {
                            self.alloc_type_parameter(TypeParameter {
                                span,
                                name: self.make_name(name, span),
                            })
                        })
                        .collect::<Vec<_>>();
                    let mut scope = parameters.clone();
                    scope.extend(&local);
                    let annotation = self.project_class_type(
                        &member.ty,
                        &scope,
                        definition.source_module.as_deref(),
                        span,
                    )?;
                    let context = self.project_class_constraints(
                        &member.context,
                        &scope,
                        definition.source_module.as_deref(),
                        span,
                    )?;
                    class.members.push(ClassMember {
                        span,
                        name: self.make_name(&member.name, span),
                        type_parameters: local,
                        context,
                        annotation,
                    });
                }
                Ok(())
            })();
            match projected {
                Ok(()) => {
                    *self
                        .module
                        .arenas
                        .items
                        .get_mut(id)
                        .expect("allocated class") = Item::Class(class)
                }
                Err(reason) => self.emit_error(
                    span,
                    format!("cannot import class `{}`: {reason}", definition.name),
                    code("invalid-imported-class"),
                ),
            }
        }
    }

    fn class_projection(&self, identity: &crate::ClassIdentity) -> Option<ItemId> {
        self.module
            .items()
            .iter()
            .find_map(|(id, item)| match item {
                Item::Class(class) if &class.identity == identity => Some(id),
                _ => None,
            })
    }

    fn register_imported_class(
        &mut self,
        identity: &crate::ClassIdentity,
        name: &Name,
        span: SourceSpan,
        namespaces: &mut Namespaces,
    ) {
        self.project_imported_classes();
        let Some(id) = self.class_projection(identity) else {
            self.emit_error(
                span,
                format!("imported class `{}` has no signature metadata", name.text()),
                code("invalid-imported-class"),
            );
            return;
        };
        if !namespaces
            .type_items
            .get(name.text())
            .is_some_and(|sites| sites.iter().any(|site| site.value == id))
        {
            insert_site(&mut namespaces.type_items, name.text(), id, span);
            insert_site(&mut namespaces.any_items, name.text(), id, span);
        }
        let Item::Class(class) = &self.module.items()[id] else {
            unreachable!()
        };
        for (member_index, member) in class.members.iter().enumerate() {
            let resolution = crate::ClassMemberResolution {
                class: id,
                member_index,
            };
            if !namespaces
                .class_terms
                .get(member.name.text())
                .is_some_and(|sites| sites.iter().any(|site| site.value == resolution))
            {
                insert_site(
                    &mut namespaces.class_terms,
                    member.name.text(),
                    resolution,
                    span,
                );
            }
        }
    }

    fn project_class_constraints(
        &mut self,
        constraints: &[crate::ImportedClassConstraint],
        parameters: &[TypeParameterId],
        source_module: Option<&str>,
        span: SourceSpan,
    ) -> Result<Vec<TypeId>, String> {
        constraints
            .iter()
            .map(|constraint| {
                let id = self
                    .class_projection(&constraint.class_identity)
                    .ok_or_else(|| {
                        format!(
                            "prerequisite class `{}` has no signature metadata",
                            constraint.class_name
                        )
                    })?;
                let callee = self.project_type_reference(
                    &constraint.class_name,
                    TypeResolution::Item(id),
                    span,
                );
                let subject = match &constraint.subject {
                    crate::ImportedTypeBinding::Type(ty) => {
                        self.project_class_type(ty, parameters, source_module, span)?
                    }
                    crate::ImportedTypeBinding::Constructor { head, arguments } => {
                        let callee = match head {
                            crate::ImportedTypeConstructor::Builtin(builtin) => self
                                .project_type_reference(
                                    &format!("{builtin:?}"),
                                    TypeResolution::Builtin(*builtin),
                                    span,
                                ),
                            crate::ImportedTypeConstructor::Parameter { index, name, .. } => {
                                let parameter = parameters.get(*index).ok_or_else(|| {
                                    format!(
                                        "constructor parameter `{name}` is outside its binder scope"
                                    )
                                })?;
                                self.project_type_reference(
                                    name,
                                    TypeResolution::TypeParameter(*parameter),
                                    span,
                                )
                            }
                            crate::ImportedTypeConstructor::Named {
                                name,
                                arity,
                                definition,
                                origin,
                            } => {
                                let import = self.project_class_named_type(
                                    name,
                                    *arity,
                                    origin.as_ref(),
                                    definition.as_deref(),
                                    source_module,
                                    span,
                                );
                                self.project_type_reference(
                                    name,
                                    TypeResolution::Import(import),
                                    span,
                                )
                            }
                        };
                        let arguments = arguments
                            .iter()
                            .map(|ty| self.project_class_type(ty, parameters, source_module, span))
                            .collect::<Result<Vec<_>, _>>()?;
                        self.project_type_application(callee, arguments, span)
                    }
                };
                Ok(self.project_type_application(callee, vec![subject], span))
            })
            .collect()
    }

    fn project_type_reference(
        &mut self,
        name: &str,
        resolution: TypeResolution,
        span: SourceSpan,
    ) -> TypeId {
        let name = self.make_name(name, span);
        let path = self.make_path(&[name]);
        self.alloc_type(TypeNode {
            span,
            kind: TypeKind::Name(TypeReference {
                path,
                resolution: ResolutionState::Resolved(resolution),
            }),
        })
    }

    fn project_type_application(
        &mut self,
        callee: TypeId,
        arguments: Vec<TypeId>,
        span: SourceSpan,
    ) -> TypeId {
        match NonEmpty::from_vec(arguments) {
            Ok(arguments) => self.alloc_type(TypeNode {
                span,
                kind: TypeKind::Apply { callee, arguments },
            }),
            Err(_) => callee,
        }
    }

    fn project_class_named_type(
        &mut self,
        name: &str,
        arity: usize,
        origin: Option<&crate::ImportedTypeOrigin>,
        definition: Option<&crate::ImportTypeDefinition>,
        source_module: Option<&str>,
        span: SourceSpan,
    ) -> ImportId {
        let source_module = origin
            .and_then(|origin| origin.source_module.as_deref())
            .or(source_module);
        let name = origin.map_or(name, |origin| origin.name());
        if let Some((id, _)) = self.module.imports().iter().find(|(_, import)| {
            origin.is_some_and(|origin| {
                import
                    .metadata
                    .type_origin()
                    .is_some_and(|candidate| candidate.identity == origin.identity)
            }) || (origin.is_none()
                && import.source_module.as_deref() == source_module
                && import.imported_name.text() == name
                && matches!(
                    import.metadata,
                    ImportBindingMetadata::TypeConstructor { .. }
                        | ImportBindingMetadata::Domain { .. }
                        | ImportBindingMetadata::BuiltinType(_)
                ))
        }) {
            return id;
        }
        let metadata = match definition {
            Some(crate::ImportTypeDefinition::Domain(carrier)) => ImportBindingMetadata::Domain {
                origin: origin.cloned(),
                kind: Kind::constructor(arity),
                literal_suffixes: Vec::new(),
                carrier: Some(carrier.clone()),
            },
            _ => ImportBindingMetadata::TypeConstructor {
                origin: origin.cloned(),
                type_item: None,
                constructors: None,
                kind: Kind::constructor(arity),
                fields: None,
                definition: definition.cloned(),
            },
        };
        self.alloc_import(ImportBinding {
            span,
            source_module: source_module.map(Into::into),
            imported_name: self.make_name(name, span),
            local_name: self.make_name(&format!("__class_type_{name}"), span),
            resolution: ImportBindingResolution::Resolved,
            metadata,
            callable_type: None,
            deprecation: None,
        })
    }

    /// Portable signatures use indexed binders. Build consumer-owned type nodes
    /// with a postorder worklist, preserving method-local and class binder IDs.
    fn project_class_type(
        &mut self,
        ty: &ImportValueType,
        parameters: &[TypeParameterId],
        source_module: Option<&str>,
        span: SourceSpan,
    ) -> Result<TypeId, String> {
        enum Work<'a> {
            Visit(&'a ImportValueType),
            Finish(&'a ImportValueType, usize),
        }
        let mut work = vec![Work::Visit(ty)];
        let mut values = Vec::new();
        let mut steps = 0;
        while let Some(step) = work.pop() {
            steps += 1;
            if steps > 131_072 {
                return Err("signature exceeds the compiler type complexity limit".into());
            }
            let (ty, children) = match step {
                Work::Visit(ty) => {
                    let children = match ty {
                        ImportValueType::Primitive(_) | ImportValueType::TypeVariable { .. } => {
                            Vec::new()
                        }
                        ImportValueType::Tuple(children)
                        | ImportValueType::TypeApplication {
                            arguments: children,
                            ..
                        }
                        | ImportValueType::Named {
                            arguments: children,
                            ..
                        } => children.iter().collect::<Vec<_>>(),
                        ImportValueType::Record(fields) => {
                            fields.iter().map(|field| &field.ty).collect()
                        }
                        ImportValueType::Arrow { parameter, result } => {
                            vec![parameter.as_ref(), result.as_ref()]
                        }
                        ImportValueType::List(child)
                        | ImportValueType::Set(child)
                        | ImportValueType::Option(child)
                        | ImportValueType::Signal(child) => vec![child.as_ref()],
                        ImportValueType::Map { key, value } => vec![key.as_ref(), value.as_ref()],
                        ImportValueType::Result { error, value }
                        | ImportValueType::Validation { error, value }
                        | ImportValueType::Task { error, value } => {
                            vec![error.as_ref(), value.as_ref()]
                        }
                    };
                    work.push(Work::Finish(ty, children.len()));
                    work.extend(children.into_iter().rev().map(Work::Visit));
                    continue;
                }
                Work::Finish(ty, count) => (ty, values.split_off(values.len() - count)),
            };
            let id = match ty {
                ImportValueType::Primitive(builtin) => self.project_type_reference(
                    &format!("{builtin:?}"),
                    TypeResolution::Builtin(*builtin),
                    span,
                ),
                ImportValueType::TypeVariable { index, name } => {
                    let parameter = parameters.get(*index).ok_or_else(|| {
                        format!("type parameter `{name}` is outside its binder scope")
                    })?;
                    self.project_type_reference(
                        name,
                        TypeResolution::TypeParameter(*parameter),
                        span,
                    )
                }
                ImportValueType::TypeApplication { index, name, .. } => {
                    let parameter = parameters.get(*index).ok_or_else(|| {
                        format!("constructor parameter `{name}` is outside its binder scope")
                    })?;
                    let callee = self.project_type_reference(
                        name,
                        TypeResolution::TypeParameter(*parameter),
                        span,
                    );
                    self.project_type_application(callee, children, span)
                }
                ImportValueType::Named {
                    type_name,
                    definition,
                    origin,
                    ..
                } => {
                    let import = self.project_class_named_type(
                        type_name,
                        children.len(),
                        origin.as_ref(),
                        definition.as_deref(),
                        source_module,
                        span,
                    );
                    let callee = self.project_type_reference(
                        type_name,
                        TypeResolution::Import(import),
                        span,
                    );
                    self.project_type_application(callee, children, span)
                }
                ImportValueType::Tuple(_) => {
                    let elements = AtLeastTwo::from_vec(children)
                        .map_err(|_| "imported tuple needs at least two elements".to_owned())?;
                    self.alloc_type(TypeNode {
                        span,
                        kind: TypeKind::Tuple(elements),
                    })
                }
                ImportValueType::Record(fields) => {
                    let fields = fields
                        .iter()
                        .zip(children)
                        .map(|(field, ty)| TypeField {
                            span,
                            label: self.make_name(&field.name, span),
                            ty,
                        })
                        .collect();
                    self.alloc_type(TypeNode {
                        span,
                        kind: TypeKind::Record(fields),
                    })
                }
                ImportValueType::Arrow { .. } => self.alloc_type(TypeNode {
                    span,
                    kind: TypeKind::Arrow {
                        parameter: children[0],
                        result: children[1],
                    },
                }),
                _ => {
                    let builtin = match ty {
                        ImportValueType::List(_) => BuiltinType::List,
                        ImportValueType::Set(_) => BuiltinType::Set,
                        ImportValueType::Option(_) => BuiltinType::Option,
                        ImportValueType::Signal(_) => BuiltinType::Signal,
                        ImportValueType::Map { .. } => BuiltinType::Map,
                        ImportValueType::Result { .. } => BuiltinType::Result,
                        ImportValueType::Validation { .. } => BuiltinType::Validation,
                        ImportValueType::Task { .. } => BuiltinType::Task,
                        _ => unreachable!(),
                    };
                    let callee = self.project_type_reference(
                        &format!("{builtin:?}"),
                        TypeResolution::Builtin(builtin),
                        span,
                    );
                    self.project_type_application(callee, children, span)
                }
            };
            values.push(id);
        }
        Ok(values.pop().expect("one projected type"))
    }
}
