fn apply_static_decorators(modules: &mut [Module]) -> Vec<FileDiagnostic> {
    fn has_decorator(decorators: &[Decorator], name: &str) -> bool {
        decorators
            .iter()
            .any(|decorator| decorator.name.name == name)
    }

    fn emit_diag(
        module_path: &str,
        out: &mut Vec<FileDiagnostic>,
        code: &str,
        message: String,
        span: Span,
    ) {
        out.push(FileDiagnostic {
            path: module_path.to_string(),
            diagnostic: Diagnostic {
                code: code.to_string(),
                severity: DiagnosticSeverity::Error,
                message,
                span,
                labels: Vec::new(),
                hints: Vec::new(),
                suggestion: None,
            },
        });
    }

    fn apply_static_to_def(
        module_path: &str,
        base_dir: &std::path::Path,
        is_static: bool,
        def: &mut Def,
        out: &mut Vec<FileDiagnostic>,
        extra_items: &mut Vec<ModuleItem>,
        type_aliases: &std::collections::HashMap<String, TypeExpr>,
    ) {
        if !is_static {
            return;
        }
        if !def.params.is_empty() {
            emit_diag(
                module_path,
                out,
                "E1514",
                "`@static` can only be applied to value definitions (no parameters)".to_string(),
                def.span.clone(),
            );
            return;
        }

        // Compile-time evaluation for deterministic source calls.
        let original_span = expr_span(&def.expr);
        let expr = def.expr.clone();
        let Expr::Call { func, args, .. } = &expr else {
            // `@static` is allowed on any value definition; compile-time evaluation is best-effort.
            return;
        };
        let (base_name, field_name) = match func.as_ref() {
            Expr::FieldAccess { base, field, .. } => match base.as_ref() {
                Expr::Ident(name) => (Some(name.name.as_str()), Some(field.name.as_str())),
                _ => (None, None),
            },
            _ => (None, None),
        };
        if args.len() == 1 {
            if base_name == Some("env") && field_name == Some("get") {
                let Some(Expr::Literal(Literal::String { text: key, .. })) = args.first() else {
                    return;
                };
                let value = std::env::var(key).unwrap_or_default();
                def.expr = Expr::Literal(Literal::String {
                    text: value,
                    span: original_span,
                });
                return;
            }
            if base_name == Some("openapi")
                && matches!(field_name, Some("fromUrl" | "fromFile"))
            {
                let is_url = field_name == Some("fromUrl");
                let source = match args.first() {
                    Some(Expr::Literal(Literal::Sigil { tag, body, .. }))
                        if (tag == "u" || tag == "url") && is_url =>
                    {
                        body.clone()
                    }
                    Some(Expr::Literal(Literal::String { text, .. })) if !is_url => text.clone(),
                    _ => {
                        let expected = if is_url {
                            "a ~url(...) sigil"
                        } else {
                            "a string literal file path"
                        };
                        emit_diag(
                            module_path,
                            out,
                            "E1518",
                            format!("`@static openapi.{}` expects {expected} as argument", field_name.unwrap_or("")),
                            original_span,
                        );
                        return;
                    }
                };
                match crate::surface::openapi::openapi_to_expr(&source, is_url, base_dir, &original_span, &def.name.name) {
                    Ok(result) => {
                        def.expr = result.expr;
                        extra_items.extend(result.items);
                    }
                    Err(err) => {
                        let code = if err.contains("parse") { "E1519" } else { "E1518" };
                        emit_diag(
                            module_path,
                            out,
                            code,
                            format!("`@static openapi.{}`: {err}", field_name.unwrap_or("")),
                            original_span,
                        );
                    }
                }
                return;
            }
            if base_name == Some("file") && matches!(field_name, Some("read" | "json" | "csv")) {
                let Some(Expr::Literal(Literal::String { text: rel, .. })) = args.first() else {
                    return;
                };
                // Try source-relative first, then fall back to CWD-relative (workspace root).
                let source_relative = base_dir.join(rel);
                let cwd_relative = std::path::PathBuf::from(rel);
                let (full_path, contents) = match std::fs::read_to_string(&source_relative) {
                    Ok(c) => (source_relative, c),
                    Err(_) => match std::fs::read_to_string(&cwd_relative) {
                        Ok(c) => (cwd_relative, c),
                        Err(err) => {
                            emit_diag(
                                module_path,
                                out,
                                "E1515",
                                format!(
                                    "`@static` failed to read {} (also tried {}): {}",
                                    source_relative.display(),
                                    cwd_relative.display(),
                                    err
                                ),
                                original_span,
                            );
                            return;
                        }
                    },
                };
                let _digest = {
                    use sha2::{Digest, Sha256};
                    let mut hasher = Sha256::new();
                    hasher.update(contents.as_bytes());
                    hasher.finalize()
                };
                match field_name {
                    Some("read") => {
                        def.expr = Expr::Literal(Literal::String {
                            text: contents,
                            span: original_span.clone(),
                        });
                    }
                    Some("json") => {
                        let parsed = match serde_json::from_str::<serde_json::Value>(&contents) {
                            Ok(value) => value,
                            Err(err) => {
                                emit_diag(
                                    module_path,
                                    out,
                                    "E1516",
                                    format!(
                                        "`@static` failed to parse JSON from {}: {}",
                                        full_path.display(),
                                        err
                                    ),
                                    original_span.clone(),
                                );
                                return;
                            }
                        };
                        def.expr = json_to_expr(&parsed, &original_span);
                    }
                    Some("csv") => match csv_to_expr(&contents, &original_span) {
                        Ok(expr) => def.expr = expr,
                        Err(err) => {
                            emit_diag(
                                module_path,
                                out,
                                "E1517",
                                format!(
                                    "`@static` failed to parse CSV from {}: {}",
                                    full_path.display(),
                                    err
                                ),
                                original_span.clone(),
                            );
                        }
                    },
                    _ => {}
                }
            }
            if base_name == Some("type") && field_name == Some("jsonSchema") {
                let type_name = match args.first() {
                    Some(Expr::Ident(name)) => name.name.clone(),
                    _ => {
                        emit_diag(
                            module_path,
                            out,
                            "E1554",
                            "`@static type.jsonSchema` expects a type name as argument".to_string(),
                            original_span.clone(),
                        );
                        return;
                    }
                };
                let type_expr = match type_aliases.get(&type_name) {
                    Some(te) => te.clone(),
                    None => {
                        emit_diag(
                            module_path,
                            out,
                            "E1555",
                            format!("`@static type.jsonSchema`: type `{type_name}` not found in this module"),
                            original_span.clone(),
                        );
                        return;
                    }
                };
                let schema = type_expr_to_openai_json_schema(&type_name, &type_expr, type_aliases);
                // Parse the JSON schema string into an AST record expression
                let json_val: serde_json::Value = match serde_json::from_str(&schema) {
                    Ok(v) => v,
                    Err(err) => {
                        emit_diag(
                            module_path,
                            out,
                            "E1556",
                            format!("`@static type.jsonSchema` internal error: {err}"),
                            original_span.clone(),
                        );
                        return;
                    }
                };
                def.expr = json_to_expr(&json_val, &original_span);
            }
        }
    }

    let mut diags = Vec::new();
    for module in modules {
        let module_path = module.path.clone();
        let base_dir = std::path::Path::new(&module_path)
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .to_path_buf();

        let mut static_sigs = std::collections::HashSet::<String>::new();
        let mut type_aliases = std::collections::HashMap::<String, TypeExpr>::new();
        for item in &module.items {
            match item {
                ModuleItem::TypeSig(sig) => {
                    if has_decorator(&sig.decorators, "static") {
                        static_sigs.insert(sig.name.name.clone());
                    }
                }
                ModuleItem::TypeAlias(ta) => {
                    type_aliases.insert(ta.name.name.clone(), ta.aliased.clone());
                }
                ModuleItem::TypeDecl(td) => {
                    // For ADTs, build a union via And (will render as enum or anyOf)
                    if !td.constructors.is_empty() {
                        let all_nullary = td.constructors.iter().all(|c| c.args.is_empty());
                        if all_nullary {
                            // Simple enum: Status = Active | Inactive
                            let span = td.span.clone();
                            let items: Vec<TypeExpr> = td.constructors.iter()
                                .map(|c| TypeExpr::Name(c.name.clone()))
                                .collect();
                            type_aliases.insert(td.name.name.clone(), TypeExpr::And { items, span });
                        }
                        // Non-nullary constructors are complex ADTs — skip for now
                    }
                }
                _ => {}
            }
        }
        // Also collect from domain items
        for item in &module.items {
            if let ModuleItem::DomainDecl(domain) = item {
                for di in &domain.items {
                    if let DomainItem::TypeAlias(td) = di {
                        // DomainItem::TypeAlias wraps TypeDecl (ADT, not alias)
                        let all_nullary = td.constructors.iter().all(|c| c.args.is_empty());
                        if all_nullary && !td.constructors.is_empty() {
                            let span = td.span.clone();
                            let items: Vec<TypeExpr> = td.constructors.iter()
                                .map(|c| TypeExpr::Name(c.name.clone()))
                                .collect();
                            type_aliases.insert(td.name.name.clone(), TypeExpr::And { items, span });
                        }
                    }
                }
            }
        }
        let mut extra_items: Vec<ModuleItem> = Vec::new();
        for item in &mut module.items {
            match item {
                ModuleItem::Def(def) => {
                    let is_static = has_decorator(&def.decorators, "static")
                        || static_sigs.contains(&def.name.name);
                    apply_static_to_def(&module_path, &base_dir, is_static, def, &mut diags, &mut extra_items, &type_aliases)
                }
                ModuleItem::InstanceDecl(instance) => {
                    for def in &mut instance.defs {
                        let is_static = has_decorator(&def.decorators, "static");
                        apply_static_to_def(&module_path, &base_dir, is_static, def, &mut diags, &mut extra_items, &type_aliases);
                    }
                }
                ModuleItem::DomainDecl(domain) => {
                    for domain_item in &mut domain.items {
                        match domain_item {
                            DomainItem::Def(def) | DomainItem::LiteralDef(def) => {
                                let is_static = has_decorator(&def.decorators, "static");
                                apply_static_to_def(
                                    &module_path,
                                    &base_dir,
                                    is_static,
                                    def,
                                    &mut diags,
                                    &mut extra_items,
                                    &type_aliases,
                                );
                            }
                            DomainItem::TypeAlias(_) | DomainItem::TypeSig(_) => {}
                        }
                    }
                }
                ModuleItem::TypeSig(_)
                | ModuleItem::TypeDecl(_)
                | ModuleItem::TypeAlias(_)
                | ModuleItem::ClassDecl(_)
                | ModuleItem::MachineDecl(_) => {}
            }
        }
        // Insert synthesised type items before existing items so they're
        // visible for name resolution of the binding that follows.
        if !extra_items.is_empty() {
            extra_items.append(&mut module.items);
            module.items = extra_items;
        }
    }
    diags
}

fn apply_native_decorators(modules: &mut [Module]) -> Vec<FileDiagnostic> {
    fn native_target(decorators: &[Decorator]) -> Option<String> {
        decorators.iter().find_map(|decorator| {
            if decorator.name.name != "native" {
                return None;
            }
            match decorator.arg.as_ref() {
                Some(Expr::Literal(Literal::String { text, .. })) => Some(text.clone()),
                _ => None,
            }
        })
    }

    fn emit_diag(
        module_path: &str,
        out: &mut Vec<FileDiagnostic>,
        code: &str,
        message: String,
        span: Span,
    ) {
        out.push(FileDiagnostic {
            path: module_path.to_string(),
            diagnostic: Diagnostic {
                code: code.to_string(),
                severity: DiagnosticSeverity::Error,
                message,
                span,
                labels: Vec::new(),
                hints: Vec::new(),
                suggestion: None,
            },
        });
    }

    /// Returns true if the `@native` path uses `::` (crate native) rather than `.` (runtime native).
    fn is_crate_native_path(path: &str) -> bool {
        path.contains("::")
    }

    /// Convert a crate-native path like `"quick_xml::de::from_str"` to
    /// a unique global name `"__crate_native__quick_xml__de__from_str"`.
    fn crate_native_global_name(path: &str) -> String {
        let sanitized = path.replace("::", "__").replace('-', "_");
        format!("__crate_native__{sanitized}")
    }

    fn native_target_expr(path: &str, span: &Span) -> Option<Expr> {
        // Crate native: `"crate::path::fn"` → single Ident with unique global name
        if is_crate_native_path(path) {
            let global_name = crate_native_global_name(path);
            return Some(Expr::Ident(SpannedName {
                name: global_name,
                span: span.clone(),
            }));
        }
        // Runtime native: `"module.function"` → field-access chain
        let mut segments = path.split('.').filter(|seg| !seg.is_empty());
        let first = segments.next()?;
        if !is_valid_ident(first) {
            return None;
        }
        let mut expr = Expr::Ident(SpannedName {
            name: first.to_string(),
            span: span.clone(),
        });
        for seg in segments {
            if !is_valid_ident(seg) {
                return None;
            }
            expr = Expr::FieldAccess {
                base: Box::new(expr),
                field: SpannedName {
                    name: seg.to_string(),
                    span: span.clone(),
                },
                span: span.clone(),
            };
        }
        Some(expr)
    }

    fn is_valid_ident(seg: &str) -> bool {
        let mut chars = seg.chars();
        let Some(first) = chars.next() else {
            return false;
        };
        (first.is_ascii_alphabetic() || first == '_')
            && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    }

    /// Validate that each `::` segment in a crate-native path is a valid Rust identifier.
    fn is_valid_crate_native_path(path: &str) -> bool {
        path.split("::").all(|seg| {
            !seg.is_empty()
                && seg
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
                && seg
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        })
    }

    fn apply_native_to_def(
        module_path: &str,
        type_sigs: &std::collections::HashSet<String>,
        target: Option<String>,
        allow_in_context: bool,
        def: &mut Def,
        out: &mut Vec<FileDiagnostic>,
    ) {
        let Some(path) = target else {
            return;
        };
        if !allow_in_context {
            emit_diag(
                module_path,
                out,
                "E1526",
                "`@native` is only supported on top-level module definitions".to_string(),
                def.span.clone(),
            );
            return;
        }
        if !type_sigs.contains(&def.name.name) {
            emit_diag(
                module_path,
                out,
                "E1526",
                format!(
                    "`@native` definition `{}` requires an explicit type signature",
                    def.name.name
                ),
                def.span.clone(),
            );
            return;
        }
        // Validate path syntax
        if is_crate_native_path(&path) && !is_valid_crate_native_path(&path) {
            emit_diag(
                module_path,
                out,
                "E1526",
                format!("`@native` crate path must be valid Rust identifiers separated by `::`, got `{path}`"),
                def.span.clone(),
            );
            return;
        }
        let Some(target_expr) = native_target_expr(&path, &def.span) else {
            emit_diag(
                module_path,
                out,
                "E1526",
                format!("`@native` target must be a dotted identifier path or crate::path, got `{path}`"),
                def.span.clone(),
            );
            return;
        };
        // Params come from the existing def body (if present) or from the auto-generated def.
        // Both crate natives (`::`) and runtime natives (`.`) auto-generate defs from the type sig.
        let params: Vec<Pattern> = if !def.params.is_empty() {
            def.params.clone()
        } else if let Expr::Lambda { params, .. } = def.expr.clone() {
            params
        } else {
            Vec::new()
        };
        let mut args = Vec::with_capacity(params.len());
        for param in &params {
            match param {
                Pattern::Ident(name) | Pattern::SubjectIdent(name) => {
                    args.push(Expr::Ident(name.clone()));
                }
                _ => {
                    emit_diag(
                        module_path,
                        out,
                        "E1526",
                        format!(
                            "`@native` definition `{}` only supports identifier parameters",
                            def.name.name
                        ),
                        def.span.clone(),
                    );
                    return;
                }
            }
        }
        def.expr = if args.is_empty() {
            target_expr
        } else {
            Expr::Call {
                func: Box::new(target_expr),
                args,
                span: def.span.clone(),
            }
        };
    }

    /// Count the number of parameters in a function type signature.
    /// e.g. `Text -> Int -> Bool` has arity 2, `Text` has arity 0.
    fn count_function_arity(ty: &TypeExpr) -> usize {
        match ty {
            TypeExpr::Func { params, result, .. } => {
                params.len() + count_function_arity(result)
            }
            _ => 0,
        }
    }

    let mut diags = Vec::new();
    for module in modules {
        let module_path = module.path.clone();
        let mut native_sigs = std::collections::HashSet::<String>::new();
        let mut native_sig_targets = std::collections::HashMap::<String, String>::new();
        for item in &module.items {
            let ModuleItem::TypeSig(sig) = item else {
                continue;
            };
            if let Some(path) = native_target(&sig.decorators) {
                native_sigs.insert(sig.name.name.clone());
                native_sig_targets.insert(sig.name.name.clone(), path);
            }
        }

        // Collect names of existing defs so we know which crate-native sigs need auto-generated defs
        let existing_def_names: std::collections::HashSet<String> = module
            .items
            .iter()
            .filter_map(|item| match item {
                ModuleItem::Def(def) => Some(def.name.name.clone()),
                _ => None,
            })
            .collect();

        // Auto-generate defs for @native type sigs that lack a corresponding def.
        // Neither crate natives (`::`) nor runtime natives (`.`) need a dummy body.
        let mut auto_defs: Vec<ModuleItem> = Vec::new();
        for (sig_name, target_path) in &native_sig_targets {
            if existing_def_names.contains(sig_name) {
                continue;
            }
            // Find the type sig to get its span and compute arity from the type
            let sig_item = module.items.iter().find(|item| {
                matches!(item, ModuleItem::TypeSig(sig) if sig.name.name == *sig_name)
            });
            let Some(ModuleItem::TypeSig(sig)) = sig_item else {
                continue;
            };
            let arity = count_function_arity(&sig.ty);
            let span = sig.span.clone();
            // Generate synthetic parameter names: __arg0, __arg1, ...
            let params: Vec<Pattern> = (0..arity)
                .map(|i| {
                    Pattern::Ident(SpannedName {
                        name: format!("__arg{i}"),
                        span: span.clone(),
                    })
                })
                .collect();
            let Some(target_expr) = native_target_expr(target_path, &span) else {
                continue;
            };
            let args: Vec<Expr> = params
                .iter()
                .map(|p| match p {
                    Pattern::Ident(name) => Expr::Ident(name.clone()),
                    _ => unreachable!(),
                })
                .collect();
            let body = if args.is_empty() {
                target_expr
            } else {
                Expr::Call {
                    func: Box::new(target_expr),
                    args,
                    span: span.clone(),
                }
            };
            auto_defs.push(ModuleItem::Def(Def {
                decorators: vec![Decorator {
                    name: SpannedName {
                        name: "native".to_string(),
                        span: span.clone(),
                    },
                    arg: Some(Expr::Literal(Literal::String {
                        text: target_path.clone(),
                        span: span.clone(),
                    })),
                    span: span.clone(),
                }],
                name: SpannedName {
                    name: sig_name.clone(),
                    span: span.clone(),
                },
                params,
                expr: body,
                span,
            }));
        }
        module.items.extend(auto_defs);

        for item in &mut module.items {
            match item {
                ModuleItem::Def(def) => {
                    let target = native_target(&def.decorators)
                        .or_else(|| native_sig_targets.get(&def.name.name).cloned());
                    apply_native_to_def(&module_path, &native_sigs, target, true, def, &mut diags)
                }
                ModuleItem::InstanceDecl(instance) => {
                    for def in &mut instance.defs {
                        let target = native_target(&def.decorators);
                        apply_native_to_def(
                            &module_path,
                            &native_sigs,
                            target,
                            false,
                            def,
                            &mut diags,
                        );
                    }
                }
                ModuleItem::DomainDecl(domain) => {
                    for domain_item in &mut domain.items {
                        match domain_item {
                            DomainItem::Def(def) | DomainItem::LiteralDef(def) => {
                                let target = native_target(&def.decorators);
                                apply_native_to_def(
                                    &module_path,
                                    &native_sigs,
                                    target,
                                    false,
                                    def,
                                    &mut diags,
                                );
                            }
                            DomainItem::TypeAlias(_) | DomainItem::TypeSig(_) => {}
                        }
                    }
                }
                ModuleItem::TypeSig(_)
                | ModuleItem::TypeDecl(_)
                | ModuleItem::TypeAlias(_)
                | ModuleItem::ClassDecl(_)
                | ModuleItem::MachineDecl(_) => {}
            }
        }
    }
    diags
}
fn spanned_name(name: &str, span: &Span) -> SpannedName {
    SpannedName {
        name: name.to_string(),
        span: span.clone(),
    }
}

fn json_to_expr(value: &serde_json::Value, span: &Span) -> Expr {
    match value {
        serde_json::Value::Null => Expr::Ident(spanned_name("None", span)),
        serde_json::Value::Bool(value) => Expr::Literal(Literal::Bool {
            value: *value,
            span: span.clone(),
        }),
        serde_json::Value::Number(number) => Expr::Literal(Literal::Number {
            text: number.to_string(),
            span: span.clone(),
        }),
        serde_json::Value::String(text) => Expr::Literal(Literal::String {
            text: text.clone(),
            span: span.clone(),
        }),
        serde_json::Value::Array(items) => Expr::List {
            items: items
                .iter()
                .map(|item| ListItem {
                    expr: json_to_expr(item, span),
                    spread: false,
                    span: span.clone(),
                })
                .collect(),
            span: span.clone(),
        },
        serde_json::Value::Object(object) => Expr::Record {
            fields: object
                .iter()
                .map(|(key, value)| RecordField {
                    spread: false,
                    path: vec![PathSegment::Field(spanned_name(key, span))],
                    value: json_to_expr(value, span),
                    span: span.clone(),
                })
                .collect(),
            span: span.clone(),
        },
    }
}

fn csv_to_expr(raw: &str, span: &Span) -> Result<Expr, csv::Error> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_reader(raw.as_bytes());
    let headers = reader
        .headers()?
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let mut items = Vec::new();
    for row in reader.records() {
        let row = row?;
        let mut fields = Vec::new();
        for (idx, value) in row.iter().enumerate() {
            let key = headers
                .get(idx)
                .cloned()
                .unwrap_or_else(|| format!("col{idx}"));
            fields.push(RecordField {
                spread: false,
                path: vec![PathSegment::Field(spanned_name(&key, span))],
                value: Expr::Literal(Literal::String {
                    text: value.to_string(),
                    span: span.clone(),
                }),
                span: span.clone(),
            });
        }
        items.push(ListItem {
            expr: Expr::Record {
                fields,
                span: span.clone(),
            },
            spread: false,
            span: span.clone(),
        });
    }
    Ok(Expr::List {
        items,
        span: span.clone(),
    })
}

fn type_expr_to_openai_json_schema(
    root_name: &str,
    type_expr: &TypeExpr,
    type_aliases: &std::collections::HashMap<String, TypeExpr>,
) -> String {
    fn te_to_schema(
        te: &TypeExpr,
        aliases: &std::collections::HashMap<String, TypeExpr>,
        seen: &mut std::collections::HashSet<String>,
    ) -> serde_json::Value {
        use serde_json::json;
        match te {
            TypeExpr::Name(name) => {
                let n = &name.name;
                match n.as_str() {
                    "Text" | "String" => json!({"type": "string"}),
                    "Int" => json!({"type": "integer"}),
                    "Float" => json!({"type": "number"}),
                    "Bool" => json!({"type": "boolean"}),
                    _ => {
                        if !seen.contains(n) {
                            if let Some(resolved) = aliases.get(n) {
                                seen.insert(n.clone());
                                let result = te_to_schema(resolved, aliases, seen);
                                seen.remove(n);
                                return result;
                            }
                        }
                        // Unresolved type — treat as string
                        json!({"type": "string"})
                    }
                }
            }
            TypeExpr::Record { fields, .. } => {
                let mut properties = serde_json::Map::new();
                let mut required = Vec::new();
                for (field_name, field_type) in fields {
                    let is_option = is_option_type(field_type);
                    let schema = if is_option {
                        te_to_schema(&unwrap_option(field_type), aliases, seen)
                    } else {
                        te_to_schema(field_type, aliases, seen)
                    };
                    properties.insert(field_name.name.clone(), schema);
                    if !is_option {
                        required.push(serde_json::Value::String(field_name.name.clone()));
                    }
                }
                let mut obj = serde_json::Map::new();
                obj.insert("type".to_string(), json!("object"));
                obj.insert("properties".to_string(), serde_json::Value::Object(properties));
                if !required.is_empty() {
                    obj.insert("required".to_string(), serde_json::Value::Array(required));
                }
                obj.insert("additionalProperties".to_string(), json!(false));
                serde_json::Value::Object(obj)
            }
            TypeExpr::Apply { base, args, .. } => {
                if let TypeExpr::Name(base_name) = base.as_ref() {
                    match base_name.name.as_str() {
                        "List" | "Array" => {
                            let item_schema = args.first()
                                .map(|a| te_to_schema(a, aliases, seen))
                                .unwrap_or(json!({}));
                            json!({"type": "array", "items": item_schema})
                        }
                        "Option" | "Maybe" => {
                            let inner = args.first()
                                .map(|a| te_to_schema(a, aliases, seen))
                                .unwrap_or(json!({}));
                            // For OpenAI structured output, Option types become nullable
                            let mut schema = inner.as_object().cloned().unwrap_or_default();
                            schema.insert("nullable".to_string(), json!(true));
                            serde_json::Value::Object(schema)
                        }
                        _ => {
                            if let Some(resolved) = aliases.get(&base_name.name) {
                                te_to_schema(resolved, aliases, seen)
                            } else {
                                json!({"type": "string"})
                            }
                        }
                    }
                } else {
                    json!({"type": "string"})
                }
            }
            TypeExpr::And { items, .. } => {
                // Union type — use anyOf
                let schemas: Vec<_> = items.iter().map(|i| te_to_schema(i, aliases, seen)).collect();
                if schemas.len() == 1 {
                    schemas.into_iter().next().expect("non-empty vec")
                } else {
                    // Check if all items are just names (enum-like ADT)
                    let all_names = items.iter().all(|i| matches!(i, TypeExpr::Name(_)));
                    if all_names {
                        let enum_vals: Vec<_> = items.iter().filter_map(|i| {
                            if let TypeExpr::Name(n) = i { Some(json!(camel_to_snake(&n.name))) } else { None }
                        }).collect();
                        json!({"type": "string", "enum": enum_vals})
                    } else {
                        json!({"anyOf": schemas})
                    }
                }
            }
            TypeExpr::Tuple { items, .. } => {
                let schemas: Vec<_> = items.iter().map(|i| te_to_schema(i, aliases, seen)).collect();
                json!({"type": "array", "prefixItems": schemas})
            }
            TypeExpr::Func { .. } | TypeExpr::Star { .. } | TypeExpr::Unknown { .. } => {
                json!({"type": "string"})
            }
        }
    }

    fn is_option_type(te: &TypeExpr) -> bool {
        if let TypeExpr::Apply { base, .. } = te {
            if let TypeExpr::Name(n) = base.as_ref() {
                return n.name == "Option" || n.name == "Maybe";
            }
        }
        false
    }

    fn unwrap_option(te: &TypeExpr) -> TypeExpr {
        if let TypeExpr::Apply { args, .. } = te {
            if let Some(inner) = args.first() {
                return inner.clone();
            }
        }
        te.clone()
    }

    let mut seen = std::collections::HashSet::new();
    let schema = te_to_schema(type_expr, type_aliases, &mut seen);

    // Wrap in OpenAI /chat/completions response_format structure
    let wrapper = serde_json::json!({
        "type": "json_schema",
        "json_schema": {
            "name": root_name,
            "schema": schema,
            "strict": true
        }
    });

    serde_json::to_string(&wrapper).unwrap_or_default()
}

/// Convert PascalCase/CamelCase to snake_case.
/// Handles acronyms: "USD" → "usd", "TwoFactorCode" → "two_factor_code"
fn camel_to_snake(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut result = String::new();
    for (i, &ch) in chars.iter().enumerate() {
        if ch.is_uppercase() {
            if i > 0 {
                let prev = chars[i - 1];
                let next_is_lower = chars.get(i + 1).map_or(false, |c| c.is_lowercase());
                // Insert underscore before:
                // - an uppercase letter following a lowercase letter (camelCase boundary)
                // - an uppercase letter followed by lowercase when previous was also uppercase (acronym end)
                if prev.is_lowercase() || (prev.is_uppercase() && next_is_lower) {
                    result.push('_');
                }
            }
            result.push(ch.to_lowercase().next().unwrap_or(ch));
        } else {
            result.push(ch);
        }
    }
    result
}
