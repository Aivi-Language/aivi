use std::path::PathBuf;

use aivi::{parse_modules, DomainItem, ModuleItem};
use tower_lsp::lsp_types::{DocumentSymbol, SymbolKind, Url};

use crate::backend::Backend;

impl Backend {
    #[allow(deprecated)]
    pub(super) fn build_document_symbols(text: &str, uri: &Url) -> Vec<DocumentSymbol> {
        let path = PathBuf::from(Self::path_from_uri(uri));
        let (modules, _) = parse_modules(&path, text);
        modules
            .into_iter()
            .map(|module| {
                let mut children = Vec::new();
                for export in module.exports {
                    let range = Self::span_to_range(export.name.span);
                    children.push(DocumentSymbol {
                        name: export.name.name,
                        detail: Some("export".to_string()),
                        kind: SymbolKind::PROPERTY,
                        tags: None,
                        deprecated: None,
                        range,
                        selection_range: range,
                        children: None,
                    });
                }
                for item in module.items {
                    if let Some(symbol) = Self::symbol_from_item(item) {
                        children.push(symbol);
                    }
                }
                let range = Self::span_to_range(module.span);
                DocumentSymbol {
                    name: module.name.name,
                    detail: Some("module".to_string()),
                    kind: SymbolKind::MODULE,
                    tags: None,
                    deprecated: None,
                    range,
                    selection_range: range,
                    children: Some(children),
                }
            })
            .collect()
    }

    #[allow(deprecated)]
    fn symbol_from_item(item: ModuleItem) -> Option<DocumentSymbol> {
        match item {
            ModuleItem::Def(def) => {
                let range = Self::span_to_range(def.span);
                Some(DocumentSymbol {
                    name: def.name.name,
                    detail: None,
                    kind: SymbolKind::FUNCTION,
                    tags: None,
                    deprecated: None,
                    range,
                    selection_range: range,
                    children: None,
                })
            }
            ModuleItem::TypeSig(sig) => {
                let range = Self::span_to_range(sig.span);
                Some(DocumentSymbol {
                    name: sig.name.name,
                    detail: Some("type signature".to_string()),
                    kind: SymbolKind::FUNCTION,
                    tags: None,
                    deprecated: None,
                    range,
                    selection_range: range,
                    children: None,
                })
            }
            ModuleItem::TypeDecl(decl) => {
                let range = Self::span_to_range(decl.span);
                Some(DocumentSymbol {
                    name: decl.name.name,
                    detail: Some("type".to_string()),
                    kind: SymbolKind::STRUCT,
                    tags: None,
                    deprecated: None,
                    range,
                    selection_range: range,
                    children: None,
                })
            }
            ModuleItem::TypeAlias(alias) => {
                let range = Self::span_to_range(alias.span);
                Some(DocumentSymbol {
                    name: alias.name.name,
                    detail: Some("type alias".to_string()),
                    kind: SymbolKind::INTERFACE,
                    tags: None,
                    deprecated: None,
                    range,
                    selection_range: range,
                    children: None,
                })
            }
            ModuleItem::ClassDecl(class_decl) => {
                let range = Self::span_to_range(class_decl.span);
                Some(DocumentSymbol {
                    name: class_decl.name.name,
                    detail: Some("class".to_string()),
                    kind: SymbolKind::CLASS,
                    tags: None,
                    deprecated: None,
                    range,
                    selection_range: range,
                    children: None,
                })
            }
            ModuleItem::InstanceDecl(instance_decl) => {
                let range = Self::span_to_range(instance_decl.span);
                Some(DocumentSymbol {
                    name: instance_decl.name.name,
                    detail: Some("instance".to_string()),
                    kind: SymbolKind::OBJECT,
                    tags: None,
                    deprecated: None,
                    range,
                    selection_range: range,
                    children: None,
                })
            }
            ModuleItem::DomainDecl(domain_decl) => {
                let mut children = Vec::new();
                for domain_item in domain_decl.items {
                    match domain_item {
                        DomainItem::TypeAlias(type_alias) => {
                            let range = Self::span_to_range(type_alias.span);
                            children.push(DocumentSymbol {
                                name: type_alias.name.name,
                                detail: Some("domain type".to_string()),
                                kind: SymbolKind::INTERFACE,
                                tags: None,
                                deprecated: None,
                                range,
                                selection_range: range,
                                children: None,
                            });
                        }
                        DomainItem::TypeSig(sig) => {
                            let range = Self::span_to_range(sig.span);
                            children.push(DocumentSymbol {
                                name: sig.name.name,
                                detail: Some("domain sig".to_string()),
                                kind: SymbolKind::FUNCTION,
                                tags: None,
                                deprecated: None,
                                range,
                                selection_range: range,
                                children: None,
                            });
                        }
                        DomainItem::Def(def) | DomainItem::LiteralDef(def) => {
                            let range = Self::span_to_range(def.span);
                            children.push(DocumentSymbol {
                                name: def.name.name,
                                detail: Some("domain def".to_string()),
                                kind: SymbolKind::FUNCTION,
                                tags: None,
                                deprecated: None,
                                range,
                                selection_range: range,
                                children: None,
                            });
                        }
                    }
                }
                let range = Self::span_to_range(domain_decl.span);
                Some(DocumentSymbol {
                    name: domain_decl.name.name,
                    detail: Some("domain".to_string()),
                    kind: SymbolKind::NAMESPACE,
                    tags: None,
                    deprecated: None,
                    range,
                    selection_range: range,
                    children: Some(children),
                })
            }
            ModuleItem::MachineDecl(_) => None,
        }
    }
}
