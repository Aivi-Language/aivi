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

fn type_record_field(name: &str, ty: TypeExpr, span: &Span) -> RecordTypeField {
    RecordTypeField::Named {
        name: sn(name, span),
        ty,
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
    let base_url = spec
        .servers
        .first()
        .map(|s| s.url.clone())
        .unwrap_or_default();

    // Collect operations from all paths.
    let mut ops: Vec<(String, &str, String, &Operation)> = Vec::new();
    for (path_str, path_item) in &spec.paths.paths {
        let item = match path_item {
            ReferenceOr::Item(item) => item,
            ReferenceOr::Reference { .. } => continue,
        };
        collect_ops(path_str, item, &mut ops);
    }

    // Build endpoint fields: each is a lambda `params => __openapi_call descriptor config params`
    let config_ident = Expr::Ident(sn("__cfg__", span));
    let mut endpoint_fields = Vec::new();

    for (path, method, op_id, operation) in &ops {
        let descriptor = operation_to_descriptor(&base_url, path, method, operation, spec, span);
        let endpoint_lambda = make_endpoint_lambda(&descriptor, &config_ident, span);
        endpoint_fields.push(record_field(op_id, endpoint_lambda, span));
    }

    let inner_record = record(endpoint_fields, span);

    // Wrap in outer lambda: config => { endpoint1: ..., endpoint2: ... }
    let expr = Expr::Lambda {
        params: vec![Pattern::Ident(sn("__cfg__", span))],
        body: Box::new(inner_record),
        span: span.clone(),
    };

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

/// Build an endpoint lambda: `params => __openapi_call descriptor config params`
fn make_endpoint_lambda(descriptor: &Expr, config_ident: &Expr, span: &Span) -> Expr {
    let params_ident = Expr::Ident(sn("__prm__", span));
    let call = Expr::Call {
        func: Box::new(Expr::Ident(sn("__openapi_call", span))),
        args: vec![descriptor.clone(), config_ident.clone(), params_ident],
        span: span.clone(),
    };
    Expr::Lambda {
        params: vec![Pattern::Ident(sn("__prm__", span))],
        body: Box::new(call),
        span: span.clone(),
    }
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

/// Build a descriptor record for an endpoint (consumed by `__openapi_call` at runtime).
fn operation_to_descriptor(
    base_url: &str,
    path: &str,
    method: &str,
    op: &Operation,
    spec: &OpenAPI,
    span: &Span,
) -> Expr {
    let mut fields = Vec::new();
    fields.push(record_field("__method", str_lit(method, span), span));
    fields.push(record_field("__path", str_lit(path, span), span));
    fields.push(record_field("__baseUrl", str_lit(base_url, span), span));

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
            let fields: Vec<RecordTypeField> = obj
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
                    type_record_field(name, ty, span)
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
                    .map(|v| {
                        // Avoid shadowing prelude constructors (None, Some, True, False, Ok, Err).
                        let raw = capitalize(v);
                        let ctor_name = if matches!(
                            raw.as_str(),
                            "None" | "Some" | "True" | "False" | "Ok" | "Err"
                        ) {
                            format!("{}{}", name, raw)
                        } else {
                            raw
                        };
                        TypeCtor {
                            name: sn(&ctor_name, span),
                            args: Vec::new(),
                            span: span.clone(),
                        }
                    })
                    .collect();
                items.push(ModuleItem::TypeDecl(TypeDecl {
                    decorators: Vec::new(),
                    name: sn(name, span),
                    params: Vec::new(),
                    constructors,
                    opaque: false,
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
                    opaque: false,
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
                    opaque: false,
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
                    opaque: false,
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
    // Config type: all fields are Option per spec (decorators.md §Config Record Fields).
    let header_pair_ty = TypeExpr::Tuple {
        items: vec![
            TypeExpr::Name(sn("Text", span)),
            TypeExpr::Name(sn("Text", span)),
        ],
        span: span.clone(),
    };
    let option_text = TypeExpr::Apply {
        base: Box::new(TypeExpr::Name(sn("Option", span))),
        args: vec![TypeExpr::Name(sn("Text", span))],
        span: span.clone(),
    };
    let option_int = TypeExpr::Apply {
        base: Box::new(TypeExpr::Name(sn("Option", span))),
        args: vec![TypeExpr::Name(sn("Int", span))],
        span: span.clone(),
    };
    let option_bool = TypeExpr::Apply {
        base: Box::new(TypeExpr::Name(sn("Option", span))),
        args: vec![TypeExpr::Name(sn("Bool", span))],
        span: span.clone(),
    };
    let option_header_list = TypeExpr::Apply {
        base: Box::new(TypeExpr::Name(sn("Option", span))),
        args: vec![TypeExpr::Apply {
            base: Box::new(TypeExpr::Name(sn("List", span))),
            args: vec![header_pair_ty],
            span: span.clone(),
        }],
        span: span.clone(),
    };
    let config_ty = TypeExpr::Record {
        fields: vec![
            type_record_field("bearerToken", option_text.clone(), span),
            type_record_field("headers", option_header_list, span),
            type_record_field("timeoutMs", option_int.clone(), span),
            type_record_field("retryCount", option_int, span),
            type_record_field("strictStatus", option_bool, span),
            type_record_field("baseUrl", option_text, span),
        ],
        span: span.clone(),
    };

    // Response type placeholder (for all endpoints)
    let response_ty = TypeExpr::Record {
        fields: vec![
            type_record_field("status", TypeExpr::Name(sn("Int", span)), span),
            type_record_field(
                "headers",
                TypeExpr::Apply {
                    base: Box::new(TypeExpr::Name(sn("List", span))),
                    args: vec![TypeExpr::Record {
                        fields: vec![
                            type_record_field("name", TypeExpr::Name(sn("Text", span)), span),
                            type_record_field("value", TypeExpr::Name(sn("Text", span)), span),
                        ],
                        span: span.clone(),
                    }],
                    span: span.clone(),
                },
                span,
            ),
            type_record_field("body", TypeExpr::Name(sn("Text", span)), span),
        ],
        span: span.clone(),
    };
    let error_ty = TypeExpr::Record {
        fields: vec![type_record_field(
            "message",
            TypeExpr::Name(sn("Text", span)),
            span,
        )],
        span: span.clone(),
    };
    let result_response_ty = TypeExpr::Apply {
        base: Box::new(TypeExpr::Name(sn("Result", span))),
        args: vec![error_ty, response_ty],
        span: span.clone(),
    };
    let source_ty = TypeExpr::Apply {
        base: Box::new(TypeExpr::Name(sn("Source", span))),
        args: vec![TypeExpr::Name(sn("RestApi", span)), result_response_ty],
        span: span.clone(),
    };

    // Build endpoint record: { endpoint: Params -> Source ... }
    let mut endpoint_fields: Vec<RecordTypeField> = Vec::new();

    for (_path, _method, op_id, operation) in ops {
        // Build params type from operation parameters + request body
        let mut param_fields: Vec<RecordTypeField> = Vec::new();

        for param_ref in &operation.parameters {
            if let ReferenceOr::Item(param) = param_ref {
                let data = param.parameter_data_ref();
                let param_ty = match &data.format {
                    openapiv3::ParameterSchemaOrContent::Schema(schema) => {
                        schema_ref_to_type_expr(schema, spec, span)
                    }
                    _ => TypeExpr::Name(sn("Text", span)),
                };
                let param_ty = if data.required {
                    param_ty
                } else {
                    TypeExpr::Apply {
                        base: Box::new(TypeExpr::Name(sn("Option", span))),
                        args: vec![param_ty],
                        span: span.clone(),
                    }
                };
                param_fields.push(type_record_field(&data.name, param_ty, span));
            }
        }

        // Add request body fields for POST/PUT/PATCH
        if let Some(ReferenceOr::Item(body)) = &operation.request_body {
            if let Some(mt) = body.content.get("application/json") {
                if let Some(schema_ref) = &mt.schema {
                    // If the body schema is an object, inline its fields
                    match schema_ref {
                        ReferenceOr::Item(schema) => {
                            if let SchemaKind::Type(OaType::Object(obj)) = &schema.schema_kind {
                                let required: std::collections::HashSet<&str> =
                                    obj.required.iter().map(|s| s.as_str()).collect();
                                for (name, prop_ref) in &obj.properties {
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
                                    param_fields.push(type_record_field(name, ty, span));
                                }
                            }
                        }
                        ReferenceOr::Reference { reference } => {
                            // For $ref bodies, look up the schema in components
                            let ref_name = ref_to_type_name(reference);
                            if let Some(components) = &spec.components {
                                if let Some(ReferenceOr::Item(schema)) =
                                    components.schemas.get(&ref_name)
                                {
                                    if let SchemaKind::Type(OaType::Object(obj)) =
                                        &schema.schema_kind
                                    {
                                        let required: std::collections::HashSet<&str> =
                                            obj.required.iter().map(|s| s.as_str()).collect();
                                        for (name, prop_ref) in &obj.properties {
                                            let ty =
                                                boxed_schema_ref_to_type_expr(prop_ref, spec, span);
                                            let ty = if required.contains(name.as_str()) {
                                                ty
                                            } else {
                                                TypeExpr::Apply {
                                                    base: Box::new(TypeExpr::Name(sn(
                                                        "Option", span,
                                                    ))),
                                                    args: vec![ty],
                                                    span: span.clone(),
                                                }
                                            };
                                            param_fields.push(type_record_field(name, ty, span));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        let params_ty = TypeExpr::Record {
            fields: param_fields,
            span: span.clone(),
        };

        let endpoint_ty = TypeExpr::Func {
            params: vec![params_ty],
            result: Box::new(source_ty.clone()),
            span: span.clone(),
        };

        endpoint_fields.push(type_record_field(op_id, endpoint_ty, span));
    }

    let endpoints_record_ty = TypeExpr::Record {
        fields: endpoint_fields,
        span: span.clone(),
    };

    // Final type: Config -> { endpoints... }
    let binding_ty = TypeExpr::Func {
        params: vec![config_ty],
        result: Box::new(endpoints_record_ty),
        span: span.clone(),
    };

    TypeSig {
        decorators: Vec::new(),
        name: sn(binding_name, span),
        ty: binding_ty,
        span: span.clone(),
    }
}
