use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufRead, Write};

use serde::Serialize;

use crate::diagnostics::{Position, Span};
use crate::surface::{
    BlockItem, BlockKind, Def, DomainItem, Expr, ListItem, Module, ModuleItem, Pattern,
    RecordField, TextPart, TypeExpr, TypeSig,
};
use crate::AiviError;

#[derive(Debug, Clone, Serialize, Default)]
pub struct McpManifest {
    pub tools: Vec<McpTool>,
    pub resources: Vec<McpResource>,
}

#[derive(Debug, Clone, Serialize)]
pub struct McpTool {
    pub name: String,
    pub module: String,
    pub binding: String,
    pub input_schema: serde_json::Value,
    pub effectful: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct McpResource {
    pub name: String,
    pub module: String,
    pub binding: String,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct McpPolicy {
    pub allow_effectful_tools: bool,
}

fn has_decorator(decorators: &[crate::surface::Decorator], name: &str) -> bool {
    decorators
        .iter()
        .any(|decorator| decorator.name.name == name)
}

fn qualified_name(module: &str, binding: &str) -> String {
    format!("{module}.{binding}")
}

fn schema_unknown() -> serde_json::Value {
    serde_json::json!({})
}

fn schema_for_name(name: &str) -> serde_json::Value {
    match name {
        "Int" => serde_json::json!({ "type": "integer" }),
        "Float" => serde_json::json!({ "type": "number" }),
        "Bool" => serde_json::json!({ "type": "boolean" }),
        "Text" => serde_json::json!({ "type": "string" }),
        "Unit" => serde_json::json!({ "type": "null" }),
        _ => schema_unknown(),
    }
}

fn dummy_span() -> Span {
    Span {
        start: Position { line: 0, column: 0 },
        end: Position { line: 0, column: 0 },
    }
}

fn is_row_op(name: &str) -> bool {
    matches!(
        name,
        "Pick" | "Omit" | "Optional" | "Required" | "Rename" | "Defaulted"
    )
}

fn is_option_type(expr: &TypeExpr) -> Option<&TypeExpr> {
    match expr {
        TypeExpr::Apply { base, args, .. } => match base.as_ref() {
            TypeExpr::Name(name) if name.name == "Option" && args.len() == 1 => Some(&args[0]),
            _ => None,
        },
        _ => None,
    }
}

fn wrap_option_expr(expr: &TypeExpr) -> TypeExpr {
    if is_option_type(expr).is_some() {
        return expr.clone();
    }
    TypeExpr::Apply {
        base: Box::new(TypeExpr::Name(crate::surface::SpannedName {
            name: "Option".to_string(),
            span: dummy_span(),
        })),
        args: vec![expr.clone()],
        span: dummy_span(),
    }
}

fn unwrap_option_expr(expr: &TypeExpr) -> TypeExpr {
    if let Some(inner) = is_option_type(expr) {
        return inner.clone();
    }
    expr.clone()
}

fn row_fields_from_expr(expr: &TypeExpr) -> Vec<String> {
    match expr {
        TypeExpr::Tuple { items, .. } => items
            .iter()
            .filter_map(|item| match item {
                TypeExpr::Name(name) => Some(name.name.clone()),
                _ => None,
            })
            .collect(),
        TypeExpr::Name(name) => vec![name.name.clone()],
        _ => Vec::new(),
    }
}

fn row_fields_from_record_expr(expr: &TypeExpr) -> Vec<String> {
    match expr {
        TypeExpr::Record { fields, .. } => {
            fields.iter().map(|(name, _)| name.name.clone()).collect()
        }
        _ => Vec::new(),
    }
}

fn row_rename_map_from_expr(expr: &TypeExpr) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    if let TypeExpr::Record { fields, .. } = expr {
        for (name, ty) in fields {
            if let TypeExpr::Name(new_name) = ty {
                map.insert(name.name.clone(), new_name.name.clone());
            }
        }
    }
    map
}

fn record_map_from_type_expr(expr: &TypeExpr) -> Option<BTreeMap<String, TypeExpr>> {
    match expr {
        TypeExpr::Record { fields, .. } => Some(
            fields
                .iter()
                .map(|(name, ty)| (name.name.clone(), ty.clone()))
                .collect(),
        ),
        TypeExpr::Apply { base, args, .. } => {
            let TypeExpr::Name(base) = base.as_ref() else {
                return None;
            };
            if !is_row_op(&base.name) {
                return None;
            }
            row_op_record_map(&base.name, args)
        }
        _ => None,
    }
}

fn row_op_record_map(name: &str, args: &[TypeExpr]) -> Option<BTreeMap<String, TypeExpr>> {
    if args.len() != 2 {
        return None;
    }
    let (selector, source) = (&args[0], &args[1]);
    let source_map = record_map_from_type_expr(source)?;
    match name {
        "Pick" => {
            let mut out = BTreeMap::new();
            for field in row_fields_from_expr(selector) {
                if let Some(ty) = source_map.get(&field) {
                    out.insert(field, ty.clone());
                }
            }
            Some(out)
        }
        "Omit" => {
            let omit: BTreeSet<String> = row_fields_from_expr(selector).into_iter().collect();
            Some(
                source_map
                    .into_iter()
                    .filter(|(name, _)| !omit.contains(name))
                    .collect(),
            )
        }
        "Optional" => {
            let mut out = source_map;
            for field in row_fields_from_expr(selector) {
                if let Some(ty) = out.get_mut(&field) {
                    *ty = wrap_option_expr(ty);
                }
            }
            Some(out)
        }
        "Required" => {
            let mut out = source_map;
            for field in row_fields_from_expr(selector) {
                if let Some(ty) = out.get_mut(&field) {
                    *ty = unwrap_option_expr(ty);
                }
            }
            Some(out)
        }
        "Rename" => {
            let rename_map = row_rename_map_from_expr(selector);
            let mut out = BTreeMap::new();
            for (name, ty) in source_map {
                let new_name = rename_map.get(&name).cloned().unwrap_or(name);
                if out.contains_key(&new_name) {
                    continue;
                }
                out.insert(new_name, ty);
            }
            Some(out)
        }
        "Defaulted" => {
            let mut fields = row_fields_from_expr(selector);
            if fields.is_empty() {
                fields = row_fields_from_record_expr(selector);
            }
            let mut out = source_map;
            for field in fields {
                if let Some(ty) = out.get_mut(&field) {
                    *ty = wrap_option_expr(ty);
                }
            }
            Some(out)
        }
        _ => None,
    }
}

fn schema_for_record_map(fields: &BTreeMap<String, TypeExpr>) -> serde_json::Value {
    let mut props = serde_json::Map::new();
    let mut required = Vec::new();
    for (name, ty) in fields {
        props.insert(name.clone(), schema_for_type(ty));
        if is_option_type(ty).is_none() {
            required.push(serde_json::Value::String(name.clone()));
        }
    }
    serde_json::Value::Object(serde_json::Map::from_iter([
        (
            "type".to_string(),
            serde_json::Value::String("object".to_string()),
        ),
        ("properties".to_string(), serde_json::Value::Object(props)),
        ("required".to_string(), serde_json::Value::Array(required)),
        (
            "additionalProperties".to_string(),
            serde_json::Value::Bool(false),
        ),
    ]))
}

fn schema_for_type(expr: &TypeExpr) -> serde_json::Value {
    match expr {
        TypeExpr::Name(name) => schema_for_name(&name.name),
        TypeExpr::And { items, .. } => {
            let mut merged: BTreeMap<String, TypeExpr> = BTreeMap::new();
            for item in items {
                match item {
                    TypeExpr::Record { fields, .. } => {
                        for (name, ty) in fields {
                            merged
                                .entry(name.name.clone())
                                .or_insert_with(|| ty.clone());
                        }
                    }
                    TypeExpr::Apply { base, args, .. } => {
                        let TypeExpr::Name(base) = base.as_ref() else {
                            return schema_unknown();
                        };
                        if is_row_op(&base.name) {
                            if let Some(fields) = row_op_record_map(&base.name, args) {
                                for (name, ty) in fields {
                                    merged.entry(name).or_insert(ty);
                                }
                                continue;
                            }
                        }
                        return schema_unknown();
                    }
                    _ => return schema_unknown(),
                }
            }
            schema_for_record_map(&merged)
        }
        TypeExpr::Apply { base, args, .. } => {
            let TypeExpr::Name(base) = base.as_ref() else {
                return schema_unknown();
            };
            if is_row_op(&base.name) {
                if let Some(fields) = row_op_record_map(&base.name, args) {
                    return schema_for_record_map(&fields);
                }
                return schema_unknown();
            }
            match base.name.as_str() {
                "List" if args.len() == 1 => serde_json::json!({
                    "type": "array",
                    "items": schema_for_type(&args[0]),
                }),
                "Option" if args.len() == 1 => serde_json::json!({
                    "anyOf": [schema_for_type(&args[0]), { "type": "null" }],
                }),
                "Effect" if args.len() == 2 => schema_for_type(&args[1]),
                "Resource" if args.len() == 1 => schema_for_type(&args[0]),
                _ => schema_unknown(),
            }
        }
        TypeExpr::Record { fields, .. } => {
            let map: BTreeMap<String, TypeExpr> = fields
                .iter()
                .map(|(name, ty)| (name.name.clone(), ty.clone()))
                .collect();
            schema_for_record_map(&map)
        }
        TypeExpr::Tuple { items, .. } => {
            let prefix: Vec<serde_json::Value> = items.iter().map(schema_for_type).collect();
            serde_json::json!({
                "type": "array",
                "prefixItems": prefix,
                "items": false,
            })
        }
        TypeExpr::Func { .. } => serde_json::json!({ "type": "object" }),
        TypeExpr::Star { .. } | TypeExpr::Unknown { .. } => schema_unknown(),
    }
}

fn param_name(pattern: &Pattern, index: usize) -> String {
    match pattern {
        Pattern::Ident(name) => name.name.clone(),
        Pattern::SubjectIdent(name) => name.name.clone(),
        Pattern::At { name, .. } => name.name.clone(),
        _ => format!("arg{index}"),
    }
}
