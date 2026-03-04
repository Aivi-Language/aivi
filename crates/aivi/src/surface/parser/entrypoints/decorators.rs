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
                            span: original_span,
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
                                    original_span,
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
                                original_span,
                            );
                        }
                    },
                    _ => {}
                }
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
        for item in &module.items {
            let ModuleItem::TypeSig(sig) = item else {
                continue;
            };
            if has_decorator(&sig.decorators, "static") {
                static_sigs.insert(sig.name.name.clone());
            }
        }
        let mut extra_items: Vec<ModuleItem> = Vec::new();
        for item in &mut module.items {
            match item {
                ModuleItem::Def(def) => {
                    let is_static = has_decorator(&def.decorators, "static")
                        || static_sigs.contains(&def.name.name);
                    apply_static_to_def(&module_path, &base_dir, is_static, def, &mut diags, &mut extra_items)
                }
                ModuleItem::InstanceDecl(instance) => {
                    for def in &mut instance.defs {
                        let is_static = has_decorator(&def.decorators, "static");
                        apply_static_to_def(&module_path, &base_dir, is_static, def, &mut diags, &mut extra_items);
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
                    .map_or(false, |c| c.is_ascii_alphabetic() || c == '_')
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
        if is_crate_native_path(&path) {
            if !is_valid_crate_native_path(&path) {
                emit_diag(
                    module_path,
                    out,
                    "E1526",
                    format!("`@native` crate path must be valid Rust identifiers separated by `::`, got `{path}`"),
                    def.span.clone(),
                );
                return;
            }
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
        // For crate natives, params are derived from type sig — no dummy body needed.
        // For runtime natives, params come from the existing def body.
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
            TypeExpr::Func { params, .. } => params.len(),
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

        // Auto-generate defs for crate-native type sigs that lack a corresponding def.
        // For crate natives, no dummy body is required — only the type sig.
        let mut auto_defs: Vec<ModuleItem> = Vec::new();
        for (sig_name, target_path) in &native_sig_targets {
            if !is_crate_native_path(target_path) {
                continue;
            }
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
            let global_name = crate_native_global_name(target_path);
            let target_expr = Expr::Ident(SpannedName {
                name: global_name,
                span: span.clone(),
            });
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
