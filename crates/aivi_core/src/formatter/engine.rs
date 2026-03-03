use crate::lexer::lex;
use crate::syntax;

use super::is_op;
use super::{BraceStyle, FormatOptions};

// Current formatter engine implementation.
//
// The formatter rule source-of-truth lives in this crate at
// `crates/aivi_core/src/formatter/rules.rs`.

// Core types and token classification helpers.
include!("rules/helpers.rs");

// Markup / GTK sigil formatting.
include!("rules/sigils.rs");

// Matrix sigil formatting.
include!("rules/matrix.rs");

pub fn format_text_with_options(content: &str, options: FormatOptions) -> String {
    let _ = BraceStyle::Kr; // keep the enum reachable for `include!`-local logic
    include!("rules.rs")
}
