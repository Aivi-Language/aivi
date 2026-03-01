use std::collections::HashMap;
use std::path::PathBuf;

use aivi::{
    infer_value_types, parse_modules, BlockItem, Def, DomainItem, Expr, Literal, Module,
    ModuleItem, Pattern,
};
use tower_lsp::lsp_types::{
    Documentation, MarkupContent, MarkupKind, ParameterInformation, ParameterLabel, Position,
    SignatureHelp, SignatureInformation, Url,
};

use crate::backend::Backend;
use crate::state::IndexedModule;

struct CallInfo<'a> {
    func: &'a Expr,
    active_parameter: usize,
}

impl Backend {
    pub(super) fn build_signature_help_with_workspace(
        text: &str,
        uri: &Url,
        position: Position,
        workspace_modules: &HashMap<String, IndexedModule>,
    ) -> Option<SignatureHelp> {
        let path = PathBuf::from(Self::path_from_uri(uri));
        let (modules, _) = parse_modules(&path, text);
        let current_module = Self::module_at_position(&modules, position)?;

        // Only infer types for the current file's modules + direct imports to
        // keep signature help responsive in large projects.
        let relevant_modules =
            Self::collect_relevant_modules(&modules, current_module, workspace_modules);
        let (_, inferred, _) = infer_value_types(&relevant_modules);

        let call = current_module
            .items
            .iter()
            .find_map(|item| Self::call_info_in_item(item, position))?;

        let callee_name = Self::callee_ident_name(call.func)?;
        let signature_label = Self::resolve_type_signature_label(
            current_module,
            &callee_name,
            workspace_modules,
            &inferred,
        )?;

        // Extract parameter names from the function definition.
        let param_names =
            Self::resolve_param_names(current_module, &callee_name, workspace_modules);

        // Build ParameterInformation from type signature parts.
        let parameters = Self::build_parameter_info(&signature_label, &param_names);

        // Look up documentation from the doc index (via doc comment above def).
        let doc = Self::find_def_doc_comment(current_module, &callee_name, workspace_modules);

        Some(SignatureHelp {
            signatures: vec![SignatureInformation {
                label: signature_label,
                documentation: doc.map(|d| {
                    Documentation::MarkupContent(MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: d,
                    })
                }),
                parameters: if parameters.is_empty() {
                    None
                } else {
                    Some(parameters)
                },
                active_parameter: Some(call.active_parameter as u32),
            }],
            active_signature: Some(0),
            active_parameter: Some(call.active_parameter as u32),
        })
    }

    fn resolve_type_signature_label(
        current_module: &Module,
        ident: &str,
        workspace_modules: &HashMap<String, IndexedModule>,
        inferred: &HashMap<String, HashMap<String, String>>,
    ) -> Option<String> {
        if let Some(label) = Self::type_signature_label_in_module(current_module, ident) {
            return Some(label);
        }
        if let Some(label) =
            Self::inferred_signature_label(&current_module.name.name, ident, inferred)
        {
            return Some(label);
        }

        for use_decl in current_module.uses.iter() {
            let imported =
                use_decl.wildcard || use_decl.items.iter().any(|item| item.name.name == ident);
            if !imported {
                continue;
            }
            let Some(indexed) = workspace_modules.get(&use_decl.module.name) else {
                continue;
            };
            if let Some(label) = Self::type_signature_label_in_module(&indexed.module, ident) {
                return Some(label);
            }
            if let Some(label) =
                Self::inferred_signature_label(&indexed.module.name.name, ident, inferred)
            {
                return Some(label);
            }
        }

        None
    }

    fn inferred_signature_label(
        module_name: &str,
        ident: &str,
        inferred: &HashMap<String, HashMap<String, String>>,
    ) -> Option<String> {
        inferred
            .get(module_name)
            .and_then(|types| types.get(ident))
            .map(|ty| format!("`{ident}` : `{ty}`"))
    }

    fn type_signature_label_in_module(module: &Module, ident: &str) -> Option<String> {
        for item in module.items.iter() {
            if let ModuleItem::TypeSig(sig) = item {
                if sig.name.name == ident {
                    return Some(format!(
                        "`{}` : `{}`",
                        sig.name.name,
                        Self::type_expr_to_string(&sig.ty)
                    ));
                }
            }
        }
        None
    }

    fn call_info_in_item(item: &ModuleItem, position: Position) -> Option<CallInfo<'_>> {
        match item {
            ModuleItem::Def(def) => Self::find_call_info(&def.expr, position),
            ModuleItem::InstanceDecl(instance_decl) => instance_decl
                .defs
                .iter()
                .find_map(|def| Self::find_call_info(&def.expr, position)),
            ModuleItem::DomainDecl(domain_decl) => {
                domain_decl
                    .items
                    .iter()
                    .find_map(|domain_item| match domain_item {
                        DomainItem::Def(def) | DomainItem::LiteralDef(def) => {
                            Self::find_call_info(&def.expr, position)
                        }
                        DomainItem::TypeAlias(_) | DomainItem::TypeSig(_) => None,
                    })
            }
            _ => None,
        }
    }

    fn callee_ident_name(expr: &Expr) -> Option<String> {
        match expr {
            Expr::Ident(name) => Some(name.name.clone()),
            Expr::FieldAccess { field, .. } => Some(field.name.clone()),
            _ => None,
        }
    }

    fn find_call_info(expr: &Expr, position: Position) -> Option<CallInfo<'_>> {
        if !Self::expr_contains_position(expr, position) {
            return None;
        }

        match expr {
            Expr::Call {
                func,
                args,
                span: _,
            } => {
                if let Some(inner) = Self::find_call_info(func, position) {
                    return Some(inner);
                }
                for arg in args.iter() {
                    if let Some(inner) = Self::find_call_info(arg, position) {
                        return Some(inner);
                    }
                }
                let active_parameter = Self::active_call_parameter(args, position);
                Some(CallInfo {
                    func: func.as_ref(),
                    active_parameter,
                })
            }
            Expr::List { items, .. } => items
                .iter()
                .find_map(|item| Self::find_call_info(&item.expr, position)),
            Expr::UnaryNeg { expr, .. } => Self::find_call_info(expr, position),
            Expr::Suffixed { base, .. } => Self::find_call_info(base, position),
            Expr::TextInterpolate { parts, .. } => parts.iter().find_map(|part| match part {
                aivi::TextPart::Text { .. } => None,
                aivi::TextPart::Expr { expr, .. } => Self::find_call_info(expr, position),
            }),
            Expr::Tuple { items, .. } => items
                .iter()
                .find_map(|item| Self::find_call_info(item, position)),
            Expr::Record { fields, .. } => fields
                .iter()
                .find_map(|field| Self::find_call_info(&field.value, position)),
            Expr::PatchLit { fields, .. } => fields
                .iter()
                .find_map(|field| Self::find_call_info(&field.value, position)),
            Expr::FieldAccess { base, .. } => Self::find_call_info(base, position),
            Expr::Index { base, index, .. } => Self::find_call_info(base, position)
                .or_else(|| Self::find_call_info(index, position)),
            Expr::Lambda { body, .. } => Self::find_call_info(body, position),
            Expr::Match {
                scrutinee, arms, ..
            } => {
                if let Some(scrutinee) = scrutinee {
                    if let Some(inner) = Self::find_call_info(scrutinee, position) {
                        return Some(inner);
                    }
                }
                for arm in arms.iter() {
                    if let Some(guard) = arm.guard.as_ref() {
                        if let Some(inner) = Self::find_call_info(guard, position) {
                            return Some(inner);
                        }
                    }
                    if let Some(inner) = Self::find_call_info(&arm.body, position) {
                        return Some(inner);
                    }
                }
                None
            }
            Expr::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => Self::find_call_info(cond, position)
                .or_else(|| Self::find_call_info(then_branch, position))
                .or_else(|| Self::find_call_info(else_branch, position)),
            Expr::Binary { left, right, .. } => Self::find_call_info(left, position)
                .or_else(|| Self::find_call_info(right, position)),
            Expr::Block { items, .. } => items.iter().find_map(|item| match item {
                BlockItem::Bind { expr, .. } => Self::find_call_info(expr, position),
                BlockItem::Let { expr, .. } => Self::find_call_info(expr, position),
                BlockItem::Filter { expr, .. }
                | BlockItem::Yield { expr, .. }
                | BlockItem::Recurse { expr, .. }
                | BlockItem::Expr { expr, .. } => Self::find_call_info(expr, position),
                BlockItem::When { cond, effect, .. } | BlockItem::Unless { cond, effect, .. } => {
                    Self::find_call_info(cond, position)
                        .or_else(|| Self::find_call_info(effect, position))
                }
                BlockItem::Given {
                    cond, fail_expr, ..
                } => Self::find_call_info(cond, position)
                    .or_else(|| Self::find_call_info(fail_expr, position)),
                BlockItem::On {
                    transition,
                    handler,
                    ..
                } => Self::find_call_info(transition, position)
                    .or_else(|| Self::find_call_info(handler, position)),
            }),
            Expr::Mock {
                substitutions,
                body,
                ..
            } => {
                for sub in substitutions {
                    if let Some(val) = &sub.value {
                        if let Some(inner) = Self::find_call_info(val, position) {
                            return Some(inner);
                        }
                    }
                }
                Self::find_call_info(body, position)
            }
            Expr::Ident(_) | Expr::Literal(_) | Expr::FieldSection { .. } | Expr::Raw { .. } => {
                None
            }
        }
    }

    fn active_call_parameter(args: &[Expr], position: Position) -> usize {
        if args.is_empty() {
            return 0;
        }

        for (index, arg) in args.iter().enumerate() {
            if Self::expr_contains_position(arg, position) {
                return index;
            }
        }

        let ended_before = args
            .iter()
            .filter(|arg| Self::expr_ends_before_position(arg, position))
            .count();
        ended_before.min(args.len().saturating_sub(1))
    }

    fn expr_contains_position(expr: &Expr, position: Position) -> bool {
        let range = Self::span_to_range(Self::expr_span(expr).clone());
        Self::range_contains_position(&range, position)
    }

    fn expr_ends_before_position(expr: &Expr, position: Position) -> bool {
        let range = Self::span_to_range(Self::expr_span(expr).clone());
        position.line > range.end.line
            || (position.line == range.end.line && position.character >= range.end.character)
    }

    pub(super) fn expr_span(expr: &Expr) -> &aivi::Span {
        match expr {
            Expr::Ident(name) => &name.span,
            Expr::Literal(lit) => match lit {
                Literal::Number { span, .. }
                | Literal::String { span, .. }
                | Literal::Sigil { span, .. }
                | Literal::Bool { span, .. }
                | Literal::DateTime { span, .. } => span,
            },
            Expr::UnaryNeg { span, .. } => span,
            Expr::TextInterpolate { span, .. } => span,
            Expr::List { span, .. }
            | Expr::Tuple { span, .. }
            | Expr::Record { span, .. }
            | Expr::PatchLit { span, .. }
            | Expr::Suffixed { span, .. }
            | Expr::FieldAccess { span, .. }
            | Expr::FieldSection { span, .. }
            | Expr::Index { span, .. }
            | Expr::Call { span, .. }
            | Expr::Lambda { span, .. }
            | Expr::Match { span, .. }
            | Expr::If { span, .. }
            | Expr::Binary { span, .. }
            | Expr::Block { span, .. }
            | Expr::Raw { span, .. }
            | Expr::Mock { span, .. } => span,
        }
    }

    /// Extract parameter names from the Def of a function in current or imported modules.
    fn resolve_param_names(
        current_module: &Module,
        ident: &str,
        workspace_modules: &HashMap<String, IndexedModule>,
    ) -> Vec<String> {
        // Look in current module first
        if let Some(names) = Self::param_names_from_module(current_module, ident) {
            return names;
        }
        // Look in imported modules
        for use_decl in &current_module.uses {
            let imported =
                use_decl.wildcard || use_decl.items.iter().any(|item| item.name.name == ident);
            if !imported {
                continue;
            }
            if let Some(indexed) = workspace_modules.get(&use_decl.module.name) {
                if let Some(names) = Self::param_names_from_module(&indexed.module, ident) {
                    return names;
                }
            }
        }
        Vec::new()
    }

    fn param_names_from_module(module: &Module, ident: &str) -> Option<Vec<String>> {
        for item in &module.items {
            if let ModuleItem::Def(def) = item {
                if def.name.name == ident {
                    let names: Vec<String> =
                        def.params.iter().map(Self::pattern_display_name).collect();
                    if !names.is_empty() {
                        return Some(names);
                    }
                }
            }
        }
        None
    }

    fn pattern_display_name(pat: &Pattern) -> String {
        match pat {
            Pattern::Ident(name) | Pattern::SubjectIdent(name) => name.name.clone(),
            Pattern::Wildcard(_) => "_".to_string(),
            Pattern::At { name, .. } => name.name.clone(),
            Pattern::Constructor { name, .. } => name.name.clone(),
            Pattern::Record { .. } => "{..}".to_string(),
            Pattern::Tuple { .. } => "(..)".to_string(),
            Pattern::List { .. } => "[..]".to_string(),
            Pattern::Literal(_) => "_".to_string(),
        }
    }

    /// Build ParameterInformation from a type signature string like "`f` : `Int -> Text -> Bool`".
    /// Splits on ` -> ` to identify parameter types, and pairs with param names if available.
    fn build_parameter_info(
        signature_label: &str,
        param_names: &[String],
    ) -> Vec<ParameterInformation> {
        // Extract the type part from format: `name` : `Type1 -> Type2 -> Result`
        let type_part = signature_label
            .rsplit_once(": `")
            .map(|(_, t)| t.trim_end_matches('`'))
            .unwrap_or(signature_label);

        // Split into arrow-separated parts; last part is the return type.
        let parts: Vec<&str> = type_part.split(" -> ").collect();
        if parts.len() < 2 {
            return Vec::new();
        }

        // All parts except the last are parameter types.
        parts[..parts.len() - 1]
            .iter()
            .enumerate()
            .map(|(i, &part)| {
                let label = if let Some(name) = param_names.get(i) {
                    format!("{name}: {part}")
                } else {
                    part.to_string()
                };
                ParameterInformation {
                    label: ParameterLabel::Simple(label),
                    documentation: None,
                }
            })
            .collect()
    }

    /// Find a doc comment above the function definition.
    fn find_def_doc_comment(
        current_module: &Module,
        ident: &str,
        workspace_modules: &HashMap<String, IndexedModule>,
    ) -> Option<String> {
        // Check current module
        if let Some(def) = Self::find_def_in_module(current_module, ident) {
            if !def.decorators.is_empty() {
                // Could extract doc from decorators in the future
            }
        }
        // Check imported modules with text
        for use_decl in &current_module.uses {
            if let Some(indexed) = workspace_modules.get(&use_decl.module.name) {
                if let Some(text) = &indexed.text {
                    if let Some(def) = Self::find_def_in_module(&indexed.module, ident) {
                        return Self::extract_doc_comment_above(text, &def.span);
                    }
                }
            }
        }
        None
    }

    fn find_def_in_module<'a>(module: &'a Module, ident: &str) -> Option<&'a Def> {
        module.items.iter().find_map(|item| {
            if let ModuleItem::Def(def) = item {
                if def.name.name == ident {
                    return Some(def);
                }
            }
            None
        })
    }

    fn extract_doc_comment_above(text: &str, span: &aivi::Span) -> Option<String> {
        let lines: Vec<&str> = text.lines().collect();
        let def_line = span.start.line.saturating_sub(1); // 1-based to 0-based
        let mut doc_lines = Vec::new();
        let mut line_idx = def_line;
        while line_idx > 0 {
            line_idx -= 1;
            let line = lines.get(line_idx)?;
            let trimmed = line.trim();
            if let Some(comment) = trimmed.strip_prefix("//") {
                doc_lines.push(comment.trim().to_string());
            } else {
                break;
            }
        }
        if doc_lines.is_empty() {
            return None;
        }
        doc_lines.reverse();
        Some(doc_lines.join("\n"))
    }
}
