//! Declaration-time coherence. Terms live in a flat arena: normalization,
//! unification, and the occurs check use bounded worklists, including aliases.
use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
};

use crate::{
    BuiltinType, ClassIdentity, ImportBindingMetadata, ImportTypeDefinition, ImportValueType,
    ImportedTypeBinding, ImportedTypeConstructor, Item, Module, RecordRowTransform,
    ResolutionState, TypeId, TypeItemBody, TypeKind, TypeParameterId, TypeResolution,
};

const MAX_STEPS: usize = 131_072;
type TermId = usize;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Constructor {
    Builtin(BuiltinType),
    Declared(crate::TypeIdentity),
    Nominal {
        module: Option<Box<str>>,
        name: Box<str>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Shape {
    Constructor(Constructor),
    Tuple,
    Record(Vec<String>),
    Arrow,
}

#[derive(Clone, Debug)]
enum Term {
    Variable,
    Rigid(usize),
    Apply(TermId, TermId),
    Shape(Shape, Vec<TermId>),
}

#[derive(Default)]
pub(crate) struct CoherenceTerms {
    terms: Vec<Term>,
}

type HirScope = Rc<HashMap<TypeParameterId, TermId>>;
type ImportScope = Rc<HashMap<usize, TermId>>;

enum Work<'a> {
    Hir(TypeId, HirScope, Vec<TermId>),
    Imported(
        &'a ImportValueType,
        ImportScope,
        Option<&'a str>,
        Vec<TermId>,
    ),
    HirReference(TypeResolution, HirScope, usize, Vec<TermId>),
    ImportedReference(
        &'a str,
        Option<&'a ImportTypeDefinition>,
        Option<&'a crate::ImportedTypeOrigin>,
        ImportScope,
        Option<&'a str>,
        usize,
        Vec<TermId>,
    ),
    ImportedParameter(usize, ImportScope, usize, Vec<TermId>),
    Finish(Shape, usize),
    Builtin(BuiltinType, usize, Vec<TermId>),
    Transform(&'a RecordRowTransform),
}

impl CoherenceTerms {
    fn alloc(&mut self, term: Term) -> TermId {
        let id = self.terms.len();
        self.terms.push(term);
        id
    }
    fn atom(&mut self, constructor: Constructor) -> TermId {
        self.alloc(Term::Shape(Shape::Constructor(constructor), Vec::new()))
    }
    fn nominal(
        &mut self,
        origin: Option<&crate::ImportedTypeOrigin>,
        source: Option<&str>,
        name: &str,
    ) -> TermId {
        self.atom(match origin {
            Some(origin) => Constructor::Declared(origin.identity.clone()),
            None => Constructor::Nominal {
                module: source.map(Into::into),
                name: name.into(),
            },
        })
    }

    fn apply(&mut self, mut head: TermId, args: impl IntoIterator<Item = TermId>) -> TermId {
        for argument in args {
            head = self.alloc(Term::Apply(head, argument));
        }
        head
    }
    fn witnesses(&mut self, arity: usize) -> Vec<TermId> {
        (0..arity)
            .map(|index| self.alloc(Term::Rigid(index)))
            .collect()
    }

    pub(crate) fn hir_head(
        &mut self,
        module: &Module,
        ty: TypeId,
        parameters: &[TypeParameterId],
        arity: usize,
    ) -> Result<TermId, String> {
        let scope = Rc::new(
            parameters
                .iter()
                .map(|parameter| (*parameter, self.alloc(Term::Variable)))
                .collect(),
        );
        let extra = self.witnesses(arity);
        self.normalize(module, vec![Work::Hir(ty, scope, extra)])
    }

    pub(crate) fn imported_head(
        &mut self,
        module: &Module,
        head: &ImportedTypeBinding,
        source_module: Option<&str>,
        arity: usize,
    ) -> Result<TermId, String> {
        let mut indexed = HashSet::new();
        let mut pending = match head {
            ImportedTypeBinding::Type(ty) => vec![ty],
            ImportedTypeBinding::Constructor { arguments, .. } => arguments.iter().collect(),
        };
        if let ImportedTypeBinding::Constructor {
            head: ImportedTypeConstructor::Parameter { index, .. },
            ..
        } = head
        {
            indexed.insert(*index);
        }
        while let Some(ty) = pending.pop() {
            match ty {
                ImportValueType::TypeVariable { index, .. } => {
                    indexed.insert(*index);
                }
                ImportValueType::TypeApplication {
                    index, arguments, ..
                } => {
                    indexed.insert(*index);
                    pending.extend(arguments);
                }
                ImportValueType::Named { arguments, .. } | ImportValueType::Tuple(arguments) => {
                    pending.extend(arguments)
                }
                ImportValueType::Record(fields) => {
                    pending.extend(fields.iter().map(|field| &field.ty))
                }
                ImportValueType::Arrow { parameter, result } => {
                    pending.extend([parameter.as_ref(), result.as_ref()])
                }
                ImportValueType::List(child)
                | ImportValueType::Set(child)
                | ImportValueType::Option(child)
                | ImportValueType::Signal(child) => pending.push(child),
                ImportValueType::Map { key, value } => {
                    pending.extend([key.as_ref(), value.as_ref()])
                }
                ImportValueType::Result { error, value }
                | ImportValueType::Validation { error, value }
                | ImportValueType::Task { error, value } => {
                    pending.extend([error.as_ref(), value.as_ref()])
                }
                ImportValueType::Primitive(_) => {}
            }
            if indexed.len() + pending.len() > MAX_STEPS {
                return Err("imported head exceeds the compiler type complexity limit".into());
            }
        }
        let scope = Rc::new(
            indexed
                .into_iter()
                .map(|index| (index, self.alloc(Term::Variable)))
                .collect(),
        );
        let extra = self.witnesses(arity);
        let mut work = Vec::new();
        match head {
            ImportedTypeBinding::Type(ty) => {
                work.push(Work::Imported(ty, scope, source_module, extra))
            }
            ImportedTypeBinding::Constructor { head, arguments } => {
                match head {
                    ImportedTypeConstructor::Builtin(builtin) => {
                        work.push(Work::Builtin(*builtin, arguments.len(), extra))
                    }
                    ImportedTypeConstructor::Named {
                        name,
                        definition,
                        origin,
                        ..
                    } => work.push(Work::ImportedReference(
                        name,
                        definition.as_deref(),
                        origin.as_ref(),
                        scope.clone(),
                        source_module,
                        arguments.len(),
                        extra,
                    )),
                    ImportedTypeConstructor::Parameter { index, .. } => work.push(
                        Work::ImportedParameter(*index, scope.clone(), arguments.len(), extra),
                    ),
                }
                work.extend(
                    arguments
                        .iter()
                        .rev()
                        .map(|ty| Work::Imported(ty, scope.clone(), source_module, Vec::new())),
                );
            }
        }
        self.normalize(module, work)
    }

    fn normalize<'a>(
        &mut self,
        module: &'a Module,
        mut work: Vec<Work<'a>>,
    ) -> Result<TermId, String> {
        let mut values = Vec::new();
        let mut steps = 0;
        let initial_terms = self.terms.len();
        while let Some(step) = work.pop() {
            steps += 1;
            if steps > MAX_STEPS || self.terms.len() - initial_terms > MAX_STEPS * 4 {
                return Err("instance head normalization exceeded the compiler type complexity limit (possibly cyclic aliases)".into());
            }
            match step {
                Work::Hir(id, scope, extra) => match &module.types()[id].kind {
                    TypeKind::Name(reference) => {
                        let ResolutionState::Resolved(resolution) = reference.resolution else {
                            return Err("instance head has an unresolved type".into());
                        };
                        work.push(Work::HirReference(resolution, scope, 0, extra));
                    }
                    TypeKind::Apply { callee, arguments } => {
                        // Surface applications have a named callee. Flatten any
                        // nested application before evaluating constructor args.
                        let mut callee = *callee;
                        let mut args = arguments.iter().copied().collect::<Vec<_>>();
                        while let TypeKind::Apply {
                            callee: inner,
                            arguments,
                        } = &module.types()[callee].kind
                        {
                            let mut prefix = arguments.iter().copied().collect::<Vec<_>>();
                            prefix.extend(args);
                            args = prefix;
                            callee = *inner;
                            steps += 1;
                            if steps > MAX_STEPS {
                                return Err("instance application exceeds the compiler type complexity limit".into());
                            }
                        }
                        let TypeKind::Name(reference) = &module.types()[callee].kind else {
                            return Err("instance constructor is not a named type".into());
                        };
                        let ResolutionState::Resolved(resolution) = reference.resolution else {
                            return Err("instance constructor is unresolved".into());
                        };
                        work.push(Work::HirReference(
                            resolution,
                            scope.clone(),
                            args.len(),
                            extra,
                        ));
                        work.extend(
                            args.into_iter()
                                .rev()
                                .map(|ty| Work::Hir(ty, scope.clone(), Vec::new())),
                        );
                    }
                    TypeKind::Tuple(children) => {
                        if !extra.is_empty() {
                            return Err("tuple is over-applied in instance head".into());
                        }
                        work.push(Work::Finish(Shape::Tuple, children.len()));
                        work.extend(
                            children
                                .iter()
                                .rev()
                                .map(|ty| Work::Hir(*ty, scope.clone(), Vec::new())),
                        );
                    }
                    TypeKind::Record(fields) => {
                        if !extra.is_empty() {
                            return Err("record is over-applied in instance head".into());
                        }
                        work.push(Work::Finish(
                            Shape::Record(
                                fields
                                    .iter()
                                    .map(|field| field.label.text().to_owned())
                                    .collect(),
                            ),
                            fields.len(),
                        ));
                        work.extend(
                            fields
                                .iter()
                                .rev()
                                .map(|field| Work::Hir(field.ty, scope.clone(), Vec::new())),
                        );
                    }
                    TypeKind::Arrow { parameter, result } => {
                        if !extra.is_empty() {
                            return Err("function is over-applied in instance head".into());
                        }
                        work.push(Work::Finish(Shape::Arrow, 2));
                        work.push(Work::Hir(*result, scope.clone(), Vec::new()));
                        work.push(Work::Hir(*parameter, scope, Vec::new()));
                    }
                    TypeKind::RecordTransform { transform, source } => {
                        if !extra.is_empty() {
                            return Err("record transform is over-applied in instance head".into());
                        }
                        work.push(Work::Transform(transform));
                        work.push(Work::Hir(*source, scope, Vec::new()));
                    }
                },
                Work::HirReference(resolution, scope, count, extra) => {
                    let mut args = values.split_off(values.len() - count);
                    args.extend(extra);
                    match resolution {
                        TypeResolution::Builtin(builtin) => {
                            let head = self.atom(Constructor::Builtin(builtin));
                            values.push(self.apply(head, args));
                        }
                        TypeResolution::TypeParameter(parameter) => {
                            let head = *scope.get(&parameter).ok_or_else(|| {
                                "instance variable is outside its binder scope".to_owned()
                            })?;
                            values.push(self.apply(head, args));
                        }
                        TypeResolution::Item(item_id) => match &module.items()[item_id] {
                            Item::Type(item) if matches!(item.body, TypeItemBody::Alias(_)) => {
                                let TypeItemBody::Alias(body) = item.body else {
                                    unreachable!()
                                };
                                if args.len() != item.parameters.len() {
                                    return Err(
                                        "instance alias has an invalid constructor arity".into()
                                    );
                                }
                                let mut aliases = (*scope).clone();
                                aliases.extend(item.parameters.iter().copied().zip(args));
                                work.push(Work::Hir(body, Rc::new(aliases), Vec::new()));
                            }
                            Item::Type(_) => {
                                let head = self.atom(Constructor::Declared(
                                    module
                                        .type_origin(item_id)
                                        .expect("data declaration origin")
                                        .identity,
                                ));
                                values.push(self.apply(head, args));
                            }
                            Item::Domain(_) => {
                                let head = self.atom(Constructor::Declared(
                                    module
                                        .type_origin(item_id)
                                        .expect("data declaration origin")
                                        .identity,
                                ));
                                values.push(self.apply(head, args));
                            }
                            _ => return Err("instance carrier is not a data type".into()),
                        },
                        TypeResolution::Import(id) => {
                            let import = &module.imports()[id];
                            match &import.metadata {
                                ImportBindingMetadata::BuiltinType(builtin) => {
                                    let head = self.atom(Constructor::Builtin(*builtin));
                                    values.push(self.apply(head, args));
                                }
                                ImportBindingMetadata::TypeConstructor { definition, .. } => {
                                    work.push(Work::ImportedReference(
                                        import.imported_name.text(),
                                        definition.as_ref(),
                                        import.metadata.type_origin(),
                                        Rc::new(HashMap::new()),
                                        import.source_module.as_deref(),
                                        0,
                                        args,
                                    ));
                                }
                                ImportBindingMetadata::Domain { .. }
                                | ImportBindingMetadata::AmbientType => {
                                    // Ambient carriers share their compiler declaration,
                                    // regardless of which stdlib facade imported them.
                                    if matches!(import.metadata, ImportBindingMetadata::AmbientType)
                                        && let Some((item_id, _)) =
                                            module.items().iter().find(|(id, item)| {
                                                module.ambient_items().contains(id)
                                                    && match item {
                                                        Item::Type(item) => {
                                                            item.name.text()
                                                                == import.imported_name.text()
                                                        }
                                                        Item::Domain(item) => {
                                                            item.name.text()
                                                                == import.imported_name.text()
                                                        }
                                                        _ => false,
                                                    }
                                            })
                                    {
                                        work.push(Work::HirReference(
                                            TypeResolution::Item(item_id),
                                            scope,
                                            0,
                                            args,
                                        ));
                                    } else {
                                        let head = self.nominal(
                                            import.metadata.type_origin(),
                                            import.source_module.as_deref(),
                                            import.imported_name.text(),
                                        );
                                        values.push(self.apply(head, args));
                                    }
                                }
                                _ => {
                                    return Err(
                                        "instance import has no data constructor metadata".into()
                                    );
                                }
                            }
                        }
                    }
                }
                Work::Imported(ty, scope, source, extra) => {
                    let (shape, children): (Option<Shape>, Vec<&ImportValueType>) =
                        match ty {
                            ImportValueType::Primitive(builtin) => {
                                work.push(Work::Builtin(*builtin, 0, extra));
                                continue;
                            }
                            ImportValueType::TypeVariable { index, .. } => {
                                work.push(Work::ImportedParameter(*index, scope, 0, extra));
                                continue;
                            }
                            ImportValueType::TypeApplication {
                                index, arguments, ..
                            } => {
                                work.push(Work::ImportedParameter(
                                    *index,
                                    scope.clone(),
                                    arguments.len(),
                                    extra,
                                ));
                                work.extend(arguments.iter().rev().map(|ty| {
                                    Work::Imported(ty, scope.clone(), source, Vec::new())
                                }));
                                continue;
                            }
                            ImportValueType::Named {
                                type_name,
                                arguments,
                                definition,
                                origin,
                            } => {
                                work.push(Work::ImportedReference(
                                    type_name,
                                    definition.as_deref(),
                                    origin.as_ref(),
                                    scope.clone(),
                                    source,
                                    arguments.len(),
                                    extra,
                                ));
                                work.extend(arguments.iter().rev().map(|ty| {
                                    Work::Imported(ty, scope.clone(), source, Vec::new())
                                }));
                                continue;
                            }
                            ImportValueType::Tuple(children) => {
                                (Some(Shape::Tuple), children.iter().collect())
                            }
                            ImportValueType::Record(fields) => (
                                Some(Shape::Record(
                                    fields.iter().map(|field| field.name.to_string()).collect(),
                                )),
                                fields.iter().map(|field| &field.ty).collect(),
                            ),
                            ImportValueType::Arrow { parameter, result } => {
                                (Some(Shape::Arrow), vec![parameter, result])
                            }
                            ImportValueType::List(child)
                            | ImportValueType::Set(child)
                            | ImportValueType::Option(child)
                            | ImportValueType::Signal(child) => (None, vec![child]),
                            ImportValueType::Map { key, value } => (None, vec![key, value]),
                            ImportValueType::Result { error, value }
                            | ImportValueType::Validation { error, value }
                            | ImportValueType::Task { error, value } => (None, vec![error, value]),
                        };
                    if let Some(shape) = shape {
                        if !extra.is_empty() {
                            return Err("imported structural type is over-applied".into());
                        }
                        work.push(Work::Finish(shape, children.len()));
                    } else {
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
                        work.push(Work::Builtin(builtin, children.len(), extra));
                    }
                    work.extend(
                        children
                            .into_iter()
                            .rev()
                            .map(|ty| Work::Imported(ty, scope.clone(), source, Vec::new())),
                    );
                }
                Work::ImportedReference(name, definition, origin, _scope, source, count, extra) => {
                    let mut args = values.split_off(values.len() - count);
                    args.extend(extra);
                    if let Some(ImportTypeDefinition::Alias(body)) = definition {
                        let scope = Rc::new(args.into_iter().enumerate().collect());
                        work.push(Work::Imported(body, scope, source, Vec::new()));
                    } else {
                        let head = self.nominal(origin, source, name);
                        values.push(self.apply(head, args));
                    }
                }
                Work::ImportedParameter(index, scope, count, extra) => {
                    let mut args = values.split_off(values.len() - count);
                    args.extend(extra);
                    let head = *scope.get(&index).ok_or_else(|| {
                        "imported variable is outside its binder scope".to_owned()
                    })?;
                    values.push(self.apply(head, args));
                }
                Work::Builtin(builtin, count, extra) => {
                    let mut args = values.split_off(values.len() - count);
                    args.extend(extra);
                    let head = self.atom(Constructor::Builtin(builtin));
                    values.push(self.apply(head, args));
                }
                Work::Finish(mut shape, count) => {
                    let mut children = values.split_off(values.len() - count);
                    if let Shape::Record(labels) = &mut shape {
                        let mut fields = labels.drain(..).zip(children).collect::<Vec<_>>();
                        fields.sort_unstable_by(|a, b| a.0.cmp(&b.0));
                        (*labels, children) = fields.into_iter().unzip();
                    }
                    values.push(self.alloc(Term::Shape(shape, children)));
                }
                Work::Transform(transform) => {
                    let source = values.pop().expect("record transform source");
                    let Term::Shape(Shape::Record(labels), children) = &self.terms[source] else {
                        return Err("record transform source is not a closed record".into());
                    };
                    let mut fields = labels
                        .iter()
                        .cloned()
                        .zip(children.iter().copied())
                        .collect::<Vec<_>>();
                    match transform {
                        RecordRowTransform::Pick(labels) => fields
                            .retain(|(name, _)| labels.iter().any(|label| label.text() == name)),
                        RecordRowTransform::Omit(labels) => fields
                            .retain(|(name, _)| !labels.iter().any(|label| label.text() == name)),
                        RecordRowTransform::Optional(labels)
                        | RecordRowTransform::Defaulted(labels) => {
                            for (name, ty) in &mut fields {
                                if labels.iter().any(|label| label.text() == name)
                                    && !self.is_option(*ty)
                                {
                                    let head = self.atom(Constructor::Builtin(BuiltinType::Option));
                                    *ty = self.apply(head, [*ty]);
                                }
                            }
                        }
                        RecordRowTransform::Required(labels) => {
                            for (name, ty) in &mut fields {
                                if labels.iter().any(|label| label.text() == name)
                                    && self.is_option(*ty)
                                    && let Term::Apply(_, inner) = self.terms[*ty]
                                {
                                    *ty = inner;
                                }
                            }
                        }
                        RecordRowTransform::Rename(renames) => {
                            for (name, _) in &mut fields {
                                if let Some(rename) =
                                    renames.iter().find(|rename| rename.from.text() == name)
                                {
                                    *name = rename.to.text().to_owned();
                                }
                            }
                        }
                    }
                    fields.sort_unstable_by(|a, b| a.0.cmp(&b.0));
                    let (labels, children) = fields.into_iter().unzip();
                    values.push(self.alloc(Term::Shape(Shape::Record(labels), children)));
                }
            }
        }
        if values.len() != 1 {
            return Err("instance head normalization did not produce one type".into());
        }
        Ok(values[0])
    }

    fn is_option(&self, ty: TermId) -> bool {
        matches!(self.terms[ty], Term::Apply(head, _) if matches!(&self.terms[head], Term::Shape(Shape::Constructor(Constructor::Builtin(BuiltinType::Option)), args) if args.is_empty()))
    }

    /// Fresh variables belong to each declaration. Equal rigid witnesses eta
    /// expand constructor heads without choosing a concrete type such as Unit.
    pub(crate) fn overlap(&self, left: TermId, right: TermId) -> Result<bool, String> {
        let mut substitutions = HashMap::new();
        let mut pending = vec![(left, right)];
        let mut steps = 0;
        while let Some((left, right)) = pending.pop() {
            let left = self.resolve(left, &substitutions, &mut steps)?;
            let right = self.resolve(right, &substitutions, &mut steps)?;
            if left == right {
                continue;
            }
            match (&self.terms[left], &self.terms[right]) {
                (Term::Variable, _) | (_, Term::Variable) => {
                    let (variable, term) = if matches!(self.terms[left], Term::Variable) {
                        (left, right)
                    } else {
                        (right, left)
                    };
                    if self.occurs(variable, term, &substitutions, &mut steps)? {
                        return Ok(false);
                    }
                    substitutions.insert(variable, term);
                }
                (Term::Rigid(left), Term::Rigid(right)) if left == right => {}
                (Term::Apply(lh, la), Term::Apply(rh, ra)) => {
                    pending.push((*lh, *rh));
                    pending.push((*la, *ra));
                }
                (Term::Shape(lshape, largs), Term::Shape(rshape, rargs))
                    if lshape == rshape && largs.len() == rargs.len() =>
                {
                    pending.extend(largs.iter().copied().zip(rargs.iter().copied()))
                }
                _ => {
                    // A flexible higher-kinded head may denote a transparent
                    // type function with a structural body. First-order terms
                    // cannot prove those heads disjoint; reject that ambiguity.
                    if self.flexible_head(left, &substitutions, &mut steps)?
                        || self.flexible_head(right, &substitutions, &mut steps)?
                    {
                    } else {
                        return Ok(false);
                    }
                }
            }
            steps += 1;
            if steps > MAX_STEPS {
                return Err("instance overlap check exceeded the compiler complexity limit".into());
            }
        }
        Ok(true)
    }

    fn resolve(
        &self,
        mut term: TermId,
        substitutions: &HashMap<TermId, TermId>,
        steps: &mut usize,
    ) -> Result<TermId, String> {
        while let Some(next) = substitutions.get(&term) {
            term = *next;
            *steps += 1;
            if *steps > MAX_STEPS {
                return Err("instance unification exceeded the compiler complexity limit".into());
            }
        }
        Ok(term)
    }

    fn flexible_head(
        &self,
        mut term: TermId,
        substitutions: &HashMap<TermId, TermId>,
        steps: &mut usize,
    ) -> Result<bool, String> {
        loop {
            term = self.resolve(term, substitutions, steps)?;
            match self.terms[term] {
                Term::Apply(head, _) => term = head,
                Term::Variable => return Ok(true),
                _ => return Ok(false),
            }
            *steps += 1;
            if *steps > MAX_STEPS {
                return Err(
                    "instance head inspection exceeded the compiler complexity limit".into(),
                );
            }
        }
    }

    fn occurs(
        &self,
        variable: TermId,
        term: TermId,
        substitutions: &HashMap<TermId, TermId>,
        steps: &mut usize,
    ) -> Result<bool, String> {
        let mut work = vec![term];
        let mut seen = HashSet::new();
        while let Some(term) = work.pop() {
            let term = self.resolve(term, substitutions, steps)?;
            if term == variable {
                return Ok(true);
            }
            if !seen.insert(term) {
                continue;
            }
            match &self.terms[term] {
                Term::Apply(head, argument) => work.extend([*head, *argument]),
                Term::Shape(_, children) => work.extend(children),
                _ => {}
            }
            *steps += 1;
            if *steps > MAX_STEPS {
                return Err("instance occurs check exceeded the compiler complexity limit".into());
            }
        }
        Ok(false)
    }
}

/// An alias cannot manufacture ownership of an imported or primitive carrier.
/// Named records are structural, but their defining module owns the declaration.
pub(crate) fn owns_carrier(module: &Module, mut ty: TypeId) -> bool {
    let mut seen = HashSet::new();
    while seen.insert(ty) {
        let reference = match &module.types()[ty].kind {
            TypeKind::Name(reference) => reference,
            TypeKind::Apply { callee, .. } => {
                ty = *callee;
                continue;
            }
            _ => return false,
        };
        let ResolutionState::Resolved(TypeResolution::Item(id)) = reference.resolution else {
            return false;
        };
        if !module.root_items().contains(&id) {
            return false;
        }
        match &module.items()[id] {
            Item::Domain(_) => return true,
            Item::Type(item) => match &item.body {
                TypeItemBody::Sum(_) => return true,
                TypeItemBody::Alias(body) => match &module.types()[*body].kind {
                    TypeKind::Record(_) => return true,
                    _ => ty = *body,
                },
            },
            _ => return false,
        }
    }
    false
}

pub(crate) fn provider_owns(module: &Module, class: &ClassIdentity, argument: TypeId) -> bool {
    let ClassIdentity::Standard(class) = class else {
        return false;
    };
    let builtin = match &module.types()[argument].kind {
        TypeKind::Name(reference) => match reference.resolution {
            ResolutionState::Resolved(TypeResolution::Builtin(builtin)) => Some(builtin),
            _ => None,
        },
        _ => None,
    };
    match module.builtin_instance_provider() {
        Some(crate::BuiltinInstanceProvider::Defaults) => {
            class.as_ref() == "Default"
                && matches!(
                    builtin,
                    Some(BuiltinType::Text | BuiltinType::Int | BuiltinType::Bool)
                )
        }
        Some(crate::BuiltinInstanceProvider::Bytes) => {
            matches!(class.as_ref(), "Default" | "Semigroup" | "Monoid")
                && builtin == Some(BuiltinType::Bytes)
        }
        Some(crate::BuiltinInstanceProvider::NonEmpty) => {
            let mut head = argument;
            while let TypeKind::Apply { callee, .. } = &module.types()[head].kind {
                head = *callee;
            }
            matches!(&module.types()[head].kind, TypeKind::Name(reference) if matches!(reference.resolution, ResolutionState::Resolved(TypeResolution::Item(id)) if module.ambient_items().contains(&id) && matches!(&module.items()[id], Item::Domain(domain) if domain.name.text() == "NonEmptyList")))
                && matches!(
                    class.as_ref(),
                    "Functor"
                        | "Foldable"
                        | "Traversable"
                        | "Semigroup"
                        | "Applicative"
                        | "Apply"
                        | "Chain"
                        | "Monad"
                )
        }
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nonempty_traversal_ownership_requires_the_provider_and_standard_identities() {
        let mut sources = aivi_base::SourceDatabase::new();
        let file = sources.add_file(
            "provider.aivi",
            "value marker : NonEmptyList Int = pure 1\n",
        );
        let parsed = aivi_syntax::parse_module(&sources[file]);
        assert!(!parsed.has_errors());
        let lowered = crate::lower_module(&parsed.module);
        assert!(!lowered.has_errors(), "{:?}", lowered.diagnostics());
        let (mut module, _) = lowered.into_parts();
        let head = module
            .root_items()
            .iter()
            .find_map(|id| match &module.items()[*id] {
                Item::Value(value) => value.annotation,
                _ => None,
            })
            .unwrap();
        let standard = ClassIdentity::Standard("Traversable".into());
        assert!(!provider_owns(&module, &standard, head));
        module.builtin_instance_provider = Some(crate::BuiltinInstanceProvider::NonEmpty);
        assert!(provider_owns(&module, &standard, head));
        assert!(!provider_owns(
            &module,
            &ClassIdentity::Source {
                file,
                name: "Traversable".into()
            },
            head
        ));
        for class in ["Monoid", "Default", "Filterable"] {
            assert!(!provider_owns(
                &module,
                &ClassIdentity::Standard(class.into()),
                head
            ));
        }
        let foreign = module
            .arenas
            .types
            .alloc(crate::TypeNode {
                span: aivi_base::SourceSpan::default(),
                kind: TypeKind::Name(crate::TypeReference {
                    path: crate::NamePath::from_vec(vec![
                        crate::Name::new("List", aivi_base::SourceSpan::default()).unwrap(),
                    ])
                    .unwrap(),
                    resolution: ResolutionState::Resolved(TypeResolution::Builtin(
                        BuiltinType::List,
                    )),
                }),
            })
            .unwrap();
        assert!(!provider_owns(&module, &standard, foreign));
    }

    #[test]
    fn normalization_and_unification_handle_twenty_thousand_nested_types() {
        let mut module = Module::new(aivi_base::FileId::new(0));
        let span = aivi_base::SourceSpan::default();
        let reference = |name, builtin| crate::TypeNode {
            span,
            kind: TypeKind::Name(crate::TypeReference {
                path: crate::NamePath::from_vec(vec![crate::Name::new(name, span).unwrap()])
                    .unwrap(),
                resolution: ResolutionState::Resolved(TypeResolution::Builtin(builtin)),
            }),
        };
        let list = module
            .arenas
            .types
            .alloc(reference("List", BuiltinType::List))
            .unwrap();
        let int = module
            .arenas
            .types
            .alloc(reference("Int", BuiltinType::Int))
            .unwrap();
        let text = module
            .arenas
            .types
            .alloc(reference("Text", BuiltinType::Text))
            .unwrap();
        let mut left = int;
        let mut right = text;
        for _ in 0..20_000 {
            left = module
                .arenas
                .types
                .alloc(crate::TypeNode {
                    span,
                    kind: TypeKind::Apply {
                        callee: list,
                        arguments: crate::NonEmpty::from_vec(vec![left]).unwrap(),
                    },
                })
                .unwrap();
            right = module
                .arenas
                .types
                .alloc(crate::TypeNode {
                    span,
                    kind: TypeKind::Apply {
                        callee: list,
                        arguments: crate::NonEmpty::from_vec(vec![right]).unwrap(),
                    },
                })
                .unwrap();
        }
        let mut terms = CoherenceTerms::default();
        let normalized_left = terms.hir_head(&module, left, &[], 0).unwrap();
        let equivalent = terms.hir_head(&module, left, &[], 0).unwrap();
        let normalized_right = terms.hir_head(&module, right, &[], 0).unwrap();
        assert!(terms.overlap(normalized_left, equivalent).unwrap());
        assert!(!terms.overlap(normalized_left, normalized_right).unwrap());
    }

    #[test]
    fn unification_rejects_indirect_occurs_cycles() {
        let mut terms = CoherenceTerms::default();
        let a = terms.alloc(Term::Variable);
        let b = terms.alloc(Term::Variable);
        let list = terms.atom(Constructor::Builtin(BuiltinType::List));
        let list_b = terms.apply(list, [b]);
        let left = terms.alloc(Term::Shape(Shape::Tuple, vec![a, a]));
        let right = terms.alloc(Term::Shape(Shape::Tuple, vec![b, list_b]));
        assert!(!terms.overlap(left, right).unwrap());
    }
}
