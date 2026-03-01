use std::path::PathBuf;

use aivi::{parse_modules, BlockItem, DomainItem, Expr, ModuleItem};
use tower_lsp::lsp_types::{FoldingRange, FoldingRangeKind, Url};

use crate::backend::Backend;

impl Backend {
    pub(super) fn build_folding_ranges(text: &str, uri: &Url) -> Vec<FoldingRange> {
        let path = PathBuf::from(Self::path_from_uri(uri));
        let (modules, _) = parse_modules(&path, text);
        let mut ranges = Vec::new();

        // Collect contiguous use-declaration blocks as import folds.
        for module in &modules {
            Self::collect_use_fold(&mut ranges, module);
            for item in &module.items {
                Self::collect_item_folds(&mut ranges, item);
            }
            // Module-level fold (if multi-line).
            let start = module.span.start.line.saturating_sub(1) as u32;
            let end = module.span.end.line.saturating_sub(1) as u32;
            if end > start {
                ranges.push(FoldingRange {
                    start_line: start,
                    start_character: None,
                    end_line: end,
                    end_character: None,
                    kind: Some(FoldingRangeKind::Region),
                    collapsed_text: Some(format!("module {} …", module.name.name)),
                });
            }
        }

        ranges
    }

    fn collect_use_fold(ranges: &mut Vec<FoldingRange>, module: &aivi::Module) {
        if module.uses.len() < 2 {
            return;
        }
        let first = &module.uses[0];
        let last = &module.uses[module.uses.len() - 1];
        let start = first.span.start.line.saturating_sub(1) as u32;
        let end = last.span.end.line.saturating_sub(1) as u32;
        if end > start {
            ranges.push(FoldingRange {
                start_line: start,
                start_character: None,
                end_line: end,
                end_character: None,
                kind: Some(FoldingRangeKind::Imports),
                collapsed_text: Some("imports …".to_string()),
            });
        }
    }

    fn collect_item_folds(ranges: &mut Vec<FoldingRange>, item: &ModuleItem) {
        let span = match item {
            ModuleItem::Def(d) => {
                Self::collect_expr_folds(ranges, &d.expr);
                &d.span
            }
            ModuleItem::TypeDecl(d) => &d.span,
            ModuleItem::TypeAlias(d) => &d.span,
            ModuleItem::TypeSig(d) => &d.span,
            ModuleItem::ClassDecl(d) => &d.span,
            ModuleItem::InstanceDecl(d) => {
                for def in &d.defs {
                    Self::collect_expr_folds(ranges, &def.expr);
                }
                &d.span
            }
            ModuleItem::DomainDecl(d) => {
                for di in &d.items {
                    match di {
                        DomainItem::Def(def) | DomainItem::LiteralDef(def) => {
                            Self::collect_expr_folds(ranges, &def.expr);
                        }
                        _ => {}
                    }
                }
                &d.span
            }
            ModuleItem::MachineDecl(d) => &d.span,
        };
        let start = span.start.line.saturating_sub(1) as u32;
        let end = span.end.line.saturating_sub(1) as u32;
        if end > start {
            ranges.push(FoldingRange {
                start_line: start,
                start_character: None,
                end_line: end,
                end_character: None,
                kind: Some(FoldingRangeKind::Region),
                collapsed_text: None,
            });
        }
    }

    fn collect_expr_folds(ranges: &mut Vec<FoldingRange>, expr: &Expr) {
        let span = match expr {
            Expr::Block { items, span, .. } => {
                for item in items {
                    Self::collect_block_item_folds(ranges, item);
                }
                span
            }
            Expr::Match { arms, span, .. } => {
                for arm in arms {
                    Self::collect_expr_folds(ranges, &arm.body);
                }
                span
            }
            Expr::Lambda { body, span, .. } => {
                Self::collect_expr_folds(ranges, body);
                span
            }
            Expr::Record { span, .. } | Expr::PatchLit { span, .. } => span,
            Expr::List { span, .. } => span,
            Expr::If {
                then_branch,
                else_branch,
                span,
                ..
            } => {
                Self::collect_expr_folds(ranges, then_branch);
                Self::collect_expr_folds(ranges, else_branch);
                span
            }
            Expr::Call { func, args, span } => {
                Self::collect_expr_folds(ranges, func);
                for arg in args {
                    Self::collect_expr_folds(ranges, arg);
                }
                span
            }
            _ => return,
        };
        let start = span.start.line.saturating_sub(1) as u32;
        let end = span.end.line.saturating_sub(1) as u32;
        if end > start {
            ranges.push(FoldingRange {
                start_line: start,
                start_character: None,
                end_line: end,
                end_character: None,
                kind: Some(FoldingRangeKind::Region),
                collapsed_text: None,
            });
        }
    }

    fn collect_block_item_folds(ranges: &mut Vec<FoldingRange>, item: &BlockItem) {
        match item {
            BlockItem::Bind { expr, .. }
            | BlockItem::Let { expr, .. }
            | BlockItem::Filter { expr, .. }
            | BlockItem::Yield { expr, .. }
            | BlockItem::Recurse { expr, .. }
            | BlockItem::Expr { expr, .. } => {
                Self::collect_expr_folds(ranges, expr);
            }
            BlockItem::When { cond, effect, .. } | BlockItem::Unless { cond, effect, .. } => {
                Self::collect_expr_folds(ranges, cond);
                Self::collect_expr_folds(ranges, effect);
            }
            BlockItem::Given {
                cond, fail_expr, ..
            } => {
                Self::collect_expr_folds(ranges, cond);
                Self::collect_expr_folds(ranges, fail_expr);
            }
            BlockItem::On {
                transition,
                handler,
                ..
            } => {
                Self::collect_expr_folds(ranges, transition);
                Self::collect_expr_folds(ranges, handler);
            }
        }
    }
}
