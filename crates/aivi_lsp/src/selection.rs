use std::path::PathBuf;

use aivi::{parse_modules, BlockItem, DomainItem, Expr, ModuleItem, Span};
use tower_lsp::lsp_types::{Position, SelectionRange, Url};

use crate::backend::Backend;

impl Backend {
    pub(super) fn build_selection_ranges(
        text: &str,
        uri: &Url,
        positions: &[Position],
    ) -> Vec<SelectionRange> {
        let path = PathBuf::from(Self::path_from_uri(uri));
        let (modules, _) = parse_modules(&path, text);

        positions
            .iter()
            .map(|pos| {
                let mut spans: Vec<Span> = Vec::new();

                for module in &modules {
                    if Self::sel_span_contains(&module.span, *pos) {
                        spans.push(module.span.clone());
                    }
                    for item in &module.items {
                        Self::collect_sel_item_spans(&mut spans, item, *pos);
                    }
                }

                // Sort spans from largest to smallest (outermost first).
                spans.sort_by(|a, b| {
                    let a_size = Self::sel_span_area(a);
                    let b_size = Self::sel_span_area(b);
                    b_size.cmp(&a_size)
                });

                // Deduplicate equal ranges.
                spans.dedup_by(|a, b| {
                    Self::span_to_range(a.clone()) == Self::span_to_range(b.clone())
                });

                // Build nested SelectionRange from outermost to innermost.
                let mut result = SelectionRange {
                    range: Self::full_document_range(text),
                    parent: None,
                };
                for span in &spans {
                    result = SelectionRange {
                        range: Self::span_to_range(span.clone()),
                        parent: Some(Box::new(result)),
                    };
                }
                result
            })
            .collect()
    }

    fn sel_span_contains(span: &Span, pos: Position) -> bool {
        let start_line = span.start.line.saturating_sub(1) as u32;
        let end_line = span.end.line.saturating_sub(1) as u32;
        let start_col = span.start.column.saturating_sub(1) as u32;
        let end_col = span.end.column as u32;

        if pos.line < start_line || pos.line > end_line {
            return false;
        }
        if pos.line == start_line && pos.character < start_col {
            return false;
        }
        if pos.line == end_line && pos.character > end_col {
            return false;
        }
        true
    }

    fn sel_span_area(span: &Span) -> u64 {
        let lines = span.end.line.saturating_sub(span.start.line) as u64;
        let cols = span.end.column.saturating_sub(span.start.column) as u64;
        lines * 10000 + cols
    }

    fn collect_sel_item_spans(spans: &mut Vec<Span>, item: &ModuleItem, pos: Position) {
        match item {
            ModuleItem::Def(d) => {
                if Self::sel_span_contains(&d.span, pos) {
                    spans.push(d.span.clone());
                    Self::collect_sel_expr_spans(spans, &d.expr, pos);
                }
            }
            ModuleItem::TypeDecl(d) => {
                if Self::sel_span_contains(&d.span, pos) {
                    spans.push(d.span.clone());
                }
            }
            ModuleItem::TypeAlias(d) => {
                if Self::sel_span_contains(&d.span, pos) {
                    spans.push(d.span.clone());
                }
            }
            ModuleItem::TypeSig(d) => {
                if Self::sel_span_contains(&d.span, pos) {
                    spans.push(d.span.clone());
                }
            }
            ModuleItem::ClassDecl(d) => {
                if Self::sel_span_contains(&d.span, pos) {
                    spans.push(d.span.clone());
                }
            }
            ModuleItem::InstanceDecl(d) => {
                if Self::sel_span_contains(&d.span, pos) {
                    spans.push(d.span.clone());
                    for def in &d.defs {
                        if Self::sel_span_contains(&def.span, pos) {
                            spans.push(def.span.clone());
                            Self::collect_sel_expr_spans(spans, &def.expr, pos);
                        }
                    }
                }
            }
            ModuleItem::DomainDecl(d) => {
                if Self::sel_span_contains(&d.span, pos) {
                    spans.push(d.span.clone());
                    for di in &d.items {
                        match di {
                            DomainItem::Def(def) | DomainItem::LiteralDef(def) => {
                                if Self::sel_span_contains(&def.span, pos) {
                                    spans.push(def.span.clone());
                                    Self::collect_sel_expr_spans(spans, &def.expr, pos);
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            ModuleItem::MachineDecl(d) => {
                if Self::sel_span_contains(&d.span, pos) {
                    spans.push(d.span.clone());
                }
            }
        }
    }

    fn collect_sel_expr_spans(spans: &mut Vec<Span>, expr: &Expr, pos: Position) {
        let span = Self::expr_span(expr);
        if !Self::sel_span_contains(span, pos) {
            return;
        }
        spans.push(span.clone());

        match expr {
            Expr::Block { items, .. } => {
                for item in items {
                    Self::collect_sel_block_item_spans(spans, item, pos);
                }
            }
            Expr::Match {
                scrutinee, arms, ..
            } => {
                if let Some(s) = scrutinee {
                    Self::collect_sel_expr_spans(spans, s, pos);
                }
                for arm in arms {
                    if Self::sel_span_contains(&arm.span, pos) {
                        spans.push(arm.span.clone());
                        Self::collect_sel_expr_spans(spans, &arm.body, pos);
                    }
                }
            }
            Expr::Lambda { body, .. } => {
                Self::collect_sel_expr_spans(spans, body, pos);
            }
            Expr::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => {
                Self::collect_sel_expr_spans(spans, cond, pos);
                Self::collect_sel_expr_spans(spans, then_branch, pos);
                Self::collect_sel_expr_spans(spans, else_branch, pos);
            }
            Expr::Call { func, args, .. } => {
                Self::collect_sel_expr_spans(spans, func, pos);
                for arg in args {
                    Self::collect_sel_expr_spans(spans, arg, pos);
                }
            }
            Expr::Binary { left, right, .. } => {
                Self::collect_sel_expr_spans(spans, left, pos);
                Self::collect_sel_expr_spans(spans, right, pos);
            }
            Expr::FieldAccess { base, .. } => {
                Self::collect_sel_expr_spans(spans, base, pos);
            }
            Expr::Index { base, index, .. } => {
                Self::collect_sel_expr_spans(spans, base, pos);
                Self::collect_sel_expr_spans(spans, index, pos);
            }
            Expr::Record { fields, .. } | Expr::PatchLit { fields, .. } => {
                for field in fields {
                    Self::collect_sel_expr_spans(spans, &field.value, pos);
                }
            }
            Expr::List { items, .. } => {
                for item in items {
                    Self::collect_sel_expr_spans(spans, &item.expr, pos);
                }
            }
            Expr::Tuple { items, .. } => {
                for item in items {
                    Self::collect_sel_expr_spans(spans, item, pos);
                }
            }
            Expr::UnaryNeg { expr, .. } | Expr::Suffixed { base: expr, .. } => {
                Self::collect_sel_expr_spans(spans, expr, pos);
            }
            Expr::Mock { body, .. } => {
                Self::collect_sel_expr_spans(spans, body, pos);
            }
            _ => {}
        }
    }

    fn collect_sel_block_item_spans(spans: &mut Vec<Span>, item: &BlockItem, pos: Position) {
        match item {
            BlockItem::Bind { expr, span, .. }
            | BlockItem::Let { expr, span, .. }
            | BlockItem::Filter { expr, span, .. }
            | BlockItem::Yield { expr, span, .. }
            | BlockItem::Recurse { expr, span, .. }
            | BlockItem::Expr { expr, span, .. } => {
                if Self::sel_span_contains(span, pos) {
                    spans.push(span.clone());
                    Self::collect_sel_expr_spans(spans, expr, pos);
                }
            }
            BlockItem::When {
                cond, effect, span, ..
            }
            | BlockItem::Unless {
                cond, effect, span, ..
            } => {
                if Self::sel_span_contains(span, pos) {
                    spans.push(span.clone());
                    Self::collect_sel_expr_spans(spans, cond, pos);
                    Self::collect_sel_expr_spans(spans, effect, pos);
                }
            }
            BlockItem::Given {
                cond,
                fail_expr,
                span,
                ..
            } => {
                if Self::sel_span_contains(span, pos) {
                    spans.push(span.clone());
                    Self::collect_sel_expr_spans(spans, cond, pos);
                    Self::collect_sel_expr_spans(spans, fail_expr, pos);
                }
            }
            BlockItem::On {
                transition,
                handler,
                span,
                ..
            } => {
                if Self::sel_span_contains(span, pos) {
                    spans.push(span.clone());
                    Self::collect_sel_expr_spans(spans, transition, pos);
                    Self::collect_sel_expr_spans(spans, handler, pos);
                }
            }
        }
    }
}
