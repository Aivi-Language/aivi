#[test]
fn test_pipe_preserved_without_alignment() {
    // Single arm - no alignment group should form
    let input = "f = x ?\n  | a => b\n";
    let formatted = aivi::format_text(input);
    eprintln!("Input: {:?}", input);
    eprintln!("Output: {:?}", formatted);
    assert!(formatted.contains("| a"), "Pipe should be preserved");
}

#[test]
fn test_pipe_preserved_with_alignment() {
    // Two arms - alignment group should form
    let input = "f = x ?\n  | a => b\n  | c => d\n";
    let formatted = aivi::format_text(input);
    eprintln!("Input: {:?}", input);
    eprintln!("Output: {:?}", formatted);
    assert!(formatted.contains("| a"), "Pipe should be preserved");
    assert!(formatted.contains("| c"), "Pipe should be preserved");
}

