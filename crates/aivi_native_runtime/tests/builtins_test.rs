#![allow(clippy::approx_constant)]

use aivi_native_runtime::{Runtime, Value};

/// Helper: look up a builtin by name from the global registry.
fn get(name: &str) -> Value {
    aivi_native_runtime::get_builtin(name).unwrap_or_else(|| panic!("builtin `{name}` not found"))
}

/// Apply a builtin (record-field) function to arguments through the runtime.
fn call(rt: &mut Runtime, func: Value, args: Vec<Value>) -> Value {
    rt.call(func, args).expect("call failed")
}

/// Get a field from a Record value.
fn field(rec: &Value, name: &str) -> Value {
    match rec {
        Value::Record(fields) => fields
            .get(name)
            .cloned()
            .unwrap_or_else(|| panic!("field `{name}` not found")),
        _ => panic!("expected Record"),
    }
}

/// Unwrap a Constructor value.
fn ctor_name(value: &Value) -> &str {
    match value {
        Value::Constructor { name, .. } => name.as_str(),
        _ => panic!(
            "expected Constructor, got {:?}",
            aivi_native_runtime::format_value(value)
        ),
    }
}

fn ctor_arg(value: &Value, idx: usize) -> Value {
    match value {
        Value::Constructor { args, .. } => args[idx].clone(),
        _ => panic!("expected Constructor"),
    }
}

fn as_int(value: &Value) -> i64 {
    match value {
        Value::Int(v) => *v,
        _ => panic!(
            "expected Int, got {}",
            aivi_native_runtime::format_value(value)
        ),
    }
}

fn as_float(value: &Value) -> f64 {
    match value {
        Value::Float(v) => *v,
        _ => panic!(
            "expected Float, got {}",
            aivi_native_runtime::format_value(value)
        ),
    }
}

fn as_text(value: &Value) -> &str {
    match value {
        Value::Text(v) => v.as_str(),
        _ => panic!(
            "expected Text, got {}",
            aivi_native_runtime::format_value(value)
        ),
    }
}

fn as_bool(value: &Value) -> bool {
    match value {
        Value::Bool(v) => *v,
        _ => panic!(
            "expected Bool, got {}",
            aivi_native_runtime::format_value(value)
        ),
    }
}

fn as_list(value: &Value) -> &[Value] {
    match value {
        Value::List(items) => items.as_slice(),
        _ => panic!(
            "expected List, got {}",
            aivi_native_runtime::format_value(value)
        ),
    }
}

fn as_tuple(value: &Value) -> &[Value] {
    match value {
        Value::Tuple(items) => items.as_slice(),
        _ => panic!(
            "expected Tuple, got {}",
            aivi_native_runtime::format_value(value)
        ),
    }
}

// ===========================================================================
// Math builtins
// ===========================================================================

mod math {
    use super::*;

    fn math() -> Value {
        get("math")
    }

    #[test]
    fn constants() {
        let m = math();
        assert!((as_float(&field(&m, "pi")) - std::f64::consts::PI).abs() < 1e-15);
        assert!((as_float(&field(&m, "e")) - std::f64::consts::E).abs() < 1e-15);
        assert!((as_float(&field(&m, "tau")) - std::f64::consts::TAU).abs() < 1e-15);
        assert!(as_float(&field(&m, "inf")).is_infinite());
        assert!(as_float(&field(&m, "nan")).is_nan());
    }

    #[test]
    fn abs_int() {
        let rt = &mut Runtime::new();
        let result = call(rt, field(&math(), "abs"), vec![Value::Int(-5)]);
        assert_eq!(as_int(&result), 5);
    }

    #[test]
    fn abs_float() {
        let rt = &mut Runtime::new();
        let result = call(rt, field(&math(), "abs"), vec![Value::Float(-3.14)]);
        assert!((as_float(&result) - 3.14).abs() < 1e-10);
    }

    #[test]
    fn sign() {
        let rt = &mut Runtime::new();
        assert_eq!(
            as_float(&call(rt, field(&math(), "sign"), vec![Value::Float(5.0)])),
            1.0
        );
        assert_eq!(
            as_float(&call(rt, field(&math(), "sign"), vec![Value::Float(-5.0)])),
            -1.0
        );
        assert_eq!(
            as_float(&call(rt, field(&math(), "sign"), vec![Value::Float(0.0)])),
            0.0
        );
    }

    #[test]
    fn min_max() {
        let rt = &mut Runtime::new();
        let min = call(
            rt,
            field(&math(), "min"),
            vec![Value::Float(1.0), Value::Float(2.0)],
        );
        assert_eq!(as_float(&min), 1.0);
        let max = call(
            rt,
            field(&math(), "max"),
            vec![Value::Float(1.0), Value::Float(2.0)],
        );
        assert_eq!(as_float(&max), 2.0);
    }

    #[test]
    fn clamp() {
        let rt = &mut Runtime::new();
        let r = call(
            rt,
            field(&math(), "clamp"),
            vec![Value::Float(0.0), Value::Float(10.0), Value::Float(15.0)],
        );
        assert_eq!(as_float(&r), 10.0);
    }

    #[test]
    fn floor_ceil_trunc_round() {
        let rt = &mut Runtime::new();
        assert_eq!(
            as_float(&call(rt, field(&math(), "floor"), vec![Value::Float(2.7)])),
            2.0
        );
        assert_eq!(
            as_float(&call(rt, field(&math(), "ceil"), vec![Value::Float(2.3)])),
            3.0
        );
        assert_eq!(
            as_float(&call(rt, field(&math(), "trunc"), vec![Value::Float(2.9)])),
            2.0
        );
        assert_eq!(
            as_float(&call(rt, field(&math(), "round"), vec![Value::Float(2.5)])),
            2.0
        ); // banker's rounding
        assert_eq!(
            as_float(&call(rt, field(&math(), "round"), vec![Value::Float(3.5)])),
            4.0
        ); // banker's rounding
    }

    #[test]
    fn pow_sqrt_cbrt() {
        let rt = &mut Runtime::new();
        assert!(
            (as_float(&call(
                rt,
                field(&math(), "pow"),
                vec![Value::Float(2.0), Value::Float(3.0)]
            )) - 8.0)
                .abs()
                < 1e-10
        );
        assert!(
            (as_float(&call(rt, field(&math(), "sqrt"), vec![Value::Float(9.0)])) - 3.0).abs()
                < 1e-10
        );
        assert!(
            (as_float(&call(rt, field(&math(), "cbrt"), vec![Value::Float(27.0)])) - 3.0).abs()
                < 1e-10
        );
    }

    #[test]
    fn exp_log() {
        let rt = &mut Runtime::new();
        let e_val = as_float(&call(rt, field(&math(), "exp"), vec![Value::Float(1.0)]));
        assert!((e_val - std::f64::consts::E).abs() < 1e-10);

        let ln_e = as_float(&call(
            rt,
            field(&math(), "log"),
            vec![Value::Float(std::f64::consts::E)],
        ));
        assert!((ln_e - 1.0).abs() < 1e-10);

        let log10 = as_float(&call(
            rt,
            field(&math(), "log10"),
            vec![Value::Float(100.0)],
        ));
        assert!((log10 - 2.0).abs() < 1e-10);

        let log2 = as_float(&call(rt, field(&math(), "log2"), vec![Value::Float(8.0)]));
        assert!((log2 - 3.0).abs() < 1e-10);
    }

    #[test]
    fn trig() {
        let rt = &mut Runtime::new();
        // sin/cos take an Angle record { radians: Float }
        let angle = |rad: f64| {
            let mut map = std::collections::HashMap::new();
            map.insert("radians".to_string(), Value::Float(rad));
            Value::Record(std::sync::Arc::new(map))
        };
        let sin0 = as_float(&call(rt, field(&math(), "sin"), vec![angle(0.0)]));
        assert!(sin0.abs() < 1e-10);

        let cos0 = as_float(&call(rt, field(&math(), "cos"), vec![angle(0.0)]));
        assert!((cos0 - 1.0).abs() < 1e-10);

        let sin_pi2 = as_float(&call(
            rt,
            field(&math(), "sin"),
            vec![angle(std::f64::consts::FRAC_PI_2)],
        ));
        assert!((sin_pi2 - 1.0).abs() < 1e-10);
    }

    #[test]
    fn hypot() {
        let rt = &mut Runtime::new();
        let r = call(
            rt,
            field(&math(), "hypot"),
            vec![Value::Float(3.0), Value::Float(4.0)],
        );
        assert!((as_float(&r) - 5.0).abs() < 1e-10);
    }

    #[test]
    fn gcd_lcm() {
        let rt = &mut Runtime::new();
        assert_eq!(
            as_int(&call(
                rt,
                field(&math(), "gcd"),
                vec![Value::Int(12), Value::Int(8)]
            )),
            4
        );
        assert_eq!(
            as_int(&call(
                rt,
                field(&math(), "lcm"),
                vec![Value::Int(4), Value::Int(6)]
            )),
            12
        );
    }

    #[test]
    fn factorial() {
        let rt = &mut Runtime::new();
        let result = call(rt, field(&math(), "factorial"), vec![Value::Int(5)]);
        match &result {
            Value::BigInt(v) => assert_eq!(**v, num_bigint::BigInt::from(120)),
            other => panic!(
                "expected BigInt, got {}",
                aivi_native_runtime::format_value(other)
            ),
        }
    }

    #[test]
    fn sum() {
        let rt = &mut Runtime::new();
        let list = Value::List(std::sync::Arc::new(vec![
            Value::Float(1.0),
            Value::Float(2.0),
            Value::Float(3.0),
        ]));
        let r = call(rt, field(&math(), "sum"), vec![list]);
        assert!((as_float(&r) - 6.0).abs() < 1e-10);
    }

    #[test]
    fn sum_int() {
        let rt = &mut Runtime::new();
        let list = Value::List(std::sync::Arc::new(vec![
            Value::Int(1),
            Value::Int(2),
            Value::Int(3),
        ]));
        let r = call(rt, field(&math(), "sumInt"), vec![list]);
        assert_eq!(as_int(&r), 6);
    }

    #[test]
    fn divmod() {
        let rt = &mut Runtime::new();
        let r = call(
            rt,
            field(&math(), "divmod"),
            vec![Value::Int(7), Value::Int(3)],
        );
        let t = as_tuple(&r);
        assert_eq!(as_int(&t[0]), 2);
        assert_eq!(as_int(&t[1]), 1);
    }

    #[test]
    fn divmod_negative() {
        let rt = &mut Runtime::new();
        let r = call(
            rt,
            field(&math(), "divmod"),
            vec![Value::Int(-7), Value::Int(3)],
        );
        let t = as_tuple(&r);
        // floored division: -7 / 3 = -3, remainder = 2
        assert_eq!(as_int(&t[0]), -3);
        assert_eq!(as_int(&t[1]), 2);
    }

    #[test]
    fn mod_pow() {
        let rt = &mut Runtime::new();
        // 2^10 mod 1000 = 1024 mod 1000 = 24
        let r = call(
            rt,
            field(&math(), "modPow"),
            vec![Value::Int(2), Value::Int(10), Value::Int(1000)],
        );
        assert_eq!(as_int(&r), 24);
    }

    #[test]
    fn is_finite_inf_nan() {
        let rt = &mut Runtime::new();
        assert!(as_bool(&call(
            rt,
            field(&math(), "isFinite"),
            vec![Value::Float(1.0)]
        )));
        assert!(!as_bool(&call(
            rt,
            field(&math(), "isFinite"),
            vec![Value::Float(f64::INFINITY)]
        )));
        assert!(as_bool(&call(
            rt,
            field(&math(), "isInf"),
            vec![Value::Float(f64::INFINITY)]
        )));
        assert!(as_bool(&call(
            rt,
            field(&math(), "isNaN"),
            vec![Value::Float(f64::NAN)]
        )));
        assert!(!as_bool(&call(
            rt,
            field(&math(), "isNaN"),
            vec![Value::Float(1.0)]
        )));
    }

    #[test]
    fn fract() {
        let rt = &mut Runtime::new();
        let r = call(rt, field(&math(), "fract"), vec![Value::Float(3.75)]);
        assert!((as_float(&r) - 0.75).abs() < 1e-10);
    }

    #[test]
    fn modf() {
        let rt = &mut Runtime::new();
        let r = call(rt, field(&math(), "modf"), vec![Value::Float(3.75)]);
        let t = as_tuple(&r);
        assert!((as_float(&t[0]) - 3.0).abs() < 1e-10);
        assert!((as_float(&t[1]) - 0.75).abs() < 1e-10);
    }

    #[test]
    fn copysign() {
        let rt = &mut Runtime::new();
        let r = call(
            rt,
            field(&math(), "copysign"),
            vec![Value::Float(5.0), Value::Float(-1.0)],
        );
        assert_eq!(as_float(&r), -5.0);
    }

    #[test]
    fn comb_perm() {
        let rt = &mut Runtime::new();
        // C(5,2) = 10
        let c = call(
            rt,
            field(&math(), "comb"),
            vec![Value::Int(5), Value::Int(2)],
        );
        match &c {
            Value::BigInt(v) => assert_eq!(**v, num_bigint::BigInt::from(10)),
            _ => panic!("expected BigInt"),
        }
        // P(5,2) = 20
        let p = call(
            rt,
            field(&math(), "perm"),
            vec![Value::Int(5), Value::Int(2)],
        );
        match &p {
            Value::BigInt(v) => assert_eq!(**v, num_bigint::BigInt::from(20)),
            _ => panic!("expected BigInt"),
        }
    }

    #[test]
    fn min_all_max_all() {
        let rt = &mut Runtime::new();
        let list = Value::List(std::sync::Arc::new(vec![
            Value::Float(3.0),
            Value::Float(1.0),
            Value::Float(2.0),
        ]));
        let min = call(rt, field(&math(), "minAll"), vec![list.clone()]);
        assert_eq!(ctor_name(&min), "Some");
        assert_eq!(as_float(&ctor_arg(&min, 0)), 1.0);

        let max = call(rt, field(&math(), "maxAll"), vec![list]);
        assert_eq!(ctor_name(&max), "Some");
        assert_eq!(as_float(&ctor_arg(&max, 0)), 3.0);

        // empty list -> None
        let empty = Value::List(std::sync::Arc::new(vec![]));
        let min_empty = call(rt, field(&math(), "minAll"), vec![empty]);
        assert_eq!(ctor_name(&min_empty), "None");
    }

    #[test]
    fn fmod() {
        let rt = &mut Runtime::new();
        let r = call(
            rt,
            field(&math(), "fmod"),
            vec![Value::Float(5.5), Value::Float(2.0)],
        );
        assert!((as_float(&r) - 1.5).abs() < 1e-10);
    }

    #[test]
    fn exp2_expm1_log1p() {
        let rt = &mut Runtime::new();
        assert!(
            (as_float(&call(rt, field(&math(), "exp2"), vec![Value::Float(3.0)])) - 8.0).abs()
                < 1e-10
        );
        let expm1_0 = as_float(&call(rt, field(&math(), "expm1"), vec![Value::Float(0.0)]));
        assert!(expm1_0.abs() < 1e-10);
        let log1p_0 = as_float(&call(rt, field(&math(), "log1p"), vec![Value::Float(0.0)]));
        assert!(log1p_0.abs() < 1e-10);
    }

    #[test]
    fn gcd_all_lcm_all() {
        let rt = &mut Runtime::new();
        let list = Value::List(std::sync::Arc::new(vec![
            Value::Int(12),
            Value::Int(8),
            Value::Int(4),
        ]));
        let gcd = call(rt, field(&math(), "gcdAll"), vec![list.clone()]);
        assert_eq!(ctor_name(&gcd), "Some");
        assert_eq!(as_int(&ctor_arg(&gcd, 0)), 4);

        let lcm_list = Value::List(std::sync::Arc::new(vec![Value::Int(4), Value::Int(6)]));
        let lcm = call(rt, field(&math(), "lcmAll"), vec![lcm_list]);
        assert_eq!(ctor_name(&lcm), "Some");
        assert_eq!(as_int(&ctor_arg(&lcm, 0)), 12);
    }
}

// ===========================================================================
// Text builtins
// ===========================================================================

mod text {
    use super::*;

    fn text() -> Value {
        get("text")
    }

    #[test]
    fn length() {
        let rt = &mut Runtime::new();
        let r = call(
            rt,
            field(&text(), "length"),
            vec![Value::Text("hello".into())],
        );
        assert_eq!(as_int(&r), 5);
    }

    #[test]
    fn length_unicode() {
        let rt = &mut Runtime::new();
        // grapheme cluster: single emoji
        let r = call(rt, field(&text(), "length"), vec![Value::Text("é".into())]);
        assert_eq!(as_int(&r), 1);
    }

    #[test]
    fn is_empty() {
        let rt = &mut Runtime::new();
        assert!(as_bool(&call(
            rt,
            field(&text(), "isEmpty"),
            vec![Value::Text("".into())]
        )));
        assert!(!as_bool(&call(
            rt,
            field(&text(), "isEmpty"),
            vec![Value::Text("a".into())]
        )));
    }

    #[test]
    fn contains() {
        let rt = &mut Runtime::new();
        assert!(as_bool(&call(
            rt,
            field(&text(), "contains"),
            vec![
                Value::Text("hello world".into()),
                Value::Text("world".into()),
            ]
        )));
        assert!(!as_bool(&call(
            rt,
            field(&text(), "contains"),
            vec![Value::Text("hello".into()), Value::Text("world".into()),]
        )));
    }

    #[test]
    fn starts_ends_with() {
        let rt = &mut Runtime::new();
        assert!(as_bool(&call(
            rt,
            field(&text(), "startsWith"),
            vec![
                Value::Text("hello world".into()),
                Value::Text("hello".into()),
            ]
        )));
        assert!(as_bool(&call(
            rt,
            field(&text(), "endsWith"),
            vec![
                Value::Text("hello world".into()),
                Value::Text("world".into()),
            ]
        )));
    }

    #[test]
    fn index_of() {
        let rt = &mut Runtime::new();
        let r = call(
            rt,
            field(&text(), "indexOf"),
            vec![Value::Text("hello".into()), Value::Text("ll".into())],
        );
        assert_eq!(ctor_name(&r), "Some");
        assert_eq!(as_int(&ctor_arg(&r, 0)), 2);

        let r2 = call(
            rt,
            field(&text(), "indexOf"),
            vec![Value::Text("hello".into()), Value::Text("xyz".into())],
        );
        assert_eq!(ctor_name(&r2), "None");
    }

    #[test]
    fn slice() {
        let rt = &mut Runtime::new();
        let r = call(
            rt,
            field(&text(), "slice"),
            vec![
                Value::Text("hello world".into()),
                Value::Int(0),
                Value::Int(5),
            ],
        );
        assert_eq!(as_text(&r), "hello");
    }

    #[test]
    fn slice_negative() {
        let rt = &mut Runtime::new();
        let r = call(
            rt,
            field(&text(), "slice"),
            vec![
                Value::Text("hello".into()),
                Value::Int(-3),
                Value::Int(5), // length
            ],
        );
        assert_eq!(as_text(&r), "llo");
    }

    #[test]
    fn split() {
        let rt = &mut Runtime::new();
        let r = call(
            rt,
            field(&text(), "split"),
            vec![Value::Text("a,b,c".into()), Value::Text(",".into())],
        );
        let items = as_list(&r);
        assert_eq!(items.len(), 3);
        assert_eq!(as_text(&items[0]), "a");
        assert_eq!(as_text(&items[1]), "b");
        assert_eq!(as_text(&items[2]), "c");
    }

    #[test]
    fn trim() {
        let rt = &mut Runtime::new();
        assert_eq!(
            as_text(&call(
                rt,
                field(&text(), "trim"),
                vec![Value::Text("  hi  ".into())]
            )),
            "hi"
        );
        assert_eq!(
            as_text(&call(
                rt,
                field(&text(), "trimStart"),
                vec![Value::Text("  hi  ".into())]
            )),
            "hi  "
        );
        assert_eq!(
            as_text(&call(
                rt,
                field(&text(), "trimEnd"),
                vec![Value::Text("  hi  ".into())]
            )),
            "  hi"
        );
    }

    #[test]
    fn to_lower_to_upper() {
        let rt = &mut Runtime::new();
        assert_eq!(
            as_text(&call(
                rt,
                field(&text(), "toLower"),
                vec![Value::Text("HELLO".into())]
            )),
            "hello"
        );
        assert_eq!(
            as_text(&call(
                rt,
                field(&text(), "toUpper"),
                vec![Value::Text("hello".into())]
            )),
            "HELLO"
        );
    }

    #[test]
    fn capitalize() {
        let rt = &mut Runtime::new();
        assert_eq!(
            as_text(&call(
                rt,
                field(&text(), "capitalize"),
                vec![Value::Text("hello".into())]
            )),
            "Hello"
        );
    }

    #[test]
    fn title_case() {
        let rt = &mut Runtime::new();
        assert_eq!(
            as_text(&call(
                rt,
                field(&text(), "titleCase"),
                vec![Value::Text("hello world".into())]
            )),
            "Hello World"
        );
    }

    #[test]
    fn replace() {
        let rt = &mut Runtime::new();
        let r = call(
            rt,
            field(&text(), "replace"),
            vec![
                Value::Text("aababab".into()),
                Value::Text("ab".into()),
                Value::Text("X".into()),
            ],
        );
        assert_eq!(as_text(&r), "aXabab"); // replaces first occurrence
    }

    #[test]
    fn replace_all() {
        let rt = &mut Runtime::new();
        let r = call(
            rt,
            field(&text(), "replaceAll"),
            vec![
                Value::Text("aababab".into()),
                Value::Text("ab".into()),
                Value::Text("X".into()),
            ],
        );
        assert_eq!(as_text(&r), "aXXX"); // replaces all
    }

    #[test]
    fn repeat() {
        let rt = &mut Runtime::new();
        let r = call(
            rt,
            field(&text(), "repeat"),
            vec![Value::Text("ab".into()), Value::Int(3)],
        );
        assert_eq!(as_text(&r), "ababab");
    }

    #[test]
    fn reverse() {
        let rt = &mut Runtime::new();
        assert_eq!(
            as_text(&call(
                rt,
                field(&text(), "reverse"),
                vec![Value::Text("abc".into())]
            )),
            "cba"
        );
    }

    #[test]
    fn concat() {
        let rt = &mut Runtime::new();
        let list = Value::List(std::sync::Arc::new(vec![
            Value::Text("a".into()),
            Value::Text("b".into()),
            Value::Text("c".into()),
        ]));
        assert_eq!(
            as_text(&call(rt, field(&text(), "concat"), vec![list])),
            "abc"
        );
    }

    #[test]
    fn pad_start_end() {
        let rt = &mut Runtime::new();
        let r = call(
            rt,
            field(&text(), "padStart"),
            vec![
                Value::Text("hi".into()),
                Value::Int(5),
                Value::Text("0".into()),
            ],
        );
        assert_eq!(as_text(&r), "000hi");

        let r2 = call(
            rt,
            field(&text(), "padEnd"),
            vec![
                Value::Text("hi".into()),
                Value::Int(5),
                Value::Text("0".into()),
            ],
        );
        assert_eq!(as_text(&r2), "hi000");
    }

    #[test]
    fn count() {
        let rt = &mut Runtime::new();
        let r = call(
            rt,
            field(&text(), "count"),
            vec![Value::Text("abcabc".into()), Value::Text("abc".into())],
        );
        assert_eq!(as_int(&r), 2);
    }

    #[test]
    fn compare() {
        let rt = &mut Runtime::new();
        let r = call(
            rt,
            field(&text(), "compare"),
            vec![Value::Text("a".into()), Value::Text("b".into())],
        );
        assert!(as_int(&r) < 0);
    }

    #[test]
    fn split_lines() {
        let rt = &mut Runtime::new();
        let r = call(
            rt,
            field(&text(), "splitLines"),
            vec![Value::Text("a\nb\nc".into())],
        );
        let items = as_list(&r);
        assert_eq!(items.len(), 3);
    }

    #[test]
    fn chunk() {
        let rt = &mut Runtime::new();
        let r = call(
            rt,
            field(&text(), "chunk"),
            vec![Value::Text("abcde".into()), Value::Int(2)],
        );
        let items = as_list(&r);
        assert_eq!(items.len(), 3);
        assert_eq!(as_text(&items[0]), "ab");
        assert_eq!(as_text(&items[1]), "cd");
        assert_eq!(as_text(&items[2]), "e");
    }

    #[test]
    fn remove() {
        let rt = &mut Runtime::new();
        let r = call(
            rt,
            field(&text(), "remove"),
            vec![
                Value::Text("hello world".into()),
                Value::Text("world".into()),
            ],
        );
        assert_eq!(as_text(&r), "hello ");
    }

    #[test]
    fn parse_int() {
        let rt = &mut Runtime::new();
        let r = call(
            rt,
            field(&text(), "parseInt"),
            vec![Value::Text("42".into())],
        );
        assert_eq!(ctor_name(&r), "Some");
        assert_eq!(as_int(&ctor_arg(&r, 0)), 42);

        let r2 = call(
            rt,
            field(&text(), "parseInt"),
            vec![Value::Text("abc".into())],
        );
        assert_eq!(ctor_name(&r2), "None");
    }

    #[test]
    fn parse_float() {
        let rt = &mut Runtime::new();
        let r = call(
            rt,
            field(&text(), "parseFloat"),
            vec![Value::Text("3.14".into())],
        );
        assert_eq!(ctor_name(&r), "Some");
        assert!((as_float(&ctor_arg(&r, 0)) - 3.14).abs() < 1e-10);
    }

    #[test]
    fn is_digit_alpha() {
        let rt = &mut Runtime::new();
        assert!(as_bool(&call(
            rt,
            field(&text(), "isDigit"),
            vec![Value::Text("5".into())]
        )));
        assert!(!as_bool(&call(
            rt,
            field(&text(), "isDigit"),
            vec![Value::Text("a".into())]
        )));
        assert!(as_bool(&call(
            rt,
            field(&text(), "isAlpha"),
            vec![Value::Text("a".into())]
        )));
        assert!(!as_bool(&call(
            rt,
            field(&text(), "isAlpha"),
            vec![Value::Text("5".into())]
        )));
    }

    #[test]
    fn to_bytes_from_bytes_utf8() {
        let rt = &mut Runtime::new();
        let utf8 = Value::Constructor {
            name: "Utf8".into(),
            args: vec![],
        };
        let bytes = call(
            rt,
            field(&text(), "toBytes"),
            vec![utf8.clone(), Value::Text("hello".into())],
        );
        match &bytes {
            Value::Bytes(b) => assert_eq!(b.as_slice(), b"hello"),
            _ => panic!("expected Bytes"),
        }

        let decoded = call(rt, field(&text(), "fromBytes"), vec![utf8, bytes]);
        assert_eq!(ctor_name(&decoded), "Ok");
        assert_eq!(as_text(&ctor_arg(&decoded, 0)), "hello");
    }

    #[test]
    fn normalize_nfc() {
        let rt = &mut Runtime::new();
        // e + combining acute -> é (NFC form)
        let r = call(
            rt,
            field(&text(), "normalizeNFC"),
            vec![Value::Text("e\u{0301}".into())],
        );
        assert_eq!(as_text(&r), "é");
    }

    #[test]
    fn last_index_of() {
        let rt = &mut Runtime::new();
        let r = call(
            rt,
            field(&text(), "lastIndexOf"),
            vec![Value::Text("abcabc".into()), Value::Text("abc".into())],
        );
        assert_eq!(ctor_name(&r), "Some");
        assert_eq!(as_int(&ctor_arg(&r, 0)), 3);
    }

    #[test]
    fn case_fold() {
        let rt = &mut Runtime::new();
        let r = call(
            rt,
            field(&text(), "caseFold"),
            vec![Value::Text("HELLO".into())],
        );
        assert_eq!(as_text(&r), "hello");
    }

    #[test]
    fn to_text() {
        let rt = &mut Runtime::new();
        let r = call(rt, field(&text(), "toText"), vec![Value::Int(42)]);
        assert_eq!(as_text(&r), "42");
    }
}

// ===========================================================================
// Collections builtins
// ===========================================================================

mod collections {
    use super::*;

    fn collections() -> Value {
        get("collections")
    }

    // --- Map ---
    mod map {
        use super::*;

        fn map_rec() -> Value {
            field(&collections(), "map")
        }

        #[test]
        fn empty_and_size() {
            let rt = &mut Runtime::new();
            let empty = field(&map_rec(), "empty");
            let size = call(rt, field(&map_rec(), "size"), vec![empty]);
            assert_eq!(as_int(&size), 0);
        }

        #[test]
        fn insert_get_has() {
            let rt = &mut Runtime::new();
            let empty = field(&map_rec(), "empty");
            let m = call(
                rt,
                field(&map_rec(), "insert"),
                vec![Value::Text("key".into()), Value::Int(42), empty],
            );
            let got = call(
                rt,
                field(&map_rec(), "get"),
                vec![Value::Text("key".into()), m.clone()],
            );
            assert_eq!(ctor_name(&got), "Some");
            assert_eq!(as_int(&ctor_arg(&got, 0)), 42);

            let has = call(
                rt,
                field(&map_rec(), "has"),
                vec![Value::Text("key".into()), m.clone()],
            );
            assert!(as_bool(&has));

            let missing = call(
                rt,
                field(&map_rec(), "get"),
                vec![Value::Text("nope".into()), m],
            );
            assert_eq!(ctor_name(&missing), "None");
        }

        #[test]
        fn remove() {
            let rt = &mut Runtime::new();
            let empty = field(&map_rec(), "empty");
            let m = call(
                rt,
                field(&map_rec(), "insert"),
                vec![Value::Text("a".into()), Value::Int(1), empty],
            );
            let removed = call(
                rt,
                field(&map_rec(), "remove"),
                vec![Value::Text("a".into()), m],
            );
            let size = call(rt, field(&map_rec(), "size"), vec![removed]);
            assert_eq!(as_int(&size), 0);
        }

        #[test]
        fn keys_values_entries() {
            let rt = &mut Runtime::new();
            let empty = field(&map_rec(), "empty");
            let m = call(
                rt,
                field(&map_rec(), "insert"),
                vec![Value::Text("x".into()), Value::Int(10), empty],
            );
            let keys = call(rt, field(&map_rec(), "keys"), vec![m.clone()]);
            assert_eq!(as_list(&keys).len(), 1);

            let values = call(rt, field(&map_rec(), "values"), vec![m.clone()]);
            assert_eq!(as_list(&values).len(), 1);

            let entries = call(rt, field(&map_rec(), "entries"), vec![m]);
            assert_eq!(as_list(&entries).len(), 1);
        }

        #[test]
        fn from_list() {
            let rt = &mut Runtime::new();
            let list = Value::List(std::sync::Arc::new(vec![
                Value::Tuple(vec![Value::Text("a".into()), Value::Int(1)]),
                Value::Tuple(vec![Value::Text("b".into()), Value::Int(2)]),
            ]));
            let m = call(rt, field(&map_rec(), "fromList"), vec![list]);
            let size = call(rt, field(&map_rec(), "size"), vec![m]);
            assert_eq!(as_int(&size), 2);
        }

        #[test]
        fn union() {
            let rt = &mut Runtime::new();
            let empty = field(&map_rec(), "empty");
            let m1 = call(
                rt,
                field(&map_rec(), "insert"),
                vec![Value::Text("a".into()), Value::Int(1), empty.clone()],
            );
            let m2 = call(
                rt,
                field(&map_rec(), "insert"),
                vec![Value::Text("b".into()), Value::Int(2), empty],
            );
            let merged = call(rt, field(&map_rec(), "union"), vec![m1, m2]);
            let size = call(rt, field(&map_rec(), "size"), vec![merged]);
            assert_eq!(as_int(&size), 2);
        }
    }

    // --- Set ---
    mod set {
        use super::*;

        fn set_rec() -> Value {
            field(&collections(), "set")
        }

        #[test]
        fn empty_and_size() {
            let rt = &mut Runtime::new();
            let empty = field(&set_rec(), "empty");
            let size = call(rt, field(&set_rec(), "size"), vec![empty]);
            assert_eq!(as_int(&size), 0);
        }

        #[test]
        fn insert_has_remove() {
            let rt = &mut Runtime::new();
            let empty = field(&set_rec(), "empty");
            let s = call(rt, field(&set_rec(), "insert"), vec![Value::Int(1), empty]);
            assert!(as_bool(&call(
                rt,
                field(&set_rec(), "has"),
                vec![Value::Int(1), s.clone()]
            )));

            let removed = call(rt, field(&set_rec(), "remove"), vec![Value::Int(1), s]);
            let size = call(rt, field(&set_rec(), "size"), vec![removed]);
            assert_eq!(as_int(&size), 0);
        }

        #[test]
        fn union_intersection_difference() {
            let rt = &mut Runtime::new();
            let empty = field(&set_rec(), "empty");
            let s1 = call(
                rt,
                field(&set_rec(), "insert"),
                vec![Value::Int(1), empty.clone()],
            );
            let s1 = call(rt, field(&set_rec(), "insert"), vec![Value::Int(2), s1]);
            let s2 = call(
                rt,
                field(&set_rec(), "insert"),
                vec![Value::Int(2), empty.clone()],
            );
            let s2 = call(rt, field(&set_rec(), "insert"), vec![Value::Int(3), s2]);

            let union = call(rt, field(&set_rec(), "union"), vec![s1.clone(), s2.clone()]);
            assert_eq!(as_int(&call(rt, field(&set_rec(), "size"), vec![union])), 3);

            let inter = call(
                rt,
                field(&set_rec(), "intersection"),
                vec![s1.clone(), s2.clone()],
            );
            assert_eq!(as_int(&call(rt, field(&set_rec(), "size"), vec![inter])), 1);

            let diff = call(rt, field(&set_rec(), "difference"), vec![s1, s2]);
            assert_eq!(as_int(&call(rt, field(&set_rec(), "size"), vec![diff])), 1);
        }

        #[test]
        fn from_list_to_list() {
            let rt = &mut Runtime::new();
            let list = Value::List(std::sync::Arc::new(vec![
                Value::Int(1),
                Value::Int(2),
                Value::Int(1),
            ]));
            let s = call(rt, field(&set_rec(), "fromList"), vec![list]);
            let size = call(rt, field(&set_rec(), "size"), vec![s.clone()]);
            assert_eq!(as_int(&size), 2);

            let back = call(rt, field(&set_rec(), "toList"), vec![s]);
            assert_eq!(as_list(&back).len(), 2);
        }
    }

    // --- Queue ---
    mod queue {
        use super::*;

        fn queue_rec() -> Value {
            field(&collections(), "queue")
        }

        #[test]
        fn enqueue_dequeue_peek() {
            let rt = &mut Runtime::new();
            let empty = field(&queue_rec(), "empty");
            let q = call(
                rt,
                field(&queue_rec(), "enqueue"),
                vec![Value::Int(1), empty],
            );
            let q = call(rt, field(&queue_rec(), "enqueue"), vec![Value::Int(2), q]);

            let peeked = call(rt, field(&queue_rec(), "peek"), vec![q.clone()]);
            assert_eq!(ctor_name(&peeked), "Some");
            assert_eq!(as_int(&ctor_arg(&peeked, 0)), 1);

            let dequeued = call(rt, field(&queue_rec(), "dequeue"), vec![q]);
            assert_eq!(ctor_name(&dequeued), "Some");
            let inner = ctor_arg(&dequeued, 0);
            let pair = as_tuple(&inner);
            assert_eq!(as_int(&pair[0]), 1);
        }

        #[test]
        fn dequeue_empty() {
            let rt = &mut Runtime::new();
            let empty = field(&queue_rec(), "empty");
            let result = call(rt, field(&queue_rec(), "dequeue"), vec![empty]);
            assert_eq!(ctor_name(&result), "None");
        }
    }

    // --- Deque ---
    mod deque {
        use super::*;

        fn deque_rec() -> Value {
            field(&collections(), "deque")
        }

        #[test]
        fn push_pop_front_back() {
            let rt = &mut Runtime::new();
            let empty = field(&deque_rec(), "empty");
            let d = call(
                rt,
                field(&deque_rec(), "pushFront"),
                vec![Value::Int(1), empty],
            );
            let d = call(rt, field(&deque_rec(), "pushBack"), vec![Value::Int(2), d]);

            let front = call(rt, field(&deque_rec(), "peekFront"), vec![d.clone()]);
            assert_eq!(ctor_name(&front), "Some");
            assert_eq!(as_int(&ctor_arg(&front, 0)), 1);

            let back = call(rt, field(&deque_rec(), "peekBack"), vec![d.clone()]);
            assert_eq!(ctor_name(&back), "Some");
            assert_eq!(as_int(&ctor_arg(&back, 0)), 2);

            let popped = call(rt, field(&deque_rec(), "popFront"), vec![d]);
            assert_eq!(ctor_name(&popped), "Some");
        }
    }

    // --- Heap ---
    mod heap {
        use super::*;

        fn heap_rec() -> Value {
            field(&collections(), "heap")
        }

        #[test]
        fn push_pop_peek() {
            let rt = &mut Runtime::new();
            let empty = field(&heap_rec(), "empty");
            let h = call(rt, field(&heap_rec(), "push"), vec![Value::Int(3), empty]);
            let h = call(rt, field(&heap_rec(), "push"), vec![Value::Int(1), h]);
            let h = call(rt, field(&heap_rec(), "push"), vec![Value::Int(2), h]);

            let size = call(rt, field(&heap_rec(), "size"), vec![h.clone()]);
            assert_eq!(as_int(&size), 3);

            let min = call(rt, field(&heap_rec(), "peekMin"), vec![h.clone()]);
            assert_eq!(ctor_name(&min), "Some");
            assert_eq!(as_int(&ctor_arg(&min, 0)), 1);

            let popped = call(rt, field(&heap_rec(), "popMin"), vec![h]);
            assert_eq!(ctor_name(&popped), "Some");
            let inner = ctor_arg(&popped, 0);
            let pair = as_tuple(&inner);
            assert_eq!(as_int(&pair[0]), 1); // min element
        }

        #[test]
        fn from_list() {
            let rt = &mut Runtime::new();
            let list = Value::List(std::sync::Arc::new(vec![
                Value::Int(5),
                Value::Int(1),
                Value::Int(3),
            ]));
            let h = call(rt, field(&heap_rec(), "fromList"), vec![list]);
            let size = call(rt, field(&heap_rec(), "size"), vec![h]);
            assert_eq!(as_int(&size), 3);
        }
    }
}

// ===========================================================================
// Crypto builtins
// ===========================================================================

mod crypto {
    use super::*;

    fn crypto() -> Value {
        get("crypto")
    }

    #[test]
    fn sha256() {
        let rt = &mut Runtime::new();
        let r = call(
            rt,
            field(&crypto(), "sha256"),
            vec![Value::Text("hello".into())],
        );
        let hex = as_text(&r);
        // SHA-256 of "hello" is well-known
        assert_eq!(
            hex,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn random_uuid_is_effect() {
        let rt = &mut Runtime::new();
        let effect = call(rt, field(&crypto(), "randomUuid"), vec![Value::Unit]);
        // randomUuid returns an Effect
        match &effect {
            Value::Effect(_) => {}
            _ => panic!("expected Effect"),
        }
        // Run the effect and check UUID format
        let uuid = rt.run_effect_value(effect).expect("effect failed");
        let uuid_str = as_text(&uuid);
        assert_eq!(uuid_str.len(), 36);
        assert_eq!(uuid_str.chars().filter(|c| *c == '-').count(), 4);
    }

    #[test]
    fn random_bytes_effect() {
        let rt = &mut Runtime::new();
        let effect = call(rt, field(&crypto(), "randomBytes"), vec![Value::Int(16)]);
        match &effect {
            Value::Effect(_) => {}
            _ => panic!("expected Effect"),
        }
        let bytes = rt.run_effect_value(effect).expect("effect failed");
        match &bytes {
            Value::Bytes(b) => assert_eq!(b.len(), 16),
            _ => panic!("expected Bytes"),
        }
    }
}

// ===========================================================================
// Color builtins
// ===========================================================================

mod color {
    use super::*;

    fn color() -> Value {
        get("color")
    }

    fn rgb(r: i64, g: i64, b: i64) -> Value {
        let mut map = std::collections::HashMap::new();
        map.insert("r".to_string(), Value::Int(r));
        map.insert("g".to_string(), Value::Int(g));
        map.insert("b".to_string(), Value::Int(b));
        Value::Record(std::sync::Arc::new(map))
    }

    #[test]
    fn to_hex() {
        let rt = &mut Runtime::new();
        let r = call(rt, field(&color(), "toHex"), vec![rgb(255, 128, 0)]);
        assert_eq!(as_text(&r), "#ff8000");
    }

    #[test]
    fn to_hex_black() {
        let rt = &mut Runtime::new();
        let r = call(rt, field(&color(), "toHex"), vec![rgb(0, 0, 0)]);
        assert_eq!(as_text(&r), "#000000");
    }

    #[test]
    fn to_hsl_and_back() {
        let rt = &mut Runtime::new();
        let original = rgb(255, 0, 0); // pure red
        let hsl = call(rt, field(&color(), "toHsl"), vec![original.clone()]);
        let back = call(rt, field(&color(), "toRgb"), vec![hsl]);
        // Should be approximately the same
        let r_val = as_int(&field(&back, "r"));
        assert!((r_val - 255).abs() <= 1);
        assert!(as_int(&field(&back, "g")).abs() <= 1);
        assert!(as_int(&field(&back, "b")).abs() <= 1);
    }

    #[test]
    fn adjust_lightness() {
        let rt = &mut Runtime::new();
        let r = call(
            rt,
            field(&color(), "adjustLightness"),
            vec![rgb(128, 128, 128), Value::Int(10)],
        );
        // Should be lighter
        let new_r = as_int(&field(&r, "r"));
        assert!(new_r > 128);
    }

    #[test]
    fn adjust_saturation() {
        let rt = &mut Runtime::new();
        // With a colored input, adjusting saturation should change things
        let r = call(
            rt,
            field(&color(), "adjustSaturation"),
            vec![rgb(255, 0, 0), Value::Int(-50)],
        );
        // Red with less saturation should have more balanced RGB
        let g_val = as_int(&field(&r, "g"));
        assert!(g_val > 0);
    }

    #[test]
    fn adjust_hue() {
        let rt = &mut Runtime::new();
        // Adjusting hue by 120 degrees from red should give green-ish
        let r = call(
            rt,
            field(&color(), "adjustHue"),
            vec![rgb(255, 0, 0), Value::Int(120)],
        );
        let g_val = as_int(&field(&r, "g"));
        assert!(g_val > 200);
    }
}

// ===========================================================================
// Calendar builtins
// ===========================================================================

mod calendar {
    use super::*;

    fn calendar() -> Value {
        get("calendar")
    }

    fn date(year: i64, month: i64, day: i64) -> Value {
        let mut map = std::collections::HashMap::new();
        map.insert("year".to_string(), Value::Int(year));
        map.insert("month".to_string(), Value::Int(month));
        map.insert("day".to_string(), Value::Int(day));
        Value::Record(std::sync::Arc::new(map))
    }

    #[test]
    fn is_leap_year() {
        let rt = &mut Runtime::new();
        assert!(as_bool(&call(
            rt,
            field(&calendar(), "isLeapYear"),
            vec![date(2024, 1, 1)]
        )));
        assert!(!as_bool(&call(
            rt,
            field(&calendar(), "isLeapYear"),
            vec![date(2023, 1, 1)]
        )));
        assert!(as_bool(&call(
            rt,
            field(&calendar(), "isLeapYear"),
            vec![date(2000, 1, 1)]
        )));
        assert!(!as_bool(&call(
            rt,
            field(&calendar(), "isLeapYear"),
            vec![date(1900, 1, 1)]
        )));
    }

    #[test]
    fn days_in_month() {
        let rt = &mut Runtime::new();
        assert_eq!(
            as_int(&call(
                rt,
                field(&calendar(), "daysInMonth"),
                vec![date(2024, 2, 1)]
            )),
            29
        );
        assert_eq!(
            as_int(&call(
                rt,
                field(&calendar(), "daysInMonth"),
                vec![date(2023, 2, 1)]
            )),
            28
        );
        assert_eq!(
            as_int(&call(
                rt,
                field(&calendar(), "daysInMonth"),
                vec![date(2024, 1, 1)]
            )),
            31
        );
    }

    #[test]
    fn end_of_month() {
        let rt = &mut Runtime::new();
        let r = call(
            rt,
            field(&calendar(), "endOfMonth"),
            vec![date(2024, 2, 15)],
        );
        assert_eq!(as_int(&field(&r, "day")), 29);
    }

    #[test]
    fn add_days() {
        let rt = &mut Runtime::new();
        let r = call(
            rt,
            field(&calendar(), "addDays"),
            vec![date(2024, 1, 30), Value::Int(5)],
        );
        assert_eq!(as_int(&field(&r, "month")), 2);
        assert_eq!(as_int(&field(&r, "day")), 4);
    }

    #[test]
    fn add_months() {
        let rt = &mut Runtime::new();
        let r = call(
            rt,
            field(&calendar(), "addMonths"),
            vec![date(2024, 1, 31), Value::Int(1)],
        );
        assert_eq!(as_int(&field(&r, "month")), 2);
        // Jan 31 + 1 month should clamp to Feb 29 (2024 is leap year)
        assert_eq!(as_int(&field(&r, "day")), 29);
    }

    #[test]
    fn add_years() {
        let rt = &mut Runtime::new();
        let r = call(
            rt,
            field(&calendar(), "addYears"),
            vec![date(2024, 2, 29), Value::Int(1)],
        );
        // Feb 29, 2024 + 1 year -> Feb 28, 2025 (non-leap)
        assert_eq!(as_int(&field(&r, "year")), 2025);
        assert_eq!(as_int(&field(&r, "day")), 28);
    }
}

// ===========================================================================
// Regex builtins
// ===========================================================================

mod regex_tests {
    use super::*;

    fn regex() -> Value {
        get("regex")
    }

    #[test]
    fn compile_ok() {
        let rt = &mut Runtime::new();
        let r = call(
            rt,
            field(&regex(), "compile"),
            vec![Value::Text(r"\d+".into())],
        );
        assert_eq!(ctor_name(&r), "Ok");
    }

    #[test]
    fn compile_err() {
        let rt = &mut Runtime::new();
        let r = call(
            rt,
            field(&regex(), "compile"),
            vec![Value::Text("[invalid".into())],
        );
        assert_eq!(ctor_name(&r), "Err");
    }

    #[test]
    fn test_match() {
        let rt = &mut Runtime::new();
        let compiled = call(
            rt,
            field(&regex(), "compile"),
            vec![Value::Text(r"\d+".into())],
        );
        let rx = ctor_arg(&compiled, 0);

        assert!(as_bool(&call(
            rt,
            field(&regex(), "test"),
            vec![rx.clone(), Value::Text("abc123def".into())]
        )));
        assert!(!as_bool(&call(
            rt,
            field(&regex(), "test"),
            vec![rx, Value::Text("abcdef".into())]
        )));
    }

    #[test]
    fn match_captures() {
        let rt = &mut Runtime::new();
        let compiled = call(
            rt,
            field(&regex(), "compile"),
            vec![Value::Text(r"(\d+)-(\w+)".into())],
        );
        let rx = ctor_arg(&compiled, 0);

        let m = call(
            rt,
            field(&regex(), "match"),
            vec![rx, Value::Text("abc 42-hello xyz".into())],
        );
        assert_eq!(ctor_name(&m), "Some");
        let rec = ctor_arg(&m, 0);
        let full_val = field(&rec, "full");
        let full = as_text(&full_val);
        assert_eq!(full, "42-hello");
    }

    #[test]
    fn find() {
        let rt = &mut Runtime::new();
        let compiled = call(
            rt,
            field(&regex(), "compile"),
            vec![Value::Text(r"\d+".into())],
        );
        let rx = ctor_arg(&compiled, 0);

        let found = call(
            rt,
            field(&regex(), "find"),
            vec![rx, Value::Text("abc123def".into())],
        );
        assert_eq!(ctor_name(&found), "Some");
        let inner = ctor_arg(&found, 0);
        let pair = as_tuple(&inner);
        assert_eq!(as_int(&pair[0]), 3); // start
        assert_eq!(as_int(&pair[1]), 6); // end
    }

    #[test]
    fn find_all() {
        let rt = &mut Runtime::new();
        let compiled = call(
            rt,
            field(&regex(), "compile"),
            vec![Value::Text(r"\d+".into())],
        );
        let rx = ctor_arg(&compiled, 0);

        let results = call(
            rt,
            field(&regex(), "findAll"),
            vec![rx, Value::Text("1 and 22 and 333".into())],
        );
        assert_eq!(as_list(&results).len(), 3);
    }

    #[test]
    fn split() {
        let rt = &mut Runtime::new();
        let compiled = call(
            rt,
            field(&regex(), "compile"),
            vec![Value::Text(r"\s+".into())],
        );
        let rx = ctor_arg(&compiled, 0);

        let parts = call(
            rt,
            field(&regex(), "split"),
            vec![rx, Value::Text("a  b   c".into())],
        );
        let items = as_list(&parts);
        assert_eq!(items.len(), 3);
        assert_eq!(as_text(&items[0]), "a");
        assert_eq!(as_text(&items[1]), "b");
        assert_eq!(as_text(&items[2]), "c");
    }

    #[test]
    fn replace() {
        let rt = &mut Runtime::new();
        let compiled = call(
            rt,
            field(&regex(), "compile"),
            vec![Value::Text(r"\d+".into())],
        );
        let rx = ctor_arg(&compiled, 0);

        let result = call(
            rt,
            field(&regex(), "replace"),
            vec![
                rx,
                Value::Text("abc 123 def 456".into()),
                Value::Text("NUM".into()),
            ],
        );
        assert_eq!(as_text(&result), "abc NUM def 456");
    }

    #[test]
    fn replace_all() {
        let rt = &mut Runtime::new();
        let compiled = call(
            rt,
            field(&regex(), "compile"),
            vec![Value::Text(r"\d+".into())],
        );
        let rx = ctor_arg(&compiled, 0);

        let result = call(
            rt,
            field(&regex(), "replaceAll"),
            vec![
                rx,
                Value::Text("abc 123 def 456".into()),
                Value::Text("NUM".into()),
            ],
        );
        assert_eq!(as_text(&result), "abc NUM def NUM");
    }

    #[test]
    fn matches_multiple() {
        let rt = &mut Runtime::new();
        let compiled = call(
            rt,
            field(&regex(), "compile"),
            vec![Value::Text(r"(\w+)@(\w+)".into())],
        );
        let rx = ctor_arg(&compiled, 0);

        let results = call(
            rt,
            field(&regex(), "matches"),
            vec![rx, Value::Text("a@b c@d".into())],
        );
        let items = as_list(&results);
        assert_eq!(items.len(), 2);
    }
}

// ===========================================================================
// Number builtins (BigInt, Rational, Decimal)
// ===========================================================================

mod number {
    use super::*;

    fn bigint() -> Value {
        get("bigint")
    }

    fn rational() -> Value {
        get("rational")
    }

    fn decimal() -> Value {
        get("decimal")
    }

    #[test]
    fn bigint_from_int_round_trip() {
        let rt = &mut Runtime::new();
        let b = call(rt, field(&bigint(), "fromInt"), vec![Value::Int(42)]);
        let back = call(rt, field(&bigint(), "toInt"), vec![b]);
        assert_eq!(as_int(&back), 42);
    }

    #[test]
    fn bigint_add_sub_mul() {
        let rt = &mut Runtime::new();
        let a = call(rt, field(&bigint(), "fromInt"), vec![Value::Int(10)]);
        let b = call(rt, field(&bigint(), "fromInt"), vec![Value::Int(20)]);

        let sum = call(rt, field(&bigint(), "add"), vec![a.clone(), b.clone()]);
        let sum_int = call(rt, field(&bigint(), "toInt"), vec![sum]);
        assert_eq!(as_int(&sum_int), 30);

        let diff = call(rt, field(&bigint(), "sub"), vec![a.clone(), b.clone()]);
        let diff_int = call(rt, field(&bigint(), "toInt"), vec![diff]);
        assert_eq!(as_int(&diff_int), -10);

        let prod = call(rt, field(&bigint(), "mul"), vec![a, b]);
        let prod_int = call(rt, field(&bigint(), "toInt"), vec![prod]);
        assert_eq!(as_int(&prod_int), 200);
    }

    #[test]
    fn rational_from_big_ints() {
        let rt = &mut Runtime::new();
        let n = call(rt, field(&bigint(), "fromInt"), vec![Value::Int(1)]);
        let d = call(rt, field(&bigint(), "fromInt"), vec![Value::Int(3)]);
        let r = call(rt, field(&rational(), "fromBigInts"), vec![n, d]);

        let numer = call(rt, field(&rational(), "numerator"), vec![r.clone()]);
        let denom = call(rt, field(&rational(), "denominator"), vec![r]);
        let n_int = call(rt, field(&bigint(), "toInt"), vec![numer]);
        let d_int = call(rt, field(&bigint(), "toInt"), vec![denom]);
        assert_eq!(as_int(&n_int), 1);
        assert_eq!(as_int(&d_int), 3);
    }

    #[test]
    fn rational_arithmetic() {
        let rt = &mut Runtime::new();
        let mut make_rat = |n: i64, d: i64| {
            let n = call(rt, field(&bigint(), "fromInt"), vec![Value::Int(n)]);
            let d = call(rt, field(&bigint(), "fromInt"), vec![Value::Int(d)]);
            call(rt, field(&rational(), "fromBigInts"), vec![n, d])
        };
        let a = make_rat(1, 2);
        let b = make_rat(1, 3);

        let sum = call(rt, field(&rational(), "add"), vec![a.clone(), b.clone()]);
        // 1/2 + 1/3 = 5/6
        let numer = call(rt, field(&rational(), "numerator"), vec![sum.clone()]);
        let denom = call(rt, field(&rational(), "denominator"), vec![sum]);
        let n_int = call(rt, field(&bigint(), "toInt"), vec![numer]);
        let d_int = call(rt, field(&bigint(), "toInt"), vec![denom]);
        assert_eq!(as_int(&n_int), 5);
        assert_eq!(as_int(&d_int), 6);
    }

    #[test]
    fn decimal_from_float_round_trip() {
        let rt = &mut Runtime::new();
        let d = call(rt, field(&decimal(), "fromFloat"), vec![Value::Float(3.14)]);
        let back = call(rt, field(&decimal(), "toFloat"), vec![d]);
        assert!((as_float(&back) - 3.14).abs() < 1e-10);
    }

    #[test]
    fn decimal_arithmetic() {
        let rt = &mut Runtime::new();
        let a = call(rt, field(&decimal(), "fromFloat"), vec![Value::Float(1.5)]);
        let b = call(rt, field(&decimal(), "fromFloat"), vec![Value::Float(2.5)]);

        let sum = call(rt, field(&decimal(), "add"), vec![a.clone(), b.clone()]);
        let sum_f = call(rt, field(&decimal(), "toFloat"), vec![sum]);
        assert!((as_float(&sum_f) - 4.0).abs() < 1e-10);

        let diff = call(rt, field(&decimal(), "sub"), vec![a.clone(), b.clone()]);
        let diff_f = call(rt, field(&decimal(), "toFloat"), vec![diff]);
        assert!((as_float(&diff_f) - (-1.0)).abs() < 1e-10);

        let prod = call(rt, field(&decimal(), "mul"), vec![a.clone(), b.clone()]);
        let prod_f = call(rt, field(&decimal(), "toFloat"), vec![prod]);
        assert!((as_float(&prod_f) - 3.75).abs() < 1e-10);

        let div_r = call(rt, field(&decimal(), "div"), vec![a, b]);
        let div_f = call(rt, field(&decimal(), "toFloat"), vec![div_r]);
        assert!((as_float(&div_f) - 0.6).abs() < 1e-10);
    }

    #[test]
    fn decimal_round() {
        let rt = &mut Runtime::new();
        let d = call(
            rt,
            field(&decimal(), "fromFloat"),
            vec![Value::Float(3.14159)],
        );
        let rounded = call(rt, field(&decimal(), "round"), vec![d, Value::Int(2)]);
        let f = call(rt, field(&decimal(), "toFloat"), vec![rounded]);
        assert!((as_float(&f) - 3.14).abs() < 1e-10);
    }
}

// ===========================================================================
// Core builtins (pure, fail, bind, attempt, map, chain, etc.)
// ===========================================================================

mod core_builtins {
    use super::*;

    #[test]
    fn unit_true_false_none() {
        assert!(matches!(get("Unit"), Value::Unit));
        assert!(matches!(get("True"), Value::Bool(true)));
        assert!(matches!(get("False"), Value::Bool(false)));
        assert_eq!(ctor_name(&get("None")), "None");
    }

    #[test]
    fn some_ok_err_constructors() {
        let rt = &mut Runtime::new();
        let some = call(rt, get("Some"), vec![Value::Int(42)]);
        assert_eq!(ctor_name(&some), "Some");
        assert_eq!(as_int(&ctor_arg(&some, 0)), 42);

        let ok = call(rt, get("Ok"), vec![Value::Int(1)]);
        assert_eq!(ctor_name(&ok), "Ok");

        let err = call(rt, get("Err"), vec![Value::Text("oops".into())]);
        assert_eq!(ctor_name(&err), "Err");
    }

    #[test]
    fn pure_and_run() {
        let rt = &mut Runtime::new();
        let eff = call(rt, get("pure"), vec![Value::Int(42)]);
        let result = rt.run_effect_value(eff).unwrap();
        assert_eq!(as_int(&result), 42);
    }

    #[test]
    fn fail_and_run() {
        let rt = &mut Runtime::new();
        let eff = call(rt, get("fail"), vec![Value::Text("boom".into())]);
        let result = rt.run_effect_value(eff);
        assert!(result.is_err());
    }

    #[test]
    fn bind_chaining() {
        let rt = &mut Runtime::new();
        let eff1 = call(rt, get("pure"), vec![Value::Int(10)]);
        // bind eff1 (\x -> pure (x + 1))
        let add_one = Value::Closure(std::sync::Arc::new(aivi_native_runtime::ClosureValue {
            func: std::sync::Arc::new(|arg, rt| {
                let n = match arg {
                    Value::Int(n) => n,
                    _ => {
                        return Err(aivi_native_runtime::RuntimeError::Message(
                            "expected Int".into(),
                        ))
                    }
                };
                let inner = rt.call(
                    aivi_native_runtime::get_builtin("pure").unwrap(),
                    vec![Value::Int(n + 1)],
                )?;
                Ok(inner)
            }),
        }));
        let chained = call(rt, get("bind"), vec![eff1, add_one]);
        let result = rt.run_effect_value(chained).unwrap();
        assert_eq!(as_int(&result), 11);
    }

    #[test]
    fn attempt_ok() {
        let rt = &mut Runtime::new();
        let eff = call(rt, get("pure"), vec![Value::Int(42)]);
        let attempted = call(rt, get("attempt"), vec![eff]);
        let result = rt.run_effect_value(attempted).unwrap();
        assert_eq!(ctor_name(&result), "Ok");
        assert_eq!(as_int(&ctor_arg(&result, 0)), 42);
    }

    #[test]
    fn attempt_err() {
        let rt = &mut Runtime::new();
        let eff = call(rt, get("fail"), vec![Value::Text("boom".into())]);
        let attempted = call(rt, get("attempt"), vec![eff]);
        let result = rt.run_effect_value(attempted).unwrap();
        assert_eq!(ctor_name(&result), "Err");
    }

    #[test]
    fn map_list() {
        let rt = &mut Runtime::new();
        let list = Value::List(std::sync::Arc::new(vec![
            Value::Int(1),
            Value::Int(2),
            Value::Int(3),
        ]));
        let double = Value::Closure(std::sync::Arc::new(aivi_native_runtime::ClosureValue {
            func: std::sync::Arc::new(|arg, _| {
                let n = match arg {
                    Value::Int(n) => n,
                    _ => {
                        return Err(aivi_native_runtime::RuntimeError::Message(
                            "expected Int".into(),
                        ))
                    }
                };
                Ok(Value::Int(n * 2))
            }),
        }));
        let result = call(rt, get("map"), vec![double, list]);
        let items = as_list(&result);
        assert_eq!(items.len(), 3);
        assert_eq!(as_int(&items[0]), 2);
        assert_eq!(as_int(&items[1]), 4);
        assert_eq!(as_int(&items[2]), 6);
    }

    #[test]
    fn map_option() {
        let rt = &mut Runtime::new();
        let some = Value::Constructor {
            name: "Some".into(),
            args: vec![Value::Int(5)],
        };
        let none = Value::Constructor {
            name: "None".into(),
            args: vec![],
        };
        let add_one = Value::Closure(std::sync::Arc::new(aivi_native_runtime::ClosureValue {
            func: std::sync::Arc::new(|arg, _| match arg {
                Value::Int(n) => Ok(Value::Int(n + 1)),
                _ => Err(aivi_native_runtime::RuntimeError::Message(
                    "expected Int".into(),
                )),
            }),
        }));
        let r1 = call(rt, get("map"), vec![add_one.clone(), some]);
        assert_eq!(ctor_name(&r1), "Some");
        assert_eq!(as_int(&ctor_arg(&r1, 0)), 6);

        let r2 = call(rt, get("map"), vec![add_one, none]);
        assert_eq!(ctor_name(&r2), "None");
    }

    #[test]
    fn chain_list() {
        let rt = &mut Runtime::new();
        let list = Value::List(std::sync::Arc::new(vec![Value::Int(1), Value::Int(2)]));
        let dup = Value::Closure(std::sync::Arc::new(aivi_native_runtime::ClosureValue {
            func: std::sync::Arc::new(|arg, _| {
                let n = match arg {
                    Value::Int(n) => n,
                    _ => {
                        return Err(aivi_native_runtime::RuntimeError::Message(
                            "expected Int".into(),
                        ))
                    }
                };
                Ok(Value::List(std::sync::Arc::new(vec![
                    Value::Int(n),
                    Value::Int(n),
                ])))
            }),
        }));
        let result = call(rt, get("chain"), vec![dup, list]);
        let items = as_list(&result);
        assert_eq!(items.len(), 4);
        assert_eq!(as_int(&items[0]), 1);
        assert_eq!(as_int(&items[1]), 1);
        assert_eq!(as_int(&items[2]), 2);
        assert_eq!(as_int(&items[3]), 2);
    }

    #[test]
    fn load_source() {
        let rt = &mut Runtime::new();
        // load on an Effect should return it unchanged
        let eff = call(rt, get("pure"), vec![Value::Int(99)]);
        let loaded = call(rt, get("load"), vec![eff]);
        let result = rt.run_effect_value(loaded).unwrap();
        assert_eq!(as_int(&result), 99);
    }

    #[test]
    fn fold_gen() {
        let rt = &mut Runtime::new();
        // A simple generator: \step z -> step z 1 |> step  2
        // foldGen gen step init => gen step init
        let gen = Value::Closure(std::sync::Arc::new(aivi_native_runtime::ClosureValue {
            func: std::sync::Arc::new(|step, _rt| {
                // returns a closure that takes `z` and folds [1, 2]
                let step2 = step.clone();
                Ok(Value::Closure(std::sync::Arc::new(
                    aivi_native_runtime::ClosureValue {
                        func: std::sync::Arc::new(move |z, rt| {
                            let r1 = rt.call(step2.clone(), vec![z, Value::Int(1)])?;
                            let r2 = rt.call(step2.clone(), vec![r1, Value::Int(2)])?;
                            Ok(r2)
                        }),
                    },
                )))
            }),
        }));
        let step = Value::Closure(std::sync::Arc::new(aivi_native_runtime::ClosureValue {
            func: std::sync::Arc::new(|acc, _rt| {
                // returns a closure waiting for the element
                Ok(Value::Closure(std::sync::Arc::new(
                    aivi_native_runtime::ClosureValue {
                        func: std::sync::Arc::new(move |elem, _| {
                            let a = match &acc {
                                Value::Int(n) => *n,
                                _ => 0,
                            };
                            let b = match elem {
                                Value::Int(n) => n,
                                _ => 0,
                            };
                            Ok(Value::Int(a + b))
                        }),
                    },
                )))
            }),
        }));
        let result = call(rt, get("foldGen"), vec![gen, step, Value::Int(0)]);
        assert_eq!(as_int(&result), 3); // 0 + 1 + 2
    }
}

// ===========================================================================
// Runtime tests
// ===========================================================================

mod runtime {
    use super::*;
    use aivi_native_runtime::{ClosureValue, Runtime, RuntimeError};

    #[test]
    fn apply_closure() {
        let rt = &mut Runtime::new();
        let add_one = Value::Closure(std::sync::Arc::new(ClosureValue {
            func: std::sync::Arc::new(|arg, _| match arg {
                Value::Int(n) => Ok(Value::Int(n + 1)),
                _ => Err(RuntimeError::Message("expected Int".into())),
            }),
        }));
        let result = rt.apply(add_one, Value::Int(5)).unwrap();
        assert_eq!(as_int(&result), 6);
    }

    #[test]
    fn apply_constructor() {
        let rt = &mut Runtime::new();
        let result = rt
            .apply(
                Value::Constructor {
                    name: "Wrap".into(),
                    args: vec![],
                },
                Value::Int(1),
            )
            .unwrap();
        assert_eq!(ctor_name(&result), "Wrap");
        assert_eq!(as_int(&ctor_arg(&result, 0)), 1);
    }

    #[test]
    fn apply_non_function_errors() {
        let rt = &mut Runtime::new();
        let result = rt.apply(Value::Int(42), Value::Int(1));
        assert!(result.is_err());
    }

    #[test]
    fn cancel_token() {
        use aivi_native_runtime::Runtime;
        let rt = &mut Runtime::new();
        assert!(rt.check_cancelled().is_ok());
        rt.cancel.cancel();
        assert!(rt.check_cancelled().is_err());
    }

    #[test]
    fn uncancelable() {
        let rt = &mut Runtime::new();
        rt.cancel.cancel();
        // Within uncancelable, check_cancelled should return Ok
        rt.uncancelable(|rt| {
            assert!(rt.check_cancelled().is_ok());
        });
        // After uncancelable, should be cancelled again
        assert!(rt.check_cancelled().is_err());
    }

    #[test]
    fn generator_to_vec() {
        let rt = &mut Runtime::new();
        // Simple generator that yields [10, 20]
        let gen = Value::Closure(std::sync::Arc::new(ClosureValue {
            func: std::sync::Arc::new(|step, _rt| {
                Ok(Value::Closure(std::sync::Arc::new(ClosureValue {
                    func: std::sync::Arc::new(move |z, rt| {
                        let r1 = rt.call(step.clone(), vec![z, Value::Int(10)])?;
                        let r2 = rt.call(step.clone(), vec![r1, Value::Int(20)])?;
                        Ok(r2)
                    }),
                })))
            }),
        }));
        let result = rt.generator_to_vec(gen).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(as_int(&result[0]), 10);
        assert_eq!(as_int(&result[1]), 20);
    }

    #[test]
    fn rng_produces_different_values() {
        let rt = &mut Runtime::new();
        let a = rt.rng_next_u64();
        let b = rt.rng_next_u64();
        assert_ne!(a, b);
    }
}

// ===========================================================================
// KeyValue tests
// ===========================================================================

mod key_value {
    use super::*;
    use aivi_native_runtime::KeyValue;

    #[test]
    fn round_trip_unit() {
        let kv = KeyValue::try_from_value(&Value::Unit).unwrap();
        assert!(aivi_native_runtime::values_equal(
            &kv.to_value(),
            &Value::Unit
        ));
    }

    #[test]
    fn round_trip_int() {
        let kv = KeyValue::try_from_value(&Value::Int(42)).unwrap();
        assert!(aivi_native_runtime::values_equal(
            &kv.to_value(),
            &Value::Int(42)
        ));
    }

    #[test]
    fn round_trip_text() {
        let kv = KeyValue::try_from_value(&Value::Text("hello".into())).unwrap();
        assert!(aivi_native_runtime::values_equal(
            &kv.to_value(),
            &Value::Text("hello".into())
        ));
    }

    #[test]
    fn round_trip_float() {
        let kv = KeyValue::try_from_value(&Value::Float(1.5)).unwrap();
        assert!(aivi_native_runtime::values_equal(
            &kv.to_value(),
            &Value::Float(1.5)
        ));
    }

    #[test]
    fn round_trip_bool() {
        let kv = KeyValue::try_from_value(&Value::Bool(true)).unwrap();
        assert!(aivi_native_runtime::values_equal(
            &kv.to_value(),
            &Value::Bool(true)
        ));
    }

    #[test]
    fn non_key_returns_none() {
        let list = Value::List(std::sync::Arc::new(vec![]));
        assert!(KeyValue::try_from_value(&list).is_none());
    }

    #[test]
    fn ordering() {
        let a = KeyValue::Int(1);
        let b = KeyValue::Int(2);
        assert!(a < b);

        let t1 = KeyValue::Text("a".into());
        let t2 = KeyValue::Text("b".into());
        assert!(t1 < t2);
    }
}
