use crate::lexer::lex;
use crate::syntax;

use super::is_op;
use super::{BraceStyle, FormatOptions};

// Current formatter engine implementation.
//
// The legacy formatter lives in `crates/aivi/src/formatter/format_text_with_options_body.rs`.
// We include it here (using an absolute path) so this module is independent of the original
// file layout while remaining behavior-identical.

pub fn format_text_with_options(content: &str, options: FormatOptions) -> String {
    let _ = BraceStyle::Kr; // keep the enum reachable for `include!`-local logic
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../aivi/src/formatter/format_text_with_options_body.rs"
    ))
}
