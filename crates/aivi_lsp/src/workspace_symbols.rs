use aivi::{DomainItem, ModuleItem, Span};
use tower_lsp::lsp_types::{Location, SymbolInformation, SymbolKind};

use crate::backend::Backend;
use crate::state::IndexedModule;

const WORKSPACE_SYMBOL_LIMIT: usize = 1_000;

struct RankedSymbol {
    symbol: SymbolInformation,
    score: u8,
    name_lower: String,
    container_lower: String,
}

impl Backend {
    #[allow(deprecated)]
    pub(super) fn build_workspace_symbols(
        query: &str,
        modules: &[IndexedModule],
    ) -> Vec<SymbolInformation> {
        let query_lower = query.to_lowercase();
        let mut symbols = Vec::new();

        for indexed in modules {
            let module = &indexed.module;
            let uri = &indexed.uri;

            for item in &module.items {
                let (name, kind, span): (&str, SymbolKind, &Span) = match item {
                    ModuleItem::Def(d) => (d.name.name.as_str(), SymbolKind::FUNCTION, &d.span),
                    ModuleItem::TypeSig(s) => (s.name.name.as_str(), SymbolKind::FUNCTION, &s.span),
                    ModuleItem::TypeDecl(d) => (d.name.name.as_str(), SymbolKind::ENUM, &d.span),
                    ModuleItem::TypeAlias(d) => {
                        (d.name.name.as_str(), SymbolKind::TYPE_PARAMETER, &d.span)
                    }
                    ModuleItem::ClassDecl(d) => {
                        (d.name.name.as_str(), SymbolKind::INTERFACE, &d.span)
                    }
                    ModuleItem::InstanceDecl(d) => {
                        (d.name.name.as_str(), SymbolKind::OBJECT, &d.span)
                    }
                    ModuleItem::DomainDecl(d) => {
                        for di in &d.items {
                            let (n, k, s): (&str, SymbolKind, &Span) = match di {
                                DomainItem::Def(def) | DomainItem::LiteralDef(def) => {
                                    (def.name.name.as_str(), SymbolKind::FUNCTION, &def.span)
                                }
                                DomainItem::TypeSig(sig) => {
                                    (sig.name.name.as_str(), SymbolKind::FUNCTION, &sig.span)
                                }
                                DomainItem::TypeAlias(ta) => {
                                    (ta.name.name.as_str(), SymbolKind::ENUM, &ta.span)
                                }
                            };
                            if Self::symbol_matches(n, &query_lower) {
                                let range = Self::span_to_range(s.clone());
                                let symbol = SymbolInformation {
                                    name: n.to_string(),
                                    kind: k,
                                    tags: None,
                                    deprecated: None,
                                    location: Location::new(uri.clone(), range),
                                    container_name: Some(format!(
                                        "{}.{}",
                                        module.name.name, d.name.name
                                    )),
                                };
                                symbols.push(Self::ranked_symbol(symbol, &query_lower));
                            }
                        }
                        (d.name.name.as_str(), SymbolKind::NAMESPACE, &d.span)
                    }
                };

                if Self::symbol_matches(name, &query_lower) {
                    let range = Self::span_to_range(span.clone());
                    let symbol = SymbolInformation {
                        name: name.to_string(),
                        kind,
                        tags: None,
                        deprecated: None,
                        location: Location::new(uri.clone(), range),
                        container_name: Some(module.name.name.clone()),
                    };
                    symbols.push(Self::ranked_symbol(symbol, &query_lower));
                }
            }
        }

        symbols.sort_by(|left, right| {
            right
                .score
                .cmp(&left.score)
                .then_with(|| left.name_lower.len().cmp(&right.name_lower.len()))
                .then_with(|| left.name_lower.cmp(&right.name_lower))
                .then_with(|| left.container_lower.cmp(&right.container_lower))
                .then_with(|| {
                    left.symbol
                        .location
                        .uri
                        .as_str()
                        .cmp(right.symbol.location.uri.as_str())
                })
        });
        symbols.truncate(WORKSPACE_SYMBOL_LIMIT);
        symbols.into_iter().map(|entry| entry.symbol).collect()
    }

    fn symbol_matches(name: &str, query_lower: &str) -> bool {
        if query_lower.is_empty() {
            return true;
        }
        name.to_lowercase().contains(query_lower)
    }

    fn ranked_symbol(symbol: SymbolInformation, query_lower: &str) -> RankedSymbol {
        let name_lower = symbol.name.to_lowercase();
        let container_lower = symbol
            .container_name
            .as_deref()
            .unwrap_or_default()
            .to_lowercase();
        RankedSymbol {
            score: Self::workspace_symbol_score(&name_lower, query_lower),
            symbol,
            name_lower,
            container_lower,
        }
    }

    fn workspace_symbol_score(name_lower: &str, query_lower: &str) -> u8 {
        if query_lower.is_empty() {
            0
        } else if name_lower == query_lower {
            3
        } else if name_lower.starts_with(query_lower) {
            2
        } else {
            1
        }
    }
}
