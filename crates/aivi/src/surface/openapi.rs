/// Compile-time OpenAPI spec → AIVI AST conversion.
///
/// Parses an OpenAPI 3.x specification (JSON or YAML) and produces an `Expr::Record`
/// whose fields are typed stub functions for each endpoint.
use crate::diagnostics::Span;
use crate::surface::ast::*;
use openapiv3::{
    OpenAPI, Operation, PathItem, ReferenceOr, Schema, SchemaKind, StatusCode, Type as OaType,
};
/// Fetch or read an OpenAPI spec, parse it, and return an `Expr::Record` representing the
/// generated typed API module.  On failure a human-readable error string is returned.
pub fn openapi_to_expr(
    source: &str,
    is_url: bool,
    base_dir: &std::path::Path,
    span: &Span,
) -> Result<Expr, String> {
    let contents = if is_url {
        fetch_spec(source)?
    } else {
        read_spec_file(source, base_dir)?
    };
    let spec = parse_spec(&contents)?;
    Ok(spec_to_expr(&spec, span))
}

fn fetch_spec(url: &str) -> Result<String, String> {
    let body = ureq::get(url)
        .call()
        .map_err(|e| format!("failed to fetch OpenAPI spec from {url}: {e}"))?
        .body_mut()
        .read_to_string()
        .map_err(|e| format!("failed to read response body from {url}: {e}"))?;
    Ok(body)
}

fn read_spec_file(rel: &str, base_dir: &std::path::Path) -> Result<String, String> {
    let source_relative = base_dir.join(rel);
    let cwd_relative = std::path::PathBuf::from(rel);
    std::fs::read_to_string(&source_relative)
        .or_else(|_| std::fs::read_to_string(&cwd_relative))
        .map_err(|e| {
            format!(
                "failed to read OpenAPI spec file {} (also tried {}): {e}",
                source_relative.display(),
                cwd_relative.display()
            )
        })
}

fn parse_spec(contents: &str) -> Result<OpenAPI, String> {
    // Try JSON first, then YAML.
    if let Ok(spec) = serde_json::from_str::<OpenAPI>(contents) {
        return Ok(spec);
    }
    serde_yml::from_str::<OpenAPI>(contents)
        .map_err(|e| format!("failed to parse OpenAPI spec: {e}"))
}

// ── AST helpers ──────────────────────────────────────────────────────────────

fn sn(name: &str, span: &Span) -> SpannedName {
    SpannedName {
        name: name.to_string(),
        span: span.clone(),
    }
}

fn ident(name: &str, span: &Span) -> Expr {
    Expr::Ident(sn(name, span))
}

fn str_lit(text: &str, span: &Span) -> Expr {
    Expr::Literal(Literal::String {
        text: text.to_string(),
        span: span.clone(),
    })
}

fn record_field(key: &str, value: Expr, span: &Span) -> RecordField {
    RecordField {
        spread: false,
        path: vec![PathSegment::Field(sn(key, span))],
        value,
        span: span.clone(),
    }
}

fn record(fields: Vec<RecordField>, span: &Span) -> Expr {
    Expr::Record {
        fields,
        span: span.clone(),
    }
}

fn list(items: Vec<Expr>, span: &Span) -> Expr {
    Expr::List {
        items: items
            .into_iter()
            .map(|expr| ListItem {
                expr,
                spread: false,
                span: span.clone(),
            })
            .collect(),
        span: span.clone(),
    }
}

// ── Spec → Expr ──────────────────────────────────────────────────────────────

fn spec_to_expr(spec: &OpenAPI, span: &Span) -> Expr {
    let mut fields = Vec::new();

    // Add a __meta field with server info.
    if let Some(server) = spec.servers.first() {
        fields.push(record_field("__baseUrl", str_lit(&server.url, span), span));
    }

    // Collect operations from all paths.
    let mut ops: Vec<(String, &str, String, &Operation)> = Vec::new();
    for (path_str, path_item) in &spec.paths.paths {
        let item = match path_item {
            ReferenceOr::Item(item) => item,
            ReferenceOr::Reference { .. } => continue,
        };
        collect_ops(path_str, item, &mut ops);
    }

    for (path, method, op_id, operation) in &ops {
        let func_expr = operation_to_expr(path, method, operation, spec, span);
        fields.push(record_field(op_id, func_expr, span));
    }

    record(fields, span)
}

fn collect_ops<'a>(
    path: &str,
    item: &'a PathItem,
    out: &mut Vec<(String, &'a str, String, &'a Operation)>,
) {
    let methods: &[(&str, &Option<Operation>)] = &[
        ("GET", &item.get),
        ("POST", &item.post),
        ("PUT", &item.put),
        ("DELETE", &item.delete),
        ("PATCH", &item.patch),
        ("HEAD", &item.head),
        ("OPTIONS", &item.options),
    ];
    for &(method, op_opt) in methods {
        if let Some(op) = op_opt {
            let id = op
                .operation_id
                .clone()
                .unwrap_or_else(|| derive_operation_id(method, path));
            out.push((path.to_string(), method, id, op));
        }
    }
}

fn derive_operation_id(method: &str, path: &str) -> String {
    // GET /pets/{petId} → getPetsPetId
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    let mut name = method.to_lowercase();
    for seg in segments {
        if seg.starts_with('{') && seg.ends_with('}') {
            let inner = &seg[1..seg.len() - 1];
            name.push_str(&capitalize(inner));
        } else {
            name.push_str(&capitalize(seg));
        }
    }
    name
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().to_string() + chars.as_str(),
    }
}

/// Build a record describing an endpoint.  In the static evaluation phase we produce
/// a **descriptor record** that the runtime REST layer can consume:
///
/// ```text
/// { __method: "GET", __path: "/pets/{petId}", __params: [...], __response: "Pet" }
/// ```
fn operation_to_expr(
    path: &str,
    method: &str,
    op: &Operation,
    spec: &OpenAPI,
    span: &Span,
) -> Expr {
    let mut fields = Vec::new();
    fields.push(record_field("__method", str_lit(method, span), span));
    fields.push(record_field("__path", str_lit(path, span), span));

    // Parameters
    let params = op
        .parameters
        .iter()
        .filter_map(|p| match p {
            ReferenceOr::Item(param) => Some(param_to_expr(param, span)),
            ReferenceOr::Reference { .. } => None,
        })
        .collect::<Vec<_>>();
    fields.push(record_field("__params", list(params, span), span));

    // Request body schema name
    if let Some(ReferenceOr::Item(body)) = &op.request_body {
        if let Some(mt) = body.content.get("application/json") {
            if let Some(schema_ref) = &mt.schema {
                let type_name = schema_type_name(schema_ref, spec);
                fields.push(record_field(
                    "__requestBody",
                    str_lit(&type_name, span),
                    span,
                ));
            }
        }
    }

    // Response schema name (first 2xx response)
    if let Some(type_name) = response_type_name(op, spec) {
        fields.push(record_field("__response", str_lit(&type_name, span), span));
    }

    // Description
    if let Some(desc) = &op.description {
        fields.push(record_field("__description", str_lit(desc, span), span));
    }

    record(fields, span)
}

fn param_to_expr(param: &openapiv3::Parameter, span: &Span) -> Expr {
    let data = param.parameter_data_ref();
    let mut fields = Vec::new();
    fields.push(record_field("name", str_lit(&data.name, span), span));
    let location = match param {
        openapiv3::Parameter::Query { .. } => "query",
        openapiv3::Parameter::Header { .. } => "header",
        openapiv3::Parameter::Path { .. } => "path",
        openapiv3::Parameter::Cookie { .. } => "cookie",
    };
    fields.push(record_field("in", str_lit(location, span), span));
    fields.push(record_field(
        "required",
        Expr::Literal(Literal::Bool {
            value: data.required,
            span: span.clone(),
        }),
        span,
    ));
    record(fields, span)
}

fn response_type_name(op: &Operation, spec: &OpenAPI) -> Option<String> {
    for (status, resp_ref) in &op.responses.responses {
        let is_2xx = match status {
            StatusCode::Code(code) => (200..300).contains(code),
            StatusCode::Range(r) => *r == 2,
        };
        if !is_2xx {
            continue;
        }
        let resp = match resp_ref {
            ReferenceOr::Item(r) => r,
            ReferenceOr::Reference { reference } => {
                return Some(ref_to_type_name(reference));
            }
        };
        if let Some(mt) = resp.content.get("application/json") {
            if let Some(schema_ref) = &mt.schema {
                return Some(schema_type_name(schema_ref, spec));
            }
        }
    }
    None
}

fn schema_type_name(schema_ref: &ReferenceOr<Schema>, spec: &OpenAPI) -> String {
    match schema_ref {
        ReferenceOr::Reference { reference } => ref_to_type_name(reference),
        ReferenceOr::Item(schema) => inline_type_name(schema, spec),
    }
}

fn boxed_schema_type_name(schema_ref: &ReferenceOr<Box<Schema>>, spec: &OpenAPI) -> String {
    match schema_ref {
        ReferenceOr::Reference { reference } => ref_to_type_name(reference),
        ReferenceOr::Item(schema) => inline_type_name(schema, spec),
    }
}

fn ref_to_type_name(reference: &str) -> String {
    // "#/components/schemas/Pet" → "Pet"
    reference
        .rsplit('/')
        .next()
        .unwrap_or("Unknown")
        .to_string()
}

fn inline_type_name(schema: &Schema, spec: &OpenAPI) -> String {
    match &schema.schema_kind {
        SchemaKind::Type(OaType::String(_)) => "Text".to_string(),
        SchemaKind::Type(OaType::Integer(_)) => "Int".to_string(),
        SchemaKind::Type(OaType::Number(_)) => "Float".to_string(),
        SchemaKind::Type(OaType::Boolean(_)) => "Bool".to_string(),
        SchemaKind::Type(OaType::Array(arr)) => {
            let item_type = arr
                .items
                .as_ref()
                .map(|i| boxed_schema_type_name(i, spec))
                .unwrap_or_else(|| "Any".to_string());
            format!("List {item_type}")
        }
        SchemaKind::Type(OaType::Object(_)) => "Record".to_string(),
        _ => "Any".to_string(),
    }
}

// ── Schema → Type declaration records ────────────────────────────────────────

/// Produce a list of `(name, Expr)` pairs for each component schema, so the
/// generated record can include a `__types` field with the schema definitions.
pub fn schema_type_records(spec: &OpenAPI, span: &Span) -> Vec<RecordField> {
    let schemas = match &spec.components {
        Some(c) => &c.schemas,
        None => return Vec::new(),
    };
    let mut out = Vec::new();
    for (name, schema_ref) in schemas {
        let schema = match schema_ref {
            ReferenceOr::Item(s) => s,
            ReferenceOr::Reference { .. } => continue,
        };
        let type_expr = schema_to_type_expr(name, schema, spec, span);
        out.push(record_field(name, type_expr, span));
    }
    out
}

fn schema_to_type_expr(_name: &str, schema: &Schema, spec: &OpenAPI, span: &Span) -> Expr {
    match &schema.schema_kind {
        SchemaKind::Type(OaType::Object(obj)) => {
            let required: std::collections::HashSet<&str> =
                obj.required.iter().map(|s| s.as_str()).collect();
            let mut fields = Vec::new();
            for (prop_name, prop_ref) in &obj.properties {
                let type_name = boxed_schema_type_name(prop_ref, spec);
                let type_str = if required.contains(prop_name.as_str()) {
                    type_name
                } else {
                    format!("Option {type_name}")
                };
                fields.push(record_field(prop_name, str_lit(&type_str, span), span));
            }
            record(fields, span)
        }
        SchemaKind::Type(OaType::String(sv)) => {
            if !sv.enumeration.is_empty() {
                let variants: Vec<Expr> = sv
                    .enumeration
                    .iter()
                    .filter_map(|v| v.as_ref())
                    .map(|v| str_lit(&capitalize(v), span))
                    .collect();
                list(variants, span)
            } else {
                str_lit("Text", span)
            }
        }
        SchemaKind::Type(OaType::Integer(_)) => str_lit("Int", span),
        SchemaKind::Type(OaType::Number(_)) => str_lit("Float", span),
        SchemaKind::Type(OaType::Boolean(_)) => str_lit("Bool", span),
        SchemaKind::Type(OaType::Array(arr)) => {
            let inner = arr
                .items
                .as_ref()
                .map(|i| boxed_schema_type_name(i, spec))
                .unwrap_or_else(|| "Any".to_string());
            str_lit(&format!("List {inner}"), span)
        }
        SchemaKind::OneOf { one_of } => {
            let variants: Vec<Expr> = one_of
                .iter()
                .map(|r| str_lit(&schema_type_name(r, spec), span))
                .collect();
            list(variants, span)
        }
        SchemaKind::AnyOf { any_of } => {
            let variants: Vec<Expr> = any_of
                .iter()
                .map(|r| str_lit(&schema_type_name(r, spec), span))
                .collect();
            list(variants, span)
        }
        _ => str_lit("Any", span),
    }
}
