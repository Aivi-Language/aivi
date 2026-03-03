/// Compile-time OpenAPI spec → AIVI AST conversion.
///
/// Parses an OpenAPI 3.x specification (JSON or YAML) and produces an `Expr::Record`
/// whose fields are typed stub functions for each endpoint, plus synthesised
/// `TypeAlias` / `TypeSig` items so the LSP can offer rich autocompletion.
use crate::diagnostics::Span;
use crate::surface::ast::*;
use openapiv3::{
    OpenAPI, Operation, PathItem, ReferenceOr, Schema, SchemaKind, StatusCode, Type as OaType,
};

/// The result of compiling an OpenAPI spec into AIVI AST nodes.
pub struct OpenApiGenResult {
    /// The value expression (record of endpoint descriptors).
    pub expr: Expr,
    /// Synthesised module items: `TypeAlias` for each schema + one `TypeSig` for the binding.
    pub items: Vec<ModuleItem>,
}

/// Fetch or read an OpenAPI spec, parse it, and return an `OpenApiGenResult` containing
/// both the value expression and synthesised type items for LSP autocompletion.
pub fn openapi_to_expr(
    source: &str,
    is_url: bool,
    base_dir: &std::path::Path,
    span: &Span,
    binding_name: &str,
) -> Result<OpenApiGenResult, String> {
    let contents = if is_url {
        fetch_spec(source)?
    } else {
        read_spec_file(source, base_dir)?
    };
    let spec = parse_spec(&contents)?;
    Ok(spec_to_result(&spec, span, binding_name))
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

// -- AST helpers --------------------------------------------------------------

fn sn(name: &str, span: &Span) -> SpannedName {
    SpannedName {
        name: name.to_string(),
        span: span.clone(),
    }
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

// -- Spec -> Result (expr + synthesised types) --------------------------------

fn spec_to_result(spec: &OpenAPI, span: &Span, binding_name: &str) -> OpenApiGenResult {
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

    let expr = record(fields, span);

    // -- Synthesise type items for LSP ----------------------------------------
    let mut items = Vec::new();

    // TypeAlias per component schema
    let schema_aliases = generate_schema_type_aliases(spec, span);
    items.extend(schema_aliases);

    // TypeSig for the binding itself
    let binding_sig = generate_binding_type_sig(spec, span, binding_name, &ops);
    items.push(ModuleItem::TypeSig(binding_sig));

    OpenApiGenResult { expr, items }
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

/// Build a record describing an endpoint descriptor for the runtime.
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

    let params = op
        .parameters
        .iter()
        .filter_map(|p| match p {
            ReferenceOr::Item(param) => Some(param_to_expr(param, span)),
            ReferenceOr::Reference { .. } => None,
        })
        .collect::<Vec<_>>();
    fields.push(record_field("__params", list(params, span), span));

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

    if let Some(type_name) = response_type_name(op, spec) {
        fields.push(record_field("__response", str_lit(&type_name, span), span));
    }

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

// -- Synthesised TypeAlias / TypeSig generation -------------------------------

fn schema_ref_to_type_expr(
    schema_ref: &ReferenceOr<Schema>,
    spec: &OpenAPI,
    span: &Span,
) -> TypeExpr {
    match schema_ref {
        ReferenceOr::Reference { reference } => {
            TypeExpr::Name(sn(&ref_to_type_name(reference), span))
        }
        ReferenceOr::Item(schema) => inline_schema_to_type_expr(schema, spec, span),
    }
}

fn boxed_schema_ref_to_type_expr(
    schema_ref: &ReferenceOr<Box<Schema>>,
    spec: &OpenAPI,
    span: &Span,
) -> TypeExpr {
    match schema_ref {
        ReferenceOr::Reference { reference } => {
            TypeExpr::Name(sn(&ref_to_type_name(reference), span))
        }
        ReferenceOr::Item(schema) => inline_schema_to_type_expr(schema.as_ref(), spec, span),
    }
}

fn inline_schema_to_type_expr(schema: &Schema, spec: &OpenAPI, span: &Span) -> TypeExpr {
    match &schema.schema_kind {
        SchemaKind::Type(OaType::String(sv)) => {
            if let openapiv3::VariantOrUnknownOrEmpty::Item(format) = &sv.format {
                match format {
                    openapiv3::StringFormat::Date => return TypeExpr::Name(sn("Date", span)),
                    openapiv3::StringFormat::DateTime => {
                        return TypeExpr::Name(sn("DateTime", span))
                    }
                    _ => {}
                }
            }
            TypeExpr::Name(sn("Text", span))
        }
        SchemaKind::Type(OaType::Integer(_)) => TypeExpr::Name(sn("Int", span)),
        SchemaKind::Type(OaType::Number(_)) => TypeExpr::Name(sn("Float", span)),
        SchemaKind::Type(OaType::Boolean(_)) => TypeExpr::Name(sn("Bool", span)),
        SchemaKind::Type(OaType::Array(arr)) => {
            let inner = arr
                .items
                .as_ref()
                .map(|i| boxed_schema_ref_to_type_expr(i, spec, span))
                .unwrap_or_else(|| TypeExpr::Name(sn("Any", span)));
            TypeExpr::Apply {
                base: Box::new(TypeExpr::Name(sn("List", span))),
                args: vec![inner],
                span: span.clone(),
            }
        }
        SchemaKind::Type(OaType::Object(obj)) => {
            let required: std::collections::HashSet<&str> =
                obj.required.iter().map(|s| s.as_str()).collect();
            let fields: Vec<(SpannedName, TypeExpr)> = obj
                .properties
                .iter()
                .map(|(name, prop_ref)| {
                    let ty = boxed_schema_ref_to_type_expr(prop_ref, spec, span);
                    let ty = if required.contains(name.as_str()) {
                        ty
                    } else {
                        TypeExpr::Apply {
                            base: Box::new(TypeExpr::Name(sn("Option", span))),
                            args: vec![ty],
                            span: span.clone(),
                        }
                    };
                    (sn(name, span), ty)
                })
                .collect();
            TypeExpr::Record {
                fields,
                span: span.clone(),
            }
        }
        _ => TypeExpr::Name(sn("Any", span)),
    }
}

fn generate_schema_type_aliases(spec: &OpenAPI, span: &Span) -> Vec<ModuleItem> {
    let schemas = match &spec.components {
        Some(c) => &c.schemas,
        None => return Vec::new(),
    };
    let mut items = Vec::new();
    for (name, schema_ref) in schemas {
        let schema = match schema_ref {
            ReferenceOr::Item(s) => s,
            ReferenceOr::Reference { .. } => continue,
        };
        match &schema.schema_kind {
            SchemaKind::Type(OaType::String(sv)) if !sv.enumeration.is_empty() => {
                let constructors = sv
                    .enumeration
                    .iter()
                    .filter_map(|v| v.as_ref())
                    .map(|v| TypeCtor {
                        name: sn(&capitalize(v), span),
                        args: Vec::new(),
                        span: span.clone(),
                    })
                    .collect();
                items.push(ModuleItem::TypeDecl(TypeDecl {
                    decorators: Vec::new(),
                    name: sn(name, span),
                    params: Vec::new(),
                    constructors,
                    span: span.clone(),
                }));
            }
            SchemaKind::OneOf { one_of } => {
                let constructors = one_of
                    .iter()
                    .map(|r| {
                        let type_name = schema_type_name(r, spec);
                        TypeCtor {
                            name: sn(&type_name, span),
                            args: vec![TypeExpr::Name(sn(&type_name, span))],
                            span: span.clone(),
                        }
                    })
                    .collect();
                items.push(ModuleItem::TypeDecl(TypeDecl {
                    decorators: Vec::new(),
                    name: sn(name, span),
                    params: Vec::new(),
                    constructors,
                    span: span.clone(),
                }));
            }
            SchemaKind::AnyOf { any_of } => {
                let constructors = any_of
                    .iter()
                    .map(|r| {
                        let type_name = schema_type_name(r, spec);
                        TypeCtor {
                            name: sn(&type_name, span),
                            args: vec![TypeExpr::Name(sn(&type_name, span))],
                            span: span.clone(),
                        }
                    })
                    .collect();
                items.push(ModuleItem::TypeDecl(TypeDecl {
                    decorators: Vec::new(),
                    name: sn(name, span),
                    params: Vec::new(),
                    constructors,
                    span: span.clone(),
                }));
            }
            _ => {
                let aliased = inline_schema_to_type_expr(schema, spec, span);
                items.push(ModuleItem::TypeAlias(TypeAlias {
                    decorators: Vec::new(),
                    name: sn(name, span),
                    params: Vec::new(),
                    aliased,
                    span: span.clone(),
                }));
            }
        }
    }
    items
}

fn generate_binding_type_sig(
    spec: &OpenAPI,
    span: &Span,
    binding_name: &str,
    ops: &[(String, &str, String, &Operation)],
) -> TypeSig {
    let effect_wrapper = |response_ty: TypeExpr, span: &Span| -> TypeExpr {
        TypeExpr::Apply {
            base: Box::new(TypeExpr::Name(sn("Effect", span))),
            args: vec![
                TypeExpr::Apply {
                    base: Box::new(TypeExpr::Name(sn("SourceError", span))),
                    args: vec![TypeExpr::Name(sn("RestApi", span))],
                    span: span.clone(),
                },
                response_ty,
            ],
            span: span.clone(),
        }
    };

    let mut record_fields: Vec<(SpannedName, TypeExpr)> = Vec::new();

    if !spec.servers.is_empty() {
        record_fields.push((sn("__baseUrl", span), TypeExpr::Name(sn("Text", span))));
    }

    for (_path, _method, op_id, operation) in ops {
        let mut param_types: Vec<TypeExpr> = Vec::new();

        // Path parameters -> positional args
        for p in &operation.parameters {
            if let ReferenceOr::Item(param) = p {
                if matches!(param, openapiv3::Parameter::Path { .. }) {
                    let data = param.parameter_data_ref();
                    let ty = param_format_to_type_expr(&data.format, spec, span)
                        .unwrap_or_else(|| TypeExpr::Name(sn("Text", span)));
                    param_types.push(ty);
                }
            }
        }

        // Query/header parameters -> optional record arg
        let option_params: Vec<(SpannedName, TypeExpr)> = operation
            .parameters
            .iter()
            .filter_map(|p| match p {
                ReferenceOr::Item(param) => {
                    if matches!(
                        param,
                        openapiv3::Parameter::Query { .. } | openapiv3::Parameter::Header { .. }
                    ) {
                        let data = param.parameter_data_ref();
                        let base_ty = param_format_to_type_expr(&data.format, spec, span)
                            .unwrap_or_else(|| TypeExpr::Name(sn("Text", span)));
                        let ty = if data.required {
                            base_ty
                        } else {
                            TypeExpr::Apply {
                                base: Box::new(TypeExpr::Name(sn("Option", span))),
                                args: vec![base_ty],
                                span: span.clone(),
                            }
                        };
                        Some((sn(&data.name, span), ty))
                    } else {
                        None
                    }
                }
                _ => None,
            })
            .collect();
        if !option_params.is_empty() {
            param_types.push(TypeExpr::Record {
                fields: option_params,
                span: span.clone(),
            });
        }

        // Request body -> typed arg
        if let Some(ReferenceOr::Item(body)) = &operation.request_body {
            if let Some(mt) = body.content.get("application/json") {
                if let Some(schema_ref) = &mt.schema {
                    param_types.push(schema_ref_to_type_expr(schema_ref, spec, span));
                }
            }
        }

        // Response type
        let response_ty = response_type_name(operation, spec)
            .map(|name| TypeExpr::Name(sn(&name, span)))
            .unwrap_or_else(|| TypeExpr::Name(sn("Unit", span)));
        let result_ty = effect_wrapper(response_ty, span);

        let endpoint_ty = if param_types.is_empty() {
            result_ty
        } else {
            TypeExpr::Func {
                params: param_types,
                result: Box::new(result_ty),
                span: span.clone(),
            }
        };

        record_fields.push((sn(op_id, span), endpoint_ty));
    }

    TypeSig {
        decorators: Vec::new(),
        name: sn(binding_name, span),
        ty: TypeExpr::Record {
            fields: record_fields,
            span: span.clone(),
        },
        span: span.clone(),
    }
}

fn param_format_to_type_expr(
    format: &openapiv3::ParameterSchemaOrContent,
    spec: &OpenAPI,
    span: &Span,
) -> Option<TypeExpr> {
    match format {
        openapiv3::ParameterSchemaOrContent::Schema(schema_ref) => {
            Some(schema_ref_to_type_expr(schema_ref, spec, span))
        }
        _ => None,
    }
}
