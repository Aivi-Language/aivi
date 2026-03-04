//! Crate-native bridge code generation.
//!
//! Scans surface modules for `@native "crate::path::fn"` declarations and generates
//! a Rust bridge module (`native_bridge.rs`) that:
//! 1. Wraps each crate function with AIVI `Value` marshalling
//! 2. Provides a registration function to install them as builtins

use crate::surface::{Decorator, Expr, Literal, Module, ModuleItem, TypeExpr};
use std::collections::HashMap;

/// A single crate-native binding extracted from the surface AST.
#[derive(Debug, Clone)]
pub struct CrateNativeBinding {
    /// The AIVI function name (e.g., `parseXml`)
    pub aivi_name: String,
    /// The full Rust path (e.g., `quick_xml::de::from_str`)
    pub rust_path: String,
    /// The crate name (first segment of the `::` path, e.g., `quick_xml`)
    pub crate_name: String,
    /// The global name used in the runtime (e.g., `__crate_native__quick_xml__de__from_str`)
    pub global_name: String,
    /// Parameter types (AIVI type expressions)
    pub param_types: Vec<AiviType>,
    /// Return type
    pub return_type: AiviType,
}

/// Simplified representation of an AIVI type for bridge generation.
#[derive(Debug, Clone)]
pub enum AiviType {
    Text,
    Int,
    Float,
    Bool,
    Unit,
    Option(Box<AiviType>),
    List(Box<AiviType>),
    Result(Box<AiviType>, Box<AiviType>),
    Record(Vec<(String, AiviType)>),
    /// A type we can't auto-bridge (will use ToString / from string)
    Opaque(String),
}

/// Extract all crate-native bindings from a set of surface modules.
pub fn collect_crate_natives(modules: &[Module]) -> Vec<CrateNativeBinding> {
    let mut bindings = Vec::new();

    for module in modules {
        // Collect type signatures with @native crate paths
        let mut sig_map: HashMap<String, (&str, &TypeExpr)> = HashMap::new();
        for item in &module.items {
            if let ModuleItem::TypeSig(sig) = item {
                if let Some(path) = native_crate_target(&sig.decorators) {
                    sig_map.insert(sig.name.name.clone(), (path, &sig.ty));
                }
            }
        }

        for (aivi_name, &(rust_path, ty)) in &sig_map {
            let crate_name: String = rust_path
                .split("::")
                .next()
                .unwrap_or(rust_path)
                .to_string();
            let global_name = crate_native_global_name(rust_path);
            let (param_types, return_type) = decompose_type(ty);

            bindings.push(CrateNativeBinding {
                aivi_name: aivi_name.clone(),
                rust_path: rust_path.to_string(),
                crate_name,
                global_name,
                param_types,
                return_type,
            });
        }
    }

    bindings
}

/// Extract the `@native` path from decorators, but only if it's a crate-native (contains `::`)
fn native_crate_target<'a>(decorators: &'a [Decorator]) -> Option<&'a str> {
    decorators.iter().find_map(|dec| {
        if dec.name.name != "native" {
            return None;
        }
        match dec.arg.as_ref() {
            Some(Expr::Literal(Literal::String { text, .. })) if text.contains("::") => {
                Some(text.as_str())
            }
            _ => None,
        }
    })
}

fn crate_native_global_name(path: &str) -> String {
    let sanitized = path.replace("::", "__").replace('-', "_");
    format!("__crate_native__{sanitized}")
}

/// Decompose a TypeExpr into parameter types and return type.
fn decompose_type(ty: &TypeExpr) -> (Vec<AiviType>, AiviType) {
    match ty {
        TypeExpr::Func { params, result, .. } => {
            let param_types: Vec<AiviType> = params.iter().map(type_expr_to_aivi_type).collect();
            let return_type = type_expr_to_aivi_type(result);
            (param_types, return_type)
        }
        _ => (Vec::new(), type_expr_to_aivi_type(ty)),
    }
}

/// Convert a surface TypeExpr to a simplified AiviType for bridge generation.
fn type_expr_to_aivi_type(ty: &TypeExpr) -> AiviType {
    match ty {
        TypeExpr::Name(name) => match name.name.as_str() {
            "Text" => AiviType::Text,
            "Int" => AiviType::Int,
            "Float" => AiviType::Float,
            "Bool" => AiviType::Bool,
            "Unit" => AiviType::Unit,
            other => AiviType::Opaque(other.to_string()),
        },
        TypeExpr::Apply { base, args, .. } => {
            if let TypeExpr::Name(name) = base.as_ref() {
                match (name.name.as_str(), args.as_slice()) {
                    ("Option", [inner]) => {
                        AiviType::Option(Box::new(type_expr_to_aivi_type(inner)))
                    }
                    ("List", [inner]) => AiviType::List(Box::new(type_expr_to_aivi_type(inner))),
                    ("Result", [err, ok]) => AiviType::Result(
                        Box::new(type_expr_to_aivi_type(err)),
                        Box::new(type_expr_to_aivi_type(ok)),
                    ),
                    _ => AiviType::Opaque(format!("{}", name.name)),
                }
            } else {
                AiviType::Opaque("unknown".to_string())
            }
        }
        TypeExpr::Record { fields, .. } => {
            let field_types: Vec<(String, AiviType)> = fields
                .iter()
                .map(|(name, ty)| (name.name.clone(), type_expr_to_aivi_type(ty)))
                .collect();
            AiviType::Record(field_types)
        }
        _ => AiviType::Opaque("unknown".to_string()),
    }
}

/// Generate the Rust source code for `native_bridge.rs`.
///
/// This file contains:
/// - Bridge functions that marshal CrateNativeValue <-> Rust types
/// - A `register_crate_natives` function that installs them all
pub fn generate_native_bridge_source(bindings: &[CrateNativeBinding]) -> String {
    if bindings.is_empty() {
        return String::from(
            "//! Auto-generated crate-native bridge (no bindings).\n\
             pub fn register_crate_natives(_reg: &mut aivi::CrateNativeRegistrar) {}\n",
        );
    }

    let mut src = String::new();
    src.push_str("//! Auto-generated crate-native bridge.\n");
    src.push_str("//! Generated by `aivi build` — do not edit.\n\n");
    src.push_str("use aivi::CrateNativeValue;\n\n");

    // Generate each bridge function
    for binding in bindings {
        generate_bridge_function(&mut src, binding);
    }

    // Generate registration function
    src.push_str("/// Register all crate-native bridge functions as builtins.\n");
    src.push_str(
        "pub fn register_crate_natives(reg: &mut aivi::CrateNativeRegistrar) {\n",
    );
    for binding in bindings {
        let fn_name = &binding.global_name;
        let arity = binding.param_types.len();
        src.push_str(&format!(
            "    reg.add(\"{fn_name}\", {arity}, {fn_name});\n"
        ));
    }
    src.push_str("}\n");

    src
}

fn generate_bridge_function(src: &mut String, binding: &CrateNativeBinding) {
    let fn_name = &binding.global_name;
    let rust_path = &binding.rust_path;
    let arity = binding.param_types.len();

    src.push_str(&format!(
        "fn {fn_name}(mut args: Vec<CrateNativeValue>) -> Result<CrateNativeValue, String> {{\n"
    ));

    // Extract args (they come in reverse order from the runtime stack)
    if arity > 0 {
        src.push_str(&format!(
            "    if args.len() != {arity} {{\n        return Err(format!(\"expected {arity} arguments, got {{}}\", args.len()));\n    }}\n"
        ));
        // Pop in reverse to get correct order
        for i in (0..arity).rev() {
            let var_name = format!("arg{i}");
            let conversion = value_to_rust_conversion(&binding.param_types[i], &var_name);
            src.push_str(&format!("    let __raw_{var_name} = args.pop().unwrap();\n"));
            src.push_str(&format!("    {conversion}\n"));
        }
    }

    // Call the crate function with appropriate argument passing
    let call_args: String = (0..arity)
        .map(|i| {
            let arg_name = format!("arg{i}");
            match &binding.param_types[i] {
                AiviType::Text => format!("&{arg_name}"),
                _ => arg_name,
            }
        })
        .collect::<Vec<_>>()
        .join(", ");

    let return_conversion = rust_to_value_conversion(&binding.return_type, "__result");

    src.push_str(&format!("    let __result = {rust_path}({call_args});\n"));
    src.push_str(&format!("    {return_conversion}\n"));
    src.push_str("}\n\n");
}

/// Generate code to convert a `CrateNativeValue` to a Rust type.
fn value_to_rust_conversion(ty: &AiviType, var_name: &str) -> String {
    match ty {
        AiviType::Text => format!(
            "let {var_name} = match __raw_{var_name} {{ CrateNativeValue::Text(s) => s, other => return Err(format!(\"expected Text, got {{:?}}\", other)) }};"
        ),
        AiviType::Int => format!(
            "let {var_name} = match __raw_{var_name} {{ CrateNativeValue::Int(n) => n, other => return Err(format!(\"expected Int, got {{:?}}\", other)) }};"
        ),
        AiviType::Float => format!(
            "let {var_name} = match __raw_{var_name} {{ CrateNativeValue::Float(f) => f, other => return Err(format!(\"expected Float, got {{:?}}\", other)) }};"
        ),
        AiviType::Bool => format!(
            "let {var_name} = match __raw_{var_name} {{ CrateNativeValue::Bool(b) => b, other => return Err(format!(\"expected Bool, got {{:?}}\", other)) }};"
        ),
        _ => format!(
            "let {var_name} = __raw_{var_name}; // opaque pass-through"
        ),
    }
}

/// Generate code to convert a Rust value back to a `CrateNativeValue`.
fn rust_to_value_conversion(ty: &AiviType, expr: &str) -> String {
    match ty {
        AiviType::Text => format!("Ok(CrateNativeValue::Text({expr}.to_string()))"),
        AiviType::Int => format!("Ok(CrateNativeValue::Int({expr}))"),
        AiviType::Float => format!("Ok(CrateNativeValue::Float({expr}))"),
        AiviType::Bool => format!("Ok(CrateNativeValue::Bool({expr}))"),
        AiviType::Unit => format!("{{ let _ = {expr}; Ok(CrateNativeValue::Unit) }}"),
        AiviType::Option(inner) => {
            let some_conv = rust_to_value_conversion(inner, "__inner");
            format!(
                "match {expr} {{ Some(__inner) => {{ let __val = {{ {some_conv} }}?; Ok(CrateNativeValue::Constructor(\"Some\".to_string(), vec![__val])) }}, None => Ok(CrateNativeValue::Constructor(\"None\".to_string(), vec![])) }}"
            )
        }
        AiviType::Result(err_ty, ok_ty) => {
            let ok_conv = rust_to_value_conversion(ok_ty, "__ok_val");
            let _ = err_ty;
            format!(
                "match {expr} {{ Ok(__ok_val) => {{ let __val = {{ {ok_conv} }}?; Ok(CrateNativeValue::Constructor(\"Ok\".to_string(), vec![__val])) }}, Err(__err) => Ok(CrateNativeValue::Constructor(\"Err\".to_string(), vec![CrateNativeValue::Text(format!(\"{{__err}}\"))])) }}"
            )
        }
        AiviType::List(inner) => {
            let item_conv = rust_to_value_conversion(inner, "__item");
            format!(
                "{{ let __items: Result<Vec<CrateNativeValue>, String> = {expr}.into_iter().map(|__item| {{ {item_conv} }}).collect(); Ok(CrateNativeValue::List(__items?)) }}"
            )
        }
        AiviType::Record(fields) => {
            let mut field_lines = String::new();
            for (name, field_ty) in fields {
                let field_conv =
                    rust_to_value_conversion(field_ty, &format!("{expr}.{name}"));
                field_lines.push_str(&format!(
                    "    __fields.push((\"{name}\".to_string(), {{ {field_conv} }}?));\n"
                ));
            }
            format!(
                "{{ let mut __fields: Vec<(String, CrateNativeValue)> = Vec::new();\n{field_lines}    Ok(CrateNativeValue::Record(__fields)) }}"
            )
        }
        AiviType::Opaque(name) => format!(
            "Ok(CrateNativeValue::Text(format!(\"{{:?}}\", {expr}))) // opaque type {name}"
        ),
    }
}

/// Validate that all referenced crates exist in Cargo.toml dependencies.
pub fn validate_crate_deps(
    cargo_toml_path: &std::path::Path,
    bindings: &[CrateNativeBinding],
) -> Result<(), Vec<String>> {
    if bindings.is_empty() {
        return Ok(());
    }

    let cargo_text = match std::fs::read_to_string(cargo_toml_path) {
        Ok(text) => text,
        Err(e) => return Err(vec![format!("failed to read Cargo.toml: {e}")]),
    };

    let cargo_doc: toml_edit::DocumentMut = match cargo_text.parse() {
        Ok(doc) => doc,
        Err(e) => return Err(vec![format!("failed to parse Cargo.toml: {e}")]),
    };

    let deps = cargo_doc
        .as_table()
        .get("dependencies")
        .and_then(|v| v.as_table());

    let mut missing = Vec::new();
    for binding in bindings {
        // Check both the exact crate name and the hyphenated variant
        let crate_name = &binding.crate_name;
        let hyphenated = crate_name.replace('_', "-");
        let found = if let Some(deps_table) = deps {
            deps_table.contains_key(crate_name) || deps_table.contains_key(&hyphenated)
        } else {
            false
        };
        if !found {
            missing.push(format!(
                "error[E1528]: crate `{crate_name}` referenced by @native binding `{}` but not found in Cargo.toml [dependencies]",
                binding.aivi_name
            ));
        }
    }

    if missing.is_empty() {
        Ok(())
    } else {
        Err(missing)
    }
}
