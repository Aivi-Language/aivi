use std::collections::HashMap;
use std::sync::Arc;

use aivi_native_runtime::{format_value, values_equal, Value};

// ---------------------------------------------------------------------------
// format_value
// ---------------------------------------------------------------------------

#[test]
fn format_unit() {
    assert_eq!(format_value(&Value::Unit), "Unit");
}

#[test]
fn format_bool() {
    assert_eq!(format_value(&Value::Bool(true)), "true");
    assert_eq!(format_value(&Value::Bool(false)), "false");
}

#[test]
fn format_int() {
    assert_eq!(format_value(&Value::Int(42)), "42");
    assert_eq!(format_value(&Value::Int(-1)), "-1");
    assert_eq!(format_value(&Value::Int(0)), "0");
}

#[test]
fn format_float() {
    assert_eq!(format_value(&Value::Float(3.14)), "3.14");
    assert_eq!(format_value(&Value::Float(0.0)), "0");
}

#[test]
fn format_text() {
    assert_eq!(format_value(&Value::Text("hello".into())), "hello");
    assert_eq!(format_value(&Value::Text(String::new())), "");
}

#[test]
fn format_list() {
    let list = Value::List(Arc::new(vec![Value::Int(1), Value::Int(2), Value::Int(3)]));
    assert_eq!(format_value(&list), "[1, 2, 3]");
}

#[test]
fn format_empty_list() {
    let list = Value::List(Arc::new(vec![]));
    assert_eq!(format_value(&list), "[]");
}

#[test]
fn format_tuple() {
    let tuple = Value::Tuple(vec![Value::Int(1), Value::Text("a".into())]);
    assert_eq!(format_value(&tuple), "(1, a)");
}

#[test]
fn format_record() {
    let mut map = HashMap::new();
    map.insert("x".to_string(), Value::Int(10));
    let rec = Value::Record(Arc::new(map));
    assert_eq!(format_value(&rec), "{x: 10}");
}

#[test]
fn format_constructor_no_args() {
    let ctor = Value::Constructor {
        name: "None".into(),
        args: vec![],
    };
    assert_eq!(format_value(&ctor), "None");
}

#[test]
fn format_constructor_with_args() {
    let ctor = Value::Constructor {
        name: "Some".into(),
        args: vec![Value::Int(42)],
    };
    assert_eq!(format_value(&ctor), "Some(42)");
}

// ---------------------------------------------------------------------------
// values_equal – primitives
// ---------------------------------------------------------------------------

#[test]
fn equal_unit() {
    assert!(values_equal(&Value::Unit, &Value::Unit));
}

#[test]
fn equal_bool() {
    assert!(values_equal(&Value::Bool(true), &Value::Bool(true)));
    assert!(!values_equal(&Value::Bool(true), &Value::Bool(false)));
}

#[test]
fn equal_int() {
    assert!(values_equal(&Value::Int(0), &Value::Int(0)));
    assert!(!values_equal(&Value::Int(0), &Value::Int(1)));
}

#[test]
fn equal_float() {
    assert!(values_equal(&Value::Float(1.5), &Value::Float(1.5)));
    assert!(!values_equal(&Value::Float(1.5), &Value::Float(2.5)));
}

#[test]
fn equal_text() {
    assert!(values_equal(
        &Value::Text("hi".into()),
        &Value::Text("hi".into())
    ));
    assert!(!values_equal(
        &Value::Text("hi".into()),
        &Value::Text("bye".into())
    ));
}

// ---------------------------------------------------------------------------
// values_equal – containers
// ---------------------------------------------------------------------------

#[test]
fn equal_list() {
    let a = Value::List(Arc::new(vec![Value::Int(1), Value::Int(2)]));
    let b = Value::List(Arc::new(vec![Value::Int(1), Value::Int(2)]));
    let c = Value::List(Arc::new(vec![Value::Int(1)]));
    assert!(values_equal(&a, &b));
    assert!(!values_equal(&a, &c));
}

#[test]
fn equal_tuple() {
    let a = Value::Tuple(vec![Value::Int(1), Value::Bool(true)]);
    let b = Value::Tuple(vec![Value::Int(1), Value::Bool(true)]);
    let c = Value::Tuple(vec![Value::Int(1), Value::Bool(false)]);
    assert!(values_equal(&a, &b));
    assert!(!values_equal(&a, &c));
}

#[test]
fn equal_record() {
    let mut m1 = HashMap::new();
    m1.insert("a".into(), Value::Int(1));
    let mut m2 = HashMap::new();
    m2.insert("a".into(), Value::Int(1));
    let mut m3 = HashMap::new();
    m3.insert("a".into(), Value::Int(2));

    assert!(values_equal(
        &Value::Record(Arc::new(m1.clone())),
        &Value::Record(Arc::new(m2))
    ));
    assert!(!values_equal(
        &Value::Record(Arc::new(m1)),
        &Value::Record(Arc::new(m3))
    ));
}

#[test]
fn equal_constructor() {
    let a = Value::Constructor {
        name: "Some".into(),
        args: vec![Value::Int(1)],
    };
    let b = Value::Constructor {
        name: "Some".into(),
        args: vec![Value::Int(1)],
    };
    let c = Value::Constructor {
        name: "None".into(),
        args: vec![],
    };
    assert!(values_equal(&a, &b));
    assert!(!values_equal(&a, &c));
}

// ---------------------------------------------------------------------------
// values_equal – cross-type inequality
// ---------------------------------------------------------------------------

#[test]
fn not_equal_cross_type() {
    assert!(!values_equal(&Value::Int(0), &Value::Float(0.0)));
    assert!(!values_equal(&Value::Int(0), &Value::Text("0".into())));
    assert!(!values_equal(&Value::Unit, &Value::Bool(false)));
}
