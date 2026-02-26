use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Codegen-friendly type representation.
///
/// `CgType` is a compact description of a type's runtime layout that the backend uses
/// to decide whether a definition can be emitted with unboxed types (the "typed path") or
/// must fall back to the dynamic `Value` enum (the "boxed path").
///
/// A `CgType` is **closed** when it contains no `Dynamic` leaves — its layout is fully known at
/// compile time. When any part of the type is `Dynamic`, codegen falls back to `Value`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CgType {
    /// Type could not be resolved to a concrete ground type — needs `Value` boxing.
    Dynamic,

    // ── Primitives ──────────────────────────────────────────────────────────
    Int,
    Float,
    Bool,
    Text,
    Unit,
    DateTime,

    // ── Compound ────────────────────────────────────────────────────────────
    /// `A -> B`
    Func(Box<CgType>, Box<CgType>),

    /// Homogeneous list with known element type: `List A`
    ListOf(Box<CgType>),

    /// Tuple with known element types: `(A, B, …)`
    Tuple(Vec<CgType>),

    /// Closed record with known field names and field types (sorted by name).
    Record(BTreeMap<String, CgType>),

    /// ADT with known constructors and their positional payload types.
    ///
    /// Example: `Option Int` has constructors `[("None", []), ("Some", [Int])]`.
    Adt {
        name: String,
        constructors: Vec<(String, Vec<CgType>)>,
    },
}

impl CgType {
    /// Returns `true` when the type is fully resolved — no `Dynamic` anywhere in the tree.
    pub fn is_closed(&self) -> bool {
        match self {
            CgType::Dynamic => false,
            CgType::Int
            | CgType::Float
            | CgType::Bool
            | CgType::Text
            | CgType::Unit
            | CgType::DateTime => true,
            CgType::Func(a, b) => a.is_closed() && b.is_closed(),
            CgType::ListOf(elem) => elem.is_closed(),
            CgType::Tuple(items) => items.iter().all(|t| t.is_closed()),
            CgType::Record(fields) => fields.values().all(|t| t.is_closed()),
            CgType::Adt { constructors, .. } => constructors
                .iter()
                .all(|(_, args)| args.iter().all(|t| t.is_closed())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primitive_types_are_closed() {
        assert!(CgType::Int.is_closed());
        assert!(CgType::Float.is_closed());
        assert!(CgType::Bool.is_closed());
        assert!(CgType::Text.is_closed());
        assert!(CgType::Unit.is_closed());
    }

    #[test]
    fn dynamic_is_not_closed() {
        assert!(!CgType::Dynamic.is_closed());
    }

    #[test]
    fn func_with_dynamic_is_not_closed() {
        let ty = CgType::Func(Box::new(CgType::Int), Box::new(CgType::Dynamic));
        assert!(!ty.is_closed());
    }

    #[test]
    fn func_closed() {
        let ty = CgType::Func(Box::new(CgType::Int), Box::new(CgType::Bool));
        assert!(ty.is_closed());
    }

    #[test]
    fn record_closed() {
        let mut fields = BTreeMap::new();
        fields.insert("x".to_string(), CgType::Int);
        fields.insert("y".to_string(), CgType::Float);
        assert!(CgType::Record(fields).is_closed());
    }

    #[test]
    fn record_with_dynamic_field_is_open() {
        let mut fields = BTreeMap::new();
        fields.insert("x".to_string(), CgType::Int);
        fields.insert("y".to_string(), CgType::Dynamic);
        assert!(!CgType::Record(fields).is_closed());
    }
}
