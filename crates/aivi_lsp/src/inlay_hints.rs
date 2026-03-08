use std::collections::HashMap;
use std::path::PathBuf;

use aivi::{
    infer_value_types, parse_modules, BlockItem, DomainItem, Expr, Module, ModuleItem, Pattern,
    Span,
};
use tower_lsp::lsp_types::{InlayHint, InlayHintKind, InlayHintLabel, Position, Range, Url};

use crate::backend::Backend;
use crate::state::IndexedModule;

impl Backend {
    pub(super) fn build_inlay_hints(
        text: &str,
        uri: &Url,
        range: Range,
        workspace_modules: &HashMap<String, IndexedModule>,
    ) -> Vec<InlayHint> {
        let path = PathBuf::from(Self::path_from_uri(uri));
        let (modules, _) = parse_modules(&path, text);

        let relevant = Self::relevant_modules_for_inlay(&modules, workspace_modules);
        let (_, type_strings, span_types) = infer_value_types(&relevant);

        let mut hints = Vec::new();

        for module in &modules {
            let mod_name = &module.name.name;
            let inferred = type_strings.get(mod_name);
            let mod_span_types = span_types.get(mod_name);

            // Type hints for top-level definitions without explicit type signatures.
            Self::collect_def_type_hints(&mut hints, module, inferred, range);

            // Span-based type hints for local bindings.
            if let Some(st) = mod_span_types {
                Self::collect_span_type_hints(&mut hints, st, range);
            }

            // Parameter name hints at call sites.
            Self::collect_param_name_hints(&mut hints, module, workspace_modules, range);
        }

        hints
    }

    fn relevant_modules_for_inlay<'a>(
        modules: &'a [Module],
        workspace_modules: &'a HashMap<String, IndexedModule>,
    ) -> Vec<Module> {
        let mut result: Vec<Module> = modules.to_vec();
        for indexed in workspace_modules.values() {
            if !result
                .iter()
                .any(|m| m.name.name == indexed.module.name.name)
            {
                result.push(indexed.module.clone());
            }
        }
        result
    }

    fn collect_def_type_hints(
        hints: &mut Vec<InlayHint>,
        module: &Module,
        inferred: Option<&HashMap<String, String>>,
        range: Range,
    ) {
        let Some(inferred) = inferred else { return };

        // Build a set of names that have explicit type signatures.
        let mut has_sig = std::collections::HashSet::new();
        for item in &module.items {
            if let ModuleItem::TypeSig(sig) = item {
                has_sig.insert(sig.name.name.clone());
            }
        }

        for item in &module.items {
            if let ModuleItem::Def(def) = item {
                if has_sig.contains(&def.name.name) {
                    continue;
                }
                let pos = Self::span_end_position(&def.name.span);
                if !Self::position_in_range(pos, range) {
                    continue;
                }
                if let Some(ty) = inferred.get(&def.name.name) {
                    hints.push(InlayHint {
                        position: pos,
                        label: InlayHintLabel::String(format!(": {ty}")),
                        kind: Some(InlayHintKind::TYPE),
                        text_edits: None,
                        tooltip: None,
                        padding_left: Some(true),
                        padding_right: Some(false),
                        data: None,
                    });
                }
            }
        }
    }

    fn collect_span_type_hints(
        hints: &mut Vec<InlayHint>,
        span_types: &[(Span, String)],
        range: Range,
    ) {
        for (span, ty) in span_types {
            let pos = Self::span_end_position(span);
            if !Self::position_in_range(pos, range) {
                continue;
            }
            hints.push(InlayHint {
                position: pos,
                label: InlayHintLabel::String(format!(": {ty}")),
                kind: Some(InlayHintKind::TYPE),
                text_edits: None,
                tooltip: None,
                padding_left: Some(true),
                padding_right: Some(false),
                data: None,
            });
        }
    }

    /// Parameter name hints at function call sites.
    fn collect_param_name_hints(
        hints: &mut Vec<InlayHint>,
        module: &Module,
        workspace_modules: &HashMap<String, IndexedModule>,
        range: Range,
    ) {
        // Build a map of function name -> parameter names from defs in scope.
        let mut param_names: HashMap<String, Vec<String>> = HashMap::new();

        // Collect from current module
        for item in &module.items {
            if let ModuleItem::Def(def) = item {
                let names: Vec<String> = def
                    .params
                    .iter()
                    .filter_map(Self::pattern_param_name)
                    .collect();
                if !names.is_empty() {
                    param_names.insert(def.name.name.clone(), names);
                }
            }
        }

        // Collect from imported modules
        for use_decl in &module.uses {
            let mod_name = &use_decl.module.name;
            if let Some(indexed) = workspace_modules.get(mod_name) {
                for item in &indexed.module.items {
                    if let ModuleItem::Def(def) = item {
                        let names: Vec<String> = def
                            .params
                            .iter()
                            .filter_map(Self::pattern_param_name)
                            .collect();
                        if !names.is_empty() {
                            param_names.entry(def.name.name.clone()).or_insert(names);
                        }
                    }
                }
            }
        }

        // Walk call expressions in module items
        for item in &module.items {
            match item {
                ModuleItem::Def(def) => {
                    Self::collect_call_hints_in_expr(&def.expr, &param_names, hints, range);
                }
                ModuleItem::InstanceDecl(inst) => {
                    for def in &inst.defs {
                        Self::collect_call_hints_in_expr(&def.expr, &param_names, hints, range);
                    }
                }
                ModuleItem::DomainDecl(dom) => {
                    for di in &dom.items {
                        if let DomainItem::Def(def) | DomainItem::LiteralDef(def) = di {
                            Self::collect_call_hints_in_expr(&def.expr, &param_names, hints, range);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// Extract a simple parameter name from a pattern (Ident only).
    fn pattern_param_name(pat: &Pattern) -> Option<String> {
        match pat {
            Pattern::Ident(name) | Pattern::SubjectIdent(name) => Some(name.name.clone()),
            _ => None,
        }
    }

    /// Walk an expression tree to find Call nodes and emit parameter name hints.
    fn collect_call_hints_in_expr(
        expr: &Expr,
        param_names: &HashMap<String, Vec<String>>,
        hints: &mut Vec<InlayHint>,
        range: Range,
    ) {
        match expr {
            Expr::Call { func, args, .. } => {
                // Check if the callee is a known function with param names
                if let Expr::Ident(name) = func.as_ref() {
                    if let Some(names) = param_names.get(&name.name) {
                        for (arg, pname) in args.iter().zip(names.iter()) {
                            let arg_span = Self::expr_span(arg);
                            let pos = Position::new(
                                arg_span.start.line.saturating_sub(1) as u32,
                                arg_span.start.column as u32,
                            );
                            if !Self::position_in_range(pos, range) {
                                continue;
                            }
                            // Skip if argument is a simple ident with same name as param
                            if let Expr::Ident(arg_name) = arg {
                                if arg_name.name == *pname {
                                    continue;
                                }
                            }
                            hints.push(InlayHint {
                                position: pos,
                                label: InlayHintLabel::String(format!("{pname}:")),
                                kind: Some(InlayHintKind::PARAMETER),
                                text_edits: None,
                                tooltip: None,
                                padding_left: Some(false),
                                padding_right: Some(true),
                                data: None,
                            });
                        }
                    }
                }
                // Recurse into func and args
                Self::collect_call_hints_in_expr(func, param_names, hints, range);
                for arg in args {
                    Self::collect_call_hints_in_expr(arg, param_names, hints, range);
                }
            }
            Expr::Lambda { body, .. } => {
                Self::collect_call_hints_in_expr(body, param_names, hints, range);
            }
            Expr::Block { items, .. } => {
                for item in items {
                    Self::collect_call_hints_in_block_item(item, param_names, hints, range);
                }
            }
            Expr::Match {
                scrutinee, arms, ..
            } => {
                if let Some(s) = scrutinee {
                    Self::collect_call_hints_in_expr(s, param_names, hints, range);
                }
                for arm in arms {
                    Self::collect_call_hints_in_expr(&arm.body, param_names, hints, range);
                }
            }
            Expr::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => {
                Self::collect_call_hints_in_expr(cond, param_names, hints, range);
                Self::collect_call_hints_in_expr(then_branch, param_names, hints, range);
                Self::collect_call_hints_in_expr(else_branch, param_names, hints, range);
            }
            Expr::Binary { left, right, .. } => {
                Self::collect_call_hints_in_expr(left, param_names, hints, range);
                Self::collect_call_hints_in_expr(right, param_names, hints, range);
            }
            Expr::List { items, .. } => {
                for item in items {
                    Self::collect_call_hints_in_expr(&item.expr, param_names, hints, range);
                }
            }
            Expr::Tuple { items, .. } => {
                for item in items {
                    Self::collect_call_hints_in_expr(item, param_names, hints, range);
                }
            }
            Expr::Record { fields, .. } | Expr::PatchLit { fields, .. } => {
                for field in fields {
                    Self::collect_call_hints_in_expr(&field.value, param_names, hints, range);
                }
            }
            Expr::FieldAccess { base, .. } | Expr::Index { base, .. } => {
                Self::collect_call_hints_in_expr(base, param_names, hints, range);
            }
            Expr::UnaryNeg { expr, .. } | Expr::Suffixed { base: expr, .. } => {
                Self::collect_call_hints_in_expr(expr, param_names, hints, range);
            }
            Expr::TextInterpolate { parts, .. } => {
                for part in parts {
                    if let aivi::TextPart::Expr { expr: e, .. } = part {
                        Self::collect_call_hints_in_expr(e, param_names, hints, range);
                    }
                }
            }
            Expr::Mock {
                substitutions,
                body,
                ..
            } => {
                for sub in substitutions {
                    if let Some(val) = &sub.value {
                        Self::collect_call_hints_in_expr(val, param_names, hints, range);
                    }
                }
                Self::collect_call_hints_in_expr(body, param_names, hints, range);
            }
            Expr::Ident(_) | Expr::Literal(_) | Expr::FieldSection { .. } | Expr::Raw { .. } => {}
        }
    }

    fn collect_call_hints_in_block_item(
        item: &BlockItem,
        param_names: &HashMap<String, Vec<String>>,
        hints: &mut Vec<InlayHint>,
        range: Range,
    ) {
        match item {
            BlockItem::Bind { expr, .. }
            | BlockItem::Let { expr, .. }
            | BlockItem::Expr { expr, .. }
            | BlockItem::Filter { expr, .. }
            | BlockItem::Yield { expr, .. }
            | BlockItem::Recurse { expr, .. } => {
                Self::collect_call_hints_in_expr(expr, param_names, hints, range);
            }
            BlockItem::When { cond, effect, .. } | BlockItem::Unless { cond, effect, .. } => {
                Self::collect_call_hints_in_expr(cond, param_names, hints, range);
                Self::collect_call_hints_in_expr(effect, param_names, hints, range);
            }
            BlockItem::Given {
                cond, fail_expr, ..
            } => {
                Self::collect_call_hints_in_expr(cond, param_names, hints, range);
                Self::collect_call_hints_in_expr(fail_expr, param_names, hints, range);
            }
            BlockItem::On {
                transition,
                handler,
                ..
            } => {
                Self::collect_call_hints_in_expr(transition, param_names, hints, range);
                Self::collect_call_hints_in_expr(handler, param_names, hints, range);
            }
        }
    }

    fn span_end_position(span: &Span) -> Position {
        Position::new(
            span.end.line.saturating_sub(1) as u32,
            span.end.column as u32,
        )
    }

    fn position_in_range(pos: Position, range: Range) -> bool {
        pos.line >= range.start.line && pos.line <= range.end.line
    }
}
