use std::path::PathBuf;

use aivi::{file_diagnostics_have_errors, format_text, parse_modules};

#[test]
fn format_parse_format_is_stable_on_small_parseable_input() {
    // Many `integration-tests/complex` files are intentionally ahead of the parser/typechecker.
    // Keep this test focused on formatter stability for a snippet we know should parse today.
    let input = r#"
module fmt.parse.format

use aivi

example = if True then 1 else 2
"#;

    let path = PathBuf::from("fmt_parse_format_test.aivi");
    let formatted1 = format_text(input);
    let (_modules, diags) = parse_modules(&path, &formatted1);
    assert!(
        !file_diagnostics_have_errors(&diags),
        "formatter output should parse without errors: {diags:?}"
    );

    let formatted2 = format_text(&formatted1);
    assert_eq!(formatted1, formatted2, "formatter must be idempotent");
}
