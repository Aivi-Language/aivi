//! Pipeline QA tests for the `@native` decorator.
//!
//! Covers: P2_03–P2_10, NATIVE_01–NATIVE_03 from the pipeline test plan.

use std::path::Path;

use aivi::{parse_modules, Expr, ModuleItem};

fn diag_codes(diags: &[aivi::FileDiagnostic]) -> Vec<String> {
    let mut codes: Vec<String> = diags.iter().map(|d| d.diagnostic.code.clone()).collect();
    codes.sort();
    codes
}

// ---------------------------------------------------------------------------
// P2_03: @native rewrites body to target call (single param)
// ---------------------------------------------------------------------------
#[test]
fn native_single_param_rewrite() {
    let src = r#"
module native.singleRewrite

@native "gtk4.windowPresent"
windowPresent : Int -> Effect Text Unit
windowPresent = windowId => Unit
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diagnostics: {:?}",
        diag_codes(&diags)
    );
    let module = modules.first().expect("module");
    let def = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::Def(def) if def.name.name == "windowPresent" => Some(def),
            _ => None,
        })
        .expect("windowPresent def");

    // Body should be rewritten to: gtk4.windowPresent windowId
    match &def.expr {
        Expr::Call { func, args, .. } => {
            assert_eq!(args.len(), 1);
            assert!(matches!(args.first(), Some(Expr::Ident(name)) if name.name == "windowId"));
            match func.as_ref() {
                Expr::FieldAccess { base, field, .. } => {
                    assert_eq!(field.name, "windowPresent");
                    assert!(matches!(base.as_ref(), Expr::Ident(root) if root.name == "gtk4"));
                }
                other => panic!("expected native field access, got {other:?}"),
            }
        }
        other => panic!("expected native call, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// P2_04: @native without string argument emits E1511
// ---------------------------------------------------------------------------
#[test]
fn native_missing_arg_emits_e1511() {
    let src = r#"
module native.noarg

@native
x : Int -> Int
x = n => n
"#;
    let (_, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diag_codes(&diags).contains(&"E1511".to_string()),
        "expected E1511, got: {:?}",
        diag_codes(&diags)
    );
}

// ---------------------------------------------------------------------------
// P2_05: @native without type signature emits E1526
// ---------------------------------------------------------------------------
#[test]
fn native_missing_type_sig_emits_e1526() {
    let src = r#"
module native.nosig

@native "gtk4.appRun"
appRun = appId => Unit
"#;
    let (_, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diag_codes(&diags).contains(&"E1526".to_string()),
        "expected E1526, got: {:?}",
        diag_codes(&diags)
    );
    assert!(
        diags.iter().any(|d| d
            .diagnostic
            .message
            .contains("requires an explicit type signature")),
        "expected 'requires an explicit type signature' message"
    );
}

// ---------------------------------------------------------------------------
// P2_06: @native in a domain block def is rejected (non-top-level)
// ---------------------------------------------------------------------------
#[test]
fn native_in_domain_rejected() {
    // Put @native on the operator definition itself (not the type sig)
    let src = r#"
module native.domain

domain Vec over Int = {
  (+) : Int -> Int -> Int
  @native "rt.vecAdd"
  (+) = a b => a + b
}
"#;
    let (_, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diag_codes(&diags).contains(&"E1526".to_string()),
        "expected E1526 for non-top-level @native in domain, got: {:?}",
        diag_codes(&diags)
    );
    assert!(
        diags
            .iter()
            .any(|d| d.diagnostic.message.contains("only supported on top-level")),
        "expected 'only supported on top-level' message"
    );
}

// ---------------------------------------------------------------------------
// P2_07: @native with invalid target path emits E1526
// ---------------------------------------------------------------------------
#[test]
fn native_bad_target_path() {
    let src = r#"
module native.badpath

@native "not a valid path!"
bad : Int -> Int
bad = n => n
"#;
    let (_, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diag_codes(&diags).contains(&"E1526".to_string()),
        "expected E1526 for bad target, got: {:?}",
        diag_codes(&diags)
    );
    assert!(
        diags
            .iter()
            .any(|d| d.diagnostic.message.contains("dotted identifier path")),
        "expected 'dotted identifier path' message"
    );
}

// ---------------------------------------------------------------------------
// P2_08: @native multi-parameter function rewrites all args
// ---------------------------------------------------------------------------
#[test]
fn native_multi_param_rewrite() {
    let src = r#"
module native.multiarg

@native "math.add"
add : Int -> Int -> Int
add = a b => 0
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diagnostics: {:?}",
        diag_codes(&diags)
    );

    let module = modules.first().expect("module");
    let def = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::Def(def) if def.name.name == "add" => Some(def),
            _ => None,
        })
        .expect("add def");

    // Body should be: math.add a b
    match &def.expr {
        Expr::Call { func, args, .. } => {
            assert_eq!(
                args.len(),
                2,
                "expected 2 args forwarded, got {}",
                args.len()
            );
            assert!(matches!(&args[0], Expr::Ident(n) if n.name == "a"));
            assert!(matches!(&args[1], Expr::Ident(n) if n.name == "b"));
            match func.as_ref() {
                Expr::FieldAccess { base, field, .. } => {
                    assert_eq!(field.name, "add");
                    assert!(matches!(base.as_ref(), Expr::Ident(root) if root.name == "math"));
                }
                other => panic!("expected native field access, got {other:?}"),
            }
        }
        other => panic!("expected native call with 2 args, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// P2_09: @native zero-parameter produces bare target expression
// ---------------------------------------------------------------------------
#[test]
fn native_zero_param_bare_target() {
    let src = r#"
module native.zeroparam

@native "config.defaultTimeout"
defaultTimeout : Int
defaultTimeout = 0
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diagnostics: {:?}",
        diag_codes(&diags)
    );

    let module = modules.first().expect("module");
    let def = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::Def(def) if def.name.name == "defaultTimeout" => Some(def),
            _ => None,
        })
        .expect("defaultTimeout def");

    // Body should be a FieldAccess (config.defaultTimeout), not a Call
    match &def.expr {
        Expr::FieldAccess { base, field, .. } => {
            assert_eq!(field.name, "defaultTimeout");
            assert!(matches!(base.as_ref(), Expr::Ident(root) if root.name == "config"));
        }
        Expr::Call { .. } => panic!("zero-param native should produce FieldAccess, not Call"),
        other => panic!("expected native field access, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// P2_10: @native on type signature propagates to matching def
// ---------------------------------------------------------------------------
#[test]
fn native_on_typesig_propagates_to_def() {
    let src = r#"
module native.sigprop

@native "system.exit"
exit : Int -> Effect Text Unit

exit = code => Unit
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diagnostics: {:?}",
        diag_codes(&diags)
    );

    let module = modules.first().expect("module");
    let def = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::Def(def) if def.name.name == "exit" => Some(def),
            _ => None,
        })
        .expect("exit def");

    // Body should be rewritten to: system.exit code
    match &def.expr {
        Expr::Call { func, args, .. } => {
            assert_eq!(args.len(), 1);
            assert!(matches!(&args[0], Expr::Ident(n) if n.name == "code"));
            match func.as_ref() {
                Expr::FieldAccess { base, field, .. } => {
                    assert_eq!(field.name, "exit");
                    assert!(matches!(base.as_ref(), Expr::Ident(root) if root.name == "system"));
                }
                other => panic!("expected native field access, got {other:?}"),
            }
        }
        other => panic!("expected native call, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// NATIVE_01: @native with non-string argument (integer) emits E1510
// ---------------------------------------------------------------------------
#[test]
fn native_integer_arg_emits_e1510() {
    let src = r#"
module native.intarg

@native 42
x : Int -> Int
x = n => n
"#;
    let (_, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diag_codes(&diags).contains(&"E1510".to_string()),
        "expected E1510, got: {:?}",
        diag_codes(&diags)
    );
    assert!(
        diags
            .iter()
            .any(|d| d.diagnostic.message.contains("string literal")),
        "expected 'string literal' message"
    );
}

// ---------------------------------------------------------------------------
// NATIVE_02: @native with destructuring parameter is rejected
// ---------------------------------------------------------------------------
#[test]
fn native_destructure_param_rejected() {
    let src = r#"
module native.destructure

@native "mod.fn"
fn : { x: Int } -> Int
fn = { x } => x
"#;
    let (_, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diag_codes(&diags).contains(&"E1526".to_string()),
        "expected E1526 for destructured param, got: {:?}",
        diag_codes(&diags)
    );
    assert!(
        diags.iter().any(|d| d
            .diagnostic
            .message
            .contains("only supports identifier parameters")),
        "expected 'only supports identifier parameters' message"
    );
}

// ---------------------------------------------------------------------------
// NATIVE_03: @native with deeply nested dotted path (3+ segments) works
// ---------------------------------------------------------------------------
#[test]
fn native_deep_dotted_path() {
    let src = r#"
module native.deeppath

@native "system.io.file.readAll"
readAll : Text -> Effect Text Text
readAll = path => ""
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diagnostics: {:?}",
        diag_codes(&diags)
    );

    let module = modules.first().expect("module");
    let def = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::Def(def) if def.name.name == "readAll" => Some(def),
            _ => None,
        })
        .expect("readAll def");

    // Body should be: system.io.file.readAll path
    match &def.expr {
        Expr::Call { func, args, .. } => {
            assert_eq!(args.len(), 1);
            assert!(matches!(&args[0], Expr::Ident(n) if n.name == "path"));
            // Verify nested field access chain: system.io.file.readAll
            let mut node = func.as_ref();
            let mut fields = Vec::new();
            loop {
                match node {
                    Expr::FieldAccess { base, field, .. } => {
                        fields.push(field.name.clone());
                        node = base.as_ref();
                    }
                    Expr::Ident(name) => {
                        fields.push(name.name.clone());
                        break;
                    }
                    other => panic!("expected nested field access, got {other:?}"),
                }
            }
            fields.reverse();
            assert_eq!(
                fields,
                vec!["system", "io", "file", "readAll"],
                "expected deeply nested path"
            );
        }
        other => panic!("expected native call, got {other:?}"),
    }
}

// ===========================================================================
// Crate-native tests (@native "crate::path::fn")
// ===========================================================================

// ---------------------------------------------------------------------------
// CRATE_NATIVE_01: @native with :: path auto-generates def
// ---------------------------------------------------------------------------
#[test]
fn crate_native_auto_generates_def() {
    let src = r#"
module native.crateBasic

@native "quick_xml::de::from_str"
parseXml : Text -> Text
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diagnostics: {:?}",
        diag_codes(&diags)
    );

    let module = modules.first().expect("module");

    // Should have auto-generated a Def for parseXml
    let def = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::Def(def) if def.name.name == "parseXml" => Some(def),
            _ => None,
        })
        .expect("parseXml def should be auto-generated");

    // The body should be an Ident referencing the global crate-native name
    match &def.expr {
        Expr::Call { func, args, .. } => {
            assert_eq!(args.len(), 1, "expected 1 arg forwarded");
            match func.as_ref() {
                Expr::Ident(name) => {
                    assert!(
                        name.name.starts_with("__crate_native__"),
                        "expected crate-native global name, got: {}",
                        name.name
                    );
                }
                other => panic!("expected Ident for crate-native, got {other:?}"),
            }
        }
        Expr::Ident(name) => {
            assert!(
                name.name.starts_with("__crate_native__"),
                "expected crate-native global name, got: {}",
                name.name
            );
        }
        other => panic!("expected crate-native Ident or Call, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// CRATE_NATIVE_02: @native with :: path — multi-param generates correct arity
// ---------------------------------------------------------------------------
#[test]
fn crate_native_multi_param() {
    let src = r#"
module native.crateMultiParam

@native "my_crate::add"
add : Int -> Int -> Int
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diagnostics: {:?}",
        diag_codes(&diags)
    );

    let module = modules.first().expect("module");
    let def = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::Def(def) if def.name.name == "add" => Some(def),
            _ => None,
        })
        .expect("add def");

    // Should have 2 args forwarded
    match &def.expr {
        Expr::Call { args, .. } => {
            assert_eq!(args.len(), 2, "expected 2 args forwarded for binary function");
        }
        other => panic!("expected Call with 2 args, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// CRATE_NATIVE_03: @native with :: path — zero-param is bare ident
// ---------------------------------------------------------------------------
#[test]
fn crate_native_zero_param() {
    let src = r#"
module native.crateZeroParam

@native "my_crate::get_version"
getVersion : Text
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diagnostics: {:?}",
        diag_codes(&diags)
    );

    let module = modules.first().expect("module");
    let def = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::Def(def) if def.name.name == "getVersion" => Some(def),
            _ => None,
        })
        .expect("getVersion def");

    // Zero-param: body should be bare Ident, not a Call
    match &def.expr {
        Expr::Ident(name) => {
            assert!(
                name.name.starts_with("__crate_native__"),
                "expected crate-native global name, got: {}",
                name.name
            );
        }
        other => panic!("expected bare Ident for zero-param crate-native, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// CRATE_NATIVE_04: @native :: path with body def does NOT auto-generate
// ---------------------------------------------------------------------------
#[test]
fn crate_native_with_body_uses_existing_def() {
    let src = r#"
module native.crateWithBody

@native "my_crate::add"
add : Int -> Int -> Int
add = a b => a
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diagnostics: {:?}",
        diag_codes(&diags)
    );

    let module = modules.first().expect("module");
    // Should have exactly one Def (not two — no auto-gen when body exists)
    let defs: Vec<_> = module
        .items
        .iter()
        .filter_map(|item| match item {
            ModuleItem::Def(def) if def.name.name == "add" => Some(def),
            _ => None,
        })
        .collect();
    assert_eq!(defs.len(), 1, "expected exactly one def for add");
}

// ---------------------------------------------------------------------------
// CRATE_NATIVE_05: bridge codegen produces valid source
// ---------------------------------------------------------------------------
#[test]
fn crate_native_bridge_codegen() {
    let bindings = vec![aivi::native_bridge::CrateNativeBinding {
        aivi_name: "parseXml".to_string(),
        rust_path: "quick_xml::de::from_str".to_string(),
        crate_name: "quick_xml".to_string(),
        global_name: "__crate_native__quick_xml__de__from_str".to_string(),
        param_types: vec![aivi::native_bridge::AiviType::Text],
        return_type: aivi::native_bridge::AiviType::Result(
            Box::new(aivi::native_bridge::AiviType::Text),
            Box::new(aivi::native_bridge::AiviType::Text),
        ),
    }];

    let source = aivi::native_bridge::generate_native_bridge_source(&bindings);
    assert!(source.contains("fn __crate_native__quick_xml__de__from_str"));
    assert!(source.contains("quick_xml::de::from_str"));
    assert!(source.contains("register_crate_natives"));
    assert!(source.contains("CrateNativeValue"));
    // Should NOT reference internal types
    assert!(!source.contains("RuntimeError"));
    assert!(!source.contains("use aivi::runtime"));
}

// ---------------------------------------------------------------------------
// CRATE_NATIVE_06: bridge codegen with zero bindings produces no-op
// ---------------------------------------------------------------------------
#[test]
fn crate_native_bridge_codegen_empty() {
    let source = aivi::native_bridge::generate_native_bridge_source(&[]);
    assert!(source.contains("register_crate_natives"));
    assert!(source.contains("no bindings"));
}

// ---------------------------------------------------------------------------
// CRATE_NATIVE_07: collect_crate_natives extracts from surface modules
// ---------------------------------------------------------------------------
#[test]
fn crate_native_collect_from_modules() {
    let src = r#"
module native.crateCollect

@native "my_crate::greet"
greet : Text -> Text
"#;
    let (modules, _diags) = parse_modules(Path::new("test.aivi"), src);
    let bindings = aivi::native_bridge::collect_crate_natives(&modules);
    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings[0].aivi_name, "greet");
    assert_eq!(bindings[0].crate_name, "my_crate");
    assert_eq!(bindings[0].rust_path, "my_crate::greet");
    assert!(bindings[0].global_name.starts_with("__crate_native__"));
}

// ---------------------------------------------------------------------------
// CRATE_NATIVE_08: dot-path natives are NOT collected as crate-natives
// ---------------------------------------------------------------------------
#[test]
fn dot_path_not_collected_as_crate_native() {
    let src = r#"
module native.dotpath

@native "gtk4.windowPresent"
windowPresent : Int -> Effect Text Unit
windowPresent = windowId => Unit
"#;
    let (modules, _diags) = parse_modules(Path::new("test.aivi"), src);
    let bindings = aivi::native_bridge::collect_crate_natives(&modules);
    assert!(
        bindings.is_empty(),
        "dot-path natives should NOT be collected as crate-natives"
    );
}

// ---------------------------------------------------------------------------
// CRATE_NATIVE_09: validate_crate_deps detects missing deps
// ---------------------------------------------------------------------------
#[test]
fn crate_native_validate_missing_dep() {
    let dir = tempfile::tempdir().unwrap();
    let cargo_toml = dir.path().join("Cargo.toml");
    std::fs::write(
        &cargo_toml,
        r#"
[package]
name = "test"
version = "0.1.0"

[dependencies]
"#,
    )
    .unwrap();

    let bindings = vec![aivi::native_bridge::CrateNativeBinding {
        aivi_name: "parseXml".to_string(),
        rust_path: "quick_xml::de::from_str".to_string(),
        crate_name: "quick_xml".to_string(),
        global_name: "__crate_native__quick_xml__de__from_str".to_string(),
        param_types: vec![aivi::native_bridge::AiviType::Text],
        return_type: aivi::native_bridge::AiviType::Text,
    }];

    let result = aivi::native_bridge::validate_crate_deps(&cargo_toml, &bindings);
    assert!(result.is_err(), "should fail for missing quick_xml dep");
    let errors = result.unwrap_err();
    assert!(errors[0].contains("quick_xml"));
}

// ---------------------------------------------------------------------------
// CRATE_NATIVE_10: validate_crate_deps passes when dep exists
// ---------------------------------------------------------------------------
#[test]
fn crate_native_validate_dep_exists() {
    let dir = tempfile::tempdir().unwrap();
    let cargo_toml = dir.path().join("Cargo.toml");
    std::fs::write(
        &cargo_toml,
        r#"
[package]
name = "test"
version = "0.1.0"

[dependencies]
quick_xml = "0.31"
"#,
    )
    .unwrap();

    let bindings = vec![aivi::native_bridge::CrateNativeBinding {
        aivi_name: "parseXml".to_string(),
        rust_path: "quick_xml::de::from_str".to_string(),
        crate_name: "quick_xml".to_string(),
        global_name: "__crate_native__quick_xml__de__from_str".to_string(),
        param_types: vec![aivi::native_bridge::AiviType::Text],
        return_type: aivi::native_bridge::AiviType::Text,
    }];

    let result = aivi::native_bridge::validate_crate_deps(&cargo_toml, &bindings);
    assert!(result.is_ok(), "should pass when dep exists: {:?}", result);
}
