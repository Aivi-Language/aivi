use crate::lexer::lex;
use crate::syntax;

use super::is_op;
use super::{BraceStyle, FormatOptions};

// Current formatter engine implementation.
//
// Formatter rules are sourced from `crates/aivi_core/src/formatter/rules.rs`
// while this crate keeps the public API stable as we incrementally migrate rules onto the `Doc`
// model (`formatter/doc.rs`).
//
// IMPORTANT: This module must remain deterministic and must not change semantics (formatting
// should always round-trip through the parser).

pub fn format_text_with_options(content: &str, options: FormatOptions) -> String {
    let _ = BraceStyle::Kr; // keep the enum reachable for `include!`-local logic
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../aivi_core/src/formatter/rules.rs"
    ))
}
