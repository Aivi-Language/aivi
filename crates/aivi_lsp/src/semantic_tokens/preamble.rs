use std::collections::{HashMap, HashSet};

use aivi::{lex_cst, syntax, CstToken, Span};
use tower_lsp::lsp_types::{
    SemanticToken, SemanticTokenModifier, SemanticTokenType, SemanticTokens, SemanticTokensLegend,
};

use crate::backend::Backend;
