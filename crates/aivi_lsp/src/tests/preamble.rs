use std::collections::HashMap;
use std::path::PathBuf;

use aivi::{format_text_with_options, parse_modules, FormatOptions, ModuleItem, Span};
use tower_lsp::lsp_types::{
    CodeActionOrCommand, DiagnosticSeverity, HoverContents, NumberOrString, Position, Url,
};

use crate::backend::Backend;
use crate::doc_index::DocIndex;
use crate::state::IndexedModule;
