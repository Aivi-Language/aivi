use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Mutex as StdMutex;

#[cfg(test)]
use aivi::{check_modules, check_types_with_checkpoint};
use aivi::{embedded_stdlib_modules, infer_value_types, parse_modules, ModuleItem, ScopeItemKind};
use tower_lsp::lsp_types::{
    CodeAction, CodeActionKind, CodeActionOrCommand, Diagnostic, DiagnosticRelatedInformation,
    DiagnosticSeverity, Location, NumberOrString, Position, Range, TextEdit, Url, WorkspaceEdit,
};

use crate::backend::Backend;
use crate::state::IndexedModule;
use crate::strict::{build_strict_diagnostics, StrictConfig};

impl Backend {
    fn diagnostic_module_map(
        file_modules: &[aivi::Module],
        workspace_modules: &HashMap<String, IndexedModule>,
    ) -> HashMap<String, aivi::Module> {
        let mut module_map = HashMap::new();
        // Include embedded stdlib so imports/prelude/classes resolve for user code, but keep
        // embedded modules authoritative for `aivi.*` names.
        for module in embedded_stdlib_modules() {
            module_map.insert(module.name.name.clone(), module);
        }
        for indexed in workspace_modules.values() {
            let module_name = indexed.module.name.name.clone();
            if module_name.starts_with("aivi.") && module_map.contains_key(&module_name) {
                continue;
            }
            module_map.insert(module_name, indexed.module.clone());
        }
        for module in file_modules {
            module_map.insert(module.name.name.clone(), module.clone());
        }
        module_map
    }

    fn collect_transitive_modules_for_diagnostics(
        file_modules: &[aivi::Module],
        module_map: &HashMap<String, aivi::Module>,
    ) -> Vec<aivi::Module> {
        let mut queue: VecDeque<String> = VecDeque::new();
        let mut seen: HashSet<String> = HashSet::new();
        let mut out = Vec::new();

        for module in file_modules {
            let name = module.name.name.clone();
            if seen.insert(name.clone()) {
                queue.push_back(name);
            }
        }

        while let Some(module_name) = queue.pop_front() {
            let Some(module) = module_map.get(&module_name) else {
                continue;
            };
            out.push(module.clone());
            for use_decl in module.uses.iter() {
                let dep = use_decl.module.name.clone();
                if module_map.contains_key(&dep) && seen.insert(dep.clone()) {
                    queue.push_back(dep);
                }
            }
        }

        out
    }

    fn is_specs_snippet_path(path: &Path) -> bool {
        let mut comps = path.components().map(|c| c.as_os_str());
        while let Some(comp) = comps.next() {
            if comp == "specs" {
                return comps.any(|c| c == "snippets");
            }
        }
        false
    }

    fn collect_transitive_target_paths_for_diagnostics(
        path: &Path,
        file_modules: &[aivi::Module],
        workspace_modules: &HashMap<String, IndexedModule>,
    ) -> Vec<PathBuf> {
        let module_map = Self::diagnostic_module_map(file_modules, workspace_modules);
        let relevant_module_names = Self::collect_transitive_modules_for_diagnostics(
            file_modules,
            &module_map,
        )
        .into_iter()
        .map(|module| module.name.name)
        .collect::<HashSet<_>>();

        let mut target_paths: Vec<PathBuf> = workspace_modules
            .iter()
            .filter(|(module_name, _)| relevant_module_names.contains(*module_name))
            .filter_map(|(_, indexed)| indexed.uri.to_file_path().ok())
            .filter(|module_path| {
                module_path.extension().and_then(|ext| ext.to_str()) == Some("aivi")
            })
            .collect();
        if !target_paths.contains(&path.to_path_buf()) {
            target_paths.push(path.to_path_buf());
        }
        target_paths.sort();
        target_paths.dedup();
        target_paths
    }

    #[cfg(test)]
    pub(super) fn build_diagnostics(text: &str, uri: &Url) -> Vec<Diagnostic> {
        Self::build_diagnostics_with_workspace(
            text,
            uri,
            &HashMap::new(),
            false,
            &StrictConfig::default(),
            None,
            None,
        )
    }

    #[cfg(test)]
    pub(super) fn build_diagnostics_strict(
        text: &str,
        uri: &Url,
        strict: &StrictConfig,
    ) -> Vec<Diagnostic> {
        Self::build_diagnostics_with_workspace(
            text,
            uri,
            &HashMap::new(),
            false,
            strict,
            None,
            None,
        )
    }

    #[cfg(test)]
    pub(super) fn build_diagnostics_with_workspace(
        text: &str,
        uri: &Url,
        workspace_modules: &HashMap<String, IndexedModule>,
        include_specs_snippets: bool,
        strict: &StrictConfig,
        // Pre-parsed diagnostics from `update_document`. When `None`, the text is re-parsed
        // (used by tests which don't go through `update_document`).
        pre_parsed_diags: Option<Vec<aivi::FileDiagnostic>>,
        // Pre-built stdlib typecheck checkpoint. When `None`, falls back to `check_types`.
        typecheck_checkpoint: Option<&aivi::CheckTypesCheckpoint>,
    ) -> Vec<Diagnostic> {
        let path = PathBuf::from(Self::path_from_uri(uri));
        if !include_specs_snippets && Self::is_specs_snippet_path(&path) {
            // `specs/snippets/**/*.aivi` contains documentation fragments, not necessarily complete
            // modules. Avoid surfacing diagnostics as "nags" when authoring specs.
            return Vec::new();
        }

        // Phase 2: use pre-parsed file_modules from workspace + pre-computed parse_diags when
        // available; fall back to parse_modules for tests (empty workspace_modules).
        let (file_modules, parse_diags) = if let Some(diags) = pre_parsed_diags {
            let file_mods: Vec<aivi::Module> = workspace_modules
                .values()
                .filter(|im| &im.uri == uri)
                .map(|im| im.module.clone())
                .collect();
            (file_mods, diags)
        } else {
            parse_modules(&path, text)
        };

        // Always surface lex/parse diagnostics first; semantic checking on malformed syntax is
        // best-effort and must never crash the server.
        let mut out: Vec<Diagnostic> = parse_diags
            .into_iter()
            .map(|file_diag| Self::file_diag_to_lsp(uri, file_diag))
            .collect();

        // Build a module set for resolver + typechecker limited to the current file's transitive
        // dependency slice so unrelated workspace errors do not suppress local diagnostics.
        let module_map = Self::diagnostic_module_map(&file_modules, workspace_modules);
        let modules = Self::collect_transitive_modules_for_diagnostics(&file_modules, &module_map);

        let semantic_diags = std::panic::catch_unwind(|| {
            let mut diags = check_modules(&modules);
            // Phase 3: use pre-built stdlib checkpoint when available to skip setup_module
            // for 63 embedded stdlib modules.
            let type_diags = if let Some(cp) = typecheck_checkpoint {
                check_types_with_checkpoint(&modules, cp)
            } else {
                aivi::check_types(&modules)
            };
            diags.extend(type_diags);
            diags
        })
        .unwrap_or_default();

        for file_diag in semantic_diags {
            // LSP publishes per-document diagnostics; keep only the ones for this file.
            if file_diag.path != path {
                continue;
            }
            out.push(Self::file_diag_to_lsp(uri, file_diag));
        }

        // Strict-mode diagnostics are an additive overlay. They must not affect parsing,
        // name resolution, or typing; they only provide additional validation and quick fixes.
        out.extend(build_strict_diagnostics(
            text,
            uri,
            &path,
            strict,
            workspace_modules,
        ));
        out.extend(Self::build_source_tooling_diagnostics(&file_modules));

        out
    }

    pub(super) fn build_diagnostics_with_session(
        text: &str,
        uri: &Url,
        workspace_modules: &HashMap<String, IndexedModule>,
        include_specs_snippets: bool,
        strict: &StrictConfig,
        session: &StdMutex<aivi_driver::WorkspaceSession>,
    ) -> Vec<Diagnostic> {
        let path = PathBuf::from(Self::path_from_uri(uri));
        if !include_specs_snippets && Self::is_specs_snippet_path(&path) {
            return Vec::new();
        }

        let target_paths =
            Self::collect_transitive_target_paths_for_diagnostics(&path, &file_modules, workspace_modules);
        let assembly = match session.lock() {
            Ok(mut session) => {
                match session
                    .assemble_paths(&target_paths, aivi_driver::FrontendAssemblyMode::InferFast)
                {
                    Ok(assembly) => assembly,
                    Err(err) => {
                        return vec![Diagnostic {
                            range: Range::new(Position::new(0, 0), Self::end_position(text)),
                            severity: Some(DiagnosticSeverity::ERROR),
                            code: Some(NumberOrString::String("LSP-INTERNAL".to_string())),
                            code_description: None,
                            source: Some("aivi.lsp".to_string()),
                            message: format!("workspace assembly failed: {err}"),
                            related_information: None,
                            tags: None,
                            data: None,
                        }];
                    }
                }
            }
            Err(_) => {
                return vec![Diagnostic {
                    range: Range::new(Position::new(0, 0), Self::end_position(text)),
                    severity: Some(DiagnosticSeverity::ERROR),
                    code: Some(NumberOrString::String("LSP-INTERNAL".to_string())),
                    code_description: None,
                    source: Some("aivi.lsp".to_string()),
                    message: "workspace session lock poisoned".to_string(),
                    related_information: None,
                    tags: None,
                    data: None,
                }];
            }
        };

        let path_text = path.to_string_lossy().to_string();
        let file_modules = assembly
            .modules
            .iter()
            .filter(|module| module.path == path_text)
            .cloned()
            .collect::<Vec<_>>();

        let mut out = assembly
            .all_diagnostics()
            .into_iter()
            .filter(|file_diag| file_diag.path == path_text)
            .map(|file_diag| Self::file_diag_to_lsp(uri, file_diag))
            .collect::<Vec<_>>();
        out.extend(build_strict_diagnostics(
            text,
            uri,
            &path,
            strict,
            workspace_modules,
        ));
        out.extend(Self::build_source_tooling_diagnostics(&file_modules));
        out
    }

    fn build_source_tooling_diagnostics(modules: &[aivi::Module]) -> Vec<Diagnostic> {
        let mut out = Vec::new();

        for module in modules {
            let declared_types: HashSet<&str> = module
                .items
                .iter()
                .filter_map(|item| match item {
                    ModuleItem::TypeSig(sig) => Some(sig.name.name.as_str()),
                    _ => None,
                })
                .collect();

            for item in &module.items {
                let ModuleItem::Def(def) = item else {
                    continue;
                };

                if !declared_types.contains(def.name.name.as_str())
                    && Self::expr_contains_ident_path(&def.expr, "source.schema.derive")
                {
                    out.push(Self::source_tooling_hint(
                        def.name.span.clone(),
                        "LSP-SOURCE003",
                        "Schema-first declarations using `source.schema.derive` should have an explicit `Source ...` type signature so hover and schema diagnostics can describe the decode contract before `load`.",
                    ));
                }

                Self::collect_source_tooling_expr_diagnostics(&def.expr, &mut out);
            }
        }

        out
    }

    fn collect_source_tooling_expr_diagnostics(expr: &aivi::Expr, out: &mut Vec<Diagnostic>) {
        use aivi::{BlockItem, Expr, PathSegment, TextPart};

        match expr {
            Expr::Ident(_) | Expr::Literal(_) | Expr::FieldSection { .. } | Expr::Raw { .. } => {}
            Expr::UnaryNeg { expr, .. } | Expr::Suffixed { base: expr, .. } => {
                Self::collect_source_tooling_expr_diagnostics(expr, out);
            }
            Expr::TextInterpolate { parts, .. } => {
                for part in parts {
                    if let TextPart::Expr { expr, .. } = part {
                        Self::collect_source_tooling_expr_diagnostics(expr, out);
                    }
                }
            }
            Expr::List { items, .. } => {
                for item in items {
                    Self::collect_source_tooling_expr_diagnostics(&item.expr, out);
                }
            }
            Expr::Tuple { items, .. } => {
                for item in items {
                    Self::collect_source_tooling_expr_diagnostics(item, out);
                }
            }
            Expr::Record { fields, .. } | Expr::PatchLit { fields, .. } => {
                for field in fields {
                    Self::collect_source_tooling_expr_diagnostics(&field.value, out);
                }
            }
            Expr::FieldAccess { base, .. } => {
                Self::collect_source_tooling_expr_diagnostics(base, out);
            }
            Expr::Index { base, index, .. } => {
                Self::collect_source_tooling_expr_diagnostics(base, out);
                Self::collect_source_tooling_expr_diagnostics(index, out);
            }
            Expr::Call { func, args, span } => {
                if let Some(path) = Self::expr_ident_path(func) {
                    match path.as_str() {
                        "file.json" | "env.decode" => {
                            if let Some(arg) = args.first() {
                                match arg {
                                    Expr::Literal(aivi::Literal::String { .. }) => out.push(
                                        Self::source_tooling_hint(
                                            span.clone(),
                                            "LSP-SOURCE001",
                                            &format!(
                                                "Prefer the schema-first record form for `{path}` so tooling can surface the connector config and decode contract before `load`."
                                            ),
                                        ),
                                    ),
                                    Expr::Record { fields, .. } => {
                                        let has_schema = fields.iter().any(|field| {
                                            matches!(
                                                field.path.first(),
                                                Some(PathSegment::Field(name))
                                                    if name.name == "schema"
                                            )
                                        });
                                        if !has_schema {
                                            out.push(Self::source_tooling_hint(
                                                span.clone(),
                                                "LSP-SOURCE002",
                                                &format!(
                                                    "Add a `schema: source.schema.derive` (or another `source.schema.*` strategy) field to this `{path}` declaration to opt into schema-first hover and decode diagnostics."
                                                ),
                                            ));
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                        _ => {}
                    }
                }

                Self::collect_source_tooling_expr_diagnostics(func, out);
                for arg in args {
                    Self::collect_source_tooling_expr_diagnostics(arg, out);
                }
            }
            Expr::Lambda { body, .. } => {
                Self::collect_source_tooling_expr_diagnostics(body, out);
            }
            Expr::Match {
                scrutinee, arms, ..
            } => {
                if let Some(scrutinee) = scrutinee {
                    Self::collect_source_tooling_expr_diagnostics(scrutinee, out);
                }
                for arm in arms {
                    if let Some(guard) = &arm.guard {
                        Self::collect_source_tooling_expr_diagnostics(guard, out);
                    }
                    Self::collect_source_tooling_expr_diagnostics(&arm.body, out);
                }
            }
            Expr::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => {
                Self::collect_source_tooling_expr_diagnostics(cond, out);
                Self::collect_source_tooling_expr_diagnostics(then_branch, out);
                Self::collect_source_tooling_expr_diagnostics(else_branch, out);
            }
            Expr::Binary { left, right, .. } => {
                Self::collect_source_tooling_expr_diagnostics(left, out);
                Self::collect_source_tooling_expr_diagnostics(right, out);
            }
            Expr::Block { items, .. } => {
                for item in items {
                    match item {
                        BlockItem::Bind { expr, .. }
                        | BlockItem::Let { expr, .. }
                        | BlockItem::Filter { expr, .. }
                        | BlockItem::Yield { expr, .. }
                        | BlockItem::Recurse { expr, .. }
                        | BlockItem::Expr { expr, .. } => {
                            Self::collect_source_tooling_expr_diagnostics(expr, out);
                        }
                        BlockItem::When { cond, effect, .. }
                        | BlockItem::Unless { cond, effect, .. } => {
                            Self::collect_source_tooling_expr_diagnostics(cond, out);
                            Self::collect_source_tooling_expr_diagnostics(effect, out);
                        }
                        BlockItem::Given {
                            cond, fail_expr, ..
                        } => {
                            Self::collect_source_tooling_expr_diagnostics(cond, out);
                            Self::collect_source_tooling_expr_diagnostics(fail_expr, out);
                        }
                    }
                }
            }
            Expr::Mock {
                substitutions,
                body,
                ..
            } => {
                for substitution in substitutions {
                    if let Some(value) = &substitution.value {
                        Self::collect_source_tooling_expr_diagnostics(value, out);
                    }
                }
                Self::collect_source_tooling_expr_diagnostics(body, out);
            }
        }
    }

    fn expr_contains_ident_path(expr: &aivi::Expr, expected: &str) -> bool {
        use aivi::{BlockItem, Expr, TextPart};

        if Self::expr_ident_path(expr).as_deref() == Some(expected) {
            return true;
        }

        match expr {
            Expr::Ident(_) | Expr::Literal(_) | Expr::FieldSection { .. } | Expr::Raw { .. } => {
                false
            }
            Expr::UnaryNeg { expr, .. } | Expr::Suffixed { base: expr, .. } => {
                Self::expr_contains_ident_path(expr, expected)
            }
            Expr::TextInterpolate { parts, .. } => parts.iter().any(|part| match part {
                TextPart::Text { .. } => false,
                TextPart::Expr { expr, .. } => Self::expr_contains_ident_path(expr, expected),
            }),
            Expr::List { items, .. } => items
                .iter()
                .any(|item| Self::expr_contains_ident_path(&item.expr, expected)),
            Expr::Tuple { items, .. } => items
                .iter()
                .any(|item| Self::expr_contains_ident_path(item, expected)),
            Expr::Record { fields, .. } | Expr::PatchLit { fields, .. } => fields
                .iter()
                .any(|field| Self::expr_contains_ident_path(&field.value, expected)),
            Expr::FieldAccess { base, .. } => Self::expr_contains_ident_path(base, expected),
            Expr::Index { base, index, .. } => {
                Self::expr_contains_ident_path(base, expected)
                    || Self::expr_contains_ident_path(index, expected)
            }
            Expr::Call { func, args, .. } => {
                Self::expr_contains_ident_path(func, expected)
                    || args
                        .iter()
                        .any(|arg| Self::expr_contains_ident_path(arg, expected))
            }
            Expr::Lambda { body, .. } => Self::expr_contains_ident_path(body, expected),
            Expr::Match {
                scrutinee, arms, ..
            } => {
                scrutinee
                    .as_ref()
                    .is_some_and(|expr| Self::expr_contains_ident_path(expr, expected))
                    || arms.iter().any(|arm| {
                        arm.guard
                            .as_ref()
                            .is_some_and(|guard| Self::expr_contains_ident_path(guard, expected))
                            || Self::expr_contains_ident_path(&arm.body, expected)
                    })
            }
            Expr::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => {
                Self::expr_contains_ident_path(cond, expected)
                    || Self::expr_contains_ident_path(then_branch, expected)
                    || Self::expr_contains_ident_path(else_branch, expected)
            }
            Expr::Binary { left, right, .. } => {
                Self::expr_contains_ident_path(left, expected)
                    || Self::expr_contains_ident_path(right, expected)
            }
            Expr::Block { items, .. } => items.iter().any(|item| match item {
                BlockItem::Bind { expr, .. }
                | BlockItem::Let { expr, .. }
                | BlockItem::Filter { expr, .. }
                | BlockItem::Yield { expr, .. }
                | BlockItem::Recurse { expr, .. }
                | BlockItem::Expr { expr, .. } => Self::expr_contains_ident_path(expr, expected),
                BlockItem::When { cond, effect, .. } | BlockItem::Unless { cond, effect, .. } => {
                    Self::expr_contains_ident_path(cond, expected)
                        || Self::expr_contains_ident_path(effect, expected)
                }
                BlockItem::Given {
                    cond, fail_expr, ..
                } => {
                    Self::expr_contains_ident_path(cond, expected)
                        || Self::expr_contains_ident_path(fail_expr, expected)
                }
            }),
            Expr::Mock {
                substitutions,
                body,
                ..
            } => {
                substitutions.iter().any(|substitution| {
                    substitution
                        .value
                        .as_ref()
                        .is_some_and(|value| Self::expr_contains_ident_path(value, expected))
                }) || Self::expr_contains_ident_path(body, expected)
            }
        }
    }

    fn expr_ident_path(expr: &aivi::Expr) -> Option<String> {
        match expr {
            aivi::Expr::Ident(name) => Some(name.name.clone()),
            aivi::Expr::FieldAccess { base, field, .. } => {
                Some(format!("{}.{}", Self::expr_ident_path(base)?, field.name))
            }
            _ => None,
        }
    }

    fn source_tooling_hint(span: aivi::Span, code: &str, message: &str) -> Diagnostic {
        Diagnostic {
            range: Self::span_to_range(span),
            severity: Some(DiagnosticSeverity::HINT),
            code: Some(NumberOrString::String(code.to_string())),
            code_description: None,
            source: Some("aivi.source".to_string()),
            message: message.to_string(),
            related_information: None,
            tags: None,
            data: None,
        }
    }

    fn file_diag_to_lsp(uri: &Url, file_diag: aivi::FileDiagnostic) -> Diagnostic {
        let diagnostic = file_diag.diagnostic;
        let related_information = (!diagnostic.labels.is_empty()).then(|| {
            diagnostic
                .labels
                .iter()
                .map(|label| DiagnosticRelatedInformation {
                    location: Location {
                        uri: uri.clone(),
                        range: Self::span_to_range(label.span.clone()),
                    },
                    message: label.message.clone(),
                })
                .collect()
        });

        let code = diagnostic.code.clone();
        Diagnostic {
            range: Self::span_to_range(diagnostic.span.clone()),
            severity: Some(match diagnostic.severity {
                aivi::DiagnosticSeverity::Error => DiagnosticSeverity::ERROR,
                aivi::DiagnosticSeverity::Warning => DiagnosticSeverity::WARNING,
                aivi::DiagnosticSeverity::Hint => DiagnosticSeverity::HINT,
            }),
            code: Some(NumberOrString::String(code.clone())),
            code_description: None,
            source: Some(format!("aivi.{}", category_for_code(&code))),
            message: Self::lsp_message_for_compiler_diagnostic(&diagnostic),
            related_information,
            tags: None,
            data: Self::lsp_data_for_compiler_diagnostic(&diagnostic),
        }
    }

    fn lsp_message_for_compiler_diagnostic(diagnostic: &aivi::Diagnostic) -> String {
        if diagnostic.hints.is_empty() {
            return diagnostic.message.clone();
        }

        let mut message = diagnostic.message.clone();
        message.push_str("\n\nHelp:");
        for hint in &diagnostic.hints {
            message.push_str("\n- ");
            message.push_str(hint);
        }
        message
    }

    fn lsp_data_for_compiler_diagnostic(
        diagnostic: &aivi::Diagnostic,
    ) -> Option<serde_json::Value> {
        let mut data = serde_json::Map::new();

        if !diagnostic.hints.is_empty() {
            data.insert("aiviHints".to_string(), serde_json::json!(diagnostic.hints));
        }

        if let Some(suggestion) = &diagnostic.suggestion {
            data.insert(
                "aiviQuickFix".to_string(),
                serde_json::json!({
                    "title": suggestion.message,
                    "is_preferred": true,
                    "edits": [{
                        "range": Self::span_to_range(suggestion.span.clone()),
                        "newText": suggestion.replacement,
                    }],
                }),
            );
        }

        (!data.is_empty()).then_some(serde_json::Value::Object(data))
    }

    pub(super) fn end_position(text: &str) -> Position {
        let mut line = 0u32;
        let mut column = 0u32;
        for ch in text.chars() {
            if ch == '\n' {
                line += 1;
                column = 0;
            } else {
                column += 1;
            }
        }
        Position::new(line, column)
    }

    fn end_of_line_position(text: &str, line: u32) -> Position {
        let parts: Vec<&str> = text.split('\n').collect();
        let column = parts
            .get(line as usize)
            .map(|line| line.chars().count() as u32)
            .unwrap_or(0);
        Position::new(line, column)
    }

    fn closing_for(open: char) -> Option<char> {
        match open {
            '{' => Some('}'),
            '(' => Some(')'),
            '[' => Some(']'),
            _ => None,
        }
    }

    fn unclosed_open_delimiter(message: &str) -> Option<char> {
        let start = message.find('\'')?;
        let rest = &message[start + 1..];
        let mut chars = rest.chars();
        let open = chars.next()?;
        let end = chars.next()?;
        (end == '\'').then_some(open)
    }

    fn unknown_name_from_message(message: &str) -> Option<String> {
        // Compiler diagnostic format: "unknown name 'x'".
        let start = message.find('\'')?;
        let rest = &message[start + 1..];
        let end = rest.find('\'')?;
        let name = &rest[..end];
        if name.is_empty() {
            None
        } else {
            Some(name.to_string())
        }
    }

    fn import_insertion_position(text: &str) -> Position {
        // Modules are file-scoped and the `module` declaration must appear first (after optional
        // decorators). We insert after the last contiguous `use ...` line, or directly after the
        // module declaration when there are no uses.
        let lines: Vec<&str> = text.split('\n').collect();
        let mut i: usize = 0;

        // Skip leading empty lines and module decorators.
        while i < lines.len() {
            let trimmed = lines[i].trim();
            if trimmed.is_empty() || trimmed.starts_with('@') {
                i += 1;
                continue;
            }
            break;
        }

        // Find `module` line.
        if i >= lines.len() || !lines[i].trim_start().starts_with("module ") {
            // If we didn't find a module line, fall back to start of document.
            return Position::new(0, 0);
        }

        let module_line = i;
        let mut last_use_line: Option<usize> = None;
        i = module_line + 1;
        while i < lines.len() {
            let trimmed = lines[i].trim_start();
            if trimmed.starts_with("use ") {
                last_use_line = Some(i);
                i += 1;
                continue;
            }
            // Stop on the first non-use line (including a blank line).
            break;
        }

        let insert_line = last_use_line.map(|l| l + 1).unwrap_or(module_line + 1);
        Position::new(insert_line as u32, 0)
    }

    fn import_quickfixes_for_unknown_name(
        text: &str,
        uri: &Url,
        diagnostic: &Diagnostic,
        workspace_modules: &HashMap<String, IndexedModule>,
    ) -> Vec<CodeActionOrCommand> {
        let Some(name) = Self::unknown_name_from_message(&diagnostic.message) else {
            return Vec::new();
        };

        let mut providers: Vec<String> = Vec::new();
        for (module_name, indexed) in workspace_modules {
            if indexed
                .module
                .exports
                .iter()
                .any(|e| matches!(e.kind, aivi::ScopeItemKind::Value) && e.name.name == name)
            {
                providers.push(module_name.clone());
            }
        }

        providers.sort();
        providers.dedup();

        // Heuristic: keep the list small to avoid spamming the user.
        const MAX_ACTIONS: usize = 8;
        if providers.is_empty() {
            return Vec::new();
        }

        let insert_at = Self::import_insertion_position(text);
        let range = Range::new(insert_at, insert_at);

        let mut out = Vec::new();
        let preferred = providers.len() == 1;
        for (idx, module_name) in providers.into_iter().take(MAX_ACTIONS).enumerate() {
            let title = format!("Add `use {module_name} ({name})`");
            let edit = TextEdit {
                range,
                new_text: format!("use {module_name} ({name})\n"),
            };
            out.push(CodeActionOrCommand::CodeAction(CodeAction {
                title,
                kind: Some(CodeActionKind::QUICKFIX),
                diagnostics: Some(vec![diagnostic.clone()]),
                edit: Some(WorkspaceEdit {
                    changes: Some(HashMap::from([(uri.clone(), vec![edit])])),
                    document_changes: None,
                    change_annotations: None,
                }),
                command: None,
                is_preferred: Some(preferred && idx == 0),
                disabled: None,
                data: None,
            }));
        }
        out
    }

    pub(super) fn build_code_actions_with_workspace(
        text: &str,
        uri: &Url,
        diagnostics: &[Diagnostic],
        workspace_modules: &HashMap<String, IndexedModule>,
        cursor_range: Range,
    ) -> Vec<CodeActionOrCommand> {
        // Parse once up front — avoids re-parsing per diagnostic or per helper call.
        let path = PathBuf::from(Self::path_from_uri(uri));
        let (file_modules, _) = parse_modules(&path, text);
        let module = file_modules.first();

        let mut out = Vec::new();

        // Position-based refactoring actions (not diagnostic-driven).
        if let Some(module) = module {
            out.extend(Self::add_type_annotation_actions(
                text,
                uri,
                cursor_range,
                workspace_modules,
                &file_modules,
                module,
            ));
        }

        // Batch source action: remove every unused import in the file.
        let unused_import_diags: Vec<&Diagnostic> = diagnostics
            .iter()
            .filter(|d| {
                matches!(
                    &d.code,
                    Some(NumberOrString::String(c)) if c == "W2100"
                )
            })
            .collect();
        if unused_import_diags.len() > 1 {
            if let Some(module) = module {
                if let Some(batch) =
                    Self::remove_all_unused_imports(text, uri, &unused_import_diags, module)
                {
                    out.push(batch);
                }
            }
        }

        for diagnostic in diagnostics {
            // Generic strict-mode (and future) quickfix embedding: Diagnostics may carry a
            // serialized `TextEdit` list in `Diagnostic.data`.
            if let Some(actions) = quickfixes_from_diagnostic_data(uri, diagnostic) {
                out.extend(actions);
            }

            let code = match diagnostic.code.as_ref() {
                Some(NumberOrString::String(code)) => code.as_str(),
                Some(NumberOrString::Number(_)) => continue,
                None => continue,
            };

            match code {
                "E3000" | "E2005" => {
                    out.extend(Self::import_quickfixes_for_unknown_name(
                        text,
                        uri,
                        diagnostic,
                        workspace_modules,
                    ));
                }
                "W2100" => {
                    if let Some(module) = module {
                        if let Some(action) =
                            Self::remove_unused_import_quickfix(text, uri, diagnostic, module)
                        {
                            out.push(action);
                        }
                    }
                }
                "E1004" => {
                    let Some(open) = Self::unclosed_open_delimiter(&diagnostic.message) else {
                        continue;
                    };
                    let Some(close) = Self::closing_for(open) else {
                        continue;
                    };
                    let position = Self::end_position(text);
                    let range = Range::new(position, position);
                    let edit = TextEdit {
                        range,
                        new_text: close.to_string(),
                    };
                    out.push(CodeActionOrCommand::CodeAction(CodeAction {
                        title: format!("Insert missing '{close}'"),
                        kind: Some(CodeActionKind::QUICKFIX),
                        diagnostics: Some(vec![diagnostic.clone()]),
                        edit: Some(WorkspaceEdit {
                            changes: Some(HashMap::from([(uri.clone(), vec![edit])])),
                            document_changes: None,
                            change_annotations: None,
                        }),
                        command: None,
                        is_preferred: Some(true),
                        disabled: None,
                        data: None,
                    }));
                }
                "E1002" => {
                    out.push(CodeActionOrCommand::CodeAction(CodeAction {
                        title: "Remove unmatched closing delimiter".to_string(),
                        kind: Some(CodeActionKind::QUICKFIX),
                        diagnostics: Some(vec![diagnostic.clone()]),
                        edit: Some(WorkspaceEdit {
                            changes: Some(HashMap::from([(
                                uri.clone(),
                                vec![TextEdit {
                                    range: diagnostic.range,
                                    new_text: String::new(),
                                }],
                            )])),
                            document_changes: None,
                            change_annotations: None,
                        }),
                        command: None,
                        is_preferred: Some(true),
                        disabled: None,
                        data: None,
                    }));
                }
                "E1001" => {
                    let position = Self::end_of_line_position(text, diagnostic.range.end.line);
                    let range = Range::new(position, position);
                    out.push(CodeActionOrCommand::CodeAction(CodeAction {
                        title: "Insert missing closing quote".to_string(),
                        kind: Some(CodeActionKind::QUICKFIX),
                        diagnostics: Some(vec![diagnostic.clone()]),
                        edit: Some(WorkspaceEdit {
                            changes: Some(HashMap::from([(
                                uri.clone(),
                                vec![TextEdit {
                                    range,
                                    new_text: "\"".to_string(),
                                }],
                            )])),
                            document_changes: None,
                            change_annotations: None,
                        }),
                        command: None,
                        is_preferred: Some(true),
                        disabled: None,
                        data: None,
                    }));
                }
                "E3100" => {
                    // Non-exhaustive match: extract missing constructors from message and
                    // generate skeleton arms.
                    if let Some(action) =
                        Self::add_missing_match_cases_action(text, uri, diagnostic)
                    {
                        out.push(action);
                    }
                }
                _ => {}
            }
        }
        out
    }

    /// Refactoring action: offer to insert an inferred type annotation above a top-level
    /// `Def` that currently lacks one, when the cursor is on (or near) the definition.
    fn add_type_annotation_actions(
        _text: &str,
        uri: &Url,
        cursor_range: Range,
        workspace_modules: &HashMap<String, IndexedModule>,
        file_modules: &[aivi::Module],
        module: &aivi::Module,
    ) -> Vec<CodeActionOrCommand> {
        // Names that already have an explicit type signature in this module.
        let has_type_sig: HashSet<&str> = module
            .items
            .iter()
            .filter_map(|item| match item {
                ModuleItem::TypeSig(sig) => Some(sig.name.name.as_str()),
                _ => None,
            })
            .collect();

        // Find a Def whose name line matches the cursor line and has no type sig.
        let cursor_line = cursor_range.start.line; // 0-based LSP line
        let target_def = module.items.iter().find_map(|item| match item {
            ModuleItem::Def(def) => {
                // span lines are 1-based; convert to 0-based for comparison.
                let def_line = def.name.span.start.line.saturating_sub(1) as u32;
                let first_line = if def.decorators.is_empty() {
                    def_line
                } else {
                    def.decorators
                        .iter()
                        .map(|d| d.span.start.line.saturating_sub(1) as u32)
                        .min()
                        .unwrap_or(def_line)
                };
                // Accept if cursor is anywhere between first decorator and def name.
                if cursor_line >= first_line
                    && cursor_line <= def_line
                    && !has_type_sig.contains(def.name.name.as_str())
                {
                    Some(def)
                } else {
                    None
                }
            }
            _ => None,
        });

        let Some(def) = target_def else {
            return Vec::new();
        };

        // Build a module map limited to transitively reachable modules (mirrors the diagnostics
        // path) so type inference is scoped rather than running over the whole workspace.
        let module_map = Self::diagnostic_module_map(file_modules, workspace_modules);
        // Use transitive collection so inference runs only over needed modules.
        let modules_for_infer =
            Self::collect_transitive_modules_for_diagnostics(file_modules, &module_map);

        let (_, type_strings, _) =
            std::panic::catch_unwind(|| infer_value_types(&modules_for_infer)).unwrap_or_default();

        let Some(module_types) = type_strings.get(&module.name.name) else {
            return Vec::new();
        };
        let Some(inferred_type) = module_types.get(&def.name.name) else {
            return Vec::new();
        };

        // Insert the type annotation on the line before the first decorator (or before the def).
        let insert_line = if def.decorators.is_empty() {
            def.name.span.start.line.saturating_sub(1) as u32
        } else {
            def.decorators
                .iter()
                .map(|d| d.span.start.line.saturating_sub(1) as u32)
                .min()
                .unwrap_or(def.name.span.start.line.saturating_sub(1) as u32)
        };
        let insert_pos = Position::new(insert_line, 0);
        let insert_range = Range::new(insert_pos, insert_pos);

        vec![CodeActionOrCommand::CodeAction(CodeAction {
            title: format!("Add type annotation for '{}'", def.name.name),
            kind: Some(CodeActionKind::REFACTOR_REWRITE),
            diagnostics: None,
            edit: Some(WorkspaceEdit {
                changes: Some(HashMap::from([(
                    uri.clone(),
                    vec![TextEdit {
                        range: insert_range,
                        new_text: format!("{} : {}\n", def.name.name, inferred_type),
                    }],
                )])),
                document_changes: None,
                change_annotations: None,
            }),
            command: None,
            is_preferred: None,
            disabled: None,
            data: None,
        })]
    }

    /// QuickFix action: remove a single unused import name from its `use` declaration.
    ///
    /// For `W2100` diagnostics. When the import list has a single item, the whole
    /// `use` line is removed. When there are multiple items, only the unused name is
    /// stripped and the remaining `use` line is reconstructed.
    fn remove_unused_import_quickfix(
        text: &str,
        uri: &Url,
        diagnostic: &Diagnostic,
        module: &aivi::Module,
    ) -> Option<CodeActionOrCommand> {
        let diag_start = diagnostic.range.start;

        // Find the use_decl that contains the unused import item at the diagnostic position.
        let (use_decl, item_name) = module.uses.iter().find_map(|use_decl| {
            use_decl.items.iter().find_map(|item| {
                let item_range = Self::span_to_range(item.name.span.clone());
                if item_range.start.line == diag_start.line
                    && item_range.start.character == diag_start.character
                {
                    Some((use_decl, item.name.name.clone()))
                } else {
                    None
                }
            })
        })?;

        // The use declaration starts on the line of the `use` keyword.
        // `use_decl.span.start.line` is 1-based; convert to 0-based.
        let use_line = use_decl.span.start.line.saturating_sub(1) as u32;
        let lines: Vec<&str> = text.split('\n').collect();
        let line_len = lines.get(use_line as usize).map_or(0, |l| l.len() as u32);

        let (new_text, replace_range) = if use_decl.items.len() == 1 {
            // Only one import: remove the entire line (including the newline).
            let range = Range::new(Position::new(use_line, 0), Position::new(use_line + 1, 0));
            (String::new(), range)
        } else {
            // Multiple imports: reconstruct the use line without the unused name.
            let remaining: Vec<String> = use_decl
                .items
                .iter()
                .filter(|it| it.name.name != item_name)
                .map(|it| {
                    if it.kind == ScopeItemKind::Domain {
                        format!("domain {}", it.name.name)
                    } else {
                        it.name.name.clone()
                    }
                })
                .collect();
            let alias_part = use_decl
                .alias
                .as_ref()
                .map(|a| format!(" as {}", a.name))
                .unwrap_or_default();
            let new_line = format!(
                "use {} ({}){}",
                use_decl.module.name,
                remaining.join(", "),
                alias_part,
            );
            let range = Range::new(
                Position::new(use_line, 0),
                Position::new(use_line, line_len),
            );
            (new_line, range)
        };

        Some(CodeActionOrCommand::CodeAction(CodeAction {
            title: format!("Remove unused import '{item_name}'"),
            kind: Some(CodeActionKind::QUICKFIX),
            diagnostics: Some(vec![diagnostic.clone()]),
            edit: Some(WorkspaceEdit {
                changes: Some(HashMap::from([(
                    uri.clone(),
                    vec![TextEdit {
                        range: replace_range,
                        new_text,
                    }],
                )])),
                document_changes: None,
                change_annotations: None,
            }),
            command: None,
            is_preferred: Some(true),
            disabled: None,
            data: None,
        }))
    }

    /// Source action: remove all unused imports in the file in a single edit.
    fn remove_all_unused_imports(
        text: &str,
        uri: &Url,
        unused_diags: &[&Diagnostic],
        module: &aivi::Module,
    ) -> Option<CodeActionOrCommand> {
        // Collect all unused import names from diagnostics.
        let unused_names: HashSet<String> = unused_diags
            .iter()
            .filter_map(|d| Self::unknown_name_from_message(&d.message))
            .collect();

        let lines: Vec<&str> = text.split('\n').collect();
        let mut edits: Vec<TextEdit> = Vec::new();

        for use_decl in &module.uses {
            if use_decl.wildcard {
                continue;
            }
            let unused_in_decl: Vec<&str> = use_decl
                .items
                .iter()
                .filter(|it| unused_names.contains(&it.name.name))
                .map(|it| it.name.name.as_str())
                .collect();
            if unused_in_decl.is_empty() {
                continue;
            }
            let use_line = use_decl.span.start.line.saturating_sub(1) as u32;
            let line_len = lines.get(use_line as usize).map_or(0, |l| l.len() as u32);

            let remaining: Vec<String> = use_decl
                .items
                .iter()
                .filter(|it| !unused_names.contains(&it.name.name))
                .map(|it| {
                    if it.kind == ScopeItemKind::Domain {
                        format!("domain {}", it.name.name)
                    } else {
                        it.name.name.clone()
                    }
                })
                .collect();

            if remaining.is_empty() {
                // Remove the whole line.
                edits.push(TextEdit {
                    range: Range::new(Position::new(use_line, 0), Position::new(use_line + 1, 0)),
                    new_text: String::new(),
                });
            } else {
                let alias_part = use_decl
                    .alias
                    .as_ref()
                    .map(|a| format!(" as {}", a.name))
                    .unwrap_or_default();
                edits.push(TextEdit {
                    range: Range::new(
                        Position::new(use_line, 0),
                        Position::new(use_line, line_len),
                    ),
                    new_text: format!(
                        "use {} ({}){}",
                        use_decl.module.name,
                        remaining.join(", "),
                        alias_part,
                    ),
                });
            }
        }

        if edits.is_empty() {
            return None;
        }

        Some(CodeActionOrCommand::CodeAction(CodeAction {
            title: "Remove all unused imports".to_string(),
            kind: Some(CodeActionKind::SOURCE),
            diagnostics: None,
            edit: Some(WorkspaceEdit {
                changes: Some(HashMap::from([(uri.clone(), edits)])),
                document_changes: None,
                change_annotations: None,
            }),
            command: None,
            is_preferred: None,
            disabled: None,
            data: None,
        }))
    }

    /// Code action for E3100: insert missing match arms based on the diagnostic message.
    fn add_missing_match_cases_action(
        text: &str,
        uri: &Url,
        diagnostic: &Diagnostic,
    ) -> Option<CodeActionOrCommand> {
        // Parse missing constructors from "non-exhaustive match (missing: Foo, Bar, Baz)"
        let msg = &diagnostic.message;
        let start_marker = "missing: ";
        let start_idx = msg.find(start_marker)?;
        let after = &msg[start_idx + start_marker.len()..];
        let end_idx = after.find(')')?;
        let missing_str = &after[..end_idx];
        let missing: Vec<&str> = missing_str.split(", ").map(|s| s.trim()).collect();
        if missing.is_empty() {
            return None;
        }

        // Determine indentation from the match expression location.
        let match_line = diagnostic.range.end.line as usize;
        let indent = Self::line_indent(text, match_line);
        let arm_indent = format!("{indent}  ");

        // Build the arms text.
        let mut arms_text = String::new();
        for ctor in &missing {
            arms_text.push_str(&format!("\n{arm_indent}| {ctor} => _"));
        }

        // Insert at the end of the match expression (end of diagnostic range).
        let insert_pos = diagnostic.range.end;
        let range = Range::new(insert_pos, insert_pos);

        Some(CodeActionOrCommand::CodeAction(CodeAction {
            title: format!("Add missing match cases ({})", missing.join(", ")),
            kind: Some(CodeActionKind::QUICKFIX),
            diagnostics: Some(vec![diagnostic.clone()]),
            edit: Some(WorkspaceEdit {
                changes: Some(HashMap::from([(
                    uri.clone(),
                    vec![TextEdit {
                        range,
                        new_text: arms_text,
                    }],
                )])),
                document_changes: None,
                change_annotations: None,
            }),
            command: None,
            is_preferred: Some(true),
            disabled: None,
            data: None,
        }))
    }

    fn line_indent(text: &str, line_0based: usize) -> String {
        text.lines()
            .nth(line_0based)
            .map(|l| {
                let trimmed = l.trim_start();
                l[..l.len() - trimmed.len()].to_string()
            })
            .unwrap_or_default()
    }
}

fn category_for_code(code: &str) -> &'static str {
    // Keep the mapping coarse and stable; strict-mode uses its own source.
    if code.starts_with('E') {
        match &code.get(1..2) {
            Some("1") => "Syntax",
            Some("2") => "NameResolution",
            Some("3") => "Type",
            _ => "Syntax",
        }
    } else if code.starts_with('W') {
        "Style"
    } else if code.starts_with("AIVI-S") {
        "Strict"
    } else {
        "Syntax"
    }
}

fn quickfixes_from_diagnostic_data(
    uri: &Url,
    diagnostic: &Diagnostic,
) -> Option<Vec<CodeActionOrCommand>> {
    let data = diagnostic.data.as_ref()?;
    let obj = data.as_object()?;
    let fix = obj.get("aiviQuickFix")?;

    #[derive(serde::Deserialize)]
    struct FixPayload {
        title: String,
        #[serde(default)]
        is_preferred: bool,
        edits: Vec<TextEdit>,
    }

    let payload: FixPayload = serde_json::from_value::<FixPayload>(fix.clone()).ok()?;
    if payload.edits.is_empty() {
        return None;
    }

    Some(vec![CodeActionOrCommand::CodeAction(CodeAction {
        title: payload.title,
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(vec![diagnostic.clone()]),
        edit: Some(WorkspaceEdit {
            changes: Some(HashMap::from([(uri.clone(), payload.edits)])),
            document_changes: None,
            change_annotations: None,
        }),
        command: None,
        is_preferred: Some(payload.is_preferred),
        disabled: None,
        data: None,
    })])
}
