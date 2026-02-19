use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Codegen-friendly type representation.
///
/// `CgType` is a compact description of a type's runtime layout that the native Rust backend uses
/// to decide whether a definition can be emitted with unboxed Rust types (the "typed path") or
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

    /// Emit the Rust type string for this `CgType`.
    ///
    /// For `Dynamic`, returns `"Value"` (the boxed enum).
    pub fn rust_type(&self) -> String {
        match self {
            CgType::Dynamic => "Value".to_string(),
            CgType::Int => "i64".to_string(),
            CgType::Float => "f64".to_string(),
            CgType::Bool => "bool".to_string(),
            CgType::Text => "String".to_string(),
            CgType::Unit => "()".to_string(),
            CgType::DateTime => "String".to_string(),
            CgType::Func(a, b) => {
                format!(
                    "Box<dyn Fn({}) -> Result<{}, RuntimeError>>",
                    a.rust_type(),
                    b.rust_type()
                )
            }
            CgType::ListOf(elem) => format!("Vec<{}>", elem.rust_type()),
            CgType::Tuple(items) => {
                let parts: Vec<_> = items.iter().map(|t| t.rust_type()).collect();
                format!("({})", parts.join(", "))
            }
            CgType::Record(fields) => {
                // For now, records with typed fields use a BTreeMap representation.
                // Future: could generate named structs.
                if fields.values().all(|t| t == &CgType::Dynamic) {
                    return "Arc<HashMap<String, Value>>".to_string();
                }
                // Closed records get a tuple struct representation.
                // The field order is sorted by name (BTreeMap guarantees this).
                let parts: Vec<_> = fields.values().map(|t| t.rust_type()).collect();
                format!("({})", parts.join(", "))
            }
            CgType::Adt { .. } => {
                // ADTs fall back to Value for now — full enum generation is a future
                // enhancement.
                "Value".to_string()
            }
        }
    }

    /// Emit Rust code that boxes a typed value into `Value`.
    ///
    /// `expr` is the Rust expression to box.
    pub fn emit_box(&self, expr: &str) -> String {
        match self {
            CgType::Dynamic => expr.to_string(),
            CgType::Int => format!("Value::Int({expr})"),
            CgType::Float => format!("Value::Float({expr})"),
            CgType::Bool => format!("Value::Bool({expr})"),
            CgType::Text => format!("Value::Text({expr})"),
            CgType::Unit => "Value::Unit".to_string(),
            CgType::DateTime => format!("Value::DateTime({expr})"),
            CgType::ListOf(elem) => {
                format!(
                    "Value::List(Arc::new({expr}.into_iter().map(|e| {}).collect()))",
                    elem.emit_box("e")
                )
            }
            CgType::Tuple(items) => {
                let parts: Vec<_> = items
                    .iter()
                    .enumerate()
                    .map(|(i, t)| t.emit_box(&format!("{expr}.{i}")))
                    .collect();
                format!("Value::Tuple(vec![{}])", parts.join(", "))
            }
            CgType::Record(fields) => {
                let mut parts = Vec::new();
                for (i, (name, ty)) in fields.iter().enumerate() {
                    parts.push(format!(
                        "({name:?}.to_string(), {})",
                        ty.emit_box(&format!("{expr}.{i}"))
                    ));
                }
                format!(
                    "Value::Record(Arc::new(HashMap::from([{}])))",
                    parts.join(", ")
                )
            }
            CgType::Func(_, _) | CgType::Adt { .. } => {
                // For complex types, fall back to identity (already Value).
                expr.to_string()
            }
        }
    }

    /// Emit Rust code that unboxes a `Value` to this typed representation.
    ///
    /// `expr` is the Rust expression of type `Value`.
    pub fn emit_unbox(&self, expr: &str) -> String {
        match self {
            CgType::Dynamic => expr.to_string(),
            CgType::Int => format!(
                "match {expr} {{ Value::Int(v) => Ok(v), other => Err(RuntimeError::Message(format!(\"expected Int, got {{}}\", aivi_native_runtime::format_value(&other)))) }}"
            ),
            CgType::Float => format!(
                "match {expr} {{ Value::Float(v) => Ok(v), Value::Int(v) => Ok(v as f64), other => Err(RuntimeError::Message(format!(\"expected Float, got {{}}\", aivi_native_runtime::format_value(&other)))) }}"
            ),
            CgType::Bool => format!(
                "match {expr} {{ Value::Bool(v) => Ok(v), other => Err(RuntimeError::Message(format!(\"expected Bool, got {{}}\", aivi_native_runtime::format_value(&other)))) }}"
            ),
            CgType::Text => format!(
                "match {expr} {{ Value::Text(v) => Ok(v), other => Err(RuntimeError::Message(format!(\"expected Text, got {{}}\", aivi_native_runtime::format_value(&other)))) }}"
            ),
            CgType::Unit => format!(
                "match {expr} {{ Value::Unit => Ok(()), other => Err(RuntimeError::Message(format!(\"expected Unit, got {{}}\", aivi_native_runtime::format_value(&other)))) }}"
            ),
            CgType::DateTime => format!(
                "match {expr} {{ Value::DateTime(v) => Ok(v), other => Err(RuntimeError::Message(format!(\"expected DateTime, got {{}}\", aivi_native_runtime::format_value(&other)))) }}"
            ),
            CgType::ListOf(elem) => {
                let elem_unbox = elem.emit_unbox("e");
                format!(
                    "match {expr} {{ Value::List(xs) => {{ let mut out = Vec::new(); for e in xs.iter().cloned() {{ out.push(({elem_unbox})?); }} Ok(out) }}, other => Err(RuntimeError::Message(format!(\"expected List, got {{}}\", aivi_native_runtime::format_value(&other)))) }}"
                )
            }
            CgType::Tuple(items) => {
                let mut parts = Vec::new();
                for (i, item_ty) in items.iter().enumerate() {
                    parts.push(format!("({})?", item_ty.emit_unbox(&format!("items[{i}].clone()"))));
                }
                format!(
                    "match {expr} {{ Value::Tuple(items) if items.len() == {} => Ok(({})), other => Err(RuntimeError::Message(format!(\"expected Tuple({}), got {{}}\", aivi_native_runtime::format_value(&other)))) }}",
                    items.len(),
                    parts.join(", "),
                    items.len()
                )
            }
            CgType::Record(fields) => {
                let mut parts = Vec::new();
                for (name, field_ty) in fields {
                    parts.push(format!(
                        "({})?",
                        field_ty.emit_unbox(&format!("m.get({name:?}).cloned().unwrap_or(Value::Unit)"))
                    ));
                }
                format!(
                    "match {expr} {{ Value::Record(m) => Ok(({})), other => Err(RuntimeError::Message(format!(\"expected Record, got {{}}\", aivi_native_runtime::format_value(&other)))) }}",
                    parts.join(", ")
                )
            }
            CgType::Func(_, _) | CgType::Adt { .. } => {
                // Complex types stay as Value
                format!("Ok({expr})")
            }
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

    #[test]
    fn rust_type_primitives() {
        assert_eq!(CgType::Int.rust_type(), "i64");
        assert_eq!(CgType::Float.rust_type(), "f64");
        assert_eq!(CgType::Bool.rust_type(), "bool");
        assert_eq!(CgType::Text.rust_type(), "String");
        assert_eq!(CgType::Unit.rust_type(), "()");
    }

    #[test]
    fn rust_type_list() {
        assert_eq!(
            CgType::ListOf(Box::new(CgType::Int)).rust_type(),
            "Vec<i64>"
        );
    }

    #[test]
    fn emit_box_int() {
        assert_eq!(CgType::Int.emit_box("x"), "Value::Int(x)");
    }

    #[test]
    fn emit_box_dynamic_is_identity() {
        assert_eq!(CgType::Dynamic.emit_box("x"), "x");
    }

    #[test]
    fn emit_unbox_dynamic_is_identity() {
        assert_eq!(CgType::Dynamic.emit_unbox("x"), "x");
    }
}
