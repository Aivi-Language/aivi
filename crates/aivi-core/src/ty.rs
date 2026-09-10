use std::{fmt, rc::Rc};

use aivi_hir::{
    BuiltinType, GateType as HirGateType, ImportId as HirImportId, ImportTypeDefinition,
    ImportValueType, ItemId as HirItemId, TypeParameterId as HirTypeParameterId,
};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RecordField {
    pub name: Box<str>,
    pub ty: Type,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Type {
    Primitive(BuiltinType),
    TypeParameter {
        parameter: HirTypeParameterId,
        name: Box<str>,
    },
    TypeApplication {
        parameter: HirTypeParameterId,
        name: Box<str>,
        arguments: Vec<Type>,
    },
    Tuple(Vec<Type>),
    Record(Vec<RecordField>),
    Arrow {
        parameter: Box<Type>,
        result: Box<Type>,
    },
    List(Box<Type>),
    Map {
        key: Box<Type>,
        value: Box<Type>,
    },
    Set(Box<Type>),
    Option(Box<Type>),
    Result {
        error: Box<Type>,
        value: Box<Type>,
    },
    Validation {
        error: Box<Type>,
        value: Box<Type>,
    },
    Signal(Box<Type>),
    Task {
        error: Box<Type>,
        value: Box<Type>,
    },
    Domain {
        item: HirItemId,
        name: Box<str>,
        arguments: Vec<Type>,
        /// Resolved in the defining module, before module-local HIR IDs lose context.
        carrier: Option<Box<Type>>,
    },
    OpaqueItem {
        item: HirItemId,
        name: Box<str>,
        arguments: Vec<Type>,
    },
    OpaqueImport {
        import: HirImportId,
        name: Box<str>,
        arguments: Vec<Type>,
        definition: Option<Box<ImportTypeDefinition>>,
    },
}

impl Type {
    pub fn lower(root: &HirGateType) -> Self {
        Self::lower_in_context(root, None)
    }

    pub fn lower_in_module(root: &HirGateType, module: &aivi_hir::Module) -> Self {
        Self::lower_in_context(root, Some(module))
    }

    fn lower_in_context(root: &HirGateType, module: Option<&aivi_hir::Module>) -> Self {
        #[allow(clippy::enum_variant_names)]
        enum Task {
            Visit(HirGateType),
            BuildTypeApplication {
                parameter: HirTypeParameterId,
                name: Box<str>,
                arguments: usize,
            },
            BuildTuple(usize),
            BuildRecord(Vec<Box<str>>),
            BuildArrow,
            BuildList,
            BuildMap,
            BuildSet,
            BuildOption,
            BuildResult,
            BuildValidation,
            BuildSignal,
            BuildTask,
            BuildDomain {
                item: HirItemId,
                name: Box<str>,
                arguments: usize,
                has_carrier: bool,
            },
            BuildImportedCarrier {
                carrier: ImportValueType,
                domain_name: Option<Box<str>>,
                arguments: usize,
            },
            BuildOpaqueItem {
                item: HirItemId,
                name: Box<str>,
                arguments: usize,
            },
            BuildOpaqueImport {
                import: HirImportId,
                name: Box<str>,
                arguments: usize,
                definition: Option<Box<ImportTypeDefinition>>,
            },
        }

        let mut tasks = vec![Task::Visit(root.clone())];
        let mut values = Vec::new();

        while let Some(task) = tasks.pop() {
            match task {
                Task::Visit(ty) => match ty {
                    HirGateType::TypeApplication {
                        parameter,
                        name,
                        arguments,
                    } => {
                        tasks.push(Task::BuildTypeApplication {
                            parameter,
                            name: name.into(),
                            arguments: arguments.len(),
                        });
                        for argument in arguments.into_iter().rev() {
                            tasks.push(Task::Visit(argument));
                        }
                    }
                    HirGateType::Primitive(builtin) => values.push(Self::Primitive(builtin)),
                    HirGateType::TypeParameter { parameter, name } => {
                        values.push(Self::TypeParameter {
                            parameter,
                            name: name.clone().into_boxed_str(),
                        });
                    }
                    HirGateType::Tuple(elements) => {
                        tasks.push(Task::BuildTuple(elements.len()));
                        for element in elements.into_iter().rev() {
                            tasks.push(Task::Visit(element));
                        }
                    }
                    HirGateType::Record(fields) => {
                        tasks.push(Task::BuildRecord(
                            fields
                                .iter()
                                .map(|field| field.name.clone().into_boxed_str())
                                .collect(),
                        ));
                        for field in fields.into_iter().rev() {
                            tasks.push(Task::Visit(field.ty));
                        }
                    }
                    HirGateType::Arrow { parameter, result } => {
                        tasks.push(Task::BuildArrow);
                        tasks.push(Task::Visit(*result));
                        tasks.push(Task::Visit(*parameter));
                    }
                    HirGateType::List(element) => {
                        tasks.push(Task::BuildList);
                        tasks.push(Task::Visit(*element));
                    }
                    HirGateType::Map { key, value } => {
                        tasks.push(Task::BuildMap);
                        tasks.push(Task::Visit(*value));
                        tasks.push(Task::Visit(*key));
                    }
                    HirGateType::Set(element) => {
                        tasks.push(Task::BuildSet);
                        tasks.push(Task::Visit(*element));
                    }
                    HirGateType::Option(element) => {
                        tasks.push(Task::BuildOption);
                        tasks.push(Task::Visit(*element));
                    }
                    HirGateType::Result { error, value } => {
                        tasks.push(Task::BuildResult);
                        tasks.push(Task::Visit(*value));
                        tasks.push(Task::Visit(*error));
                    }
                    HirGateType::Validation { error, value } => {
                        tasks.push(Task::BuildValidation);
                        tasks.push(Task::Visit(*value));
                        tasks.push(Task::Visit(*error));
                    }
                    HirGateType::Signal(inner) => {
                        tasks.push(Task::BuildSignal);
                        tasks.push(Task::Visit(*inner));
                    }
                    HirGateType::Task { error, value } => {
                        tasks.push(Task::BuildTask);
                        tasks.push(Task::Visit(*value));
                        tasks.push(Task::Visit(*error));
                    }
                    HirGateType::Domain {
                        item,
                        name,
                        arguments,
                    } => {
                        let carrier = module.and_then(|module| {
                            aivi_hir::domain_carrier_type(module, item, &arguments)
                        });
                        tasks.push(Task::BuildDomain {
                            item,
                            name: name.into_boxed_str(),
                            arguments: arguments.len(),
                            has_carrier: carrier.is_some(),
                        });
                        if let Some(carrier) = carrier {
                            tasks.push(Task::Visit(carrier));
                        }
                        for argument in arguments.into_iter().rev() {
                            tasks.push(Task::Visit(argument));
                        }
                    }
                    HirGateType::OpaqueItem {
                        item,
                        name,
                        arguments,
                    } => {
                        if let Some(carrier) = module.and_then(|module| {
                            aivi_hir::opaque_type_carrier_type(module, item, &arguments)
                        }) {
                            tasks.push(Task::Visit(carrier));
                            continue;
                        }
                        tasks.push(Task::BuildOpaqueItem {
                            item,
                            name: name.clone().into_boxed_str(),
                            arguments: arguments.len(),
                        });
                        for argument in arguments.into_iter().rev() {
                            tasks.push(Task::Visit(argument));
                        }
                    }
                    HirGateType::OpaqueImport {
                        import,
                        name,
                        arguments,
                        definition,
                    } => match definition.as_deref() {
                        Some(
                            definition @ (ImportTypeDefinition::Alias(_)
                            | ImportTypeDefinition::Domain(_)),
                        ) => {
                            let (carrier, domain_name) = match definition {
                                ImportTypeDefinition::Alias(carrier) => (carrier, None),
                                ImportTypeDefinition::Domain(carrier) => {
                                    (carrier, Some(name.into_boxed_str()))
                                }
                                ImportTypeDefinition::Sum(_) => {
                                    unreachable!("carrier definition was matched")
                                }
                            };
                            tasks.push(Task::BuildImportedCarrier {
                                carrier: carrier.clone(),
                                domain_name,
                                arguments: arguments.len(),
                            });
                            for argument in arguments.into_iter().rev() {
                                tasks.push(Task::Visit(argument));
                            }
                        }
                        Some(ImportTypeDefinition::Sum(_)) | None => {
                            tasks.push(Task::BuildOpaqueImport {
                                import,
                                name: name.clone().into_boxed_str(),
                                definition: definition.clone(),
                                arguments: arguments.len(),
                            });
                            for argument in arguments.into_iter().rev() {
                                tasks.push(Task::Visit(argument));
                            }
                        }
                    },
                },
                Task::BuildTypeApplication {
                    parameter,
                    name,
                    arguments,
                } => {
                    let arguments = drain_tail(&mut values, arguments);
                    values.push(Self::TypeApplication {
                        parameter,
                        name,
                        arguments,
                    });
                }
                Task::BuildTuple(len) => {
                    let tuple = Self::Tuple(drain_tail(&mut values, len));
                    values.push(tuple);
                }
                Task::BuildRecord(names) => {
                    let len = names.len();
                    let fields = names
                        .into_iter()
                        .zip(drain_tail(&mut values, len))
                        .map(|(name, ty)| RecordField { name, ty })
                        .collect();
                    values.push(Self::Record(fields));
                }
                Task::BuildArrow => {
                    let mut drained = drain_tail(&mut values, 2);
                    let parameter = drained.remove(0);
                    let result = drained.remove(0);
                    values.push(Self::Arrow {
                        parameter: Box::new(parameter),
                        result: Box::new(result),
                    });
                }
                Task::BuildList => {
                    let child = values.pop().expect("list child should exist");
                    values.push(Self::List(Box::new(child)));
                }
                Task::BuildMap => {
                    let mut drained = drain_tail(&mut values, 2);
                    let key = drained.remove(0);
                    let value = drained.remove(0);
                    values.push(Self::Map {
                        key: Box::new(key),
                        value: Box::new(value),
                    });
                }
                Task::BuildSet => {
                    let child = values.pop().expect("set child should exist");
                    values.push(Self::Set(Box::new(child)));
                }
                Task::BuildOption => {
                    let child = values.pop().expect("option child should exist");
                    values.push(Self::Option(Box::new(child)));
                }
                Task::BuildResult => {
                    let mut drained = drain_tail(&mut values, 2);
                    let error = drained.remove(0);
                    let value = drained.remove(0);
                    values.push(Self::Result {
                        error: Box::new(error),
                        value: Box::new(value),
                    });
                }
                Task::BuildValidation => {
                    let mut drained = drain_tail(&mut values, 2);
                    let error = drained.remove(0);
                    let value = drained.remove(0);
                    values.push(Self::Validation {
                        error: Box::new(error),
                        value: Box::new(value),
                    });
                }
                Task::BuildSignal => {
                    let child = values.pop().expect("signal child should exist");
                    values.push(Self::Signal(Box::new(child)));
                }
                Task::BuildTask => {
                    let mut drained = drain_tail(&mut values, 2);
                    let error = drained.remove(0);
                    let value = drained.remove(0);
                    values.push(Self::Task {
                        error: Box::new(error),
                        value: Box::new(value),
                    });
                }
                Task::BuildDomain {
                    item,
                    name,
                    arguments,
                    has_carrier,
                } => {
                    let carrier = has_carrier
                        .then(|| Box::new(values.pop().expect("domain carrier should exist")));
                    let arguments = drain_tail(&mut values, arguments);
                    values.push(Self::Domain {
                        item,
                        name,
                        arguments,
                        carrier,
                    });
                }
                Task::BuildImportedCarrier {
                    carrier,
                    domain_name,
                    arguments,
                } => {
                    let arguments = drain_tail(&mut values, arguments);
                    let carrier = Self::lower_import_with_substitutions(
                        &carrier,
                        Rc::from(arguments.clone().into_boxed_slice()),
                    );
                    values.push(match domain_name {
                        Some(name) => Self::Domain {
                            item: HirItemId::from_raw(u32::MAX),
                            name,
                            arguments,
                            carrier: Some(Box::new(carrier)),
                        },
                        None => carrier,
                    });
                }
                Task::BuildOpaqueItem {
                    item,
                    name,
                    arguments,
                } => {
                    let arguments = drain_tail(&mut values, arguments);
                    values.push(Self::OpaqueItem {
                        item,
                        name,
                        arguments,
                    });
                }
                Task::BuildOpaqueImport {
                    import,
                    name,
                    arguments,
                    definition,
                } => {
                    let arguments = drain_tail(&mut values, arguments);
                    values.push(Self::OpaqueImport {
                        import,
                        name,
                        arguments,
                        definition,
                    });
                }
            }
        }

        values
            .pop()
            .expect("typed-core type lowering should always produce one result")
    }

    pub fn lower_import(root: &ImportValueType) -> Self {
        Self::lower_import_with_substitutions(root, Rc::from(Vec::<Type>::new().into_boxed_slice()))
    }

    fn lower_import_with_substitutions(root: &ImportValueType, substitutions: Rc<[Type]>) -> Self {
        #[allow(clippy::enum_variant_names)]
        enum Task<'a> {
            Visit(&'a ImportValueType, Rc<[Type]>),
            BuildTypeApplication {
                parameter: HirTypeParameterId,
                name: Box<str>,
                arguments: usize,
            },
            BuildTuple(usize),
            BuildRecord(Vec<Box<str>>),
            BuildArrow,
            BuildList,
            BuildMap,
            BuildSet,
            BuildOption,
            BuildResult,
            BuildValidation,
            BuildSignal,
            BuildTask,
            BuildOpaqueImport {
                name: Box<str>,
                arguments: usize,
                definition: Option<Box<ImportTypeDefinition>>,
            },
            WrapDomain {
                name: Box<str>,
                arguments: Vec<Type>,
            },
            EnterDomain {
                name: Box<str>,
                carrier: &'a ImportValueType,
                arguments: usize,
            },
            EnterAlias {
                alias: &'a ImportValueType,
                arguments: usize,
            },
        }

        let mut tasks = vec![Task::Visit(root, substitutions)];
        let mut values = Vec::new();

        while let Some(task) = tasks.pop() {
            match task {
                Task::Visit(ty, substitutions) => match ty {
                    ImportValueType::Primitive(builtin) => values.push(Self::Primitive(*builtin)),
                    ImportValueType::Tuple(elements) => {
                        tasks.push(Task::BuildTuple(elements.len()));
                        for element in elements.iter().rev() {
                            tasks.push(Task::Visit(element, substitutions.clone()));
                        }
                    }
                    ImportValueType::Record(fields) => {
                        tasks.push(Task::BuildRecord(
                            fields.iter().map(|field| field.name.clone()).collect(),
                        ));
                        for field in fields.iter().rev() {
                            tasks.push(Task::Visit(&field.ty, substitutions.clone()));
                        }
                    }
                    ImportValueType::Arrow { parameter, result } => {
                        tasks.push(Task::BuildArrow);
                        tasks.push(Task::Visit(result, substitutions.clone()));
                        tasks.push(Task::Visit(parameter, substitutions.clone()));
                    }
                    ImportValueType::List(element) => {
                        tasks.push(Task::BuildList);
                        tasks.push(Task::Visit(element, substitutions.clone()));
                    }
                    ImportValueType::Map { key, value } => {
                        tasks.push(Task::BuildMap);
                        tasks.push(Task::Visit(value, substitutions.clone()));
                        tasks.push(Task::Visit(key, substitutions.clone()));
                    }
                    ImportValueType::Set(element) => {
                        tasks.push(Task::BuildSet);
                        tasks.push(Task::Visit(element, substitutions.clone()));
                    }
                    ImportValueType::Option(element) => {
                        tasks.push(Task::BuildOption);
                        tasks.push(Task::Visit(element, substitutions.clone()));
                    }
                    ImportValueType::Result { error, value } => {
                        tasks.push(Task::BuildResult);
                        tasks.push(Task::Visit(value, substitutions.clone()));
                        tasks.push(Task::Visit(error, substitutions.clone()));
                    }
                    ImportValueType::Validation { error, value } => {
                        tasks.push(Task::BuildValidation);
                        tasks.push(Task::Visit(value, substitutions.clone()));
                        tasks.push(Task::Visit(error, substitutions.clone()));
                    }
                    ImportValueType::Signal(inner) => {
                        tasks.push(Task::BuildSignal);
                        tasks.push(Task::Visit(inner, substitutions.clone()));
                    }
                    ImportValueType::Task { error, value } => {
                        tasks.push(Task::BuildTask);
                        tasks.push(Task::Visit(value, substitutions.clone()));
                        tasks.push(Task::Visit(error, substitutions.clone()));
                    }
                    ImportValueType::TypeApplication {
                        index,
                        name,
                        arguments,
                    } => {
                        tasks.push(Task::BuildTypeApplication {
                            parameter: HirTypeParameterId::from_raw(u32::MAX - *index as u32),
                            name: name.clone().into(),
                            arguments: arguments.len(),
                        });
                        for argument in arguments.iter().rev() {
                            tasks.push(Task::Visit(argument, substitutions.clone()));
                        }
                    }
                    ImportValueType::TypeVariable { index, name } => {
                        if let Some(ty) = substitutions.get(*index).cloned() {
                            values.push(ty);
                        } else {
                            values.push(Self::TypeParameter {
                                parameter: HirTypeParameterId::from_raw(u32::MAX - *index as u32),
                                name: name.clone().into(),
                            });
                        }
                    }
                    ImportValueType::Named {
                        type_name,
                        arguments,
                        definition,
                    } => match definition.as_deref() {
                        Some(ImportTypeDefinition::Alias(alias)) => {
                            tasks.push(Task::EnterAlias {
                                alias,
                                arguments: arguments.len(),
                            });
                            for argument in arguments.iter().rev() {
                                tasks.push(Task::Visit(argument, substitutions.clone()));
                            }
                        }
                        Some(ImportTypeDefinition::Domain(carrier)) => {
                            tasks.push(Task::EnterDomain {
                                name: type_name.clone().into_boxed_str(),
                                carrier,
                                arguments: arguments.len(),
                            });
                            for argument in arguments.iter().rev() {
                                tasks.push(Task::Visit(argument, substitutions.clone()));
                            }
                        }
                        Some(ImportTypeDefinition::Sum(_)) | None => {
                            tasks.push(Task::BuildOpaqueImport {
                                name: type_name.clone().into_boxed_str(),
                                definition: definition.clone(),
                                arguments: arguments.len(),
                            });
                            for argument in arguments.iter().rev() {
                                tasks.push(Task::Visit(argument, substitutions.clone()));
                            }
                        }
                    },
                },
                Task::BuildTypeApplication {
                    parameter,
                    name,
                    arguments,
                } => {
                    let arguments = drain_tail(&mut values, arguments);
                    values.push(Self::TypeApplication {
                        parameter,
                        name,
                        arguments,
                    });
                }
                Task::BuildTuple(len) => {
                    let tuple = Self::Tuple(drain_tail(&mut values, len));
                    values.push(tuple);
                }
                Task::BuildRecord(names) => {
                    let len = names.len();
                    let record = Self::Record(
                        names
                            .into_iter()
                            .zip(drain_tail(&mut values, len))
                            .map(|(name, ty)| RecordField { name, ty })
                            .collect(),
                    );
                    values.push(record);
                }
                Task::BuildArrow => {
                    let mut drained = drain_tail(&mut values, 2);
                    let parameter = drained.remove(0);
                    let result = drained.remove(0);
                    values.push(Self::Arrow {
                        parameter: Box::new(parameter),
                        result: Box::new(result),
                    });
                }
                Task::BuildList => {
                    let child = values.pop().expect("list child should exist");
                    values.push(Self::List(Box::new(child)));
                }
                Task::BuildMap => {
                    let mut drained = drain_tail(&mut values, 2);
                    let key = drained.remove(0);
                    let value = drained.remove(0);
                    values.push(Self::Map {
                        key: Box::new(key),
                        value: Box::new(value),
                    });
                }
                Task::BuildSet => {
                    let child = values.pop().expect("set child should exist");
                    values.push(Self::Set(Box::new(child)));
                }
                Task::BuildOption => {
                    let child = values.pop().expect("option child should exist");
                    values.push(Self::Option(Box::new(child)));
                }
                Task::BuildResult => {
                    let mut drained = drain_tail(&mut values, 2);
                    let error = drained.remove(0);
                    let value = drained.remove(0);
                    values.push(Self::Result {
                        error: Box::new(error),
                        value: Box::new(value),
                    });
                }
                Task::BuildValidation => {
                    let mut drained = drain_tail(&mut values, 2);
                    let error = drained.remove(0);
                    let value = drained.remove(0);
                    values.push(Self::Validation {
                        error: Box::new(error),
                        value: Box::new(value),
                    });
                }
                Task::BuildSignal => {
                    let child = values.pop().expect("signal child should exist");
                    values.push(Self::Signal(Box::new(child)));
                }
                Task::BuildTask => {
                    let mut drained = drain_tail(&mut values, 2);
                    let error = drained.remove(0);
                    let value = drained.remove(0);
                    values.push(Self::Task {
                        error: Box::new(error),
                        value: Box::new(value),
                    });
                }
                Task::BuildOpaqueImport {
                    name,
                    arguments,
                    definition,
                } => {
                    let arguments = drain_tail(&mut values, arguments);
                    values.push(Self::OpaqueImport {
                        import: aivi_hir::ImportId::from_raw(u32::MAX),
                        name,
                        arguments,
                        definition,
                    });
                }
                Task::EnterDomain {
                    name,
                    carrier,
                    arguments,
                } => {
                    let arguments = drain_tail(&mut values, arguments);
                    let substitutions = Rc::from(arguments.clone().into_boxed_slice());
                    tasks.push(Task::WrapDomain { name, arguments });
                    tasks.push(Task::Visit(carrier, substitutions));
                }
                Task::WrapDomain { name, arguments } => {
                    let carrier = values.pop().expect("domain carrier should exist");
                    values.push(Self::Domain {
                        item: HirItemId::from_raw(u32::MAX),
                        name,
                        arguments,
                        carrier: Some(Box::new(carrier)),
                    });
                }
                Task::EnterAlias { alias, arguments } => {
                    let substitutions =
                        Rc::from(drain_tail(&mut values, arguments).into_boxed_slice());
                    tasks.push(Task::Visit(alias, substitutions));
                }
            }
        }

        values
            .pop()
            .expect("typed-core import type lowering should always produce one result")
    }

    pub fn is_bool(&self) -> bool {
        matches!(self, Self::Primitive(BuiltinType::Bool))
    }

    pub fn is_signal(&self) -> bool {
        matches!(self, Self::Signal(_))
    }

    pub fn same_shape(&self, other: &Self) -> bool {
        self == other
    }

    pub fn has_named_type(&self, expected: &str) -> bool {
        match self {
            Self::Domain { name, .. }
            | Self::OpaqueItem { name, .. }
            | Self::OpaqueImport { name, .. } => name.as_ref() == expected,
            _ => false,
        }
    }
}

fn drain_tail<T>(values: &mut Vec<T>, len: usize) -> Vec<T> {
    let split = values
        .len()
        .checked_sub(len)
        .expect("requested more lowered values than available");
    values.drain(split..).collect()
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Primitive(builtin) => write!(f, "{}", builtin_type_name(*builtin)),
            Type::TypeParameter { name, .. } => write!(f, "{name}"),
            Type::TypeApplication {
                name, arguments, ..
            } => {
                write!(f, "{name}")?;
                for argument in arguments {
                    write!(f, " ({argument})")?;
                }
                Ok(())
            }
            Type::Tuple(elements) => {
                write!(f, "(")?;
                for (index, element) in elements.iter().enumerate() {
                    if index > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{element}")?;
                }
                write!(f, ")")
            }
            Type::Record(fields) => {
                write!(f, "{{ ")?;
                for (index, field) in fields.iter().enumerate() {
                    if index > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}: {}", field.name, field.ty)?;
                }
                write!(f, " }}")
            }
            Type::Arrow { parameter, result } => write!(f, "{parameter} -> {result}"),
            Type::List(element) => write!(f, "List {element}"),
            Type::Map { key, value } => write!(f, "Map {key} {value}"),
            Type::Set(element) => write!(f, "Set {element}"),
            Type::Option(element) => write!(f, "Option {element}"),
            Type::Result { error, value } => write!(f, "Result {error} {value}"),
            Type::Validation { error, value } => write!(f, "Validation {error} {value}"),
            Type::Signal(inner) => write!(f, "Signal {inner}"),
            Type::Task { error, value } => write!(f, "Task {error} {value}"),
            Type::Domain {
                name, arguments, ..
            }
            | Type::OpaqueItem {
                name, arguments, ..
            }
            | Type::OpaqueImport {
                name, arguments, ..
            } => {
                write!(f, "{name}")?;
                for argument in arguments {
                    write!(f, " {argument}")?;
                }
                Ok(())
            }
        }
    }
}

fn builtin_type_name(builtin: BuiltinType) -> &'static str {
    match builtin {
        BuiltinType::Int => "Int",
        BuiltinType::Float => "Float",
        BuiltinType::Decimal => "Decimal",
        BuiltinType::BigInt => "BigInt",
        BuiltinType::Bool => "Bool",
        BuiltinType::Text => "Text",
        BuiltinType::Unit => "Unit",
        BuiltinType::Bytes => "Bytes",
        BuiltinType::List => "List",
        BuiltinType::Map => "Map",
        BuiltinType::Set => "Set",
        BuiltinType::Option => "Option",
        BuiltinType::Result => "Result",
        BuiltinType::Validation => "Validation",
        BuiltinType::Signal => "Signal",
        BuiltinType::Task => "Task",
    }
}

#[cfg(test)]
mod tests {
    use super::Type;
    use aivi_hir::{
        BuiltinType, GateType as HirGateType, ImportId, ImportTypeDefinition, ImportValueType,
    };

    #[test]
    fn imported_sum_keeps_unobserved_constructor_definitions() {
        let definition = Some(Box::new(ImportTypeDefinition::Sum(vec![
            aivi_hir::ImportSumVariant {
                name: "Empty".into(),
                fields: vec![],
            },
            aivi_hir::ImportSumVariant {
                name: "Full".into(),
                fields: vec![ImportValueType::Primitive(BuiltinType::Int)],
            },
        ])));
        let gate = HirGateType::OpaqueImport {
            import: ImportId::from_raw(0),
            name: "Container".into(),
            arguments: vec![],
            definition: definition.clone(),
        };
        let Type::OpaqueImport {
            definition: lowered,
            ..
        } = Type::lower(&gate)
        else {
            panic!("sum must retain its nominal type");
        };
        assert_eq!(lowered, definition);
        let portable = ImportValueType::Named {
            type_name: "Container".into(),
            arguments: vec![],
            definition: definition.clone(),
        };
        let Type::OpaqueImport {
            definition: lowered,
            ..
        } = Type::lower_import(&portable)
        else {
            panic!("portable sum must retain its nominal type");
        };
        assert_eq!(lowered, definition);
    }

    #[test]
    fn lower_hir_import_alias_substitutes_type_arguments() {
        let ty = HirGateType::OpaqueImport {
            import: ImportId::from_raw(0),
            name: "Envelope".into(),
            arguments: vec![HirGateType::Option(Box::new(HirGateType::Primitive(
                BuiltinType::Int,
            )))],
            definition: Some(Box::new(ImportTypeDefinition::Alias(
                ImportValueType::TypeVariable {
                    index: 0,
                    name: "A".into(),
                },
            ))),
        };

        assert_eq!(
            Type::lower(&ty),
            Type::Option(Box::new(Type::Primitive(BuiltinType::Int)))
        );
    }

    #[test]
    fn lower_import_alias_substitutes_direct_arguments() {
        let ty = ImportValueType::Named {
            type_name: "Envelope".into(),
            arguments: vec![ImportValueType::Primitive(BuiltinType::Int)],
            definition: Some(Box::new(ImportTypeDefinition::Alias(
                ImportValueType::TypeVariable {
                    index: 0,
                    name: "A".into(),
                },
            ))),
        };

        assert_eq!(Type::lower_import(&ty), Type::Primitive(BuiltinType::Int));
    }

    #[test]
    fn lower_import_alias_substitutes_outer_arguments_inside_nested_aliases() {
        let ty = ImportValueType::Named {
            type_name: "Wrap".into(),
            arguments: vec![ImportValueType::Primitive(BuiltinType::Int)],
            definition: Some(Box::new(ImportTypeDefinition::Alias(
                ImportValueType::Named {
                    type_name: "Envelope".into(),
                    arguments: vec![ImportValueType::TypeVariable {
                        index: 0,
                        name: "A".into(),
                    }],
                    definition: Some(Box::new(ImportTypeDefinition::Alias(
                        ImportValueType::Option(Box::new(ImportValueType::TypeVariable {
                            index: 0,
                            name: "B".into(),
                        })),
                    ))),
                },
            ))),
        };

        assert_eq!(
            Type::lower_import(&ty),
            Type::Option(Box::new(Type::Primitive(BuiltinType::Int)))
        );
    }
}
