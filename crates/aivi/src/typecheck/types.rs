use std::collections::{BTreeMap, HashMap, HashSet};

use crate::diagnostics::Span;

use super::TypeChecker;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub(super) struct TypeVarId(pub(super) u32);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    Star,
    Arrow(Box<Kind>, Box<Kind>),
}

impl std::fmt::Display for Kind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Kind::Star => write!(f, "*"),
            Kind::Arrow(a, b) => match **a {
                Kind::Arrow(_, _) => write!(f, "({}) -> {}", a, b),
                _ => write!(f, "{} -> {}", a, b),
            },
        }
    }
}

#[derive(Clone, Debug)]
pub(super) enum Type {
    Var(TypeVarId),
    Con(String, Vec<Type>),
    App(Box<Type>, Vec<Type>),
    Func(Box<Type>, Box<Type>),
    Tuple(Vec<Type>),
    Record {
        fields: BTreeMap<String, Type>,
        open: bool,
    },
}

#[derive(Clone, Debug)]
pub(super) struct Scheme {
    pub(super) vars: Vec<TypeVarId>,
    pub(super) ty: Type,
    pub(super) origin: Option<SchemeOrigin>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SchemeOrigin {
    pub(super) module: String,
    pub(super) domain: Option<String>,
}

impl SchemeOrigin {
    pub(super) fn new(module: impl Into<String>, domain: Option<String>) -> Self {
        Self {
            module: module.into(),
            domain,
        }
    }

    pub(super) fn render(&self) -> String {
        match &self.domain {
            Some(domain) => format!("{}.{}", self.module, domain),
            None => self.module.clone(),
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct AliasInfo {
    pub(super) params: Vec<TypeVarId>,
    pub(super) body: Type,
}

#[derive(Clone, Debug, Default)]
pub(super) struct TypeEnv {
    values: HashMap<String, Vec<Scheme>>,
}

impl TypeEnv {
    pub(super) fn insert(&mut self, name: String, scheme: Scheme) {
        // `insert` overwrites, matching the previous single-scheme environment semantics.
        self.values.insert(name, vec![scheme]);
    }

    pub(super) fn get(&self, name: &str) -> Option<&Scheme> {
        self.values.get(name).and_then(|items| {
            if items.len() == 1 {
                items.first()
            } else {
                None
            }
        })
    }

    pub(super) fn get_all(&self, name: &str) -> Option<&[Scheme]> {
        self.values.get(name).map(|items| items.as_slice())
    }

    pub(super) fn insert_overloads(&mut self, name: String, schemes: Vec<Scheme>) {
        self.values.insert(name, schemes);
    }

    pub(super) fn free_vars(&self, checker: &mut TypeChecker) -> HashSet<TypeVarId> {
        let mut vars = HashSet::new();
        for schemes in self.values.values() {
            for scheme in schemes {
                vars.extend(checker.free_vars_scheme(scheme));
            }
        }
        vars
    }
}

#[derive(Debug)]
pub(super) struct TypeError {
    pub(super) span: Span,
    pub(super) message: String,
    pub(super) expected: Option<Box<Type>>,
    pub(super) found: Option<Box<Type>>,
}

#[derive(Copy, Clone, Debug)]
pub(super) enum NumberKind {
    Int,
    Float,
}

pub(super) fn number_kind(text: &str) -> Option<NumberKind> {
    let mut chars = text.chars().peekable();
    if matches!(chars.peek(), Some('-')) {
        chars.next();
    }
    let mut saw_digit = false;
    let mut saw_dot = false;
    while let Some(&ch) = chars.peek() {
        if ch.is_ascii_digit() {
            saw_digit = true;
            chars.next();
            continue;
        }
        if ch == '.' && !saw_dot {
            saw_dot = true;
            chars.next();
            continue;
        }
        return None;
    }
    if !saw_digit {
        return None;
    }
    if saw_dot && !text.chars().last().is_some_and(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(if saw_dot {
        NumberKind::Float
    } else {
        NumberKind::Int
    })
}

pub(super) fn split_suffixed_number(text: &str) -> Option<(String, String, NumberKind)> {
    let mut chars = text.chars().peekable();
    let mut number = String::new();
    if matches!(chars.peek(), Some('-')) {
        number.push('-');
        chars.next();
    }
    let mut saw_digit = false;
    let mut saw_dot = false;
    while let Some(&ch) = chars.peek() {
        if ch.is_ascii_digit() {
            saw_digit = true;
            number.push(ch);
            chars.next();
            continue;
        }
        if ch == '.' && !saw_dot {
            saw_dot = true;
            number.push(ch);
            chars.next();
            continue;
        }
        break;
    }
    if !saw_digit {
        return None;
    }
    let suffix: String = chars.collect();
    if suffix.is_empty() {
        return None;
    }
    if !suffix
        .chars()
        .all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
    {
        return None;
    }
    Some((
        number,
        suffix,
        if saw_dot {
            NumberKind::Float
        } else {
            NumberKind::Int
        },
    ))
}

impl Scheme {
    pub(super) fn mono(ty: Type) -> Scheme {
        Scheme {
            vars: Vec::new(),
            ty,
            origin: None,
        }
    }
}

impl Type {
    pub(super) fn con(name: &str) -> Type {
        Type::Con(name.to_string(), Vec::new())
    }

    pub(super) fn app(self, args: Vec<Type>) -> Type {
        match self {
            Type::Con(name, mut existing) => {
                existing.extend(args);
                Type::Con(name, existing)
            }
            Type::App(base, mut existing) => {
                existing.extend(args);
                Type::App(base, existing)
            }
            other => Type::App(Box::new(other), args),
        }
    }
}

pub(super) struct TypeContext {
    pub(super) type_vars: HashMap<String, TypeVarId>,
    pub(super) type_constructors: HashMap<String, Kind>,
}

impl TypeContext {
    pub(super) fn new(type_constructors: &HashMap<String, Kind>) -> Self {
        Self {
            type_vars: HashMap::new(),
            type_constructors: type_constructors.clone(),
        }
    }
}

pub(super) struct TypePrinter {
    names: HashMap<TypeVarId, String>,
    next_id: u8,
}

impl TypePrinter {
    pub(super) fn new() -> Self {
        Self {
            names: HashMap::new(),
            next_id: 0,
        }
    }

    pub(super) fn print(&mut self, ty: &Type) -> String {
        match ty {
            Type::Var(id) => self.name_for(*id),
            Type::Con(name, args) => {
                if args.is_empty() {
                    name.clone()
                } else {
                    let args_str = args.iter().map(|arg| self.print(arg)).collect::<Vec<_>>();
                    format!("{} {}", name, args_str.join(" "))
                }
            }
            Type::App(base, args) => {
                let base_str = match **base {
                    Type::Func(_, _) | Type::Tuple(_) | Type::Record { .. } => {
                        format!("({})", self.print(base))
                    }
                    _ => self.print(base),
                };
                let args_str = args.iter().map(|arg| self.print(arg)).collect::<Vec<_>>();
                format!("{} {}", base_str, args_str.join(" "))
            }
            Type::Func(a, b) => {
                let left = match **a {
                    Type::Func(_, _) => format!("({})", self.print(a)),
                    _ => self.print(a),
                };
                format!("{} -> {}", left, self.print(b))
            }
            Type::Tuple(items) => {
                let items_str = items
                    .iter()
                    .map(|item| self.print(item))
                    .collect::<Vec<_>>();
                format!("({})", items_str.join(", "))
            }
            Type::Record { fields, open } => {
                let mut parts = Vec::new();
                for (name, ty) in fields {
                    parts.push(format!("{}: {}", name, self.print(ty)));
                }
                if *open {
                    parts.push("..".to_string());
                }
                format!("{{ {} }}", parts.join(", "))
            }
        }
    }

    fn name_for(&mut self, id: TypeVarId) -> String {
        if let Some(name) = self.names.get(&id) {
            return name.clone();
        }
        let letter = (b'a' + (self.next_id % 26)) as char;
        let suffix = self.next_id / 26;
        self.next_id += 1;
        let name = if suffix == 0 {
            format!("'{}", letter)
        } else {
            format!("'{}{}", letter, suffix)
        };
        self.names.insert(id, name.clone());
        name
    }
}
