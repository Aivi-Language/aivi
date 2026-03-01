use aivi::{DomainItem, ModuleItem, Span};
use tower_lsp::lsp_types::{Location, SymbolInformation, SymbolKind};

use crate::backend::Backend;
use crate::state::IndexedModule;

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
                                symbols.push(SymbolInformation {
                                    name: n.to_string(),
                                    kind: k,
                                    tags: None,
                                    deprecated: None,
                                    location: Location::new(uri.clone(), range),
                                    container_name: Some(format!(
                                        "{}.{}",
                                        module.name.name, d.name.name
                                    )),
                                });
                            }
                        }
                        (d.name.name.as_str(), SymbolKind::NAMESPACE, &d.span)
                    }
                    ModuleItem::MachineDecl(d) => {
                        (d.name.name.as_str(), SymbolKind::CLASS, &d.span)
                    }
                };

                if Self::symbol_matches(name, &query_lower) {
                    let range = Self::span_to_range(span.clone());
                    symbols.push(SymbolInformation {
                        name: name.to_string(),
                        kind,
                        tags: None,
                        deprecated: None,
                        location: Location::new(uri.clone(), range),
                        container_name: Some(module.name.name.clone()),
                    });
                }
            }
        }

        symbols
    }

    fn symbol_matches(name: &str, query_lower: &str) -> bool {
        if query_lower.is_empty() {
            return true;
        }
        name.to_lowercase().contains(query_lower)
    }
}
