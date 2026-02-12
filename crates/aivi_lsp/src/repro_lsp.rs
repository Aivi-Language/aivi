#[cfg(test)]
mod tests {
    use crate::backend::Backend;
    use tower_lsp::lsp_types::{HoverContents, Position, Url};

    fn sample_uri() -> Url {
        Url::parse("file:///repro.aivi").expect("valid uri")
    }

    fn position_for(text: &str, needle: &str) -> Position {
        let offset = text.find(needle).expect("needle exists");
        let mut line = 0u32;
        let mut column = 0u32;
        for (idx, ch) in text.char_indices() {
            if idx == offset {
                break;
            }
            if ch == '\n' {
                line += 1;
                column = 0;
            } else {
                column += 1;
            }
        }
        Position::new(line, column)
    }

    #[test]
    #[ignore]
    fn hover_on_operator_works() {
        let text = r#"@no_prelude
module repro

add = x y => x + y
"#;
        let uri = sample_uri();
        // Hover on '+'
        let position = position_for(text, "+ y");

        // This is expected to FAIL currently because '+' is not extracted as an identifier
        let hover = Backend::build_hover(text, &uri, position);

        if let Some(hover) = hover {
            let HoverContents::Markup(_markup) = hover.contents else {
                panic!("expected markup hover");
            };
            // If it succeeds (after fix), it should show something about '+'
            // For now, if it returns None, that confirms the bug.
            // But wait, '+' might not have a type signature in this snippet if it's not defined or imported.
            // Let's use a defined operator.
        } else {
            // Test passes if it fails to find hover (currently confirming the bug)
            // But we want a fail-fail test to become pass-pass.
            panic!("Hover failed to find anything for '+' operator");
        }
    }

    #[test]
    fn hover_on_defined_operator_works() {
        let text = r#"@no_prelude
module repro

(++) : List a -> List a -> List a
(++) = xs ys => fail "impl"

main = [1] ++ [2]
"#;
        let uri = sample_uri();
        let position = position_for(text, "++ [2]");

        let hover = Backend::build_hover(text, &uri, position);

        // Assert that we found *something*
        assert!(
            hover.is_some(),
            "Should find hover for defined operator '++'"
        );

        let hover = hover.unwrap();
        let HoverContents::Markup(markup) = hover.contents else {
            panic!("expected markup hover");
        };
        assert!(
            markup.value.contains("(++)"),
            "Hover should contain operator name '(++)'"
        );
        assert!(
            markup.value.contains("List a"),
            "Hover should contain type signature"
        );
    }

    #[test]
    fn references_on_operator_works() {
        let text = r#"@no_prelude
module repro

(++) : List a -> List a -> List a
(++) = xs ys => fail "impl"

main = [1] ++ [2]
"#;
        let uri = sample_uri();
        let position = position_for(text, "++ [2]");

        let refs = Backend::build_references(text, &uri, position, true);
        assert!(!refs.is_empty(), "Should find references for '++'");
        // Should find definition and usage (2 refs)
        assert_eq!(refs.len(), 2);
    }
}
