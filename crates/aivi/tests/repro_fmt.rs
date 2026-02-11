use aivi::format_text;

#[test]
fn test_repro_user_issues() {
    let input = r#"
module examples.compiler.literals
export ints, floats, texts, suffixes, instant, tuple, records, nested, palette

ints = [0, 1, 42, -7]
floats = [0.0, 3.14, -2.5]
texts = ["plain", "Count: {1 + 2}", "user: { { name: \"A\" }.name }"]

suffixes = [10px, 100%, 30s, 1min, 3.14dec, 42n]
instant = 2024-05-21T12:00:00Z

tuple = (1, "ok", True)

records = [
  { id: 1, label: "alpha", meta: { score: 9.5, active: True } }
  { id: 2, label: "beta", meta: { score: 7.0, active: False } }
]

nested = {
  title: "Report",
  stats: { count: 3, avg: 1.5 },
  tags: ["a", "b", "c"]
}

palette = [
  { name: "ink", rgb: (12, 15, 20) },
  { name: "sand", rgb: (242, 233, 210) }
]
"#;
    let formatted = format_text(input);
    println!(
        "--- Formatted Output ---\n{}\n------------------------",
        formatted
    );

    // Asserting that these things should NOT happen
    assert!(
        !formatted.contains("- 7"),
        "Should not have space in negative numbers"
    );
    assert!(
        !formatted.contains("10 px"),
        "Should not have space in suffixes (px)"
    );
    assert!(
        !formatted.contains("100 %"),
        "Should not have space in suffixes (%)"
    );
    assert!(
        !formatted.contains("2024 - 05"),
        "Should not have space in dates"
    );
    assert!(
        !formatted.contains("}\n,"),
        "Should not have dangling commas on new lines"
    );

    // Check records have spaces inside { }
    assert!(
        formatted.contains("{id: 1, label: \"alpha\", meta: {score: 9.5, active: True}}")
            || formatted
                .contains("{ id: 1, label: \"alpha\", meta: { score: 9.5, active: True } }"),
        "Should have spaces inside record braces if on one line"
    );
}
