use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufRead, Write};

use serde::Serialize;

use crate::surface::{
    BlockItem, BlockKind, Def, DomainItem, Expr, ListItem, MatchArm, Module, ModuleItem, Pattern,
    RecordField, TypeExpr, TypeSig,
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

#[derive(Debug, Clone, Copy)]
pub struct McpPolicy {
    pub allow_effectful_tools: bool,
}

impl Default for McpPolicy {
    fn default() -> Self {
        Self {
            allow_effectful_tools: false,
        }
    }
}

fn has_decorator(decorators: &[crate::surface::SpannedName], name: &str) -> bool {
    decorators.iter().any(|decorator| decorator.name == name)
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

fn schema_for_type(expr: &TypeExpr) -> serde_json::Value {
    match expr {
        TypeExpr::Name(name) => schema_for_name(&name.name),
        TypeExpr::Apply { base, args, .. } => {
            let TypeExpr::Name(base) = base.as_ref() else {
                return schema_unknown();
            };
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
            let mut props = serde_json::Map::new();
            let mut required = Vec::new();
            for (name, ty) in fields {
                props.insert(name.name.clone(), schema_for_type(ty));
                required.push(serde_json::Value::String(name.name.clone()));
            }
            serde_json::Value::Object(serde_json::Map::from_iter([
                ("type".to_string(), serde_json::Value::String("object".to_string())),
                ("properties".to_string(), serde_json::Value::Object(props)),
                ("required".to_string(), serde_json::Value::Array(required)),
                (
                    "additionalProperties".to_string(),
                    serde_json::Value::Bool(false),
                ),
            ]))
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
        _ => format!("arg{index}"),
    }
}

fn tool_input_schema(sig: Option<&TypeSig>, def: Option<&Def>) -> serde_json::Value {
    let Some(sig) = sig else {
        return serde_json::json!({ "type": "object" });
    };
    fn flatten_params<'a>(ty: &'a TypeExpr, out: &mut Vec<&'a TypeExpr>) {
        match ty {
            TypeExpr::Func { params, result, .. } => {
                for param in params {
                    out.push(param);
                }
                flatten_params(result, out);
            }
            _ => {}
        }
    }

    let mut param_types = Vec::new();
    flatten_params(&sig.ty, &mut param_types);
    if param_types.is_empty() {
        return serde_json::json!({ "type": "object" });
    }

    let param_names: Vec<String> = if let Some(def) = def {
        param_types
            .iter()
            .enumerate()
            .map(|(idx, _ty)| {
                def.params
                    .get(idx)
                    .map(|pattern| param_name(pattern, idx))
                    .unwrap_or_else(|| format!("arg{idx}"))
            })
            .collect()
    } else {
        (0..param_types.len())
            .map(|idx| format!("arg{idx}"))
            .collect()
    };

    let mut props = serde_json::Map::new();
    let mut required = Vec::new();
    for (idx, ty) in param_types.iter().enumerate() {
        let name = param_names.get(idx).cloned().unwrap_or_else(|| format!("arg{idx}"));
        props.insert(name.clone(), schema_for_type(ty));
        required.push(serde_json::Value::String(name));
    }
    serde_json::Value::Object(serde_json::Map::from_iter([
        ("type".to_string(), serde_json::Value::String("object".to_string())),
        ("properties".to_string(), serde_json::Value::Object(props)),
        ("required".to_string(), serde_json::Value::Array(required)),
        (
            "additionalProperties".to_string(),
            serde_json::Value::Bool(false),
        ),
    ]))
}

fn type_is_effectful_return(ty: &TypeExpr) -> bool {
    fn peel_result<'a>(ty: &'a TypeExpr) -> &'a TypeExpr {
        match ty {
            TypeExpr::Func { result, .. } => peel_result(result),
            other => other,
        }
    }

    fn is_effect_or_resource(ty: &TypeExpr) -> bool {
        match ty {
            TypeExpr::Name(name) => matches!(name.name.as_str(), "Effect" | "Resource"),
            TypeExpr::Apply { base, .. } => matches!(
                base.as_ref(),
                TypeExpr::Name(name) if matches!(name.name.as_str(), "Effect" | "Resource")
            ),
            _ => false,
        }
    }

    is_effect_or_resource(peel_result(ty))
}

fn expr_is_effectful(expr: &Expr) -> bool {
    match expr {
        Expr::Block { kind, items, .. } => {
            if matches!(kind, BlockKind::Effect | BlockKind::Resource | BlockKind::Generate) {
                return true;
            }
            items.iter().any(|item| match item {
                BlockItem::Bind { expr, .. }
                | BlockItem::Filter { expr, .. }
                | BlockItem::Yield { expr, .. }
                | BlockItem::Recurse { expr, .. }
                | BlockItem::Expr { expr, .. } => expr_is_effectful(expr),
            })
        }
        Expr::Call { func, args, .. } => expr_is_effectful(func) || args.iter().any(expr_is_effectful),
        Expr::Lambda { body, .. } => expr_is_effectful(body),
        Expr::Match { scrutinee, arms, .. } => {
            scrutinee
                .as_ref()
                .map(|expr| expr_is_effectful(expr))
                .unwrap_or(false)
                || arms.iter().any(|arm| {
                    arm.guard.as_ref().is_some_and(expr_is_effectful)
                        || expr_is_effectful(&arm.body)
                })
        }
        Expr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => expr_is_effectful(cond) || expr_is_effectful(then_branch) || expr_is_effectful(else_branch),
        Expr::Binary { left, right, .. } => expr_is_effectful(left) || expr_is_effectful(right),
        Expr::List { items, .. } => items.iter().any(|item: &ListItem| expr_is_effectful(&item.expr)),
        Expr::Tuple { items, .. } => items.iter().any(expr_is_effectful),
        Expr::Record { fields, .. } => fields.iter().any(|field: &RecordField| expr_is_effectful(&field.value)),
        Expr::FieldAccess { base, .. } => expr_is_effectful(base),
        Expr::Index { base, index, .. } => expr_is_effectful(base) || expr_is_effectful(index),
        Expr::FieldSection { .. } | Expr::Ident(_) | Expr::Literal(_) | Expr::Raw { .. } => false,
    }
}

pub fn collect_mcp_manifest(modules: &[Module]) -> McpManifest {
    let mut tools: BTreeMap<String, McpTool> = BTreeMap::new();
    let mut resources: BTreeMap<String, McpResource> = BTreeMap::new();

    for module in modules {
        let module_name = module.name.name.clone();
        let mut sigs = BTreeMap::new();
        let mut defs = BTreeMap::new();
        let mut tool_names = BTreeSet::new();
        let mut resource_names = BTreeSet::new();

        for item in module.items.iter() {
            match item {
                ModuleItem::TypeSig(sig) => {
                    sigs.insert(sig.name.name.clone(), sig);
                    if has_decorator(&sig.decorators, "mcp_tool") {
                        tool_names.insert(sig.name.name.clone());
                    }
                    if has_decorator(&sig.decorators, "mcp_resource") {
                        resource_names.insert(sig.name.name.clone());
                    }
                }
                ModuleItem::Def(def) => {
                    defs.insert(def.name.name.clone(), def);
                    if has_decorator(&def.decorators, "mcp_tool") {
                        tool_names.insert(def.name.name.clone());
                    }
                    if has_decorator(&def.decorators, "mcp_resource") {
                        resource_names.insert(def.name.name.clone());
                    }
                }
                ModuleItem::DomainDecl(domain) => {
                    for domain_item in domain.items.iter() {
                        match domain_item {
                            DomainItem::TypeSig(sig) => {
                                sigs.insert(sig.name.name.clone(), sig);
                                if has_decorator(&sig.decorators, "mcp_tool") {
                                    tool_names.insert(sig.name.name.clone());
                                }
                                if has_decorator(&sig.decorators, "mcp_resource") {
                                    resource_names.insert(sig.name.name.clone());
                                }
                            }
                            DomainItem::Def(def) | DomainItem::LiteralDef(def) => {
                                defs.insert(def.name.name.clone(), def);
                                if has_decorator(&def.decorators, "mcp_tool") {
                                    tool_names.insert(def.name.name.clone());
                                }
                                if has_decorator(&def.decorators, "mcp_resource") {
                                    resource_names.insert(def.name.name.clone());
                                }
                            }
                            DomainItem::TypeAlias(_) => {}
                        }
                    }
                }
                _ => {}
            }
        }

        for binding in tool_names {
            let name = qualified_name(&module_name, &binding);
            let sig = sigs.get(&binding).copied();
            let def = defs.get(&binding).copied();
            tools.entry(name.clone()).or_insert_with(|| McpTool {
                effectful: sig
                    .map(|sig| type_is_effectful_return(&sig.ty))
                    .unwrap_or_else(|| def.is_some_and(|def| expr_is_effectful(&def.expr))),
                name,
                module: module_name.clone(),
                binding,
                input_schema: tool_input_schema(sig, def),
            });
        }

        for binding in resource_names {
            let name = qualified_name(&module_name, &binding);
            resources
                .entry(name.clone())
                .or_insert_with(|| McpResource {
                    name,
                    module: module_name.clone(),
                    binding,
                });
        }
    }

    McpManifest {
        tools: tools.into_values().collect(),
        resources: resources.into_values().collect(),
    }
}

fn jsonrpc_error(id: serde_json::Value, code: i64, message: &str) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message }
    })
}

fn jsonrpc_result(id: serde_json::Value, result: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    })
}

fn handle_request(
    manifest: &McpManifest,
    policy: McpPolicy,
    message: &serde_json::Value,
) -> Option<serde_json::Value> {
    let method = message.get("method")?.as_str()?;
    let id = message.get("id")?.clone();

    let response = match method {
        "initialize" => jsonrpc_result(
            id,
            serde_json::json!({
                "serverInfo": { "name": "aivi", "version": env!("CARGO_PKG_VERSION") },
                "capabilities": {
                    "tools": {},
                    "resources": {}
                }
            }),
        ),
        "tools/list" => jsonrpc_result(
            id,
            serde_json::json!({
                "tools": manifest.tools.iter().filter(|tool| policy.allow_effectful_tools || !tool.effectful).map(|tool| {
                    serde_json::json!({
                        "name": tool.name,
                        "description": null,
                        "inputSchema": tool.input_schema
                    })
                }).collect::<Vec<_>>()
            }),
        ),
        "resources/list" => jsonrpc_result(
            id,
            serde_json::json!({
                "resources": manifest.resources.iter().map(|res| {
                    serde_json::json!({
                        "name": res.name,
                        "description": null,
                        "uri": format!("aivi://{}/{}", res.module, res.binding)
                    })
                }).collect::<Vec<_>>()
            }),
        ),
        _ => jsonrpc_error(id, -32601, "method not found"),
    };

    Some(response)
}

fn read_message(reader: &mut impl BufRead) -> std::io::Result<Option<serde_json::Value>> {
    let mut content_length: Option<usize> = None;
    loop {
        let mut line = String::new();
        let bytes = reader.read_line(&mut line)?;
        if bytes == 0 {
            return Ok(None);
        }
        if line == "\r\n" || line == "\n" {
            break;
        }
        let lower = line.to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix("content-length:") {
            if let Ok(len) = rest.trim().parse::<usize>() {
                content_length = Some(len);
            }
        }
    }
    let Some(len) = content_length else {
        return Ok(None);
    };
    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf)?;
    let message: serde_json::Value = serde_json::from_slice(&buf)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
    Ok(Some(message))
}

fn write_message(mut out: impl Write, message: &serde_json::Value) -> std::io::Result<()> {
    let json = serde_json::to_vec(message)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
    write!(out, "Content-Length: {}\r\n\r\n", json.len())?;
    out.write_all(&json)?;
    out.flush()
}

pub fn serve_mcp_stdio(manifest: &McpManifest) -> Result<(), AiviError> {
    serve_mcp_stdio_with_policy(manifest, McpPolicy::default())
}

pub fn serve_mcp_stdio_with_policy(manifest: &McpManifest, policy: McpPolicy) -> Result<(), AiviError> {
    let stdin = std::io::stdin();
    let mut reader = std::io::BufReader::new(stdin.lock());
    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    while let Some(message) = read_message(&mut reader)? {
        if let Some(response) = handle_request(manifest, policy, &message) {
            write_message(&mut out, &response)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Position, Span};

    #[test]
    fn manifest_collects_tools_and_resources_from_sig_or_def_decorators() {
        let module = Module {
            name: crate::surface::SpannedName {
                name: "Example.Mod".to_string(),
                span: Span {
                    start: Position { line: 1, column: 1 },
                    end: Position { line: 1, column: 1 },
                },
            },
            exports: Vec::new(),
            uses: Vec::new(),
            items: vec![
                ModuleItem::TypeSig(TypeSig {
                    decorators: vec![crate::surface::SpannedName {
                        name: "mcp_tool".to_string(),
                        span: Span {
                            start: Position { line: 1, column: 1 },
                            end: Position { line: 1, column: 1 },
                        },
                    }],
                    name: crate::surface::SpannedName {
                        name: "search".to_string(),
                        span: Span {
                            start: Position { line: 1, column: 1 },
                            end: Position { line: 1, column: 1 },
                        },
                    },
                    ty: crate::surface::TypeExpr::Unknown {
                        span: Span {
                            start: Position { line: 1, column: 1 },
                            end: Position { line: 1, column: 1 },
                        },
                    },
                    span: Span {
                        start: Position { line: 1, column: 1 },
                        end: Position { line: 1, column: 1 },
                    },
                }),
                ModuleItem::Def(Def {
                    decorators: vec![crate::surface::SpannedName {
                        name: "mcp_resource".to_string(),
                        span: Span {
                            start: Position { line: 1, column: 1 },
                            end: Position { line: 1, column: 1 },
                        },
                    }],
                    name: crate::surface::SpannedName {
                        name: "config".to_string(),
                        span: Span {
                            start: Position { line: 1, column: 1 },
                            end: Position { line: 1, column: 1 },
                        },
                    },
                    params: Vec::new(),
                    expr: crate::surface::Expr::Raw {
                        text: String::new(),
                        span: Span {
                            start: Position { line: 1, column: 1 },
                            end: Position { line: 1, column: 1 },
                        },
                    },
                    span: Span {
                        start: Position { line: 1, column: 1 },
                        end: Position { line: 1, column: 1 },
                    },
                }),
            ],
            annotations: Vec::new(),
            span: Span {
                start: Position { line: 1, column: 1 },
                end: Position { line: 1, column: 1 },
            },
            path: "test.aivi".to_string(),
        };

        let manifest = collect_mcp_manifest(&[module]);
        assert_eq!(manifest.tools.len(), 1);
        assert_eq!(manifest.tools[0].name, "Example.Mod.search");
        assert_eq!(manifest.resources.len(), 1);
        assert_eq!(manifest.resources[0].name, "Example.Mod.config");
    }

    #[test]
    fn mcp_tools_list_returns_manifest_tools() {
        let manifest = McpManifest {
            tools: vec![McpTool {
                name: "Example.Mod.search".to_string(),
                module: "Example.Mod".to_string(),
                binding: "search".to_string(),
                input_schema: serde_json::json!({ "type": "object" }),
                effectful: false,
            }],
            resources: Vec::new(),
        };

        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/list",
            "params": {}
        });
        let response = handle_request(&manifest, McpPolicy::default(), &request).expect("response");
        assert_eq!(response["id"], 1);
        assert_eq!(response["result"]["tools"][0]["name"], "Example.Mod.search");
    }

    #[test]
    fn mcp_tools_list_filters_effectful_tools_by_default() {
        let manifest = McpManifest {
            tools: vec![
                McpTool {
                    name: "Example.Mod.pureTool".to_string(),
                    module: "Example.Mod".to_string(),
                    binding: "pureTool".to_string(),
                    input_schema: serde_json::json!({ "type": "object" }),
                    effectful: false,
                },
                McpTool {
                    name: "Example.Mod.effectTool".to_string(),
                    module: "Example.Mod".to_string(),
                    binding: "effectTool".to_string(),
                    input_schema: serde_json::json!({ "type": "object" }),
                    effectful: true,
                },
            ],
            resources: Vec::new(),
        };

        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/list",
            "params": {}
        });

        let response = handle_request(&manifest, McpPolicy::default(), &request).expect("response");
        assert_eq!(response["result"]["tools"].as_array().unwrap().len(), 1);
        assert_eq!(response["result"]["tools"][0]["name"], "Example.Mod.pureTool");

        let response = handle_request(
            &manifest,
            McpPolicy {
                allow_effectful_tools: true,
            },
            &request,
        )
        .expect("response");
        assert_eq!(response["result"]["tools"].as_array().unwrap().len(), 2);
    }
}
