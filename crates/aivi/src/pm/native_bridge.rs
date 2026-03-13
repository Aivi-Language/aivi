//! Crate-native bridge code generation.
//!
//! Scans surface modules for `@native "crate::path::fn"` declarations and generates
//! a Rust bridge module (`native_bridge.rs`) that:
//! 1. Wraps each crate function with AIVI `Value` marshalling
//! 2. Provides a registration function to install them as builtins

use crate::surface::{Decorator, Expr, Literal, Module, ModuleItem, RecordTypeField, TypeExpr};
use std::collections::{HashMap, HashSet};

/// A single crate-native binding extracted from the surface AST.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
        let type_aliases: HashMap<String, &TypeExpr> = module
            .items
            .iter()
            .filter_map(|item| match item {
                ModuleItem::TypeAlias(alias) => Some((alias.name.name.clone(), &alias.aliased)),
                _ => None,
            })
            .collect();
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
            let (param_types, return_type) = decompose_type(ty, &type_aliases);

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
fn native_crate_target(decorators: &[Decorator]) -> Option<&str> {
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

/// Decompose a TypeExpr into parameter types and the final return type.
/// Handles right-associative `->` chains: `A -> B -> C` is `Func([A], Func([B], C))`.
fn decompose_type(
    ty: &TypeExpr,
    aliases: &HashMap<String, &TypeExpr>,
) -> (Vec<AiviType>, AiviType) {
    fn collect_params(
        ty: &TypeExpr,
        aliases: &HashMap<String, &TypeExpr>,
        params: &mut Vec<AiviType>,
    ) -> AiviType {
        match ty {
            TypeExpr::Func {
                params: ps, result, ..
            } => {
                for p in ps {
                    params.push(type_expr_to_aivi_type(p, aliases));
                }
                collect_params(result, aliases, params)
            }
            _ => type_expr_to_aivi_type(ty, aliases),
        }
    }
    let mut params = Vec::new();
    let ret = collect_params(ty, aliases, &mut params);
    (params, ret)
}

/// Convert a surface TypeExpr to a simplified AiviType for bridge generation.
fn type_expr_to_aivi_type(ty: &TypeExpr, aliases: &HashMap<String, &TypeExpr>) -> AiviType {
    fn upsert_record_field(fields: &mut Vec<(String, AiviType)>, name: String, ty: AiviType) {
        if let Some((_, existing_ty)) = fields
            .iter_mut()
            .find(|(existing_name, _)| *existing_name == name)
        {
            *existing_ty = ty;
        } else {
            fields.push((name, ty));
        }
    }

    fn lower_type_expr(
        ty: &TypeExpr,
        aliases: &HashMap<String, &TypeExpr>,
        seen_aliases: &mut HashSet<String>,
    ) -> AiviType {
        match ty {
            TypeExpr::Name(name) => {
                if let Some(alias_ty) = aliases.get(&name.name) {
                    if seen_aliases.insert(name.name.clone()) {
                        let lowered = lower_type_expr(alias_ty, aliases, seen_aliases);
                        seen_aliases.remove(&name.name);
                        return lowered;
                    }
                }
                match name.name.as_str() {
                    "Text" => AiviType::Text,
                    "Int" => AiviType::Int,
                    "Float" => AiviType::Float,
                    "Bool" => AiviType::Bool,
                    "Unit" => AiviType::Unit,
                    other => AiviType::Opaque(other.to_string()),
                }
            }
            TypeExpr::Apply { base, args, .. } => {
                if let TypeExpr::Name(name) = base.as_ref() {
                    match (name.name.as_str(), args.as_slice()) {
                        ("Option", [inner]) => AiviType::Option(Box::new(lower_type_expr(
                            inner,
                            aliases,
                            seen_aliases,
                        ))),
                        ("List", [inner]) => {
                            AiviType::List(Box::new(lower_type_expr(inner, aliases, seen_aliases)))
                        }
                        ("Result", [err, ok]) => AiviType::Result(
                            Box::new(lower_type_expr(err, aliases, seen_aliases)),
                            Box::new(lower_type_expr(ok, aliases, seen_aliases)),
                        ),
                        _ => AiviType::Opaque(name.name.to_string()),
                    }
                } else {
                    AiviType::Opaque("unknown".to_string())
                }
            }
            TypeExpr::Record { fields, .. } => {
                let mut field_types = Vec::new();
                for field in fields {
                    match field {
                        RecordTypeField::Named { name, ty } => {
                            let lowered = lower_type_expr(ty, aliases, seen_aliases);
                            upsert_record_field(&mut field_types, name.name.clone(), lowered);
                        }
                        RecordTypeField::Spread { ty, .. } => {
                            if let AiviType::Record(spread_fields) =
                                lower_type_expr(ty, aliases, seen_aliases)
                            {
                                for (name, ty) in spread_fields {
                                    upsert_record_field(&mut field_types, name, ty);
                                }
                            } else {
                                return AiviType::Opaque("unknown".to_string());
                            }
                        }
                    }
                }
                AiviType::Record(field_types)
            }
            _ => AiviType::Opaque("unknown".to_string()),
        }
    }

    lower_type_expr(ty, aliases, &mut HashSet::new())
}

/// Generate the Rust source code for `native_bridge.rs`.
///
/// This file contains:
/// - Serde struct definitions for AIVI record types
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
    src.push_str("use aivi::CrateNativeValue;\n");

    // Check if any binding uses record types — emit serde import if so
    let needs_serde = bindings.iter().any(|b| {
        type_contains_record(&b.return_type) || b.param_types.iter().any(type_contains_record)
    });
    if needs_serde {
        src.push_str("use serde::{Deserialize, Serialize};\n");
    }
    src.push('\n');

    // Collect and deduplicate serde struct definitions
    let mut struct_defs = StructCollector::new();
    for binding in bindings {
        struct_defs.collect_from_type(&binding.return_type, "return");
        for (i, pt) in binding.param_types.iter().enumerate() {
            struct_defs.collect_from_type(pt, &format!("param{i}"));
        }
    }
    src.push_str(&struct_defs.emit());

    // Generate each bridge function
    for binding in bindings {
        generate_bridge_function(&mut src, binding, &struct_defs);
    }

    // Generate registration function
    src.push_str("/// Register all crate-native bridge functions as builtins.\n");
    src.push_str("pub fn register_crate_natives(reg: &mut aivi::CrateNativeRegistrar) {\n");
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

/// Check if a type tree contains any Record nodes.
fn type_contains_record(ty: &AiviType) -> bool {
    match ty {
        AiviType::Record(_) => true,
        AiviType::Option(inner) | AiviType::List(inner) => type_contains_record(inner),
        AiviType::Result(e, o) => type_contains_record(e) || type_contains_record(o),
        _ => false,
    }
}

type StructEntry = (String, Vec<(String, AiviType)>, &'static str);

/// Collector for serde struct definitions. Deduplicates by field signature.
struct StructCollector {
    /// Map from struct name → (fields, derives)
    structs: Vec<StructEntry>,
    /// Dedup key → struct name
    seen: std::collections::HashMap<String, String>,
    counter: usize,
}

impl StructCollector {
    fn new() -> Self {
        Self {
            structs: Vec::new(),
            seen: std::collections::HashMap::new(),
            counter: 0,
        }
    }

    /// Walk a type tree and collect struct definitions for all Record nodes.
    fn collect_from_type(&mut self, ty: &AiviType, _ctx: &str) {
        match ty {
            AiviType::Record(fields) => {
                self.ensure_struct(fields);
                for (_, ft) in fields {
                    self.collect_from_type(ft, "nested");
                }
            }
            AiviType::Option(inner) | AiviType::List(inner) => {
                self.collect_from_type(inner, "inner");
            }
            AiviType::Result(e, o) => {
                self.collect_from_type(e, "err");
                self.collect_from_type(o, "ok");
            }
            _ => {}
        }
    }

    /// Get or create a struct name for a given set of fields.
    fn ensure_struct(&mut self, fields: &[(String, AiviType)]) -> String {
        let key = fields_dedup_key(fields);
        if let Some(name) = self.seen.get(&key) {
            return name.clone();
        }
        let name = format!("__NativeStruct{}", self.counter);
        self.counter += 1;
        self.seen.insert(key, name.clone());
        self.structs
            .push((name.clone(), fields.to_vec(), "Deserialize, Serialize"));
        name
    }

    /// Emit all struct definitions as Rust source.
    fn emit(&self) -> String {
        let mut out = String::new();
        for (name, fields, derives) in &self.structs {
            out.push_str(&format!("#[derive({derives})]\n"));
            out.push_str(&format!("struct {name} {{\n"));
            for (field_name, field_ty) in fields {
                let rust_field = to_snake_case(field_name);
                if rust_field != *field_name {
                    out.push_str(&format!("    #[serde(rename = \"{field_name}\")]\n"));
                }
                let rust_type = aivi_type_to_rust_type(field_ty, self);
                out.push_str(&format!("    {rust_field}: {rust_type},\n"));
            }
            out.push_str("}\n\n");
        }
        out
    }

    /// Look up the struct name for a given set of record fields.
    fn struct_name_for(&self, fields: &[(String, AiviType)]) -> Option<String> {
        let key = fields_dedup_key(fields);
        self.seen.get(&key).cloned()
    }
}

/// Dedup key for a record: sorted field names + types.
fn fields_dedup_key(fields: &[(String, AiviType)]) -> String {
    fields
        .iter()
        .map(|(name, ty)| format!("{name}:{ty:?}"))
        .collect::<Vec<_>>()
        .join(",")
}

/// Convert camelCase to snake_case.
fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    for (i, ch) in s.chars().enumerate() {
        if ch.is_ascii_uppercase() {
            if i > 0 {
                result.push('_');
            }
            result.push(ch.to_ascii_lowercase());
        } else {
            result.push(ch);
        }
    }
    result
}

/// Map an AiviType to a Rust type string for struct field declarations.
fn aivi_type_to_rust_type(ty: &AiviType, collector: &StructCollector) -> String {
    match ty {
        AiviType::Text => "String".to_string(),
        AiviType::Int => "i64".to_string(),
        AiviType::Float => "f64".to_string(),
        AiviType::Bool => "bool".to_string(),
        AiviType::Unit => "()".to_string(),
        AiviType::Option(inner) => format!("Option<{}>", aivi_type_to_rust_type(inner, collector)),
        AiviType::List(inner) => format!("Vec<{}>", aivi_type_to_rust_type(inner, collector)),
        AiviType::Result(e, o) => {
            format!(
                "Result<{}, {}>",
                aivi_type_to_rust_type(o, collector),
                aivi_type_to_rust_type(e, collector)
            )
        }
        AiviType::Record(fields) => collector
            .struct_name_for(fields)
            .unwrap_or_else(|| "String".to_string()),
        AiviType::Opaque(name) => name.clone(),
    }
}

fn generate_bridge_function(
    src: &mut String,
    binding: &CrateNativeBinding,
    collector: &StructCollector,
) {
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
            src.push_str(&format!(
                "    let __raw_{var_name} = args.pop().unwrap();\n"
            ));
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

    // For generic serde functions, add turbofish type annotation when
    // the return type resolves to a generated struct
    let turbofish = compute_turbofish(&binding.return_type, collector);

    let return_conversion = rust_to_value_conversion(&binding.return_type, "__result", collector);

    src.push_str(&format!(
        "    let __result = {rust_path}{turbofish}({call_args});\n"
    ));
    src.push_str(&format!("    {return_conversion}\n"));
    src.push_str("}\n\n");
}

/// Compute turbofish type annotation for generic crate functions.
/// When the innermost "ok" type of a Result (or the return type itself)
/// is a generated struct, emit `::<StructName>`.
fn compute_turbofish(return_type: &AiviType, collector: &StructCollector) -> String {
    let target = match return_type {
        AiviType::Result(_, ok_ty) => ok_ty.as_ref(),
        other => other,
    };
    match target {
        AiviType::Record(fields) => {
            if let Some(name) = collector.struct_name_for(fields) {
                format!("::<{name}>")
            } else {
                String::new()
            }
        }
        _ => String::new(),
    }
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
#[allow(clippy::only_used_in_recursion)]
fn rust_to_value_conversion(ty: &AiviType, expr: &str, collector: &StructCollector) -> String {
    match ty {
        AiviType::Text => format!("Ok(CrateNativeValue::Text({expr}.to_string()))"),
        AiviType::Int => format!("Ok(CrateNativeValue::Int({expr}))"),
        AiviType::Float => format!("Ok(CrateNativeValue::Float({expr}))"),
        AiviType::Bool => format!("Ok(CrateNativeValue::Bool({expr}))"),
        AiviType::Unit => format!("{{ let _ = {expr}; Ok(CrateNativeValue::Unit) }}"),
        AiviType::Option(inner) => {
            let some_conv = rust_to_value_conversion(inner, "__inner", collector);
            format!(
                "match {expr} {{ Some(__inner) => {{ let __val = {{ {some_conv} }}?; Ok(CrateNativeValue::Constructor(\"Some\".to_string(), vec![__val])) }}, None => Ok(CrateNativeValue::Constructor(\"None\".to_string(), vec![])) }}"
            )
        }
        AiviType::Result(err_ty, ok_ty) => {
            let ok_conv = rust_to_value_conversion(ok_ty, "__ok_val", collector);
            let _ = err_ty;
            format!(
                "match {expr} {{ Ok(__ok_val) => {{ let __val = {{ {ok_conv} }}?; Ok(CrateNativeValue::Constructor(\"Ok\".to_string(), vec![__val])) }}, Err(__err) => Ok(CrateNativeValue::Constructor(\"Err\".to_string(), vec![CrateNativeValue::Text(format!(\"{{__err}}\"))])) }}"
            )
        }
        AiviType::List(inner) => {
            let item_conv = rust_to_value_conversion(inner, "__item", collector);
            format!(
                "{{ let __items: Result<Vec<CrateNativeValue>, String> = {expr}.into_iter().map(|__item| {{ {item_conv} }}).collect(); Ok(CrateNativeValue::List(__items?)) }}"
            )
        }
        AiviType::Record(fields) => {
            // Convert struct fields to CrateNativeValue::Record
            let mut field_lines = String::new();
            for (name, field_ty) in fields {
                let snake = to_snake_case(name);
                let field_conv =
                    rust_to_value_conversion(field_ty, &format!("{expr}.{snake}"), collector);
                field_lines.push_str(&format!(
                    "    __fields.push((\"{name}\".to_string(), {{ {field_conv} }}?));\n"
                ));
            }
            format!(
                "{{ let mut __fields: Vec<(String, CrateNativeValue)> = Vec::new();\n{field_lines}    Ok(CrateNativeValue::Record(__fields)) }}"
            )
        }
        AiviType::Opaque(name) => {
            format!("Ok(CrateNativeValue::Text(format!(\"{{:?}}\", {expr}))) // opaque type {name}")
        }
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
