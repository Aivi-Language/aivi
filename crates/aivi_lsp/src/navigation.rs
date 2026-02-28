use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

use aivi::{infer_value_types, parse_modules, Module, ModuleItem};
use tower_lsp::lsp_types::{
    Hover, HoverContents, Location, MarkupContent, MarkupKind, Position, TextEdit, Url,
    WorkspaceEdit,
};

use crate::backend::Backend;
use crate::doc_index::DocIndex;
use crate::state::IndexedModule;

impl Backend {
    fn hover_debug_enabled() -> bool {
        std::env::var_os("AIVI_LSP_DEBUG_HOVER").is_some()
    }

    fn hover_debug(message: impl std::fmt::Display) {
        if Self::hover_debug_enabled() {
            eprintln!("[aivi-lsp:hover] {message}");
        }
    }

    fn hover_markdown(value: String) -> Hover {
        Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value,
            }),
            range: None,
        }
    }

    /// Find the TypeExpr from a TypeSig for the given identifier in a module.
    fn find_type_sig_in_module(module: &Module, ident: &str) -> Option<aivi::TypeExpr> {
        let paren_ident = format!("({ident})");
        for item in module.items.iter() {
            match item {
                ModuleItem::TypeSig(sig)
                    if sig.name.name == ident || sig.name.name == paren_ident =>
                {
                    return Some(sig.ty.clone());
                }
                ModuleItem::ClassDecl(class_decl) => {
                    for member in class_decl.members.iter() {
                        if member.name.name == ident || member.name.name == paren_ident {
                            return Some(member.ty.clone());
                        }
                    }
                }
                ModuleItem::DomainDecl(domain_decl) => {
                    for di in domain_decl.items.iter() {
                        if let aivi::DomainItem::TypeSig(sig) = di {
                            if sig.name.name == ident || sig.name.name == paren_ident {
                                return Some(sig.ty.clone());
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        None
    }

    /// Append type definitions for non-primitive types referenced in the ident's signature.
    fn append_type_definitions(
        contents: &mut String,
        ident: &str,
        resolved_module: &Module,
        current_module: &Module,
        workspace_modules: &HashMap<String, IndexedModule>,
    ) {
        let Some(type_expr) = Self::find_type_sig_in_module(resolved_module, ident) else {
            return;
        };
        let names = Self::collect_type_names(&type_expr);
        if names.is_empty() {
            return;
        }
        let mut defs = Vec::new();
        for name in &names {
            if let Some(brief) = Self::find_type_definition_brief(current_module, name) {
                defs.push(brief);
                continue;
            }
            if !std::ptr::eq(resolved_module, current_module) {
                if let Some(brief) = Self::find_type_definition_brief(resolved_module, name) {
                    defs.push(brief);
                    continue;
                }
            }
            let mut found = false;
            for use_decl in current_module.uses.iter() {
                let imported = use_decl.wildcard
                    || use_decl.items.is_empty()
                    || use_decl.items.iter().any(|item| item.name.name == *name);
                if !imported {
                    continue;
                }
                if let Some(indexed) = workspace_modules.get(&use_decl.module.name) {
                    if let Some(brief) = Self::find_type_definition_brief(&indexed.module, name) {
                        defs.push(brief);
                        found = true;
                        break;
                    }
                }
            }
            if !found {
                // Search all workspace modules as last resort
                for indexed in workspace_modules.values() {
                    if let Some(brief) = Self::find_type_definition_brief(&indexed.module, name) {
                        defs.push(brief);
                        break;
                    }
                }
            }
        }
        if defs.is_empty() {
            return;
        }
        contents.push_str("\n\n---\n\n");
        for (i, def) in defs.iter().enumerate() {
            contents.push('`');
            contents.push_str(def);
            contents.push('`');
            if i < defs.len() - 1 {
                contents.push_str("\n\n");
            }
        }
    }

    fn hover_fallback_for_unresolved_ident(ident: &str) -> String {
        let is_operator = !ident.is_empty()
            && ident
                .chars()
                .any(|ch| !ch.is_alphanumeric() && ch != '_' && ch != '.');
        let kind = if is_operator { "operator" } else { "value" };
        let base = format!(
            "`{ident}` : `_unresolved_`\n\nNo type signature was resolved in the current module/import scope."
        );
        Self::hover_badge_markdown(kind, base)
    }

    fn expr_contains_position_for_hover(expr: &aivi::Expr, position: Position) -> bool {
        let range = Self::span_to_range(Self::expr_span(expr).clone());
        Self::range_contains_position(&range, position)
    }

    fn collect_pattern_binders(pattern: &aivi::Pattern, out: &mut Vec<String>) {
        match pattern {
            aivi::Pattern::Ident(name) | aivi::Pattern::SubjectIdent(name) => {
                out.push(name.name.clone())
            }
            aivi::Pattern::At { name, pattern, .. } => {
                out.push(name.name.clone());
                Self::collect_pattern_binders(pattern, out);
            }
            aivi::Pattern::Tuple { items, .. } => {
                for item in items {
                    Self::collect_pattern_binders(item, out);
                }
            }
            aivi::Pattern::List { items, rest, .. } => {
                for item in items {
                    Self::collect_pattern_binders(item, out);
                }
                if let Some(rest) = rest {
                    Self::collect_pattern_binders(rest, out);
                }
            }
            aivi::Pattern::Record { fields, .. } => {
                for field in fields {
                    Self::collect_pattern_binders(&field.pattern, out);
                }
            }
            aivi::Pattern::Constructor { args, .. } => {
                for arg in args {
                    Self::collect_pattern_binders(arg, out);
                }
            }
            aivi::Pattern::Wildcard(_) | aivi::Pattern::Literal(_) => {}
        }
    }

    fn pattern_has_binding_at_position(
        pattern: &aivi::Pattern,
        ident: &str,
        position: Position,
    ) -> bool {
        match pattern {
            aivi::Pattern::Ident(name) | aivi::Pattern::SubjectIdent(name) => {
                name.name == ident
                    && Self::range_contains_position(
                        &Self::span_to_range(name.span.clone()),
                        position,
                    )
            }
            aivi::Pattern::At { name, pattern, .. } => {
                (name.name == ident
                    && Self::range_contains_position(
                        &Self::span_to_range(name.span.clone()),
                        position,
                    ))
                    || Self::pattern_has_binding_at_position(pattern, ident, position)
            }
            aivi::Pattern::Tuple { items, .. } => items
                .iter()
                .any(|item| Self::pattern_has_binding_at_position(item, ident, position)),
            aivi::Pattern::List { items, rest, .. } => {
                items
                    .iter()
                    .any(|item| Self::pattern_has_binding_at_position(item, ident, position))
                    || rest.as_deref().is_some_and(|rest| {
                        Self::pattern_has_binding_at_position(rest, ident, position)
                    })
            }
            aivi::Pattern::Record { fields, .. } => fields.iter().any(|field| {
                Self::pattern_has_binding_at_position(&field.pattern, ident, position)
            }),
            aivi::Pattern::Constructor { args, .. } => args
                .iter()
                .any(|arg| Self::pattern_has_binding_at_position(arg, ident, position)),
            aivi::Pattern::Wildcard(_) | aivi::Pattern::Literal(_) => false,
        }
    }

    fn local_binding_visible_in_expr(
        expr: &aivi::Expr,
        ident: &str,
        position: Position,
        in_scope: &mut Vec<String>,
    ) -> bool {
        if !Self::expr_contains_position_for_hover(expr, position) {
            return false;
        }
        match expr {
            aivi::Expr::Ident(name) => {
                name.name == ident
                    && in_scope.iter().any(|bound| bound == ident)
                    && Self::range_contains_position(
                        &Self::span_to_range(name.span.clone()),
                        position,
                    )
            }
            aivi::Expr::Lambda { params, body, .. } => {
                if params
                    .iter()
                    .any(|param| Self::pattern_has_binding_at_position(param, ident, position))
                {
                    return true;
                }
                let mut scoped = in_scope.clone();
                for param in params {
                    Self::collect_pattern_binders(param, &mut scoped);
                }
                Self::local_binding_visible_in_expr(body, ident, position, &mut scoped)
            }
            aivi::Expr::Match {
                scrutinee, arms, ..
            } => {
                if scrutinee.as_ref().is_some_and(|s| {
                    Self::local_binding_visible_in_expr(s, ident, position, in_scope)
                }) {
                    return true;
                }
                for arm in arms {
                    let arm_range = Self::span_to_range(arm.span.clone());
                    if !Self::range_contains_position(&arm_range, position) {
                        continue;
                    }
                    if Self::pattern_has_binding_at_position(&arm.pattern, ident, position) {
                        return true;
                    }
                    let mut scoped = in_scope.clone();
                    Self::collect_pattern_binders(&arm.pattern, &mut scoped);
                    if arm.guard.as_ref().is_some_and(|g| {
                        Self::local_binding_visible_in_expr(g, ident, position, &mut scoped)
                    }) || Self::local_binding_visible_in_expr(
                        &arm.body,
                        ident,
                        position,
                        &mut scoped,
                    ) {
                        return true;
                    }
                }
                false
            }
            aivi::Expr::Block { items, .. } => {
                let mut scoped = in_scope.clone();
                for item in items {
                    match item {
                        aivi::BlockItem::Bind { pattern, expr, .. }
                        | aivi::BlockItem::Let { pattern, expr, .. } => {
                            if Self::local_binding_visible_in_expr(
                                expr,
                                ident,
                                position,
                                &mut scoped,
                            ) {
                                return true;
                            }
                            if Self::pattern_has_binding_at_position(pattern, ident, position) {
                                return true;
                            }
                            Self::collect_pattern_binders(pattern, &mut scoped);
                        }
                        aivi::BlockItem::Filter { expr, .. }
                        | aivi::BlockItem::Yield { expr, .. }
                        | aivi::BlockItem::Recurse { expr, .. }
                        | aivi::BlockItem::Expr { expr, .. } => {
                            if Self::local_binding_visible_in_expr(
                                expr,
                                ident,
                                position,
                                &mut scoped,
                            ) {
                                return true;
                            }
                        }
                        aivi::BlockItem::When { cond, effect, .. }
                        | aivi::BlockItem::Unless { cond, effect, .. } => {
                            if Self::local_binding_visible_in_expr(
                                cond,
                                ident,
                                position,
                                &mut scoped,
                            ) || Self::local_binding_visible_in_expr(
                                effect,
                                ident,
                                position,
                                &mut scoped,
                            ) {
                                return true;
                            }
                        }
                        aivi::BlockItem::Given {
                            cond, fail_expr, ..
                        } => {
                            if Self::local_binding_visible_in_expr(
                                cond,
                                ident,
                                position,
                                &mut scoped,
                            ) || Self::local_binding_visible_in_expr(
                                fail_expr,
                                ident,
                                position,
                                &mut scoped,
                            ) {
                                return true;
                            }
                        }
                        aivi::BlockItem::On {
                            transition,
                            handler,
                            ..
                        } => {
                            if Self::local_binding_visible_in_expr(
                                transition,
                                ident,
                                position,
                                &mut scoped,
                            ) || Self::local_binding_visible_in_expr(
                                handler,
                                ident,
                                position,
                                &mut scoped,
                            ) {
                                return true;
                            }
                        }
                    }
                }
                false
            }
            aivi::Expr::Suffixed { base, .. } | aivi::Expr::UnaryNeg { expr: base, .. } => {
                Self::local_binding_visible_in_expr(base, ident, position, in_scope)
            }
            aivi::Expr::TextInterpolate { parts, .. } => parts.iter().any(|part| match part {
                aivi::TextPart::Text { .. } => false,
                aivi::TextPart::Expr { expr, .. } => {
                    Self::local_binding_visible_in_expr(expr, ident, position, in_scope)
                }
            }),
            aivi::Expr::List { items, .. } => items.iter().any(|item| {
                Self::local_binding_visible_in_expr(&item.expr, ident, position, in_scope)
            }),
            aivi::Expr::Tuple { items, .. } => items
                .iter()
                .any(|item| Self::local_binding_visible_in_expr(item, ident, position, in_scope)),
            aivi::Expr::Index { base, index, .. } => {
                Self::local_binding_visible_in_expr(base, ident, position, in_scope)
                    || Self::local_binding_visible_in_expr(index, ident, position, in_scope)
            }
            aivi::Expr::Call { func, args, .. } => {
                Self::local_binding_visible_in_expr(func, ident, position, in_scope)
                    || args.iter().any(|arg| {
                        Self::local_binding_visible_in_expr(arg, ident, position, in_scope)
                    })
            }
            aivi::Expr::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => {
                Self::local_binding_visible_in_expr(cond, ident, position, in_scope)
                    || Self::local_binding_visible_in_expr(then_branch, ident, position, in_scope)
                    || Self::local_binding_visible_in_expr(else_branch, ident, position, in_scope)
            }
            aivi::Expr::Binary { left, right, .. } => {
                Self::local_binding_visible_in_expr(left, ident, position, in_scope)
                    || Self::local_binding_visible_in_expr(right, ident, position, in_scope)
            }
            aivi::Expr::Record { fields, .. } | aivi::Expr::PatchLit { fields, .. } => {
                fields.iter().any(|field| {
                    Self::local_binding_visible_in_expr(&field.value, ident, position, in_scope)
                })
            }
            aivi::Expr::FieldAccess { base, field, .. } => {
                (field.name == ident
                    && in_scope.iter().any(|bound| bound == ident)
                    && Self::range_contains_position(
                        &Self::span_to_range(field.span.clone()),
                        position,
                    ))
                    || Self::local_binding_visible_in_expr(base, ident, position, in_scope)
            }
            aivi::Expr::FieldSection { field, .. } => {
                field.name == ident
                    && in_scope.iter().any(|bound| bound == ident)
                    && Self::range_contains_position(
                        &Self::span_to_range(field.span.clone()),
                        position,
                    )
            }
            aivi::Expr::Literal(_) | aivi::Expr::Raw { .. } => false,
        }
    }

    fn hover_contents_for_local_binding(
        module: &Module,
        ident: &str,
        position: Position,
        inferred: Option<&HashMap<String, String>>,
        workspace_modules: Option<&HashMap<String, IndexedModule>>,
    ) -> Option<String> {
        fn pattern_binds_name(pattern: &aivi::Pattern, name: &str) -> bool {
            match pattern {
                aivi::Pattern::Ident(n) | aivi::Pattern::SubjectIdent(n) => n.name == name,
                aivi::Pattern::At {
                    name: n, pattern, ..
                } => n.name == name || pattern_binds_name(pattern, name),
                aivi::Pattern::Tuple { items, .. } => {
                    items.iter().any(|item| pattern_binds_name(item, name))
                }
                aivi::Pattern::List { items, rest, .. } => {
                    items.iter().any(|item| pattern_binds_name(item, name))
                        || rest
                            .as_deref()
                            .is_some_and(|rest| pattern_binds_name(rest, name))
                }
                aivi::Pattern::Record { fields, .. } => fields
                    .iter()
                    .any(|field| pattern_binds_name(&field.pattern, name)),
                aivi::Pattern::Constructor { args, .. } => {
                    args.iter().any(|arg| pattern_binds_name(arg, name))
                }
                aivi::Pattern::Wildcard(_) | aivi::Pattern::Literal(_) => false,
            }
        }

        fn position_after(range: &tower_lsp::lsp_types::Range, position: Position) -> bool {
            position.line > range.end.line
                || (position.line == range.end.line && position.character >= range.end.character)
        }

        fn local_binding_expr_for_ident<'a>(
            expr: &'a aivi::Expr,
            ident: &str,
            position: Position,
        ) -> Option<(&'a aivi::Expr, bool)> {
            let range = Backend::span_to_range(Backend::expr_span(expr).clone());
            if !Backend::range_contains_position(&range, position) {
                return None;
            }

            let aivi::Expr::Block { items, .. } = expr else {
                return None;
            };

            let mut latest: Option<(&aivi::Expr, bool)> = None;
            for item in items {
                let item_span = match item {
                    aivi::BlockItem::Bind { span, .. }
                    | aivi::BlockItem::Let { span, .. }
                    | aivi::BlockItem::Filter { span, .. }
                    | aivi::BlockItem::Yield { span, .. }
                    | aivi::BlockItem::Recurse { span, .. }
                    | aivi::BlockItem::Expr { span, .. }
                    | aivi::BlockItem::When { span, .. }
                    | aivi::BlockItem::Unless { span, .. }
                    | aivi::BlockItem::Given { span, .. }
                    | aivi::BlockItem::On { span, .. } => span,
                };
                let item_range = Backend::span_to_range(item_span.clone());
                if !Backend::range_contains_position(&item_range, position)
                    && !position_after(&item_range, position)
                {
                    continue;
                }
                if Backend::range_contains_position(&item_range, position) {
                    if let aivi::BlockItem::Expr { expr, .. } = item {
                        if let Some(found) = local_binding_expr_for_ident(expr, ident, position) {
                            return Some(found);
                        }
                    }
                    if let aivi::BlockItem::Bind { pattern, expr, .. }
                    | aivi::BlockItem::Let { pattern, expr, .. } = item
                    {
                        if pattern_binds_name(pattern, ident) {
                            return Some((expr, matches!(item, aivi::BlockItem::Bind { .. })));
                        }
                        if let Some(found) = local_binding_expr_for_ident(expr, ident, position) {
                            return Some(found);
                        }
                    }
                } else if let aivi::BlockItem::Bind { pattern, expr, .. }
                | aivi::BlockItem::Let { pattern, expr, .. } = item
                {
                    if pattern_binds_name(pattern, ident) {
                        latest = Some((expr, matches!(item, aivi::BlockItem::Bind { .. })));
                    }
                }
            }

            latest
        }

        fn type_sig_expr_in_module(module: &Module, ident: &str) -> Option<aivi::TypeExpr> {
            module.items.iter().find_map(|item| match item {
                aivi::ModuleItem::TypeSig(sig) if sig.name.name == ident => Some(sig.ty.clone()),
                _ => None,
            })
        }

        fn resolve_type_sig_expr(
            current_module: &Module,
            ident: &str,
            workspace_modules: &HashMap<String, IndexedModule>,
        ) -> Option<aivi::TypeExpr> {
            if let Some(ty) = type_sig_expr_in_module(current_module, ident) {
                return Some(ty);
            }
            for use_decl in current_module.uses.iter() {
                let imported = use_decl.wildcard
                    || use_decl.items.is_empty()
                    || use_decl.items.iter().any(|item| item.name.name == ident);
                if !imported {
                    continue;
                }
                let Some(indexed) = workspace_modules.get(&use_decl.module.name) else {
                    continue;
                };
                if let Some(ty) = type_sig_expr_in_module(&indexed.module, ident) {
                    return Some(ty);
                }
            }
            None
        }

        fn apply_call_args(ty: &aivi::TypeExpr, mut args: usize) -> aivi::TypeExpr {
            let mut current = ty.clone();
            while args > 0 {
                match current {
                    aivi::TypeExpr::Func { params, result, .. } => {
                        if params.len() <= args {
                            args -= params.len();
                            current = *result;
                        } else {
                            return *result;
                        }
                    }
                    _ => return current,
                }
            }
            current
        }

        fn extract_bind_value_type(ty: &aivi::TypeExpr) -> aivi::TypeExpr {
            match ty {
                aivi::TypeExpr::Apply { base, args, .. }
                    if args.len() == 1
                        && matches!(
                            base.as_ref(),
                            aivi::TypeExpr::Name(name)
                                if name.name == "Effect" || name.name == "Task" || name.name == "Result"
                        ) =>
                {
                    args[0].clone()
                }
                _ => ty.clone(),
            }
        }

        fn root_type_name(ty: &aivi::TypeExpr) -> Option<String> {
            match ty {
                aivi::TypeExpr::Name(name) => Some(
                    name.name
                        .rsplit('.')
                        .next()
                        .unwrap_or(&name.name)
                        .to_string(),
                ),
                aivi::TypeExpr::Apply { base, .. } => root_type_name(base),
                _ => None,
            }
        }

        fn find_alias_definition(module: &Module, alias_name: &str) -> Option<String> {
            module.items.iter().find_map(|item| match item {
                aivi::ModuleItem::TypeAlias(alias) if alias.name.name == alias_name => {
                    Some(format!(
                        "type {} = {}",
                        alias.name.name,
                        Backend::type_expr_to_string(&alias.aliased)
                    ))
                }
                _ => None,
            })
        }

        fn alias_definition_for_type(
            current_module: &Module,
            ty: &aivi::TypeExpr,
            workspace_modules: &HashMap<String, IndexedModule>,
        ) -> Option<String> {
            let alias_name = root_type_name(ty)?;
            if let Some(alias) = find_alias_definition(current_module, &alias_name) {
                return Some(alias);
            }
            for use_decl in current_module.uses.iter() {
                let imported = use_decl.wildcard
                    || use_decl.items.is_empty()
                    || use_decl
                        .items
                        .iter()
                        .any(|item| item.name.name == alias_name);
                if !imported {
                    continue;
                }
                let Some(indexed) = workspace_modules.get(&use_decl.module.name) else {
                    continue;
                };
                if let Some(alias) = find_alias_definition(&indexed.module, &alias_name) {
                    return Some(alias);
                }
            }
            None
        }

        fn callee_ident_name(expr: &aivi::Expr) -> Option<String> {
            match expr {
                aivi::Expr::Ident(name) => Some(name.name.clone()),
                aivi::Expr::FieldAccess { field, .. } => Some(field.name.clone()),
                _ => None,
            }
        }

        for item in module.items.iter() {
            let aivi::ModuleItem::Def(def) = item else {
                continue;
            };
            if !Self::expr_contains_position_for_hover(&def.expr, position) {
                continue;
            }
            if def
                .params
                .iter()
                .any(|param| Self::pattern_has_binding_at_position(param, ident, position))
            {
                return Some(Self::hover_badge_markdown("value", format!("`{ident}`")));
            }
            let mut scope = Vec::new();
            for param in def.params.iter() {
                Self::collect_pattern_binders(param, &mut scope);
            }
            #[allow(clippy::collapsible_if, clippy::collapsible_match)]
            if Self::local_binding_visible_in_expr(&def.expr, ident, position, &mut scope) {
                if let Some(workspace_modules) = workspace_modules {
                    if let Some((bound_expr, is_bind)) =
                        local_binding_expr_for_ident(&def.expr, ident, position)
                    {
                        if let aivi::Expr::Call { func, args, .. } = bound_expr {
                            if let Some(callee) = callee_ident_name(func) {
                                if let Some(sig_ty) =
                                    resolve_type_sig_expr(module, &callee, workspace_modules)
                                {
                                    let mut ty = apply_call_args(&sig_ty, args.len());
                                    if is_bind {
                                        ty = extract_bind_value_type(&ty);
                                    }
                                    let ty_string = Self::type_expr_to_string(&ty);
                                    let mut base = format!("`{ident}` : `{ty_string}`");
                                    if let Some(alias_def) =
                                        alias_definition_for_type(module, &ty, workspace_modules)
                                    {
                                        base.push_str(&format!("\n\n`{alias_def}`"));
                                    }
                                    return Some(Self::hover_badge_markdown("value", base));
                                }
                            }
                        }
                    }
                }
                let base = inferred
                    .and_then(|types| types.get(ident))
                    .map(|ty| format!("`{ident}` : `{ty}`"))
                    .unwrap_or_else(|| format!("`{ident}`"));
                return Some(Self::hover_badge_markdown("value", base));
            }
        }
        None
    }

    fn find_record_field_name_at_position(
        expr: &aivi::Expr,
        position: Position,
    ) -> Option<&aivi::SpannedName> {
        use aivi::Expr;
        match expr {
            Expr::Suffixed { base, .. } => Self::find_record_field_name_at_position(base, position),
            Expr::UnaryNeg { expr, .. } => Self::find_record_field_name_at_position(expr, position),
            Expr::Record { fields, .. } | Expr::PatchLit { fields, .. } => {
                for field in fields.iter() {
                    for segment in field.path.iter() {
                        if let aivi::PathSegment::Field(name) = segment {
                            let range = Self::span_to_range(name.span.clone());
                            if Self::range_contains_position(&range, position) {
                                return Some(name);
                            }
                        }
                    }
                    if let Some(found) =
                        Self::find_record_field_name_at_position(&field.value, position)
                    {
                        return Some(found);
                    }
                }
                None
            }
            Expr::FieldAccess { base, field, .. } => {
                let range = Self::span_to_range(field.span.clone());
                if Self::range_contains_position(&range, position) {
                    return Some(field);
                }
                Self::find_record_field_name_at_position(base, position)
            }
            Expr::FieldSection { field, .. } => {
                let range = Self::span_to_range(field.span.clone());
                if Self::range_contains_position(&range, position) {
                    return Some(field);
                }
                None
            }
            Expr::Ident(_) | Expr::Literal(_) | Expr::Raw { .. } => None,
            Expr::TextInterpolate { parts, .. } => parts.iter().find_map(|part| match part {
                aivi::TextPart::Text { .. } => None,
                aivi::TextPart::Expr { expr, .. } => {
                    Self::find_record_field_name_at_position(expr, position)
                }
            }),
            Expr::List { items, .. } => items
                .iter()
                .find_map(|item| Self::find_record_field_name_at_position(&item.expr, position)),
            Expr::Tuple { items, .. } => items
                .iter()
                .find_map(|item| Self::find_record_field_name_at_position(item, position)),
            Expr::Index { base, index, .. } => {
                Self::find_record_field_name_at_position(base, position)
                    .or_else(|| Self::find_record_field_name_at_position(index, position))
            }
            Expr::Call { func, args, .. } => {
                Self::find_record_field_name_at_position(func, position).or_else(|| {
                    args.iter()
                        .find_map(|arg| Self::find_record_field_name_at_position(arg, position))
                })
            }
            Expr::Lambda {
                params: _, body, ..
            } => Self::find_record_field_name_at_position(body, position),
            Expr::Match {
                scrutinee, arms, ..
            } => scrutinee
                .as_ref()
                .and_then(|expr| Self::find_record_field_name_at_position(expr, position))
                .or_else(|| {
                    arms.iter().find_map(|arm| {
                        Self::find_record_field_name_at_position(&arm.body, position).or_else(
                            || {
                                arm.guard.as_ref().and_then(|guard| {
                                    Self::find_record_field_name_at_position(guard, position)
                                })
                            },
                        )
                    })
                }),
            Expr::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => Self::find_record_field_name_at_position(cond, position)
                .or_else(|| Self::find_record_field_name_at_position(then_branch, position))
                .or_else(|| Self::find_record_field_name_at_position(else_branch, position)),
            Expr::Binary { left, right, .. } => {
                Self::find_record_field_name_at_position(left, position)
                    .or_else(|| Self::find_record_field_name_at_position(right, position))
            }
            Expr::Block { items, .. } => items.iter().find_map(|item| match item {
                aivi::BlockItem::Bind { expr, .. }
                | aivi::BlockItem::Let { expr, .. }
                | aivi::BlockItem::Filter { expr, .. }
                | aivi::BlockItem::Yield { expr, .. }
                | aivi::BlockItem::Recurse { expr, .. }
                | aivi::BlockItem::Expr { expr, .. } => {
                    Self::find_record_field_name_at_position(expr, position)
                }
                aivi::BlockItem::When { cond, effect, .. }
                | aivi::BlockItem::Unless { cond, effect, .. } => {
                    Self::find_record_field_name_at_position(cond, position)
                        .or_else(|| Self::find_record_field_name_at_position(effect, position))
                }
                aivi::BlockItem::Given {
                    cond, fail_expr, ..
                } => Self::find_record_field_name_at_position(cond, position)
                    .or_else(|| Self::find_record_field_name_at_position(fail_expr, position)),
                aivi::BlockItem::On {
                    transition,
                    handler,
                    ..
                } => Self::find_record_field_name_at_position(transition, position)
                    .or_else(|| Self::find_record_field_name_at_position(handler, position)),
            }),
        }
    }

    fn type_sig_for_value<'a>(module: &'a Module, value_name: &str) -> Option<&'a aivi::TypeSig> {
        for item in module.items.iter() {
            match item {
                aivi::ModuleItem::TypeSig(sig) if sig.name.name == value_name => return Some(sig),
                aivi::ModuleItem::DomainDecl(domain) => {
                    for domain_item in domain.items.iter() {
                        if let aivi::DomainItem::TypeSig(sig) = domain_item {
                            if sig.name.name == value_name {
                                return Some(sig);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        None
    }

    fn type_alias_named<'a>(module: &'a Module, type_name: &str) -> Option<&'a aivi::TypeAlias> {
        for item in module.items.iter() {
            match item {
                aivi::ModuleItem::TypeAlias(alias) if alias.name.name == type_name => {
                    return Some(alias);
                }
                _ => {}
            }
        }
        None
    }

    fn record_field_definition_range_for_type(
        module: &Module,
        ty: &aivi::TypeExpr,
        field_name: &str,
    ) -> Option<tower_lsp::lsp_types::Range> {
        use aivi::TypeExpr;

        match ty {
            TypeExpr::Record { fields, .. } => fields.iter().find_map(|(name, _)| {
                if name.name == field_name {
                    Some(Self::span_to_range(name.span.clone()))
                } else {
                    None
                }
            }),
            TypeExpr::Name(name) => {
                let bare = name.name.rsplit('.').next().unwrap_or(&name.name);
                let alias = Self::type_alias_named(module, bare)?;
                Self::record_field_definition_range_for_type(module, &alias.aliased, field_name)
            }
            TypeExpr::Apply { base, .. } => {
                // For `Foo A B`, field declarations live on `Foo` if it's a record alias.
                Self::record_field_definition_range_for_type(module, base, field_name)
            }
            TypeExpr::And { .. }
            | TypeExpr::Func { .. }
            | TypeExpr::Tuple { .. }
            | TypeExpr::Star { .. }
            | TypeExpr::Unknown { .. } => None,
        }
    }

    fn build_record_field_definition(
        text: &str,
        uri: &Url,
        position: Position,
    ) -> Option<Location> {
        let path = PathBuf::from(Self::path_from_uri(uri));
        let (modules, _) = parse_modules(&path, text);
        let module = Self::module_at_position(&modules, position)?;

        // Find the containing def so we can use its type signature to resolve the record type.
        for item in module.items.iter() {
            let aivi::ModuleItem::Def(def) = item else {
                continue;
            };
            let def_range = Self::span_to_range(Self::expr_span(&def.expr).clone());
            if !Self::range_contains_position(&def_range, position) {
                continue;
            }
            let field = Self::find_record_field_name_at_position(&def.expr, position)?;
            let sig = Self::type_sig_for_value(module, &def.name.name)?;
            let range = Self::record_field_definition_range_for_type(module, &sig.ty, &field.name)?;
            return Some(Location::new(uri.clone(), range));
        }

        None
    }

    pub(super) fn build_definition(text: &str, uri: &Url, position: Position) -> Option<Location> {
        if let Some(location) = Self::build_record_field_definition(text, uri, position) {
            return Some(location);
        }

        let ident = Self::extract_identifier(text, position)?;
        let path = PathBuf::from(Self::path_from_uri(uri));
        let (modules, _) = parse_modules(&path, text);
        for module in modules {
            if module.name.name == ident {
                let range = Self::span_to_range(module.name.span);
                return Some(Location::new(uri.clone(), range));
            }
            if let Some(range) = Self::module_member_definition_range(&module, &ident) {
                return Some(Location::new(uri.clone(), range));
            }
            for export in module.exports.iter() {
                if export.name.name == ident {
                    let range = Self::span_to_range(export.name.span.clone());
                    return Some(Location::new(uri.clone(), range));
                }
            }
        }
        None
    }

    pub(super) fn build_definition_with_workspace(
        text: &str,
        uri: &Url,
        position: Position,
        workspace_modules: &HashMap<String, IndexedModule>,
    ) -> Option<Location> {
        // Try local record-field navigation first (it relies on local type signatures and aliases).
        if let Some(location) = Self::build_record_field_definition(text, uri, position) {
            return Some(location);
        }

        let ident = Self::extract_identifier(text, position)?;

        if let Some(location) = Self::build_definition(text, uri, position) {
            return Some(location);
        }

        let path = PathBuf::from(Self::path_from_uri(uri));
        let (modules, _) = parse_modules(&path, text);
        let current_module = Self::module_at_position(&modules, position)?;

        if ident.contains('.') {
            if let Some(indexed) = workspace_modules.get(&ident) {
                let range = Self::span_to_range(indexed.module.name.span.clone());
                return Some(Location::new(indexed.uri.clone(), range));
            }
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
            if let Some(range) = Self::module_member_definition_range(&indexed.module, &ident) {
                return Some(Location::new(indexed.uri.clone(), range));
            }
        }

        None
    }

    pub(super) fn build_hover(
        text: &str,
        uri: &Url,
        position: Position,
        doc_index: &DocIndex,
    ) -> Option<Hover> {
        let started = Instant::now();
        let ident = match Self::extract_identifier(text, position) {
            Some(ident) => ident,
            None => {
                Self::hover_debug(format!(
                    "build_hover: no token at {}:{}",
                    position.line, position.character
                ));
                return None;
            }
        };
        let path = PathBuf::from(Self::path_from_uri(uri));
        let (modules, _) = parse_modules(&path, text);
        Self::hover_debug(format!(
            "build_hover: token={ident:?}, modules={}",
            modules.len()
        ));
        let (_, inferred, span_types) = infer_value_types(&modules);
        for module in modules.iter() {
            let doc = Self::doc_for_ident(text, module, &ident);
            let inferred = inferred.get(&module.name.name);
            if let Some(contents) =
                Self::hover_contents_for_module(module, &ident, inferred, doc.as_deref(), doc_index)
            {
                Self::hover_debug(format!(
                    "build_hover: resolved in module {} after {:?}",
                    module.name.name,
                    started.elapsed()
                ));
                return Some(Self::hover_markdown(contents));
            }
        }
        if let Some(module) = Self::module_at_position(&modules, position) {
            if let Some(contents) = Self::hover_contents_for_local_binding(
                module,
                &ident,
                position,
                inferred.get(&module.name.name),
                None,
            ) {
                Self::hover_debug(format!(
                    "build_hover: resolved as local binding in {} after {:?}",
                    module.name.name,
                    started.elapsed()
                ));
                return Some(Self::hover_markdown(contents));
            }
            // Fallback: look up the smallest span containing the cursor position.
            if let Some(contents) =
                Self::hover_from_span_types(&ident, position, &span_types, &module.name.name)
            {
                Self::hover_debug(format!(
                    "build_hover: resolved from span types in {} after {:?}",
                    module.name.name,
                    started.elapsed()
                ));
                return Some(Self::hover_markdown(contents));
            }
        }
        if let Some(contents) = Self::hover_contents_for_primitive_value(&ident) {
            Self::hover_debug(format!(
                "build_hover: resolved primitive token {ident:?} after {:?}",
                started.elapsed()
            ));
            return Some(Self::hover_markdown(contents));
        }
        Self::hover_debug(format!(
            "build_hover: unresolved token {ident:?}; returning generic fallback after {:?}",
            started.elapsed()
        ));
        Some(Self::hover_markdown(
            Self::hover_fallback_for_unresolved_ident(&ident),
        ))
    }

    /// Collect only the modules relevant for type inference: the current file's
    /// modules plus directly imported modules. This avoids running `infer_value_types`
    /// on the entire workspace (which is too slow for interactive hover).
    pub(crate) fn collect_relevant_modules(
        file_modules: &[Module],
        current_module: &Module,
        workspace_modules: &HashMap<String, IndexedModule>,
    ) -> Vec<Module> {
        let mut seen = HashSet::new();
        let mut result = Vec::new();

        // Add all modules from the current file.
        for m in file_modules {
            if seen.insert(m.name.name.clone()) {
                result.push(m.clone());
            }
        }

        // Add directly imported modules (via `use` declarations).
        for use_decl in current_module.uses.iter() {
            let module_name = &use_decl.module.name;
            if seen.insert(module_name.clone()) {
                if let Some(indexed) = workspace_modules.get(module_name) {
                    result.push(indexed.module.clone());
                }
            }
        }

        result
    }

    /// Resolve hover for dotted member access like `Heap.push`, `Map.empty`,
    /// Qualified value access — looks up the prefix as a type/domain name in imported
    /// modules and then finds the member in that module.
    fn hover_for_dotted_member(
        ident: &str,
        current_module: &Module,
        workspace_modules: &HashMap<String, IndexedModule>,
        inferred: &HashMap<String, HashMap<String, String>>,
        doc_index: &DocIndex,
    ) -> Option<Hover> {
        let dot_pos = ident.find('.')?;
        let prefix = &ident[..dot_pos];
        let member = &ident[dot_pos + 1..];
        if prefix.is_empty() || member.is_empty() {
            return None;
        }

        // Look through imported modules for one that exports or defines the prefix
        // as a type, domain, or type alias. Then look up the member in that module.
        let modules_to_search =
            Self::find_modules_exporting(prefix, current_module, workspace_modules);

        Self::hover_debug(format!(
            "hover_for_dotted_member: prefix={prefix:?}, member={member:?}, modules_found={}",
            modules_to_search.len()
        ));

        for indexed in &modules_to_search {
            let inf = inferred.get(&indexed.module.name.name);
            let doc_text = indexed
                .uri
                .to_file_path()
                .ok()
                .and_then(|p| fs::read_to_string(p).ok());
            let doc = doc_text
                .as_deref()
                .and_then(|text| Self::doc_for_ident(text, &indexed.module, member));

            Self::hover_debug(format!(
                "hover_for_dotted_member: checking module={} has_inferred={}",
                indexed.module.name.name,
                inf.is_some()
            ));

            // Check domain members with the member name.
            if let Some(contents) = Self::hover_contents_for_module(
                &indexed.module,
                member,
                inf,
                doc.as_deref(),
                doc_index,
            ) {
                return Some(Hover {
                    contents: HoverContents::Markup(MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: contents,
                    }),
                    range: None,
                });
            }
        }

        // Also check the current module itself (the prefix might be defined locally).
        let doc = Self::doc_for_ident("", current_module, member);
        let inf = inferred.get(&current_module.name.name);
        if let Some(contents) =
            Self::hover_contents_for_module(current_module, member, inf, doc.as_deref(), doc_index)
        {
            return Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: contents,
                }),
                range: None,
            });
        }

        None
    }

    /// Find modules that export or define a name (type, domain, type alias, etc.)
    /// matching the given prefix. Searches the current module's `use` imports.
    fn find_modules_exporting<'a>(
        name: &str,
        current_module: &Module,
        workspace_modules: &'a HashMap<String, IndexedModule>,
    ) -> Vec<&'a IndexedModule> {
        let mut result = Vec::new();
        for use_decl in current_module.uses.iter() {
            let imports_name =
                use_decl.wildcard || use_decl.items.iter().any(|item| item.name.name == name);
            if !imports_name {
                continue;
            }
            if let Some(indexed) = workspace_modules.get(&use_decl.module.name) {
                result.push(indexed);
            }
        }

        // Also check modules imported without item lists (bare `use aivi.collections`)
        // where the module itself may export the name.
        for use_decl in current_module.uses.iter() {
            if !use_decl.items.is_empty() || use_decl.wildcard {
                continue;
            }
            if let Some(indexed) = workspace_modules.get(&use_decl.module.name) {
                // Check if this module exports the name.
                let exports_name = indexed.module.exports.iter().any(|e| e.name.name == name);
                if exports_name && !result.iter().any(|r| r.uri == indexed.uri) {
                    result.push(indexed);
                }
            }
        }

        // Also check the prelude module if present.
        if let Some(prelude) = workspace_modules.get("aivi.prelude") {
            let exports_name = prelude.module.exports.iter().any(|e| e.name.name == name);
            if exports_name && !result.iter().any(|r| r.uri == prelude.uri) {
                result.push(prelude);
            }
        }

        // Finally check all workspace modules that define this as a domain/type,
        // since the name could come from the core module (e.g. `Heap` from `aivi`).
        if result.is_empty() {
            for indexed in workspace_modules.values() {
                let defines_name = indexed.module.items.iter().any(|item| match item {
                    ModuleItem::TypeDecl(decl) => decl.name.name == name,
                    ModuleItem::DomainDecl(domain) => {
                        // Domain's `over` type might match (e.g. domain MinHeap over Heap a)
                        domain.name.name == name
                    }
                    ModuleItem::TypeAlias(alias) => alias.name.name == name,
                    _ => false,
                });
                if defines_name {
                    result.push(indexed);
                }
            }
        }

        result
    }

    pub(super) fn build_hover_with_workspace(
        text: &str,
        uri: &Url,
        position: Position,
        workspace_modules: &HashMap<String, IndexedModule>,
        doc_index: &DocIndex,
    ) -> Option<Hover> {
        let started = Instant::now();
        let ident = match Self::extract_identifier(text, position) {
            Some(ident) => ident,
            None => {
                Self::hover_debug(format!(
                    "build_hover_ws: no token at {}:{}",
                    position.line, position.character
                ));
                return None;
            }
        };
        let path = PathBuf::from(Self::path_from_uri(uri));
        let (modules, _) = parse_modules(&path, text);
        Self::hover_debug(format!(
            "build_hover_ws: token={ident:?}, file_modules={}, workspace_modules={}",
            modules.len(),
            workspace_modules.len()
        ));
        let current_module = Self::module_at_position(&modules, position);
        let Some(current_module) = current_module else {
            Self::hover_debug("build_hover_ws: no module at cursor; skipping workspace hover");
            return None;
        };

        // Only infer types for the current file's modules + direct imports (not the
        // entire workspace) to keep hover responsive in large projects.
        let relevant_modules =
            Self::collect_relevant_modules(&modules, current_module, workspace_modules);
        let (_, inferred, span_types) = infer_value_types(&relevant_modules);
        Self::hover_debug(format!(
            "build_hover_ws: inferred over {} relevant modules",
            relevant_modules.len()
        ));

        // Handle dotted identifiers: first check if it's a full module name (e.g.
        // "aivi.collections"), then check Domain.method / Type.constructor patterns.
        if ident.contains('.') {
            // 1. Exact module name match.
            if let Some(indexed) = workspace_modules.get(&ident) {
                let doc_text = indexed
                    .uri
                    .to_file_path()
                    .ok()
                    .and_then(|path| fs::read_to_string(path).ok());
                let doc = doc_text
                    .as_deref()
                    .and_then(|text| Self::doc_for_ident(text, &indexed.module, &ident));
                let inferred = inferred.get(&indexed.module.name.name);
                if let Some(contents) = Self::hover_contents_for_module(
                    &indexed.module,
                    &ident,
                    inferred,
                    doc.as_deref(),
                    doc_index,
                ) {
                    Self::hover_debug(format!(
                        "build_hover_ws: resolved dotted module {} after {:?}",
                        ident,
                        started.elapsed()
                    ));
                    return Some(Self::hover_markdown(contents));
                }
            }

            // 2. Domain.method or Type.constructor (e.g. "Heap.push", "Map.empty").
            if let Some(hover) = Self::hover_for_dotted_member(
                &ident,
                current_module,
                workspace_modules,
                &inferred,
                doc_index,
            ) {
                Self::hover_debug(format!(
                    "build_hover_ws: resolved dotted member {} after {:?}",
                    ident,
                    started.elapsed()
                ));
                return Some(hover);
            }
        }

        let doc = Self::doc_for_ident(text, current_module, &ident);
        let inferred_current = inferred.get(&current_module.name.name);
        if let Some(mut contents) = Self::hover_contents_for_module(
            current_module,
            &ident,
            inferred_current,
            doc.as_deref(),
            doc_index,
        ) {
            Self::append_type_definitions(
                &mut contents,
                &ident,
                current_module,
                current_module,
                workspace_modules,
            );
            Self::hover_debug(format!(
                "build_hover_ws: resolved in current module {} after {:?}",
                current_module.name.name,
                started.elapsed()
            ));
            return Some(Self::hover_markdown(contents));
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
            let doc_text = indexed
                .uri
                .to_file_path()
                .ok()
                .and_then(|path| fs::read_to_string(path).ok());
            let doc = doc_text
                .as_deref()
                .and_then(|text| Self::doc_for_ident(text, &indexed.module, &ident));
            let inferred = inferred.get(&indexed.module.name.name);
            if let Some(mut contents) = Self::hover_contents_for_module(
                &indexed.module,
                &ident,
                inferred,
                doc.as_deref(),
                doc_index,
            ) {
                Self::append_type_definitions(
                    &mut contents,
                    &ident,
                    &indexed.module,
                    current_module,
                    workspace_modules,
                );
                Self::hover_debug(format!(
                    "build_hover_ws: resolved via import {} after {:?}",
                    use_decl.module.name,
                    started.elapsed()
                ));
                return Some(Self::hover_markdown(contents));
            }
        }

        if let Some(contents) = Self::hover_contents_for_local_binding(
            current_module,
            &ident,
            position,
            inferred_current,
            Some(workspace_modules),
        ) {
            Self::hover_debug(format!(
                "build_hover_ws: resolved local binding in {} after {:?}",
                current_module.name.name,
                started.elapsed()
            ));
            return Some(Self::hover_markdown(contents));
        }
        // Fallback: look up the smallest span containing the cursor position.
        if let Some(contents) =
            Self::hover_from_span_types(&ident, position, &span_types, &current_module.name.name)
        {
            Self::hover_debug(format!(
                "build_hover_ws: resolved from span types in {} after {:?}",
                current_module.name.name,
                started.elapsed()
            ));
            return Some(Self::hover_markdown(contents));
        }
        if let Some(contents) = Self::hover_contents_for_primitive_value(&ident) {
            Self::hover_debug(format!(
                "build_hover_ws: resolved primitive token {ident:?} after {:?}",
                started.elapsed()
            ));
            return Some(Self::hover_markdown(contents));
        }
        Self::hover_debug(format!(
            "build_hover_ws: unresolved token {ident:?}; returning generic fallback after {:?}",
            started.elapsed()
        ));
        Some(Self::hover_markdown(
            Self::hover_fallback_for_unresolved_ident(&ident),
        ))
    }

    pub(super) fn build_references(
        text: &str,
        uri: &Url,
        position: Position,
        include_declaration: bool,
    ) -> Vec<Location> {
        let Some(ident) = Self::extract_identifier(text, position) else {
            return Vec::new();
        };
        let path = PathBuf::from(Self::path_from_uri(uri));
        let (modules, _) = parse_modules(&path, text);
        let mut locations = Vec::new();
        for module in modules {
            Self::collect_module_references(
                &module,
                &ident,
                text,
                uri,
                include_declaration,
                &mut locations,
            );
        }
        locations
    }

    pub(super) fn build_references_with_workspace(
        text: &str,
        uri: &Url,
        position: Position,
        include_declaration: bool,
        workspace_modules: &HashMap<String, IndexedModule>,
    ) -> Vec<Location> {
        let Some(ident) = Self::extract_identifier(text, position) else {
            return Vec::new();
        };

        let path = PathBuf::from(Self::path_from_uri(uri));
        let (modules, _) = parse_modules(&path, text);
        let Some(current_module) = Self::module_at_position(&modules, position) else {
            return Self::build_references(text, uri, position, include_declaration);
        };

        let origin_module = if Self::module_member_definition_range(current_module, &ident)
            .is_some()
        {
            Some(current_module.name.name.clone())
        } else {
            current_module
                .uses
                .iter()
                .find(|use_decl| {
                    use_decl.wildcard || use_decl.items.iter().any(|item| item.name.name == ident)
                })
                .map(|use_decl| use_decl.module.name.clone())
        };

        let Some(origin_module) = origin_module else {
            return Self::build_references(text, uri, position, include_declaration);
        };

        let mut locations = Vec::new();
        for (module_name, indexed) in workspace_modules.iter() {
            let should_search = module_name == &origin_module
                || indexed.module.uses.iter().any(|use_decl| {
                    use_decl.module.name == origin_module
                        && (use_decl.wildcard
                            || use_decl.items.iter().any(|item| item.name.name == ident))
                });
            if !should_search {
                continue;
            }

            let include_decl_here = include_declaration && module_name == &origin_module;

            let module_text = if let Some(t) = &indexed.text {
                Some(t.clone())
            } else {
                indexed
                    .uri
                    .to_file_path()
                    .ok()
                    .and_then(|path| fs::read_to_string(path).ok())
            };

            if let Some(module_text) = module_text {
                Self::collect_module_references(
                    &indexed.module,
                    &ident,
                    &module_text,
                    &indexed.uri,
                    include_decl_here,
                    &mut locations,
                );
            }
        }

        locations
    }

    pub(super) fn build_rename_with_workspace(
        text: &str,
        uri: &Url,
        position: Position,
        new_name: &str,
        workspace_modules: &HashMap<String, IndexedModule>,
    ) -> Option<WorkspaceEdit> {
        let _ident = Self::extract_identifier(text, position)?;

        if new_name.is_empty() || new_name.contains('.') {
            return None;
        }
        let mut chars = new_name.chars();
        let first = chars.next()?;
        if !(first.is_ascii_alphabetic() || first == '_') {
            return None;
        }
        if !chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_') {
            return None;
        }

        let locations =
            Self::build_references_with_workspace(text, uri, position, true, workspace_modules);
        if locations.is_empty() {
            return None;
        }

        let mut changes: HashMap<Url, Vec<TextEdit>> = HashMap::new();
        for location in locations {
            changes.entry(location.uri).or_default().push(TextEdit {
                range: location.range,
                new_text: new_name.to_string(),
            });
        }

        Some(WorkspaceEdit {
            changes: Some(changes),
            document_changes: None,
            change_annotations: None,
        })
    }
}
