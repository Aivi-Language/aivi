use std::collections::HashMap;

use crate::{
    BuiltinTerm, BuiltinType, DecoratorPayload, DeprecatedDecorator, DeprecationNotice,
    DomainMemberKind, ExportItem, ExportResolution, ImportBindingMetadata, ImportBundleKind,
    ImportId, ImportRecordField, ImportSumVariant, ImportTypeDefinition, ImportValueType,
    ImportedDomainLiteralSuffix, Item, ItemId, LiteralSuffixBase, Module, RecordExpr,
    ResolutionState, SumConstructorHandle, TypeId, TypeItemBody, TypeKind, TypeParameterId,
    TypeReference, TypeResolution,
};

/// The kind of an exported name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ExportedNameKind {
    Type,
    Value,
    Function,
    Signal,
    Class,
    Domain,
    SourceProvider,
    Instance,
}

/// A single exported name from a module.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExportedName {
    pub name: String,
    pub kind: ExportedNameKind,
    pub metadata: ImportBindingMetadata,
    pub callable_type: Option<ImportValueType>,
    pub deprecation: Option<DeprecationNotice>,
}

/// The complete set of names exported from a module.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExportedNames {
    pub names: Vec<ExportedName>,
    pub instances: Vec<ExportedInstanceDeclaration>,
    pub classes: Vec<crate::ImportedClassDefinition>,
}

impl ExportedNames {
    pub fn find(&self, name: &str) -> Option<&ExportedName> {
        self.names.iter().find(|exported| exported.name == name)
    }

    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &ExportedName> {
        self.names.iter()
    }

    pub fn len(&self) -> usize {
        self.names.len()
    }
}

fn domain_suffix_base(module: &Module, annotation: TypeId) -> Option<LiteralSuffixBase> {
    let type_node = module.types().get(annotation)?;
    let parameter = match &type_node.kind {
        TypeKind::Arrow { parameter, .. } => *parameter,
        _ => annotation,
    };
    match import_value_type(module, parameter)? {
        ImportValueType::Primitive(BuiltinType::Int) => Some(LiteralSuffixBase::Int),
        ImportValueType::Primitive(BuiltinType::Decimal) => Some(LiteralSuffixBase::Decimal),
        _ => None,
    }
}

/// A class instance declaration exported from a module, carrying enough
/// metadata for the importing module to resolve cross-module class instances.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExportedInstanceDeclaration {
    /// Declaring implementation module, retained through re-exports. `None`
    /// lets standalone resolvers use the immediate import module as the owner.
    pub source_module: Option<Box<str>>,
    pub class_identity: crate::ClassIdentity,
    pub class_name: Box<str>,
    pub subject: Box<str>,
    pub head: crate::ImportedTypeBinding,
    pub context: Vec<crate::ImportedClassConstraint>,
    pub members: Vec<ExportedInstanceMember>,
}

/// One member of an exported class instance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExportedInstanceMember {
    pub name: Box<str>,
    pub ty: ImportValueType,
    pub evidence: Vec<crate::ImportedClassEvidence>,
    pub instance_evidence_count: usize,
}

/// Extract the set of names exported from a HIR module.
///
/// Explicit `export` declarations narrow the set; if there are none, all
/// top-level named items are considered exported.
pub fn exports(module: &Module) -> ExportedNames {
    let mut names = if module
        .root_items()
        .iter()
        .any(|item_id| matches!(module.items().get(*item_id), Some(Item::Export(_))))
    {
        explicit_exported_names(module)
    } else {
        implicit_exported_names(module)
    };

    names.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| exported_kind_rank(left.kind).cmp(&exported_kind_rank(right.kind)))
    });
    let instances = collect_instance_declarations(module, &names);
    let classes = collect_class_definitions(module);
    ExportedNames {
        names,
        instances,
        classes,
    }
}

fn collect_class_definitions(module: &Module) -> Vec<crate::ImportedClassDefinition> {
    let mut definitions = module.imported_class_definitions.clone();
    if !module
        .root_items()
        .iter()
        .any(|id| matches!(module.items()[*id], Item::Class(_)))
    {
        return definitions;
    }
    let mut typing = crate::validate::GateTypeContext::new(module);
    for id in module.root_items() {
        let Item::Class(class) = &module.items()[*id] else {
            continue;
        };
        let parameters = class
            .parameters
            .iter()
            .copied()
            .enumerate()
            .map(|(index, parameter)| (parameter, index))
            .collect::<TypeParamMap>();
        let export_constraints = |typing: &mut crate::validate::GateTypeContext<'_>,
                                  constraints: &[TypeId],
                                  parameters: &TypeParamMap| {
            constraints
                .iter()
                .map(|constraint| {
                    let binding =
                        typing.open_class_constraint_binding(*constraint, &HashMap::new())?;
                    let Item::Class(required) = &module.items()[binding.class_item] else {
                        return None;
                    };
                    Some(crate::ImportedClassConstraint {
                        class_identity: required.identity.clone(),
                        class_name: required.name.text().into(),
                        subject: export_type_binding(module, &binding.subject, parameters)?,
                    })
                })
                .collect::<Option<Vec<_>>>()
        };
        let Some(superclasses) = export_constraints(&mut typing, &class.superclasses, &parameters)
        else {
            continue;
        };
        let Some(param_constraints) =
            export_constraints(&mut typing, &class.param_constraints, &parameters)
        else {
            continue;
        };
        let members = class
            .members
            .iter()
            .map(|member| {
                let mut parameters = parameters.clone();
                for parameter in &member.type_parameters {
                    let index = parameters.len();
                    parameters.insert(*parameter, index);
                }
                Some(crate::ImportedClassMember {
                    selection_span: member.name.span(),
                    name: member.name.text().into(),
                    type_parameters: member
                        .type_parameters
                        .iter()
                        .map(|id| module.type_parameters()[*id].name.text().into())
                        .collect(),
                    context: export_constraints(&mut typing, &member.context, &parameters)?,
                    ty: poly_import_value_type(module, member.annotation, &parameters)?,
                })
            })
            .collect::<Option<Vec<_>>>();
        let Some(members) = members else {
            continue;
        };
        if !definitions
            .iter()
            .any(|definition| definition.identity == class.identity)
        {
            definitions.push(crate::ImportedClassDefinition {
                identity: class.identity.clone(),
                selection_span: class.name.span(),
                source_module: module.source_module.clone(),
                name: class.name.text().into(),
                parameters: class
                    .parameters
                    .iter()
                    .map(|id| module.type_parameters()[*id].name.text().into())
                    .collect(),
                superclasses,
                param_constraints,
                members,
            });
        }
    }
    definitions
}

fn explicit_exported_names(module: &Module) -> Vec<ExportedName> {
    let mut names = Vec::new();
    for &id in module.root_items() {
        let Some(Item::Export(export)) = module.items().get(id) else {
            continue;
        };
        if let ResolutionState::Resolved(ExportResolution::ImportedConstructor(pair)) =
            export.resolution
        {
            let name = export.target.segments().first().text();
            for import in [pair.carrier(), pair.constructor()] {
                if let Some(exported) = re_exported_import_name(module, import, name) {
                    push_unique_exported_name(&mut names, exported);
                }
            }
            continue;
        }
        let Some(exported) = export_item_to_exported_name(module, export) else {
            continue;
        };
        push_unique_exported_name(&mut names, exported);
    }
    names
}

fn implicit_exported_names(module: &Module) -> Vec<ExportedName> {
    let mut names = Vec::new();
    for &id in module.root_items() {
        let Some(item) = module.items().get(id) else {
            continue;
        };
        if let Some(exported) = item_to_exported_name(module, id, item) {
            push_unique_exported_name(&mut names, exported);
        }
        // For sum types, also export each constructor individually so that
        // `use module (ConstructorName)` works for modules using implicit exports.
        if let Item::Type(type_item) = item
            && let TypeItemBody::Sum(variants) = &type_item.body
        {
            let deprecation = item_deprecation_notice(module, item);
            let type_param_map: TypeParamMap = type_item
                .parameters
                .iter()
                .enumerate()
                .map(|(i, &p)| (p, i))
                .collect();
            let result_args: Vec<ImportValueType> = type_item
                .parameters
                .iter()
                .enumerate()
                .map(|(i, &p)| {
                    let name = module
                        .type_parameters()
                        .get(p)
                        .map(|param| param.name.text().to_owned())
                        .unwrap_or_else(|| format!("T{}", i + 1));
                    ImportValueType::TypeVariable { index: i, name }
                })
                .collect();
            for variant in variants.iter() {
                let name = variant.name.text().to_owned();
                let metadata = builtin_term_metadata(&name).unwrap_or_else(|| {
                    let owner_type_name: String = type_item.name.text().into();
                    if variant.fields.is_empty() {
                        ImportBindingMetadata::ConstructorValue {
                            variant_name: variant.name.text().to_owned(),
                            ty: ImportValueType::Named {
                                origin: module.type_origin(id),
                                type_name: owner_type_name,
                                arguments: result_args.clone(),
                                definition: None,
                            },
                        }
                    } else {
                        let result = ImportValueType::Named {
                            origin: module.type_origin(id),
                            type_name: owner_type_name,
                            arguments: result_args.clone(),
                            definition: None,
                        };
                        let ty = variant.fields.iter().rev().fold(result, |acc, field| {
                            let param_ty =
                                poly_import_value_type(module, field.ty, &type_param_map)
                                    .unwrap_or(ImportValueType::Named {
                                        origin: None,
                                        type_name: "Unknown".into(),
                                        arguments: Vec::new(),
                                        definition: None,
                                    });
                            ImportValueType::Arrow {
                                parameter: Box::new(param_ty),
                                result: Box::new(acc),
                            }
                        });
                        ImportBindingMetadata::ConstructorValue {
                            variant_name: variant.name.text().to_owned(),
                            ty,
                        }
                    }
                });
                push_unique_exported_name(
                    &mut names,
                    ExportedName {
                        name,
                        kind: ExportedNameKind::Value,
                        metadata,
                        callable_type: None,
                        deprecation: deprecation.clone(),
                    },
                );
            }
        }
    }
    names
}

fn push_unique_exported_name(names: &mut Vec<ExportedName>, exported: ExportedName) {
    if names
        .iter()
        .any(|existing| existing.name == exported.name && existing.kind == exported.kind)
    {
        return;
    }
    names.push(exported);
}

fn export_item_to_exported_name(module: &Module, export: &ExportItem) -> Option<ExportedName> {
    let ResolutionState::Resolved(resolution) = export.resolution else {
        return None;
    };
    let exported_name = export.target.segments().first().text().to_owned();
    match resolution {
        ExportResolution::BuiltinType(builtin) => Some(ExportedName {
            name: exported_name,
            kind: ExportedNameKind::Type,
            metadata: ImportBindingMetadata::BuiltinType(builtin),
            callable_type: None,
            deprecation: None,
        }),
        ExportResolution::BuiltinTerm(builtin) => Some(ExportedName {
            name: exported_name,
            kind: ExportedNameKind::Value,
            metadata: ImportBindingMetadata::BuiltinTerm(builtin),
            callable_type: None,
            deprecation: None,
        }),
        ExportResolution::Item(item_id) => {
            explicit_item_exported_name(module, item_id, exported_name.as_str())
        }
        ExportResolution::Import(import_id) => {
            re_exported_import_name(module, import_id, exported_name.as_str())
        }
        ExportResolution::ImportedConstructor(_) => None,
    }
}

fn re_exported_import_name(
    module: &Module,
    import_id: ImportId,
    exported_name: &str,
) -> Option<ExportedName> {
    let import = module.imports().get(import_id)?;
    let kind = match &import.metadata {
        ImportBindingMetadata::Class { .. } => ExportedNameKind::Class,
        ImportBindingMetadata::TypeConstructor { .. }
        | ImportBindingMetadata::Domain { .. }
        | ImportBindingMetadata::BuiltinType(_)
        | ImportBindingMetadata::AmbientType { .. } => ExportedNameKind::Type,
        _ => ExportedNameKind::Value,
    };
    Some(ExportedName {
        name: exported_name.to_owned(),
        kind,
        metadata: import.metadata.clone(),
        callable_type: import.callable_type.clone(),
        deprecation: import.deprecation.clone(),
    })
}

fn explicit_item_exported_name(
    module: &Module,
    item_id: ItemId,
    exported_name: &str,
) -> Option<ExportedName> {
    let item = module.items().get(item_id)?;
    if item_has_test_decorator(module, item) {
        return None;
    }
    let ambient = module.ambient_items().contains(&item_id);
    let deprecation = item_deprecation_notice(module, item);
    match item {
        Item::Type(item) => {
            if item.name.text() == exported_name {
                let metadata = if ambient {
                    ImportBindingMetadata::AmbientType {
                        origin: module
                            .type_origin(item_id)
                            .expect("ambient data declaration origin"),
                    }
                } else {
                    let fields = extract_type_record_fields(module, item_id, item);
                    let definition = extract_type_definition(module, item_id, item);
                    ImportBindingMetadata::TypeConstructor {
                        origin: module.type_origin(item_id),
                        type_item: Some(item_id),
                        constructors: extract_type_sum_constructors(module, item_id, item),
                        kind: aivi_typing::Kind::constructor(item.parameters.len()),
                        fields,
                        definition,
                    }
                };
                return Some(ExportedName {
                    name: exported_name.to_owned(),
                    kind: ExportedNameKind::Type,
                    metadata,
                    callable_type: None,
                    deprecation,
                });
            }

            let TypeItemBody::Sum(variants) = &item.body else {
                return None;
            };
            let type_param_map: TypeParamMap = item
                .parameters
                .iter()
                .enumerate()
                .map(|(i, &p)| (p, i))
                .collect();
            let result_args: Vec<ImportValueType> = item
                .parameters
                .iter()
                .enumerate()
                .map(|(i, &p)| {
                    let name = module
                        .type_parameters()
                        .get(p)
                        .map(|param| param.name.text().to_owned())
                        .unwrap_or_else(|| format!("T{}", i + 1));
                    ImportValueType::TypeVariable { index: i, name }
                })
                .collect();
            variants
                .iter()
                .find(|variant| variant.name.text() == exported_name)
                .map(|variant| {
                    let metadata = builtin_term_metadata(exported_name).unwrap_or_else(|| {
                        // For non-builtin sum constructors, store the owner type name so
                        // that `import_value_type` can return a non-None GateType for them.
                        // Zero-arg constructors have no Arrow wrapper; constructors with
                        // fields wrap the field types in Arrow chains.
                        let owner_type_name: String = item.name.text().into();
                        if variant.fields.is_empty() {
                            ImportBindingMetadata::ConstructorValue {
                                variant_name: variant.name.text().to_owned(),
                                ty: ImportValueType::Named {
                                    origin: module.type_origin(item_id),
                                    type_name: owner_type_name,
                                    arguments: result_args.clone(),
                                    definition: None,
                                },
                            }
                        } else {
                            // Multi-field constructors: build Arrow chain over field types,
                            // returning the owner Named type with proper type-variable arguments.
                            let result = ImportValueType::Named {
                                origin: module.type_origin(item_id),
                                type_name: owner_type_name,
                                arguments: result_args.clone(),
                                definition: None,
                            };
                            let ty = variant.fields.iter().rev().fold(result, |acc, field| {
                                let param_ty =
                                    poly_import_value_type(module, field.ty, &type_param_map)
                                        .unwrap_or(ImportValueType::Named {
                                            origin: module.type_origin(item_id),
                                            type_name: "Unknown".into(),
                                            arguments: Vec::new(),
                                            definition: None,
                                        });
                                ImportValueType::Arrow {
                                    parameter: Box::new(param_ty),
                                    result: Box::new(acc),
                                }
                            });
                            ImportBindingMetadata::ConstructorValue {
                                variant_name: variant.name.text().to_owned(),
                                ty,
                            }
                        }
                    });
                    ExportedName {
                        name: exported_name.to_owned(),
                        kind: ExportedNameKind::Value,
                        metadata,
                        callable_type: None,
                        deprecation,
                    }
                })
        }
        Item::Class(item) => Some(ExportedName {
            name: exported_name.to_owned(),
            kind: ExportedNameKind::Class,
            metadata: ImportBindingMetadata::Class {
                identity: item.identity.clone(),
            },
            callable_type: None,
            deprecation,
        }),
        Item::Domain(item) => (item.name.text() == exported_name).then(|| ExportedName {
            name: exported_name.to_owned(),
            kind: ExportedNameKind::Domain,
            metadata: if ambient {
                ImportBindingMetadata::AmbientType {
                    origin: module
                        .type_origin(item_id)
                        .expect("ambient domain declaration origin"),
                }
            } else {
                let literal_suffixes = item
                    .members
                    .iter()
                    .enumerate()
                    .filter(|&(_i, m)| {
                        m.kind == DomainMemberKind::Literal && m.name.text().chars().count() >= 2
                    })
                    .filter_map(|(i, m)| {
                        Some(ImportedDomainLiteralSuffix {
                            name: m.name.text().into(),
                            member_index: i,
                            base: domain_suffix_base(module, m.annotation)?,
                            callable_type: import_value_type(module, m.annotation),
                        })
                    })
                    .collect();
                ImportBindingMetadata::Domain {
                    origin: module.type_origin(item_id),
                    kind: aivi_typing::Kind::constructor(item.parameters.len()),
                    literal_suffixes,
                    carrier: import_value_type(module, item.carrier),
                }
            },
            callable_type: None,
            deprecation,
        }),
        Item::Value(item) => (item.name.text() == exported_name).then(|| ExportedName {
            name: exported_name.to_owned(),
            kind: ExportedNameKind::Value,
            metadata: exported_value_metadata(module, item_id, item.annotation),
            callable_type: None,
            deprecation,
        }),
        Item::Function(item) => (item.name.text() == exported_name).then(|| {
            let callable_type = exported_function_type(module, item);
            let metadata = match &callable_type {
                Some(ty) => {
                    match crate::general_expr_elaboration::export_function_evidence(module, item) {
                        Some(evidence) if !evidence.is_empty() => {
                            ImportBindingMetadata::ConstrainedValue {
                                ty: ty.clone(),
                                evidence,
                            }
                        }
                        Some(_) => ImportBindingMetadata::Value { ty: ty.clone() },
                        None => ImportBindingMetadata::OpaqueValue,
                    }
                }
                None => ImportBindingMetadata::OpaqueValue,
            };
            ExportedName {
                name: exported_name.to_owned(),
                kind: ExportedNameKind::Function,
                metadata,
                callable_type,
                deprecation,
            }
        }),
        Item::Signal(item) => (item.name.text() == exported_name
            && !item.is_source_capability_handle)
            .then(|| ExportedName {
                name: exported_name.to_owned(),
                kind: ExportedNameKind::Signal,
                metadata: exported_value_metadata(module, item_id, item.annotation),
                callable_type: None,
                deprecation,
            }),
        Item::SourceProviderContract(_)
        | Item::Instance(_)
        | Item::Use(_)
        | Item::Export(_)
        | Item::Hoist(_) => None,
    }
}

fn item_to_exported_name(module: &Module, item_id: ItemId, item: &Item) -> Option<ExportedName> {
    if item_has_test_decorator(module, item) {
        return None;
    }
    let deprecation = item_deprecation_notice(module, item);
    match item {
        Item::Type(item) => Some(ExportedName {
            name: item.name.text().to_owned(),
            kind: ExportedNameKind::Type,
            metadata: ImportBindingMetadata::TypeConstructor {
                origin: module.type_origin(item_id),
                type_item: Some(item_id),
                constructors: extract_type_sum_constructors(module, item_id, item),
                kind: aivi_typing::Kind::constructor(item.parameters.len()),
                fields: extract_type_record_fields(module, item_id, item),
                definition: extract_type_definition(module, item_id, item),
            },
            callable_type: None,
            deprecation,
        }),
        Item::Value(item) => Some(ExportedName {
            name: item.name.text().to_owned(),
            kind: ExportedNameKind::Value,
            metadata: exported_value_metadata(module, item_id, item.annotation),
            callable_type: None,
            deprecation,
        }),
        Item::Function(item) => {
            let callable_type = exported_function_type(module, item);
            let metadata = match &callable_type {
                Some(ty) => {
                    match crate::general_expr_elaboration::export_function_evidence(module, item) {
                        Some(evidence) if !evidence.is_empty() => {
                            ImportBindingMetadata::ConstrainedValue {
                                ty: ty.clone(),
                                evidence,
                            }
                        }
                        Some(_) => ImportBindingMetadata::Value { ty: ty.clone() },
                        None => ImportBindingMetadata::OpaqueValue,
                    }
                }
                None => ImportBindingMetadata::OpaqueValue,
            };
            Some(ExportedName {
                name: item.name.text().to_owned(),
                kind: ExportedNameKind::Function,
                metadata,
                callable_type,
                deprecation,
            })
        }
        Item::Signal(item) => (!item.is_source_capability_handle).then(|| ExportedName {
            name: item.name.text().to_owned(),
            kind: ExportedNameKind::Signal,
            metadata: exported_value_metadata(module, item_id, item.annotation),
            callable_type: None,
            deprecation,
        }),
        Item::Class(item) => Some(ExportedName {
            name: item.name.text().to_owned(),
            kind: ExportedNameKind::Class,
            metadata: ImportBindingMetadata::Class {
                identity: item.identity.clone(),
            },
            callable_type: None,
            deprecation,
        }),
        Item::Domain(item) => Some(ExportedName {
            name: item.name.text().to_owned(),
            kind: ExportedNameKind::Domain,
            metadata: {
                let literal_suffixes = item
                    .members
                    .iter()
                    .enumerate()
                    .filter(|&(_i, m)| {
                        m.kind == DomainMemberKind::Literal && m.name.text().chars().count() >= 2
                    })
                    .filter_map(|(i, m)| {
                        Some(ImportedDomainLiteralSuffix {
                            name: m.name.text().into(),
                            member_index: i,
                            base: domain_suffix_base(module, m.annotation)?,
                            callable_type: import_value_type(module, m.annotation),
                        })
                    })
                    .collect();
                ImportBindingMetadata::Domain {
                    origin: module.type_origin(item_id),
                    kind: aivi_typing::Kind::constructor(item.parameters.len()),
                    literal_suffixes,
                    carrier: import_value_type(module, item.carrier),
                }
            },
            callable_type: None,
            deprecation,
        }),
        Item::SourceProviderContract(_)
        | Item::Instance(_)
        | Item::Use(_)
        | Item::Export(_)
        | Item::Hoist(_) => None,
    }
}

fn exported_value_metadata(
    module: &Module,
    item_id: ItemId,
    annotation: Option<TypeId>,
) -> ImportBindingMetadata {
    annotation
        .and_then(|annotation| import_value_type(module, annotation))
        .or_else(|| inferred_item_import_value_type(module, item_id))
        .map(|ty| ImportBindingMetadata::Value { ty })
        .unwrap_or(ImportBindingMetadata::OpaqueValue)
}

fn inferred_item_import_value_type(module: &Module, item_id: ItemId) -> Option<ImportValueType> {
    let mut typing = crate::typecheck_context::GateTypeContext::new(module);
    let ty = typing.item_value_type(item_id)?;
    gate_type_import_value_type(module, &ty)
}

fn gate_type_import_value_type(module: &Module, ty: &crate::GateType) -> Option<ImportValueType> {
    poly_gate_type_import_value_type(module, ty, &HashMap::new())
}

pub(crate) fn poly_gate_type_import_value_type(
    module: &Module,
    ty: &crate::GateType,
    parameters: &HashMap<TypeParameterId, usize>,
) -> Option<ImportValueType> {
    match ty {
        crate::GateType::TypeApplication {
            parameter,
            name,
            arguments,
        } => Some(ImportValueType::TypeApplication {
            index: *parameters.get(parameter)?,
            name: name.clone(),
            arguments: arguments
                .iter()
                .map(|a| poly_gate_type_import_value_type(module, a, parameters))
                .collect::<Option<Vec<_>>>()?,
        }),
        crate::GateType::Primitive(builtin) => primitive_import_value_type_from_builtin(*builtin),
        crate::GateType::TypeParameter { parameter, name } => Some(ImportValueType::TypeVariable {
            index: *parameters.get(parameter)?,
            name: name.clone(),
        }),
        crate::GateType::Tuple(elements) => Some(ImportValueType::Tuple(
            elements
                .iter()
                .map(|ty| poly_gate_type_import_value_type(module, ty, parameters))
                .collect::<Option<Vec<_>>>()?,
        )),
        crate::GateType::Record(fields) => Some(ImportValueType::Record(
            fields
                .iter()
                .map(|field| {
                    Some(ImportRecordField {
                        name: field.name.clone().into_boxed_str(),
                        ty: poly_gate_type_import_value_type(module, &field.ty, parameters)?,
                    })
                })
                .collect::<Option<Vec<_>>>()?,
        )),
        crate::GateType::Arrow { parameter, result } => Some(ImportValueType::Arrow {
            parameter: Box::new(poly_gate_type_import_value_type(
                module, parameter, parameters,
            )?),
            result: Box::new(poly_gate_type_import_value_type(
                module, result, parameters,
            )?),
        }),
        crate::GateType::List(element) => Some(ImportValueType::List(Box::new(
            poly_gate_type_import_value_type(module, element, parameters)?,
        ))),
        crate::GateType::Map { key, value } => Some(ImportValueType::Map {
            key: Box::new(poly_gate_type_import_value_type(module, key, parameters)?),
            value: Box::new(poly_gate_type_import_value_type(module, value, parameters)?),
        }),
        crate::GateType::Set(element) => Some(ImportValueType::Set(Box::new(
            poly_gate_type_import_value_type(module, element, parameters)?,
        ))),
        crate::GateType::Option(element) => Some(ImportValueType::Option(Box::new(
            poly_gate_type_import_value_type(module, element, parameters)?,
        ))),
        crate::GateType::Result { error, value } => Some(ImportValueType::Result {
            error: Box::new(poly_gate_type_import_value_type(module, error, parameters)?),
            value: Box::new(poly_gate_type_import_value_type(module, value, parameters)?),
        }),
        crate::GateType::Validation { error, value } => Some(ImportValueType::Validation {
            error: Box::new(poly_gate_type_import_value_type(module, error, parameters)?),
            value: Box::new(poly_gate_type_import_value_type(module, value, parameters)?),
        }),
        crate::GateType::Signal(payload) => Some(ImportValueType::Signal(Box::new(
            poly_gate_type_import_value_type(module, payload, parameters)?,
        ))),
        crate::GateType::Task { error, value } => Some(ImportValueType::Task {
            error: Box::new(poly_gate_type_import_value_type(module, error, parameters)?),
            value: Box::new(poly_gate_type_import_value_type(module, value, parameters)?),
        }),
        crate::GateType::Domain {
            item,
            name,
            arguments,
            ..
        }
        | crate::GateType::OpaqueItem {
            item,
            name,
            arguments,
            ..
        } => Some(ImportValueType::Named {
            origin: module.type_origin(*item),
            type_name: name.clone(),
            arguments: arguments
                .iter()
                .map(|ty| poly_gate_type_import_value_type(module, ty, parameters))
                .collect::<Option<Vec<_>>>()?,
            definition: None,
        }),
        crate::GateType::OpaqueImport {
            import,
            name,
            arguments,
            definition,
            origin,
        } => Some(ImportValueType::Named {
            origin: module
                .imports()
                .get(*import)
                .and_then(|binding| binding.metadata.type_origin().cloned())
                .or_else(|| {
                    origin.as_ref().map(|identity| crate::ImportedTypeOrigin {
                        identity: identity.as_ref().clone(),
                        source_module: None,
                    })
                }),
            type_name: name.clone(),
            arguments: arguments
                .iter()
                .map(|ty| poly_gate_type_import_value_type(module, ty, parameters))
                .collect::<Option<Vec<_>>>()?,
            definition: definition.clone(),
        }),
    }
}

/// Collect all instance declarations from a module for cross-module instance resolution.
fn collect_instance_declarations(
    module: &Module,
    names: &[ExportedName],
) -> Vec<ExportedInstanceDeclaration> {
    let mut declarations = Vec::new();
    let mut typing = crate::validate::GateTypeContext::new(module);
    let evidence_catalog =
        crate::general_expr_elaboration::ClassEvidenceCatalog::for_instance_exports(module);
    for &item_id in module.root_items() {
        let Some(Item::Instance(instance)) = module.items().get(item_id) else {
            continue;
        };
        let ResolutionState::Resolved(TypeResolution::Item(class_item_id)) =
            instance.class.resolution.as_ref()
        else {
            continue;
        };
        let Item::Class(class_item) = &module.items()[*class_item_id] else {
            continue;
        };
        let class_name: Box<str> = instance.class.path.segments().last().text().into();
        let Some(subject_type_id) = instance.arguments.iter().next().copied() else {
            continue;
        };
        let Some(subject) = type_label_for_export(module, subject_type_id) else {
            continue;
        };
        let parameters = instance
            .type_parameters
            .iter()
            .copied()
            .enumerate()
            .map(|(index, parameter)| (parameter, index))
            .collect::<TypeParamMap>();
        let Some(mut head_binding) =
            typing.open_poly_type_binding(subject_type_id, &HashMap::new())
        else {
            continue;
        };
        if let crate::TypeBinding::Type(witness @ crate::GateType::TypeParameter { .. }) =
            &head_binding
        {
            let Some(binding) = typing.class_member_subject_binding(
                crate::ClassMemberResolution {
                    class: *class_item_id,
                    member_index: 0,
                },
                witness,
            ) else {
                continue;
            };
            head_binding = binding;
        }
        let Some(head) = export_type_binding(module, &head_binding, &parameters) else {
            continue;
        };
        let context = instance
            .context
            .iter()
            .map(|constraint| {
                let binding = typing.open_class_constraint_binding(*constraint, &HashMap::new())?;
                let Item::Class(class) = &module.items()[binding.class_item] else {
                    return None;
                };
                Some(crate::ImportedClassConstraint {
                    class_identity: class.identity.clone(),
                    class_name: class.name.text().into(),
                    subject: export_type_binding(module, &binding.subject, &parameters)?,
                })
            })
            .collect::<Option<Vec<_>>>();
        let Some(context) = context else {
            continue;
        };
        let members = instance
            .members
            .iter()
            .enumerate()
            .filter_map(|(member_index, member)| {
                let signature = class_item
                    .members
                    .iter()
                    .find(|signature| signature.name.text() == member.name.text())?;
                let mut member_parameters = parameters.clone();
                for parameter in &signature.type_parameters {
                    let next = member_parameters.len();
                    member_parameters.entry(*parameter).or_insert(next);
                }
                let ty = member
                    .annotation
                    .and_then(|annotation| import_value_type(module, annotation))
                    .or_else(|| {
                        class_item
                            .members
                            .iter()
                            .find(|class_member| class_member.name.text() == member.name.text())
                            .and_then(|class_member| {
                                exported_instance_class_member_type(
                                    module,
                                    class_member.annotation,
                                    &class_item.parameters,
                                    &instance.arguments,
                                    &member_parameters,
                                )
                            })
                    })
                    .or_else(|| exported_instance_member_type(module, member))?;
                Some(ExportedInstanceMember {
                    name: member.name.text().into(),
                    ty,
                    evidence: evidence_catalog.export_instance_member_evidence(
                        module,
                        item_id,
                        member_index,
                        &member_parameters,
                    )?,
                    instance_evidence_count: evidence_catalog
                        .instance_evidence_count(item_id, member_index)?,
                })
            })
            .collect();
        declarations.push(ExportedInstanceDeclaration {
            source_module: module.source_module.clone(),
            class_identity: class_item.identity.clone(),
            class_name,
            subject,
            head,
            context,
            members,
        });
    }
    // Re-exported classes and carriers retain their dictionaries. Private
    // imports do not enlarge the public evidence environment. Keep the original
    // implementation owner so a façade forwards its callables.
    for (_, import) in module.imports().iter() {
        let ImportBindingMetadata::InstanceMember {
            class_identity,
            class_name,
            member_name,
            subject,
            head,
            context,
            evidence,
            instance_evidence_count,
            ty,
        } = &import.metadata
        else {
            continue;
        };
        if !names.iter().any(|exported| {
            matches!(&exported.metadata, ImportBindingMetadata::Class { identity }
                if identity == class_identity)
                || exported_instance_carrier_matches(head, &exported.metadata)
        }) {
            continue;
        }
        let member = ExportedInstanceMember {
            name: member_name.clone(),
            ty: ty.clone(),
            evidence: evidence.clone(),
            instance_evidence_count: *instance_evidence_count,
        };
        if let Some(declaration) = declarations.iter_mut().find(|declaration| {
            declaration.source_module == import.source_module
                && declaration.class_identity == *class_identity
                && declaration.head == *head
                && declaration.context == *context
        }) {
            if !declaration.members.contains(&member) {
                declaration.members.push(member);
            }
        } else {
            declarations.push(ExportedInstanceDeclaration {
                source_module: import.source_module.clone(),
                class_identity: class_identity.clone(),
                class_name: class_name.clone(),
                subject: subject.clone(),
                head: head.clone(),
                context: context.clone(),
                members: vec![member],
            });
        }
    }
    declarations
}

fn exported_instance_carrier_matches(
    head: &crate::ImportedTypeBinding,
    exported: &ImportBindingMetadata,
) -> bool {
    use crate::{ImportedTypeBinding, ImportedTypeConstructor};
    let (origin, builtin) = match head {
        ImportedTypeBinding::Constructor { head, .. } => match head {
            ImportedTypeConstructor::Named { origin, .. } => (origin.as_ref(), None),
            ImportedTypeConstructor::Builtin(builtin) => (None, Some(*builtin)),
            ImportedTypeConstructor::Parameter { .. } => return false,
        },
        ImportedTypeBinding::Type(ty) => match ty {
            ImportValueType::Named { origin, .. } => (origin.as_ref(), None),
            ImportValueType::Primitive(builtin) => (None, Some(*builtin)),
            ImportValueType::List(_) => (None, Some(BuiltinType::List)),
            ImportValueType::Map { .. } => (None, Some(BuiltinType::Map)),
            ImportValueType::Set(_) => (None, Some(BuiltinType::Set)),
            ImportValueType::Option(_) => (None, Some(BuiltinType::Option)),
            ImportValueType::Result { .. } => (None, Some(BuiltinType::Result)),
            ImportValueType::Validation { .. } => (None, Some(BuiltinType::Validation)),
            ImportValueType::Signal(_) => (None, Some(BuiltinType::Signal)),
            ImportValueType::Task { .. } => (None, Some(BuiltinType::Task)),
            _ => return false,
        },
    };
    if let Some(origin) = origin {
        return exported.type_origin() == Some(origin);
    }
    matches!((builtin, exported), (Some(builtin), ImportBindingMetadata::BuiltinType(other))
        if builtin == *other)
}

pub(crate) fn export_type_binding(
    module: &Module,
    binding: &crate::TypeBinding,
    parameters: &TypeParamMap,
) -> Option<crate::ImportedTypeBinding> {
    use crate::{ImportedTypeBinding, ImportedTypeConstructor, TypeBinding, TypeConstructorHead};
    Some(match binding {
        TypeBinding::Type(ty) => ImportedTypeBinding::Type(match ty {
            crate::GateType::OpaqueItem {
                item, arguments, ..
            }
            | crate::GateType::Domain {
                item, arguments, ..
            } => {
                let arguments = arguments
                    .iter()
                    .map(|ty| poly_gate_type_import_value_type(module, ty, parameters))
                    .collect::<Option<Vec<_>>>()?;
                named_import_value_type_from_item(module, *item, arguments, &mut Vec::new())?
            }
            other => poly_gate_type_import_value_type(module, other, parameters)?,
        }),
        TypeBinding::Constructor(binding) => ImportedTypeBinding::Constructor {
            head: match binding.head() {
                TypeConstructorHead::Builtin(builtin) => ImportedTypeConstructor::Builtin(builtin),
                TypeConstructorHead::Item(id) => {
                    let (name, arity) = match &module.items()[id] {
                        Item::Type(ty) => (ty.name.text(), ty.parameters.len()),
                        Item::Domain(domain) => (domain.name.text(), domain.parameters.len()),
                        _ => return None,
                    };
                    let ImportValueType::Named {
                        origin, definition, ..
                    } = named_import_value_type_from_item(module, id, Vec::new(), &mut Vec::new())?
                    else {
                        return None;
                    };
                    ImportedTypeConstructor::Named {
                        origin,
                        name: name.into(),
                        arity,
                        definition,
                    }
                }
                TypeConstructorHead::Import(id) => {
                    let import = &module.imports()[id];
                    let arity = match &import.metadata {
                        ImportBindingMetadata::TypeConstructor { kind, .. }
                        | ImportBindingMetadata::Domain { kind, .. } => kind.arity(),
                        _ => return None,
                    };
                    let ImportValueType::Named {
                        origin, definition, ..
                    } = named_import_value_type_from_import(module, id, Vec::new())?
                    else {
                        return None;
                    };
                    ImportedTypeConstructor::Named {
                        origin,
                        name: import.imported_name.text().into(),
                        arity,
                        definition,
                    }
                }
                TypeConstructorHead::Parameter { parameter, arity } => {
                    ImportedTypeConstructor::Parameter {
                        index: *parameters.get(&parameter)?,
                        name: module.type_parameters()[parameter].name.text().into(),
                        arity,
                    }
                }
            },
            arguments: binding
                .arguments()
                .iter()
                .map(|ty| poly_gate_type_import_value_type(module, ty, parameters))
                .collect::<Option<Vec<_>>>()?,
        },
    })
}

/// Build a portable type for an instance member from its parameters and annotation,
/// falling back to a function type inferred from parameter annotations and body annotation.
fn exported_instance_member_type(
    module: &Module,
    member: &crate::InstanceMember,
) -> Option<ImportValueType> {
    if member.parameters.is_empty() {
        return None;
    }
    let type_param_map: HashMap<TypeParameterId, usize> = HashMap::new();
    let result = member
        .annotation
        .and_then(|annotation| poly_import_value_type(module, annotation, &type_param_map))?;
    let mut ty = result;
    for param in member.parameters.iter().rev() {
        let param_ty = param
            .annotation
            .and_then(|annotation| poly_import_value_type(module, annotation, &type_param_map))?;
        ty = ImportValueType::Arrow {
            parameter: Box::new(param_ty),
            result: Box::new(ty),
        };
    }
    Some(ty)
}

/// Extract a string label for a type, used for cross-module instance subject matching.
fn type_label_for_export(module: &Module, ty: TypeId) -> Option<Box<str>> {
    let type_node = module.types().get(ty)?;
    match &type_node.kind {
        TypeKind::Name(reference) => Some(reference.path.segments().last().text().into()),
        TypeKind::Apply { callee, .. } => type_label_for_export(module, *callee),
        _ => None,
    }
}

type TypeParamSubstitutions = HashMap<TypeParameterId, TypeId>;

fn exported_instance_class_member_type(
    module: &Module,
    member_annotation: TypeId,
    class_parameters: &crate::NonEmpty<TypeParameterId>,
    instance_arguments: &crate::NonEmpty<TypeId>,
    instance_parameters: &TypeParamMap,
) -> Option<ImportValueType> {
    let class_substitutions = class_parameters
        .iter()
        .copied()
        .zip(instance_arguments.iter().copied())
        .collect::<TypeParamSubstitutions>();
    let mut free_params = instance_parameters.clone();
    let mut item_stack = Vec::new();
    exported_instance_member_import_value_type_with_stack(
        module,
        member_annotation,
        &class_substitutions,
        &mut free_params,
        &mut item_stack,
    )
}

fn exported_instance_member_import_value_type_with_stack(
    module: &Module,
    ty: TypeId,
    class_substitutions: &TypeParamSubstitutions,
    free_params: &mut TypeParamMap,
    item_stack: &mut Vec<ItemId>,
) -> Option<ImportValueType> {
    let type_node = module.types().get(ty)?;
    match &type_node.kind {
        TypeKind::Name(reference) => match reference.resolution.as_ref() {
            ResolutionState::Resolved(TypeResolution::Builtin(builtin)) => {
                primitive_import_value_type_from_builtin(*builtin)
            }
            ResolutionState::Resolved(TypeResolution::TypeParameter(param_id)) => {
                if let Some(replacement) = class_substitutions.get(param_id) {
                    return exported_instance_member_import_value_type_with_stack(
                        module,
                        *replacement,
                        class_substitutions,
                        free_params,
                        item_stack,
                    );
                }
                let next_index = free_params.len();
                let index = match free_params.entry(*param_id) {
                    std::collections::hash_map::Entry::Occupied(entry) => *entry.get(),
                    std::collections::hash_map::Entry::Vacant(entry) => {
                        entry.insert(next_index);
                        next_index
                    }
                };
                let name = module
                    .type_parameters()
                    .get(*param_id)
                    .map(|param| param.name.text().to_owned())
                    .unwrap_or_else(|| format!("T{}", index + 1));
                Some(ImportValueType::TypeVariable { index, name })
            }
            ResolutionState::Resolved(TypeResolution::Item(item_id)) => {
                named_import_value_type_from_item(module, *item_id, Vec::new(), item_stack)
            }
            ResolutionState::Resolved(TypeResolution::Import(import_id)) => {
                named_import_value_type_from_import(module, *import_id, Vec::new())
            }
            _ => None,
        },
        TypeKind::Tuple(elements) => Some(ImportValueType::Tuple(
            elements
                .iter()
                .map(|element| {
                    exported_instance_member_import_value_type_with_stack(
                        module,
                        *element,
                        class_substitutions,
                        free_params,
                        item_stack,
                    )
                })
                .collect::<Option<Vec<_>>>()?,
        )),
        TypeKind::Record(fields) => Some(ImportValueType::Record(
            fields
                .iter()
                .map(|field| {
                    Some(ImportRecordField {
                        name: field.label.text().into(),
                        ty: exported_instance_member_import_value_type_with_stack(
                            module,
                            field.ty,
                            class_substitutions,
                            free_params,
                            item_stack,
                        )?,
                    })
                })
                .collect::<Option<Vec<_>>>()?,
        )),
        TypeKind::RecordTransform { transform, source } => {
            let source = exported_instance_member_import_value_type_with_stack(
                module,
                *source,
                class_substitutions,
                free_params,
                item_stack,
            )?;
            apply_record_row_transform_import_value_type(transform, source)
        }
        TypeKind::Arrow { parameter, result } => Some(ImportValueType::Arrow {
            parameter: Box::new(exported_instance_member_import_value_type_with_stack(
                module,
                *parameter,
                class_substitutions,
                free_params,
                item_stack,
            )?),
            result: Box::new(exported_instance_member_import_value_type_with_stack(
                module,
                *result,
                class_substitutions,
                free_params,
                item_stack,
            )?),
        }),
        TypeKind::Apply { callee, arguments } => {
            if let TypeKind::Name(reference) = &module.types()[*callee].kind
                && let ResolutionState::Resolved(TypeResolution::TypeParameter(parameter)) =
                    reference.resolution.as_ref()
                && !class_substitutions.contains_key(parameter)
            {
                let next = free_params.len();
                let index = *free_params.entry(*parameter).or_insert(next);
                return Some(ImportValueType::TypeApplication {
                    index,
                    name: module.type_parameters()[*parameter].name.text().to_owned(),
                    arguments: arguments
                        .iter()
                        .map(|argument| {
                            exported_instance_member_import_value_type_with_stack(
                                module,
                                *argument,
                                class_substitutions,
                                free_params,
                                item_stack,
                            )
                        })
                        .collect::<Option<Vec<_>>>()?,
                });
            }
            exported_instance_member_applied_import_value_type_with_stack(
                module,
                ty,
                class_substitutions,
                free_params,
                item_stack,
            )
        }
    }
}

fn exported_instance_member_applied_import_value_type_with_stack(
    module: &Module,
    ty: TypeId,
    class_substitutions: &TypeParamSubstitutions,
    free_params: &mut TypeParamMap,
    item_stack: &mut Vec<ItemId>,
) -> Option<ImportValueType> {
    let (constructor, arguments) =
        flatten_exported_instance_member_type_application(module, ty, class_substitutions)?;
    match constructor {
        ResolvedTypeConstructor::Builtin(builtin) => match (builtin, arguments.len()) {
            (BuiltinType::List, 1) => Some(ImportValueType::List(Box::new(
                exported_instance_member_import_value_type_with_stack(
                    module,
                    arguments[0],
                    class_substitutions,
                    free_params,
                    item_stack,
                )?,
            ))),
            (BuiltinType::Map, 2) => Some(ImportValueType::Map {
                key: Box::new(exported_instance_member_import_value_type_with_stack(
                    module,
                    arguments[0],
                    class_substitutions,
                    free_params,
                    item_stack,
                )?),
                value: Box::new(exported_instance_member_import_value_type_with_stack(
                    module,
                    arguments[1],
                    class_substitutions,
                    free_params,
                    item_stack,
                )?),
            }),
            (BuiltinType::Set, 1) => Some(ImportValueType::Set(Box::new(
                exported_instance_member_import_value_type_with_stack(
                    module,
                    arguments[0],
                    class_substitutions,
                    free_params,
                    item_stack,
                )?,
            ))),
            (BuiltinType::Option, 1) => Some(ImportValueType::Option(Box::new(
                exported_instance_member_import_value_type_with_stack(
                    module,
                    arguments[0],
                    class_substitutions,
                    free_params,
                    item_stack,
                )?,
            ))),
            (BuiltinType::Result, 2) => Some(ImportValueType::Result {
                error: Box::new(exported_instance_member_import_value_type_with_stack(
                    module,
                    arguments[0],
                    class_substitutions,
                    free_params,
                    item_stack,
                )?),
                value: Box::new(exported_instance_member_import_value_type_with_stack(
                    module,
                    arguments[1],
                    class_substitutions,
                    free_params,
                    item_stack,
                )?),
            }),
            (BuiltinType::Validation, 2) => Some(ImportValueType::Validation {
                error: Box::new(exported_instance_member_import_value_type_with_stack(
                    module,
                    arguments[0],
                    class_substitutions,
                    free_params,
                    item_stack,
                )?),
                value: Box::new(exported_instance_member_import_value_type_with_stack(
                    module,
                    arguments[1],
                    class_substitutions,
                    free_params,
                    item_stack,
                )?),
            }),
            (BuiltinType::Signal, 1) => Some(ImportValueType::Signal(Box::new(
                exported_instance_member_import_value_type_with_stack(
                    module,
                    arguments[0],
                    class_substitutions,
                    free_params,
                    item_stack,
                )?,
            ))),
            (BuiltinType::Task, 2) => Some(ImportValueType::Task {
                error: Box::new(exported_instance_member_import_value_type_with_stack(
                    module,
                    arguments[0],
                    class_substitutions,
                    free_params,
                    item_stack,
                )?),
                value: Box::new(exported_instance_member_import_value_type_with_stack(
                    module,
                    arguments[1],
                    class_substitutions,
                    free_params,
                    item_stack,
                )?),
            }),
            _ => None,
        },
        ResolvedTypeConstructor::Bundle(ImportBundleKind::BuiltinOption)
            if arguments.len() == 1 =>
        {
            Some(ImportValueType::Option(Box::new(
                exported_instance_member_import_value_type_with_stack(
                    module,
                    arguments[0],
                    class_substitutions,
                    free_params,
                    item_stack,
                )?,
            )))
        }
        ResolvedTypeConstructor::Item(item_id) => {
            let arguments = arguments
                .iter()
                .map(|argument| {
                    exported_instance_member_import_value_type_with_stack(
                        module,
                        *argument,
                        class_substitutions,
                        free_params,
                        item_stack,
                    )
                })
                .collect::<Option<Vec<_>>>()?;
            named_import_value_type_from_item(module, item_id, arguments, item_stack)
        }
        ResolvedTypeConstructor::Import(import_id) => {
            let arguments = arguments
                .iter()
                .map(|argument| {
                    exported_instance_member_import_value_type_with_stack(
                        module,
                        *argument,
                        class_substitutions,
                        free_params,
                        item_stack,
                    )
                })
                .collect::<Option<Vec<_>>>()?;
            named_import_value_type_from_import(module, import_id, arguments)
        }
        ResolvedTypeConstructor::Bundle(_) => None,
    }
}

fn flatten_exported_instance_member_type_application(
    module: &Module,
    ty: TypeId,
    class_substitutions: &TypeParamSubstitutions,
) -> Option<(ResolvedTypeConstructor, Vec<TypeId>)> {
    let type_node = module.types().get(ty)?;
    match &type_node.kind {
        TypeKind::Apply { callee, arguments } => {
            let (constructor, mut flattened) = flatten_exported_instance_member_type_application(
                module,
                *callee,
                class_substitutions,
            )?;
            flattened.extend(arguments.iter().copied());
            Some((constructor, flattened))
        }
        TypeKind::Name(reference) => match reference.resolution.as_ref() {
            ResolutionState::Resolved(TypeResolution::TypeParameter(param_id)) => {
                flatten_exported_instance_member_type_application(
                    module,
                    *class_substitutions.get(param_id)?,
                    class_substitutions,
                )
            }
            _ => resolve_type_constructor(module, reference)
                .map(|constructor| (constructor, Vec::new())),
        },
        TypeKind::Tuple(_)
        | TypeKind::Record(_)
        | TypeKind::RecordTransform { .. }
        | TypeKind::Arrow { .. } => None,
    }
}

fn exported_function_type(module: &Module, item: &crate::FunctionItem) -> Option<ImportValueType> {
    // Build a mapping from TypeParameterId → index for polymorphic functions.
    let type_param_map: HashMap<TypeParameterId, usize> = item
        .type_parameters
        .iter()
        .enumerate()
        .map(|(i, &p)| (p, i))
        .collect();
    let mut result = item
        .annotation
        .and_then(|annotation| poly_import_value_type(module, annotation, &type_param_map))?;
    for parameter in item.parameters.iter().rev() {
        let parameter_ty = parameter
            .annotation
            .and_then(|annotation| poly_import_value_type(module, annotation, &type_param_map))?;
        result = ImportValueType::Arrow {
            parameter: Box::new(parameter_ty),
            result: Box::new(result),
        };
    }
    Some(result)
}

fn item_has_test_decorator(module: &Module, item: &Item) -> bool {
    item.decorators().iter().any(|decorator_id| {
        module
            .decorators()
            .get(*decorator_id)
            .is_some_and(|decorator| matches!(decorator.payload, DecoratorPayload::Test(_)))
    })
}

fn item_deprecation_notice(module: &Module, item: &Item) -> Option<DeprecationNotice> {
    item.decorators().iter().find_map(|decorator_id| {
        let decorator = module.decorators().get(*decorator_id)?;
        let DecoratorPayload::Deprecated(deprecated) = &decorator.payload else {
            return None;
        };
        Some(deprecation_notice(module, deprecated))
    })
}

fn deprecation_notice(module: &Module, deprecated: &DeprecatedDecorator) -> DeprecationNotice {
    DeprecationNotice {
        message: deprecated
            .message
            .and_then(|message| module.expr_static_text(message)),
        replacement: deprecated.options.and_then(|options| {
            let expr = module.exprs().get(options)?;
            let crate::ExprKind::Record(RecordExpr { fields }) = &expr.kind else {
                return None;
            };
            fields
                .iter()
                .find(|field| field.label.text() == "replacement")
                .and_then(|field| module.expr_static_text(field.value))
        }),
    }
}

pub(crate) fn import_value_type(module: &Module, ty: TypeId) -> Option<ImportValueType> {
    let mut item_stack = Vec::new();
    import_value_type_with_stack(module, ty, &mut item_stack)
}

fn import_value_type_with_stack(
    module: &Module,
    ty: TypeId,
    item_stack: &mut Vec<ItemId>,
) -> Option<ImportValueType> {
    let type_node = module.types().get(ty)?;
    match &type_node.kind {
        TypeKind::Name(reference) => {
            // First try builtins.
            if let Some(prim) = primitive_import_value_type(reference) {
                return Some(prim);
            }
            // For user-defined types, emit a Named entry so field projection
            // can be resolved in importing modules via lower_import_value_type.
            match reference.resolution.as_ref() {
                ResolutionState::Resolved(TypeResolution::Item(item_id)) => {
                    named_import_value_type_from_item(module, *item_id, Vec::new(), item_stack)
                }
                ResolutionState::Resolved(TypeResolution::Import(import_id)) => {
                    named_import_value_type_from_import(module, *import_id, Vec::new())
                }
                _ => None,
            }
        }
        TypeKind::Tuple(elements) => Some(ImportValueType::Tuple(
            elements
                .iter()
                .map(|element| import_value_type_with_stack(module, *element, item_stack))
                .collect::<Option<Vec<_>>>()?,
        )),
        TypeKind::Record(fields) => Some(ImportValueType::Record(
            fields
                .iter()
                .map(|field| {
                    Some(ImportRecordField {
                        name: field.label.text().into(),
                        ty: import_value_type_with_stack(module, field.ty, item_stack)?,
                    })
                })
                .collect::<Option<Vec<_>>>()?,
        )),
        TypeKind::RecordTransform { transform, source } => {
            let source = import_value_type_with_stack(module, *source, item_stack)?;
            apply_record_row_transform_import_value_type(transform, source)
        }
        TypeKind::Arrow { parameter, result } => Some(ImportValueType::Arrow {
            parameter: Box::new(import_value_type_with_stack(
                module, *parameter, item_stack,
            )?),
            result: Box::new(import_value_type_with_stack(module, *result, item_stack)?),
        }),
        TypeKind::Apply { .. } => applied_import_value_type(module, ty, item_stack),
    }
}

fn named_import_value_type_from_item(
    module: &Module,
    item_id: ItemId,
    arguments: Vec<ImportValueType>,
    item_stack: &mut Vec<ItemId>,
) -> Option<ImportValueType> {
    let item = module.items().get(item_id)?;
    let type_name = item_type_name(item);
    let definition = if item_stack.contains(&item_id) {
        None
    } else {
        match item {
            Item::Type(type_item) => {
                item_stack.push(item_id);
                let definition =
                    extract_type_definition_with_stack(module, type_item, item_stack).map(Box::new);
                let popped = item_stack.pop();
                debug_assert_eq!(popped, Some(item_id));
                definition
            }
            Item::Domain(domain) => {
                item_stack.push(item_id);
                let parameters = domain
                    .parameters
                    .iter()
                    .enumerate()
                    .map(|(index, parameter)| (*parameter, index))
                    .collect();
                let definition = poly_import_value_type_with_stack(
                    module,
                    domain.carrier,
                    &parameters,
                    item_stack,
                )
                .map(ImportTypeDefinition::Domain)
                .map(Box::new);
                item_stack.pop();
                definition
            }
            _ => None,
        }
    };
    Some(ImportValueType::Named {
        origin: module.type_origin(item_id),
        type_name,
        arguments,
        definition,
    })
}

fn named_import_value_type_from_import(
    module: &Module,
    import_id: ImportId,
    arguments: Vec<ImportValueType>,
) -> Option<ImportValueType> {
    let binding = module.imports().get(import_id)?;
    let definition = match &binding.metadata {
        ImportBindingMetadata::TypeConstructor {
            definition: Some(definition),
            ..
        } => Some(Box::new(definition.clone())),
        ImportBindingMetadata::Domain {
            carrier: Some(carrier),
            ..
        } => Some(Box::new(ImportTypeDefinition::Domain(carrier.clone()))),
        _ => None,
    };
    Some(ImportValueType::Named {
        origin: binding.metadata.type_origin().cloned(),
        type_name: binding.metadata.type_origin().map_or_else(
            || binding.imported_name.text().to_owned(),
            |origin| origin.name().to_owned(),
        ),
        arguments,
        definition,
    })
}

fn primitive_import_value_type(reference: &TypeReference) -> Option<ImportValueType> {
    let ResolutionState::Resolved(TypeResolution::Builtin(builtin)) = reference.resolution.as_ref()
    else {
        return None;
    };
    match builtin {
        crate::BuiltinType::Int
        | crate::BuiltinType::Float
        | crate::BuiltinType::Decimal
        | crate::BuiltinType::BigInt
        | crate::BuiltinType::Bool
        | crate::BuiltinType::Text
        | crate::BuiltinType::Unit
        | crate::BuiltinType::Bytes => Some(ImportValueType::Primitive(*builtin)),
        crate::BuiltinType::List
        | crate::BuiltinType::Map
        | crate::BuiltinType::Set
        | crate::BuiltinType::Option
        | crate::BuiltinType::Result
        | crate::BuiltinType::Validation
        | crate::BuiltinType::Signal
        | crate::BuiltinType::Task => None,
    }
}

fn applied_import_value_type(
    module: &Module,
    ty: TypeId,
    item_stack: &mut Vec<ItemId>,
) -> Option<ImportValueType> {
    let (constructor, arguments) = flatten_type_application(module, ty)?;
    match constructor {
        ResolvedTypeConstructor::Builtin(crate::BuiltinType::List) if arguments.len() == 1 => {
            Some(ImportValueType::List(Box::new(
                import_value_type_with_stack(module, arguments[0], item_stack)?,
            )))
        }
        ResolvedTypeConstructor::Builtin(crate::BuiltinType::Map) if arguments.len() == 2 => {
            Some(ImportValueType::Map {
                key: Box::new(import_value_type_with_stack(
                    module,
                    arguments[0],
                    item_stack,
                )?),
                value: Box::new(import_value_type_with_stack(
                    module,
                    arguments[1],
                    item_stack,
                )?),
            })
        }
        ResolvedTypeConstructor::Builtin(crate::BuiltinType::Set) if arguments.len() == 1 => {
            Some(ImportValueType::Set(Box::new(
                import_value_type_with_stack(module, arguments[0], item_stack)?,
            )))
        }
        ResolvedTypeConstructor::Builtin(crate::BuiltinType::Option) if arguments.len() == 1 => {
            Some(ImportValueType::Option(Box::new(
                import_value_type_with_stack(module, arguments[0], item_stack)?,
            )))
        }
        ResolvedTypeConstructor::Builtin(crate::BuiltinType::Result) if arguments.len() == 2 => {
            Some(ImportValueType::Result {
                error: Box::new(import_value_type_with_stack(
                    module,
                    arguments[0],
                    item_stack,
                )?),
                value: Box::new(import_value_type_with_stack(
                    module,
                    arguments[1],
                    item_stack,
                )?),
            })
        }
        ResolvedTypeConstructor::Builtin(crate::BuiltinType::Validation)
            if arguments.len() == 2 =>
        {
            Some(ImportValueType::Validation {
                error: Box::new(import_value_type_with_stack(
                    module,
                    arguments[0],
                    item_stack,
                )?),
                value: Box::new(import_value_type_with_stack(
                    module,
                    arguments[1],
                    item_stack,
                )?),
            })
        }
        ResolvedTypeConstructor::Builtin(crate::BuiltinType::Signal) if arguments.len() == 1 => {
            Some(ImportValueType::Signal(Box::new(
                import_value_type_with_stack(module, arguments[0], item_stack)?,
            )))
        }
        ResolvedTypeConstructor::Builtin(crate::BuiltinType::Task) if arguments.len() == 2 => {
            Some(ImportValueType::Task {
                error: Box::new(import_value_type_with_stack(
                    module,
                    arguments[0],
                    item_stack,
                )?),
                value: Box::new(import_value_type_with_stack(
                    module,
                    arguments[1],
                    item_stack,
                )?),
            })
        }
        ResolvedTypeConstructor::Bundle(ImportBundleKind::BuiltinOption)
            if arguments.len() == 1 =>
        {
            Some(ImportValueType::Option(Box::new(import_value_type(
                module,
                arguments[0],
            )?)))
        }
        ResolvedTypeConstructor::Item(item_id) => {
            let lowered_arguments = arguments
                .iter()
                .map(|argument| import_value_type_with_stack(module, *argument, item_stack))
                .collect::<Option<Vec<_>>>()?;
            named_import_value_type_from_item(module, item_id, lowered_arguments, item_stack)
        }
        ResolvedTypeConstructor::Import(import_id) => {
            let lowered_arguments = arguments
                .iter()
                .map(|argument| import_value_type_with_stack(module, *argument, item_stack))
                .collect::<Option<Vec<_>>>()?;
            named_import_value_type_from_import(module, import_id, lowered_arguments)
        }
        ResolvedTypeConstructor::Builtin(_) | ResolvedTypeConstructor::Bundle(_) => None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ResolvedTypeConstructor {
    Builtin(crate::BuiltinType),
    Bundle(ImportBundleKind),
    Item(ItemId),
    Import(ImportId),
}

fn item_type_name(item: &Item) -> String {
    match item {
        Item::Type(item) => item.name.text().to_owned(),
        Item::Class(item) => item.name.text().to_owned(),
        Item::Domain(item) => item.name.text().to_owned(),
        Item::SourceProviderContract(item) => {
            item.provider.key().unwrap_or("<provider>").to_owned()
        }
        other => format!("{:?}", other.kind()),
    }
}

fn flatten_type_application(
    module: &Module,
    ty: TypeId,
) -> Option<(ResolvedTypeConstructor, Vec<TypeId>)> {
    let type_node = module.types().get(ty)?;
    match &type_node.kind {
        TypeKind::Apply { callee, arguments } => {
            let (constructor, mut flattened) = flatten_type_application(module, *callee)?;
            flattened.extend(arguments.iter().copied());
            Some((constructor, flattened))
        }
        TypeKind::Name(reference) => {
            Some((resolve_type_constructor(module, reference)?, Vec::new()))
        }
        TypeKind::Tuple(_)
        | TypeKind::Record(_)
        | TypeKind::RecordTransform { .. }
        | TypeKind::Arrow { .. } => None,
    }
}

fn apply_record_row_transform_import_value_type(
    transform: &crate::RecordRowTransform,
    source: ImportValueType,
) -> Option<ImportValueType> {
    let ImportValueType::Record(fields) = source else {
        return None;
    };
    let field_index = fields
        .iter()
        .enumerate()
        .map(|(index, field)| (field.name.as_ref(), index))
        .collect::<std::collections::HashMap<_, _>>();
    match transform {
        crate::RecordRowTransform::Pick(labels) => labels
            .iter()
            .map(|label| fields.get(*field_index.get(label.text())?).cloned())
            .collect::<Option<Vec<_>>>()
            .map(ImportValueType::Record),
        crate::RecordRowTransform::Omit(labels) => {
            let omitted = labels
                .iter()
                .map(|label| field_index.get(label.text()).copied())
                .collect::<Option<std::collections::HashSet<_>>>()?;
            Some(ImportValueType::Record(
                fields
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| !omitted.contains(index))
                    .map(|(_, field)| field.clone())
                    .collect(),
            ))
        }
        crate::RecordRowTransform::Optional(labels)
        | crate::RecordRowTransform::Defaulted(labels) => Some(ImportValueType::Record(
            fields
                .iter()
                .map(|field| {
                    if labels
                        .iter()
                        .any(|label| label.text() == field.name.as_ref())
                    {
                        ImportRecordField {
                            name: field.name.clone(),
                            ty: match &field.ty {
                                ImportValueType::Option(_) => field.ty.clone(),
                                other => ImportValueType::Option(Box::new(other.clone())),
                            },
                        }
                    } else {
                        field.clone()
                    }
                })
                .collect(),
        )),
        crate::RecordRowTransform::Required(labels) => Some(ImportValueType::Record(
            fields
                .iter()
                .map(|field| {
                    if labels
                        .iter()
                        .any(|label| label.text() == field.name.as_ref())
                    {
                        ImportRecordField {
                            name: field.name.clone(),
                            ty: match &field.ty {
                                ImportValueType::Option(inner) => inner.as_ref().clone(),
                                other => other.clone(),
                            },
                        }
                    } else {
                        field.clone()
                    }
                })
                .collect(),
        )),
        crate::RecordRowTransform::Rename(renames) => {
            let renamed = renames
                .iter()
                .map(|rename| Some((field_index.get(rename.from.text()).copied()?, rename)))
                .collect::<Option<std::collections::HashMap<_, _>>>()?;
            let mut result = Vec::with_capacity(fields.len());
            let mut seen = std::collections::HashSet::with_capacity(fields.len());
            for (index, field) in fields.iter().enumerate() {
                let name = renamed
                    .get(&index)
                    .map(|rename| rename.to.text().to_owned().into_boxed_str())
                    .unwrap_or_else(|| field.name.clone());
                if !seen.insert(name.clone()) {
                    return None;
                }
                result.push(ImportRecordField {
                    name,
                    ty: field.ty.clone(),
                });
            }
            Some(ImportValueType::Record(result))
        }
    }
}

fn resolve_type_constructor(
    module: &Module,
    reference: &TypeReference,
) -> Option<ResolvedTypeConstructor> {
    match reference.resolution.as_ref() {
        ResolutionState::Resolved(TypeResolution::Builtin(builtin)) => {
            Some(ResolvedTypeConstructor::Builtin(*builtin))
        }
        ResolutionState::Resolved(TypeResolution::Item(item_id)) => {
            Some(ResolvedTypeConstructor::Item(*item_id))
        }
        ResolutionState::Resolved(TypeResolution::Import(import_id)) => match &module.imports()
            [*import_id]
            .metadata
        {
            ImportBindingMetadata::BuiltinType(builtin) => {
                Some(ResolvedTypeConstructor::Builtin(*builtin))
            }
            ImportBindingMetadata::Bundle(bundle) => Some(ResolvedTypeConstructor::Bundle(*bundle)),
            ImportBindingMetadata::TypeConstructor { .. }
            | ImportBindingMetadata::Domain { .. }
            | ImportBindingMetadata::AmbientType { .. } => {
                Some(ResolvedTypeConstructor::Import(*import_id))
            }
            ImportBindingMetadata::Unknown
            | ImportBindingMetadata::Value { .. }
            | ImportBindingMetadata::ConstructorValue { .. }
            | ImportBindingMetadata::ConstrainedValue { .. }
            | ImportBindingMetadata::IntrinsicValue { .. }
            | ImportBindingMetadata::OpaqueValue
            | ImportBindingMetadata::AmbientValue { .. }
            | ImportBindingMetadata::DomainSuffix { .. }
            | ImportBindingMetadata::BuiltinTerm(_)
            | ImportBindingMetadata::Class { .. }
            | ImportBindingMetadata::InstanceMember { .. } => None,
        },
        ResolutionState::Resolved(TypeResolution::TypeParameter(_))
        | ResolutionState::Unresolved => None,
    }
}

// ---------------------------------------------------------------------------
// Polymorphic-aware import value type conversion
// ---------------------------------------------------------------------------

type TypeParamMap = HashMap<TypeParameterId, usize>;

/// Convert a HIR type to `ImportValueType`, supporting type parameters and named types.
fn poly_import_value_type(
    module: &Module,
    ty: TypeId,
    params: &TypeParamMap,
) -> Option<ImportValueType> {
    let mut item_stack = Vec::new();
    poly_import_value_type_with_stack(module, ty, params, &mut item_stack)
}

fn poly_import_value_type_with_stack(
    module: &Module,
    ty: TypeId,
    params: &TypeParamMap,
    item_stack: &mut Vec<ItemId>,
) -> Option<ImportValueType> {
    let type_node = module.types().get(ty)?;
    match &type_node.kind {
        TypeKind::Name(reference) => {
            poly_name_import_value_type_with_stack(module, reference, params, item_stack)
        }
        TypeKind::Tuple(elements) => Some(ImportValueType::Tuple(
            elements
                .iter()
                .map(|element| {
                    poly_import_value_type_with_stack(module, *element, params, item_stack)
                })
                .collect::<Option<Vec<_>>>()?,
        )),
        TypeKind::Record(fields) => Some(ImportValueType::Record(
            fields
                .iter()
                .map(|field| {
                    Some(ImportRecordField {
                        name: field.label.text().into(),
                        ty: poly_import_value_type_with_stack(
                            module, field.ty, params, item_stack,
                        )?,
                    })
                })
                .collect::<Option<Vec<_>>>()?,
        )),
        TypeKind::RecordTransform { transform, source } => {
            let source = poly_import_value_type_with_stack(module, *source, params, item_stack)?;
            apply_record_row_transform_import_value_type(transform, source)
        }
        TypeKind::Arrow { parameter, result } => Some(ImportValueType::Arrow {
            parameter: Box::new(poly_import_value_type_with_stack(
                module, *parameter, params, item_stack,
            )?),
            result: Box::new(poly_import_value_type_with_stack(
                module, *result, params, item_stack,
            )?),
        }),
        TypeKind::Apply { callee, arguments } => {
            if let TypeKind::Name(reference) = &module.types()[*callee].kind
                && let ResolutionState::Resolved(TypeResolution::TypeParameter(parameter)) =
                    reference.resolution.as_ref()
            {
                return Some(ImportValueType::TypeApplication {
                    index: *params.get(parameter)?,
                    name: module.type_parameters()[*parameter].name.text().to_owned(),
                    arguments: arguments
                        .iter()
                        .map(|arg| {
                            poly_import_value_type_with_stack(module, *arg, params, item_stack)
                        })
                        .collect::<Option<Vec<_>>>()?,
                });
            }
            poly_applied_import_value_type_with_stack(module, ty, params, item_stack)
        }
    }
}

/// Handle a bare name reference: builtin primitives, type parameters, or same-module items.
fn poly_name_import_value_type_with_stack(
    module: &Module,
    reference: &TypeReference,
    params: &TypeParamMap,
    item_stack: &mut Vec<ItemId>,
) -> Option<ImportValueType> {
    match reference.resolution.as_ref() {
        ResolutionState::Resolved(TypeResolution::Builtin(builtin)) => {
            primitive_import_value_type_from_builtin(*builtin)
        }
        ResolutionState::Resolved(TypeResolution::TypeParameter(param_id)) => {
            let &index = params.get(param_id)?;
            let name = module
                .type_parameters()
                .get(*param_id)
                .map(|p| p.name.text().to_owned())
                .unwrap_or_else(|| format!("T{}", index + 1));
            Some(ImportValueType::TypeVariable { index, name })
        }
        ResolutionState::Resolved(TypeResolution::Item(item_id)) => {
            named_import_value_type_from_item(module, *item_id, Vec::new(), item_stack)
        }
        ResolutionState::Resolved(TypeResolution::Import(import_id)) => {
            named_import_value_type_from_import(module, *import_id, Vec::new())
        }
        _ => None,
    }
}

fn primitive_import_value_type_from_builtin(
    builtin: crate::BuiltinType,
) -> Option<ImportValueType> {
    match builtin {
        crate::BuiltinType::Int
        | crate::BuiltinType::Float
        | crate::BuiltinType::Decimal
        | crate::BuiltinType::BigInt
        | crate::BuiltinType::Bool
        | crate::BuiltinType::Text
        | crate::BuiltinType::Unit
        | crate::BuiltinType::Bytes => Some(ImportValueType::Primitive(builtin)),
        _ => None,
    }
}

/// Handle type application: builtin constructors, same-module named types, and named fallback.
fn poly_applied_import_value_type_with_stack(
    module: &Module,
    ty: TypeId,
    params: &TypeParamMap,
    item_stack: &mut Vec<ItemId>,
) -> Option<ImportValueType> {
    let (constructor, arguments) = poly_flatten_type_application(module, ty)?;
    // Try builtin constructor first
    match &constructor {
        PolyTypeConstructor::Resolved(ResolvedTypeConstructor::Builtin(builtin)) => {
            let result = match (builtin, arguments.len()) {
                (crate::BuiltinType::List, 1) => Some(ImportValueType::List(Box::new(
                    poly_import_value_type_with_stack(module, arguments[0], params, item_stack)?,
                ))),
                (crate::BuiltinType::Map, 2) => Some(ImportValueType::Map {
                    key: Box::new(poly_import_value_type_with_stack(
                        module,
                        arguments[0],
                        params,
                        item_stack,
                    )?),
                    value: Box::new(poly_import_value_type_with_stack(
                        module,
                        arguments[1],
                        params,
                        item_stack,
                    )?),
                }),
                (crate::BuiltinType::Set, 1) => Some(ImportValueType::Set(Box::new(
                    poly_import_value_type_with_stack(module, arguments[0], params, item_stack)?,
                ))),
                (crate::BuiltinType::Option, 1) => Some(ImportValueType::Option(Box::new(
                    poly_import_value_type_with_stack(module, arguments[0], params, item_stack)?,
                ))),
                (crate::BuiltinType::Result, 2) => Some(ImportValueType::Result {
                    error: Box::new(poly_import_value_type_with_stack(
                        module,
                        arguments[0],
                        params,
                        item_stack,
                    )?),
                    value: Box::new(poly_import_value_type_with_stack(
                        module,
                        arguments[1],
                        params,
                        item_stack,
                    )?),
                }),
                (crate::BuiltinType::Validation, 2) => Some(ImportValueType::Validation {
                    error: Box::new(poly_import_value_type_with_stack(
                        module,
                        arguments[0],
                        params,
                        item_stack,
                    )?),
                    value: Box::new(poly_import_value_type_with_stack(
                        module,
                        arguments[1],
                        params,
                        item_stack,
                    )?),
                }),
                (crate::BuiltinType::Signal, 1) => Some(ImportValueType::Signal(Box::new(
                    poly_import_value_type_with_stack(module, arguments[0], params, item_stack)?,
                ))),
                (crate::BuiltinType::Task, 2) => Some(ImportValueType::Task {
                    error: Box::new(poly_import_value_type_with_stack(
                        module,
                        arguments[0],
                        params,
                        item_stack,
                    )?),
                    value: Box::new(poly_import_value_type_with_stack(
                        module,
                        arguments[1],
                        params,
                        item_stack,
                    )?),
                }),
                _ => None,
            };
            if result.is_some() {
                return result;
            }
        }
        PolyTypeConstructor::Resolved(ResolvedTypeConstructor::Bundle(
            ImportBundleKind::BuiltinOption,
        )) if arguments.len() == 1 => {
            return Some(ImportValueType::Option(Box::new(
                poly_import_value_type_with_stack(module, arguments[0], params, item_stack)?,
            )));
        }
        _ => {}
    }
    let args = arguments
        .iter()
        .map(|arg| poly_import_value_type_with_stack(module, *arg, params, item_stack))
        .collect::<Option<Vec<_>>>()?;
    match constructor {
        PolyTypeConstructor::Resolved(ResolvedTypeConstructor::Item(item)) => {
            named_import_value_type_from_item(module, item, args, item_stack)
        }
        PolyTypeConstructor::Resolved(ResolvedTypeConstructor::Import(import)) => {
            named_import_value_type_from_import(module, import, args)
        }
        PolyTypeConstructor::Named(type_name) => Some(ImportValueType::Named {
            origin: None,
            type_name,
            arguments: args,
            definition: None,
        }),
        _ => None,
    }
}

enum PolyTypeConstructor {
    Resolved(ResolvedTypeConstructor),
    Named(String),
}

fn poly_flatten_type_application(
    module: &Module,
    ty: TypeId,
) -> Option<(PolyTypeConstructor, Vec<TypeId>)> {
    let type_node = module.types().get(ty)?;
    match &type_node.kind {
        TypeKind::Apply { callee, arguments } => {
            let (constructor, mut flattened) = poly_flatten_type_application(module, *callee)?;
            flattened.extend(arguments.iter().copied());
            Some((constructor, flattened))
        }
        TypeKind::Name(reference) => {
            if let Some(resolved) = resolve_type_constructor(module, reference) {
                return Some((PolyTypeConstructor::Resolved(resolved), Vec::new()));
            }
            // Same-module item: extract type name
            if let ResolutionState::Resolved(TypeResolution::Item(item_id)) =
                reference.resolution.as_ref()
            {
                let name = item_type_name(&module.items()[*item_id]);
                return Some((PolyTypeConstructor::Named(name), Vec::new()));
            }
            // Imported type: use the imported name
            if let ResolutionState::Resolved(TypeResolution::Import(import_id)) =
                reference.resolution.as_ref()
                && let Some(binding) = module.imports().get(*import_id)
            {
                let name = binding.imported_name.text().to_owned();
                return Some((PolyTypeConstructor::Named(name), Vec::new()));
            }
            None
        }
        TypeKind::Tuple(_)
        | TypeKind::Record(_)
        | TypeKind::RecordTransform { .. }
        | TypeKind::Arrow { .. } => None,
    }
}

fn exported_kind_rank(kind: ExportedNameKind) -> u8 {
    match kind {
        ExportedNameKind::Type => 0,
        ExportedNameKind::Value => 1,
        ExportedNameKind::Function => 2,
        ExportedNameKind::Signal => 3,
        ExportedNameKind::Class => 4,
        ExportedNameKind::Domain => 5,
        ExportedNameKind::SourceProvider => 6,
        ExportedNameKind::Instance => 7,
    }
}

fn builtin_term_metadata(name: &str) -> Option<ImportBindingMetadata> {
    match name {
        "True" => Some(ImportBindingMetadata::BuiltinTerm(BuiltinTerm::True)),
        "False" => Some(ImportBindingMetadata::BuiltinTerm(BuiltinTerm::False)),
        "None" => Some(ImportBindingMetadata::BuiltinTerm(BuiltinTerm::None)),
        "Some" => Some(ImportBindingMetadata::BuiltinTerm(BuiltinTerm::Some)),
        "Ok" => Some(ImportBindingMetadata::BuiltinTerm(BuiltinTerm::Ok)),
        "Err" => Some(ImportBindingMetadata::BuiltinTerm(BuiltinTerm::Err)),
        "Valid" => Some(ImportBindingMetadata::BuiltinTerm(BuiltinTerm::Valid)),
        "Invalid" => Some(ImportBindingMetadata::BuiltinTerm(BuiltinTerm::Invalid)),
        _ => None,
    }
}

/// Extract record fields from a type alias that resolves directly to a record.
/// Returns `Some(fields)` only for zero-parameter aliases of the form
/// `type Foo = { field: Type, ... }`. Parameterised types and sum types return `None`.
fn extract_type_record_fields(
    module: &Module,
    item_id: ItemId,
    item: &crate::TypeItem,
) -> Option<Vec<ImportRecordField>> {
    let mut item_stack = vec![item_id];
    extract_type_record_fields_with_stack(module, item, &mut item_stack)
}

fn extract_type_record_fields_with_stack(
    module: &Module,
    item: &crate::TypeItem,
    item_stack: &mut Vec<ItemId>,
) -> Option<Vec<ImportRecordField>> {
    // Only monomorphic record aliases carry stable field lists.
    if !item.parameters.is_empty() {
        return None;
    }
    let TypeItemBody::Alias(alias) = &item.body else {
        return None;
    };
    match import_value_type_with_stack(module, *alias, item_stack)? {
        ImportValueType::Record(fields) => Some(fields),
        _ => None,
    }
}

fn extract_type_definition(
    module: &Module,
    item_id: ItemId,
    item: &crate::TypeItem,
) -> Option<ImportTypeDefinition> {
    let mut item_stack = vec![item_id];
    extract_type_definition_with_stack(module, item, &mut item_stack)
}

fn extract_type_sum_constructors(
    module: &Module,
    item_id: ItemId,
    item: &crate::TypeItem,
) -> Option<Vec<SumConstructorHandle>> {
    let TypeItemBody::Sum(variants) = &item.body else {
        return None;
    };
    variants
        .iter()
        .map(|variant| module.sum_constructor_handle(item_id, variant.name.text()))
        .collect()
}

fn extract_type_definition_with_stack(
    module: &Module,
    item: &crate::TypeItem,
    item_stack: &mut Vec<ItemId>,
) -> Option<ImportTypeDefinition> {
    match &item.body {
        TypeItemBody::Alias(alias) => {
            // Build a type parameter map so that type variables in the alias body
            // (e.g. `A` in `type Envelope A = A`) serialize as TypeVariable entries,
            // making transparent aliases round-trip correctly through the export surface.
            let type_param_map: TypeParamMap = item
                .parameters
                .iter()
                .enumerate()
                .map(|(i, &p)| (p, i))
                .collect();
            poly_import_value_type_with_stack(module, *alias, &type_param_map, item_stack)
                .map(ImportTypeDefinition::Alias)
        }
        TypeItemBody::Sum(variants) => {
            let params = item
                .parameters
                .iter()
                .enumerate()
                .map(|(index, id)| (*id, index))
                .collect::<TypeParamMap>();
            variants
                .iter()
                .map(|variant| {
                    Some(ImportSumVariant {
                        name: variant.name.text().into(),
                        fields: variant
                            .fields
                            .iter()
                            .map(|field| {
                                poly_import_value_type_with_stack(
                                    module, field.ty, &params, item_stack,
                                )
                            })
                            .collect::<Option<Vec<_>>>()?,
                    })
                })
                .collect::<Option<Vec<_>>>()
                .map(ImportTypeDefinition::Sum)
        }
    }
}

#[cfg(test)]
mod tests {
    use aivi_base::SourceDatabase;
    use aivi_syntax::parse_module;

    use super::{ImportBindingMetadata, ImportTypeDefinition, ImportValueType, exports};

    fn lower_text(path: &str, text: &str) -> crate::LoweringResult {
        let mut sources = SourceDatabase::new();
        let file_id = sources.add_file(path, text);
        let parsed = parse_module(&sources[file_id]);
        assert!(
            !parsed.has_errors(),
            "exports test input should parse: {:?}",
            parsed.all_diagnostics().collect::<Vec<_>>()
        );
        crate::lower_module(&parsed.module)
    }

    fn assert_named_type_variable_argument(
        ty: &ImportValueType,
        expected_name: &str,
        expected_index: usize,
    ) {
        match ty {
            ImportValueType::Named {
                type_name,
                arguments,
                ..
            } => {
                assert_eq!(type_name.as_str(), expected_name);
                assert_eq!(arguments.len(), 1);
                assert!(matches!(
                    &arguments[0],
                    ImportValueType::TypeVariable { index, .. } if *index == expected_index
                ));
            }
            other => panic!("expected `{expected_name}` applied to a type variable, got {other:?}"),
        }
    }

    #[test]
    fn exported_alias_preserves_nested_named_alias_definitions() {
        let lowered = lower_text(
            "types.aivi",
            r#"
type ComposeState = {
    to: List Text,
    subject: Text
}

type UIState = {
    compose: ComposeState
}
"#,
        );
        assert!(
            !lowered.has_errors(),
            "lowering should succeed: {:?}",
            lowered.diagnostics()
        );

        let exported = exports(lowered.module());
        let ui_state = exported
            .find("UIState")
            .expect("UIState should be exported");
        let definition = match &ui_state.metadata {
            ImportBindingMetadata::TypeConstructor {
                definition: Some(definition),
                ..
            } => definition,
            other => panic!("expected exported UIState type constructor metadata, got {other:?}"),
        };

        let compose_field_ty = match definition {
            ImportTypeDefinition::Alias(ImportValueType::Record(fields)) => fields
                .iter()
                .find(|field| field.name.as_ref() == "compose")
                .map(|field| &field.ty)
                .expect("UIState alias should include compose field"),
            other => panic!("expected UIState alias record definition, got {other:?}"),
        };

        match compose_field_ty {
            ImportValueType::Named {
                type_name,
                definition: Some(definition),
                ..
            } => {
                assert_eq!(type_name.as_str(), "ComposeState");
                assert!(matches!(
                    definition.as_ref(),
                    ImportTypeDefinition::Alias(ImportValueType::Record(fields))
                        if fields.len() == 2
                            && fields[0].name.as_ref() == "to"
                            && fields[1].name.as_ref() == "subject"
                ));
            }
            other => panic!(
                "expected compose field to retain nested ComposeState alias definition, got {other:?}"
            ),
        }
    }

    #[test]
    fn exported_instances_preserve_higher_kinded_member_signatures() {
        let lowered = lower_text(
            "box.aivi",
            r#"
type Box A = Box A

instance Functor Box = {
    map transform box =
        box
         ||> Box item -> Box (transform item)
}

instance Foldable Box = {
    reduce step seed box =
        box
         ||> Box item -> step seed item
}

value one : Box Int = Box 1

export (Box, one)
"#,
        );
        assert!(
            !lowered.has_errors(),
            "lowering should succeed: {:?}",
            lowered.diagnostics()
        );

        let exported = exports(lowered.module());

        let functor = exported
            .instances
            .iter()
            .find(|instance| {
                instance.class_name.as_ref() == "Functor" && instance.subject.as_ref() == "Box"
            })
            .expect("Functor Box instance should be exported");
        let map = functor
            .members
            .iter()
            .find(|member| member.name.as_ref() == "map")
            .expect("Functor Box export should include map");
        match &map.ty {
            ImportValueType::Arrow { parameter, result } => {
                assert!(matches!(
                    parameter.as_ref(),
                    ImportValueType::Arrow { parameter, result }
                        if matches!(parameter.as_ref(), ImportValueType::TypeVariable { index, .. } if *index == 0)
                            && matches!(result.as_ref(), ImportValueType::TypeVariable { index, .. } if *index == 1)
                ));
                match result.as_ref() {
                    ImportValueType::Arrow { parameter, result } => {
                        assert_named_type_variable_argument(parameter, "Box", 0);
                        assert_named_type_variable_argument(result, "Box", 1);
                    }
                    other => panic!("expected map result to remain a Box-arrow, got {other:?}"),
                }
            }
            other => panic!("expected exported map signature, got {other:?}"),
        }

        let foldable = exported
            .instances
            .iter()
            .find(|instance| {
                instance.class_name.as_ref() == "Foldable" && instance.subject.as_ref() == "Box"
            })
            .expect("Foldable Box instance should be exported");
        let reduce = foldable
            .members
            .iter()
            .find(|member| member.name.as_ref() == "reduce")
            .expect("Foldable Box export should include reduce");
        match &reduce.ty {
            ImportValueType::Arrow { result, .. } => match result.as_ref() {
                ImportValueType::Arrow { result, .. } => match result.as_ref() {
                    ImportValueType::Arrow { parameter, .. } => {
                        assert_named_type_variable_argument(parameter, "Box", 1);
                    }
                    other => panic!("expected reduce subject to stay Box A, got {other:?}"),
                },
                other => panic!("expected exported reduce signature, got {other:?}"),
            },
            other => panic!("expected exported reduce signature, got {other:?}"),
        }
    }

    #[test]
    fn exported_unannotated_signal_infers_signal_payload_metadata() {
        let lowered = lower_text(
            "signals.aivi",
            r#"
signal windowTitle = "Mailfox"
"#,
        );
        assert!(
            !lowered.has_errors(),
            "lowering should succeed: {:?}",
            lowered.diagnostics()
        );

        let exported = exports(lowered.module());
        let window_title = exported
            .find("windowTitle")
            .expect("windowTitle signal should be exported");
        match &window_title.metadata {
            ImportBindingMetadata::Value {
                ty: ImportValueType::Signal(payload),
            } => {
                assert_eq!(
                    payload.as_ref(),
                    &ImportValueType::Primitive(crate::BuiltinType::Text)
                );
            }
            other => {
                panic!("expected signal metadata for unannotated signal export, got {other:?}")
            }
        }
    }

    #[test]
    fn reexported_instances_retain_owner_and_deduplicate_import_paths() {
        struct Resolver {
            module: &'static str,
            dependency: Option<crate::ExportedNames>,
        }
        impl crate::ImportResolver for Resolver {
            fn resolve(&self, _: &[&str]) -> crate::ImportModuleResolution {
                self.dependency.clone().map_or(
                    crate::ImportModuleResolution::Missing,
                    crate::ImportModuleResolution::Resolved,
                )
            }
            fn current_module_path(&self) -> Option<String> {
                Some(self.module.to_owned())
            }
        }
        fn lower(text: &str, resolver: &Resolver) -> crate::Module {
            let mut sources = SourceDatabase::new();
            let file = sources.add_file(resolver.module, text);
            let parsed = parse_module(&sources[file]);
            assert!(!parsed.has_errors());
            let lowered = crate::lower_module_with_resolver(&parsed.module, Some(resolver));
            assert!(!lowered.has_errors(), "{:?}", lowered.diagnostics());
            lowered.into_parts().0
        }
        let owner = lower(
            "class Describe A = {\n render : A -> Text\n tag : A -> Int\n}\ntype Label = MkLabel Int\ninstance Describe Label = {\n render = label => \"label\"\n tag = label => 1\n}\nexport (Label, Describe)\n",
            &Resolver {
                module: "owner",
                dependency: None,
            },
        );
        let original = exports(&owner);
        assert_eq!(original.instances.len(), 1);
        assert_eq!(original.instances[0].members.len(), 2);
        assert_eq!(
            original.instances[0].source_module.as_deref(),
            Some("owner")
        );
        let bridge = lower(
            "use owner (Label as Renamed)\nuse owner (Label as Again)\nexport Renamed\n",
            &Resolver {
                module: "bridge",
                dependency: Some(original.clone()),
            },
        );
        let forwarded = exports(&bridge);
        assert_eq!(forwarded.instances, original.instances);
        let private = lower(
            "use owner (Label)\nvalue answer = 42\nexport answer\n",
            &Resolver {
                module: "private",
                dependency: Some(original.clone()),
            },
        );
        assert!(exports(&private).instances.is_empty());
        let class_bridge = lower(
            "use owner (Describe as Description)\nexport Description\n",
            &Resolver {
                module: "class_bridge",
                dependency: Some(original.clone()),
            },
        );
        assert_eq!(exports(&class_bridge).instances, original.instances);
        let consumer = lower(
            "use bridge (Renamed)\n",
            &Resolver {
                module: "consumer",
                dependency: Some(forwarded),
            },
        );
        let owners = consumer
            .imports()
            .iter()
            .filter_map(|(_, binding)| {
                matches!(
                    binding.metadata,
                    ImportBindingMetadata::InstanceMember { .. }
                )
                .then_some(binding.source_module.as_deref())
            })
            .collect::<Vec<_>>();
        assert_eq!(owners, vec![Some("owner"), Some("owner")]);
    }
}
