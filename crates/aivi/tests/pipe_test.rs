#[test]
fn test_pipe_preserved_without_alignment() {
    // Single arm - no alignment group should form
    let input = "f = x match\n  | a => b\n";
    let formatted = aivi::format_text(input);
    assert_eq!(formatted, "f = x match\n  | a => b\n");
}

#[test]
fn test_pipe_preserved_with_alignment() {
    // Two arms - alignment group should form
    let input = "f = x match\n  | a => b\n  | c => d\n";
    let formatted = aivi::format_text(input);
    assert_eq!(formatted, "f = x match\n  | a => b\n  | c => d\n");
}
