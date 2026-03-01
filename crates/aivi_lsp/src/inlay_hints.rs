use std::collections::HashMap;
use std::path::PathBuf;

use aivi::{infer_value_types, parse_modules, Module, ModuleItem, Span};
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
