use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use aivi::{parse_modules, BlockItem, Def, DomainItem, Expr, MatchArm, ModuleItem, Pattern};
use tower_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, Documentation, MarkupContent, MarkupKind, Position, Url,
};

use crate::backend::Backend;
use crate::state::IndexedModule;

impl Backend {
    pub(super) fn build_completion_items(
        text: &str,
        uri: &Url,
        position: Position,
        workspace_modules: &HashMap<String, IndexedModule>,
    ) -> Vec<CompletionItem> {
        let path = PathBuf::from(Self::path_from_uri(uri));
        let (file_modules, _) = parse_modules(&path, text);

        // Find the module in this file that contains the cursor.
        let current_module_name = file_modules
            .iter()
            .find(|m| {
                let range = Self::span_to_range(m.span.clone());
                Self::range_contains_position(&range, position)
            })
            .map(|m| m.name.name.clone());

        let mut module_map = HashMap::new();
        for module in file_modules {
            module_map.insert(module.name.name.clone(), module);
        }
        for indexed in workspace_modules.values() {
            module_map
                .entry(indexed.module.name.name.clone())
                .or_insert_with(|| indexed.module.clone());
        }

        let mut seen = HashSet::new();
        let mut items = Vec::new();
        let mut push_item = |item: CompletionItem| {
            let kind_key = item.kind.unwrap_or(CompletionItemKind::TEXT);
            let key = format!(
                "{}:{kind_key:?}:{}",
                item.label,
                item.detail.as_deref().unwrap_or("")
            );
            if seen.insert(key) {
                items.push(item);
            }
        };

        let line_prefix = Self::line_prefix(text, position);

        if let Some(prefix) = Self::use_module_prefix(&line_prefix) {
            for name in module_map.keys() {
                if name.starts_with(prefix) {
                    push_item(CompletionItem {
                        label: name.clone(),
                        kind: Some(CompletionItemKind::MODULE),
                        ..CompletionItem::default()
                    });
                }
            }
            return items;
        }

        if let Some((module_name, already_imported, member_prefix)) =
            Self::use_exports_context(&line_prefix)
        {
            if let Some(module) = module_map.get(module_name) {
                for (label, kind, detail) in Self::module_export_completions(module) {
                    if already_imported.contains(&label) {
                        continue;
                    }
                    if !member_prefix.is_empty() && !label.starts_with(member_prefix) {
                        continue;
                    }
                    push_item(CompletionItem {
                        label,
                        kind: Some(kind),
                        detail,
                        ..CompletionItem::default()
                    });
                }
            }
            return items;
        }

        if let Some((path_prefix, member_prefix)) = Self::qualified_name_context(&line_prefix) {
            let mut produced_any = false;
            let mut module_segments = HashSet::new();
            let dotted = format!("{path_prefix}.");
            for name in module_map.keys() {
                if let Some(rest) = name.strip_prefix(&dotted) {
                    let seg = rest.split('.').next().unwrap_or(rest);
                    if seg.starts_with(&member_prefix) {
                        module_segments.insert(seg.to_string());
                    }
                }
            }
            for seg in module_segments {
                push_item(CompletionItem {
                    label: seg,
                    kind: Some(CompletionItemKind::MODULE),
                    ..CompletionItem::default()
                });
                produced_any = true;
            }

            if let Some(module) = module_map.get(&path_prefix) {
                for (label, kind, detail) in Self::module_export_completions(module) {
                    if !member_prefix.is_empty() && !label.starts_with(&member_prefix) {
                        continue;
                    }
                    push_item(CompletionItem {
                        label,
                        kind: Some(kind),
                        detail,
                        ..CompletionItem::default()
                    });
                    produced_any = true;
                }
            }

            if produced_any {
                return items;
            }
        }

        // === General completion (not in import/qualified context) ===

        // Look up current module from the map by name.
        let current_module = current_module_name
            .as_deref()
            .and_then(|name| module_map.get(name));

        // 1. Local scope: defs, params, let/bind vars visible at cursor
        if let Some(module) = current_module {
            // Build a type-signature lookup for detail strings.
            let mut type_sigs: HashMap<String, String> = HashMap::new();
            for item in &module.items {
                if let ModuleItem::TypeSig(sig) = item {
                    type_sigs.insert(
                        sig.name.name.clone(),
                        format!(": {}", Self::type_expr_to_string(&sig.ty)),
                    );
                }
            }

            // Top-level defs in current module
            for item in &module.items {
                if let Some((label, kind)) = Self::completion_from_item(item.clone()) {
                    let detail = type_sigs.get(&label).cloned();
                    push_item(CompletionItem {
                        label,
                        kind: Some(kind),
                        detail,
                        sort_text: Some("0".to_string()),
                        ..CompletionItem::default()
                    });
                }
            }

            // Constructors from type decls in current module
            for item in &module.items {
                if let ModuleItem::TypeDecl(decl) = item {
                    for ctor in &decl.constructors {
                        push_item(CompletionItem {
                            label: ctor.name.name.clone(),
                            kind: Some(CompletionItemKind::ENUM_MEMBER),
                            sort_text: Some("1".to_string()),
                            detail: Some(format!("constructor of {}", decl.name.name)),
                            ..CompletionItem::default()
                        });
                    }
                }
            }

            // Local bindings: walk the AST to find params and let/bind in scope
            let mut local_names = Vec::new();
            Self::collect_locals_at_position(module, position, &mut local_names);
            for name in local_names {
                push_item(CompletionItem {
                    label: name,
                    kind: Some(CompletionItemKind::VARIABLE),
                    sort_text: Some("0".to_string()),
                    ..CompletionItem::default()
                });
            }
        }

        // 2. Imported symbols (from use declarations)
        if let Some(module) = current_module {
            for use_decl in &module.uses {
                let mod_name = &use_decl.module.name;
                if let Some(imported_module) = module_map.get(mod_name) {
                    if use_decl.wildcard {
                        // Wildcard import: all exports
                        for (label, kind, detail) in
                            Self::module_export_completions(imported_module)
                        {
                            push_item(CompletionItem {
                                label,
                                kind: Some(kind),
                                detail,
                                sort_text: Some("2".to_string()),
                                ..CompletionItem::default()
                            });
                        }
                    } else {
                        // Selective imports
                        for use_item in &use_decl.items {
                            push_item(CompletionItem {
                                label: use_item.name.name.clone(),
                                kind: Some(CompletionItemKind::FUNCTION),
                                sort_text: Some("2".to_string()),
                                ..CompletionItem::default()
                            });
                        }
                    }
                    // If aliased, also suggest the alias for qualified access
                    if let Some(alias) = &use_decl.alias {
                        push_item(CompletionItem {
                            label: alias.name.clone(),
                            kind: Some(CompletionItemKind::MODULE),
                            sort_text: Some("2".to_string()),
                            ..CompletionItem::default()
                        });
                    }
                }
            }
        }

        // 3. Module names (for qualified references)
        for module in module_map.values() {
            push_item(CompletionItem {
                label: module.name.name.clone(),
                kind: Some(CompletionItemKind::MODULE),
                sort_text: Some("4".to_string()),
                ..CompletionItem::default()
            });
        }

        // 4. Keywords and sigils (lowest priority)
        for keyword in Self::KEYWORDS {
            push_item(CompletionItem {
                label: keyword.to_string(),
                kind: Some(CompletionItemKind::KEYWORD),
                sort_text: Some("5".to_string()),
                ..CompletionItem::default()
            });
        }
        for sigil in Self::SIGILS {
            push_item(CompletionItem {
                label: sigil.to_string(),
                kind: Some(CompletionItemKind::SNIPPET),
                sort_text: Some("5".to_string()),
                ..CompletionItem::default()
            });
        }

        // 5. Remaining workspace exports (not already imported)
        for module in module_map.values() {
            for (label, kind, detail) in Self::module_export_completions(module) {
                push_item(CompletionItem {
                    label,
                    kind: Some(kind),
                    detail,
                    sort_text: Some("3".to_string()),
                    ..CompletionItem::default()
                });
            }
        }

        items
    }

    fn completion_from_item(item: ModuleItem) -> Option<(String, CompletionItemKind)> {
        match item {
            ModuleItem::Def(def) => Some((def.name.name, CompletionItemKind::FUNCTION)),
            ModuleItem::TypeSig(sig) => Some((sig.name.name, CompletionItemKind::FUNCTION)),
            ModuleItem::TypeDecl(decl) => Some((decl.name.name, CompletionItemKind::STRUCT)),
            ModuleItem::TypeAlias(alias) => Some((alias.name.name, CompletionItemKind::INTERFACE)),
            ModuleItem::ClassDecl(class_decl) => {
                Some((class_decl.name.name, CompletionItemKind::CLASS))
            }
            ModuleItem::InstanceDecl(instance_decl) => {
                Some((instance_decl.name.name, CompletionItemKind::STRUCT))
            }
            ModuleItem::DomainDecl(domain_decl) => {
                Some((domain_decl.name.name, CompletionItemKind::MODULE))
            }
            ModuleItem::MachineDecl(_) => None,
        }
    }

    fn line_prefix(text: &str, position: Position) -> String {
        let offset = Self::offset_at(text, position).min(text.len());
        let line_start = text[..offset].rfind('\n').map(|idx| idx + 1).unwrap_or(0);
        text[line_start..offset].to_string()
    }

    fn use_module_prefix(line_prefix: &str) -> Option<&str> {
        // `use <prefix>`
        let trimmed = line_prefix.trim_start();
        let rest = trimmed.strip_prefix("use ")?;
        if rest.contains('(') {
            return None;
        }
        if rest.contains(' ') || rest.contains('\t') {
            return None;
        }
        Some(rest)
    }

    fn use_exports_context(line_prefix: &str) -> Option<(&str, HashSet<String>, &str)> {
        // `use Mod (a, b, <prefix>`
        let trimmed = line_prefix.trim_start();
        let rest = trimmed.strip_prefix("use ")?;
        let (module_name, after_module) = rest.split_once('(')?;
        let module_name = module_name.trim_end();
        if module_name.is_empty() {
            return None;
        }
        let inside = after_module;
        let mut imported = HashSet::new();
        let parts: Vec<&str> = inside.split(',').collect();
        let prefix_part = parts.last().copied().unwrap_or("");
        for part in parts.iter().take(parts.len().saturating_sub(1)) {
            let name = part.trim();
            if name.is_empty() {
                continue;
            }
            // Only handle basic `use Mod (name, ...)` items for now.
            if name
                .chars()
                .all(|ch| ch.is_alphanumeric() || ch == '_' || ch == '.')
            {
                imported.insert(name.to_string());
            }
        }
        let member_prefix = prefix_part.trim_start().trim();
        Some((module_name, imported, member_prefix))
    }

    fn qualified_name_context(line_prefix: &str) -> Option<(String, String)> {
        // If the user is typing a dotted identifier, suggest either sub-modules or members.
        let suffix = line_prefix
            .chars()
            .rev()
            .take_while(|ch| ch.is_alphanumeric() || *ch == '_' || *ch == '.')
            .collect::<String>()
            .chars()
            .rev()
            .collect::<String>();
        if !suffix.contains('.') {
            return None;
        }
        let (path_prefix, member_prefix) = suffix.rsplit_once('.')?;
        Some((path_prefix.to_string(), member_prefix.to_string()))
    }

    fn module_export_completions(
        module: &aivi::Module,
    ) -> Vec<(String, CompletionItemKind, Option<String>)> {
        let mut kind_by_name: HashMap<String, CompletionItemKind> = HashMap::new();
        let mut type_sig_by_name: HashMap<String, String> = HashMap::new();

        for item in module.items.iter().cloned() {
            match item {
                ModuleItem::TypeDecl(decl) => {
                    kind_by_name.insert(decl.name.name.clone(), CompletionItemKind::STRUCT);
                    for ctor in decl.constructors {
                        kind_by_name
                            .entry(ctor.name.name)
                            .or_insert(CompletionItemKind::CONSTRUCTOR);
                    }
                }
                ModuleItem::TypeSig(sig) => {
                    kind_by_name
                        .entry(sig.name.name.clone())
                        .or_insert(CompletionItemKind::FUNCTION);
                    type_sig_by_name.insert(
                        sig.name.name,
                        format!(": {}", Self::type_expr_to_string(&sig.ty)),
                    );
                }
                other => {
                    if let Some((label, kind)) = Self::completion_from_item(other) {
                        kind_by_name.entry(label).or_insert(kind);
                    }
                }
            }
        }

        let mut out = Vec::new();
        if !module.exports.is_empty() {
            for export in module.exports.iter() {
                let label = export.name.name.clone();
                let kind = kind_by_name
                    .get(&label)
                    .copied()
                    .unwrap_or(CompletionItemKind::PROPERTY);
                let detail = type_sig_by_name.get(&label).cloned();
                out.push((label, kind, detail));
            }
        } else {
            for (label, kind) in kind_by_name {
                let detail = type_sig_by_name.get(&label).cloned();
                out.push((label, kind, detail));
            }
        }
        out
    }

    pub(super) fn resolve_completion_item(
        mut item: CompletionItem,
        doc_index: &crate::doc_index::DocIndex,
    ) -> CompletionItem {
        // Extract module hint from data field if present.
        let module_hint = item
            .data
            .as_ref()
            .and_then(|d| d.as_str())
            .map(|s| s.to_string());

        if let Some(entry) = doc_index.lookup_best(&item.label, module_hint.as_deref()) {
            let mut doc_text = String::new();
            if let Some(sig) = &entry.signature {
                doc_text.push_str(&format!("```aivi\n{sig}\n```\n\n"));
            }
            doc_text.push_str(&entry.content);
            item.documentation = Some(Documentation::MarkupContent(MarkupContent {
                kind: MarkupKind::Markdown,
                value: doc_text,
            }));
        }

        item
    }

    /// Collect locally visible names at the given cursor position within a module.
    /// Walks defs, instance decls, and domain decls to find function params and
    /// let/bind/match-bound names in scope.
    fn collect_locals_at_position(
        module: &aivi::Module,
        position: Position,
        out: &mut Vec<String>,
    ) {
        for item in &module.items {
            match item {
                ModuleItem::Def(def) => {
                    if Self::span_contains_lsp(&def.span, position) {
                        Self::collect_pattern_names(&def.params, out);
                        Self::collect_expr_locals(&def.expr, position, out);
                    }
                }
                ModuleItem::InstanceDecl(inst) => {
                    for def in &inst.defs {
                        if Self::span_contains_lsp(&def.span, position) {
                            Self::collect_pattern_names(&def.params, out);
                            Self::collect_expr_locals(&def.expr, position, out);
                        }
                    }
                }
                ModuleItem::DomainDecl(dom) => {
                    for di in &dom.items {
                        let def = match di {
                            DomainItem::Def(d) | DomainItem::LiteralDef(d) => d,
                            _ => continue,
                        };
                        if Self::span_contains_lsp(&def.span, position) {
                            Self::collect_pattern_names(&def.params, out);
                            Self::collect_expr_locals(&def.expr, position, out);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// Recursively collect names introduced by expressions enclosing `position`.
    fn collect_expr_locals(expr: &Expr, position: Position, out: &mut Vec<String>) {
        match expr {
            Expr::Lambda { params, body, .. } => {
                if Self::expr_contains_lsp(expr, position) {
                    Self::collect_pattern_names(params, out);
                    Self::collect_expr_locals(body, position, out);
                }
            }
            Expr::Block { items, .. } => {
                if Self::expr_contains_lsp(expr, position) {
                    Self::collect_block_locals(items, position, out);
                }
            }
            Expr::Match { arms, .. } => {
                if Self::expr_contains_lsp(expr, position) {
                    Self::collect_match_arm_locals(arms, position, out);
                }
            }
            Expr::If {
                then_branch,
                else_branch,
                ..
            } => {
                if Self::expr_contains_lsp(expr, position) {
                    Self::collect_expr_locals(then_branch, position, out);
                    Self::collect_expr_locals(else_branch, position, out);
                }
            }
            Expr::Call { func, args, .. } => {
                Self::collect_expr_locals(func, position, out);
                for arg in args {
                    Self::collect_expr_locals(arg, position, out);
                }
            }
            Expr::Binary { left, right, .. } => {
                Self::collect_expr_locals(left, position, out);
                Self::collect_expr_locals(right, position, out);
            }
            _ => {}
        }
    }

    /// Collect names from block items that appear before `position` (and thus are in scope).
    fn collect_block_locals(items: &[BlockItem], position: Position, out: &mut Vec<String>) {
        for item in items {
            let (pat, expr, span) = match item {
                BlockItem::Bind {
                    pattern,
                    expr,
                    span,
                } => (Some(pattern), Some(expr), span),
                BlockItem::Let {
                    pattern,
                    expr,
                    span,
                } => (Some(pattern), Some(expr), span),
                BlockItem::Expr { expr, span } => (None, Some(expr), span),
                BlockItem::Filter { expr, span } => (None, Some(expr), span),
                BlockItem::Yield { expr, span } => (None, Some(expr), span),
                BlockItem::Recurse { expr, span } => (None, Some(expr), span),
                BlockItem::When {
                    effect, span, ..
                }
                | BlockItem::Unless {
                    effect, span, ..
                } => (None, Some(effect), span),
                BlockItem::Given {
                    fail_expr, span, ..
                } => (None, Some(fail_expr), span),
                BlockItem::On {
                    handler, span, ..
                } => (None, Some(handler), span),
            };

            // Names from bindings that start before cursor are in scope
            if Self::span_starts_before_lsp(span, position) {
                if let Some(pat) = pat {
                    Self::collect_single_pattern_names(pat, out);
                }
            }

            // Recurse into the expression if cursor is inside it
            if let Some(e) = expr {
                if Self::expr_contains_lsp(e, position) {
                    Self::collect_expr_locals(e, position, out);
                }
            }
        }
    }

    /// Collect pattern-bound names from match arms enclosing the cursor.
    fn collect_match_arm_locals(arms: &[MatchArm], position: Position, out: &mut Vec<String>) {
        for arm in arms {
            let arm_range = Self::span_to_range(Self::expr_span(&arm.body).clone());
            if Self::range_contains_position(&arm_range, position) {
                Self::collect_single_pattern_names(&arm.pattern, out);
                Self::collect_expr_locals(&arm.body, position, out);
            }
        }
    }

    /// Extract bound names from a list of patterns.
    fn collect_pattern_names(patterns: &[Pattern], out: &mut Vec<String>) {
        for pat in patterns {
            Self::collect_single_pattern_names(pat, out);
        }
    }

    /// Extract all bound names from a single pattern.
    fn collect_single_pattern_names(pattern: &Pattern, out: &mut Vec<String>) {
        match pattern {
            Pattern::Ident(name) | Pattern::SubjectIdent(name) => {
                out.push(name.name.clone());
            }
            Pattern::At {
                name, pattern: p, ..
            } => {
                out.push(name.name.clone());
                Self::collect_single_pattern_names(p, out);
            }
            Pattern::Constructor { args, .. } => {
                Self::collect_pattern_names(args, out);
            }
            Pattern::Tuple { items, .. } => {
                Self::collect_pattern_names(items, out);
            }
            Pattern::List { items, rest, .. } => {
                Self::collect_pattern_names(items, out);
                if let Some(r) = rest {
                    Self::collect_single_pattern_names(r, out);
                }
            }
            Pattern::Record { fields, .. } => {
                for field in fields {
                    Self::collect_single_pattern_names(&field.pattern, out);
                }
            }
            Pattern::Wildcard(_) | Pattern::Literal(_) => {}
        }
    }

    fn span_contains_lsp(span: &aivi::Span, position: Position) -> bool {
        let range = Self::span_to_range(span.clone());
        Self::range_contains_position(&range, position)
    }

    fn expr_contains_lsp(expr: &Expr, position: Position) -> bool {
        Self::span_contains_lsp(Self::expr_span(expr), position)
    }

    fn span_starts_before_lsp(span: &aivi::Span, position: Position) -> bool {
        let range = Self::span_to_range(span.clone());
        range.start.line < position.line
            || (range.start.line == position.line && range.start.character <= position.character)
    }
}
