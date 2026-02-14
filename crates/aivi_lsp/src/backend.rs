use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use aivi::{
    BlockItem, ClassDecl, Def, DomainDecl, DomainItem, Expr, InstanceDecl, ListItem, MatchArm,
    Module, ModuleItem, PathSegment, Pattern, RecordField, RecordPatternField, Span, SpannedName,
    TypeAlias, TypeCtor, TypeDecl, TypeExpr, UseDecl,
};
use tokio::sync::Mutex;
use tower_lsp::lsp_types::{Location, Position, Range, TextEdit, Url};
use tower_lsp::Client;

use crate::doc_index::{DocIndex, QuickInfoEntry, QuickInfoKind};
use crate::state::BackendState;

pub(super) struct Backend {
    pub(super) client: Client,
    pub(super) state: Arc<Mutex<BackendState>>,
}

include!("backend/impl/formatting_and_offsets.rs");
include!("backend/impl/hover_and_docs.rs");
include!("backend/impl/module_references.rs");
include!("backend/impl/references_collect.rs");
include!("backend/impl/references_walk.rs");
