use crate::lexer::lex;
use crate::syntax;

use super::is_op;
use super::{BraceStyle, FormatOptions};

// Current formatter engine implementation.
//
// The legacy formatter lives in `formatter/format_text_with_options_body.rs` and is included
// here to keep the public API stable while we incrementally migrate rules onto the `Doc`
// model (`formatter/doc.rs`).
//
// IMPORTANT: This module must remain deterministic and must not change semantics (formatting
// should always round-trip through the parser).

pub fn format_text_with_options(content: &str, options: FormatOptions) -> String {
    let _ = BraceStyle::Kr; // keep the enum reachable for `include!`-local logic
    include!("format_text_with_options_body.rs")
}
