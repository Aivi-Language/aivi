use aivi_base::{FileId, SourceFile};
use aivi_syntax::{Formatter, TokenKind, lex_module, parse_module};

#[test]
fn line_endings_preserve_lexing_parsing_and_formatting() {
    let program = "// comment\nvalue first = 1\n\nclass Eq A = {\n    (==) : A -> A -> Bool\n}\ninstance Eq Int = {\n    (==) left right =\n        True\n}\ntype Int -> Int\nfunc identity = input =>\n    input\nvalue main = identity first\n";
    let mut expected = None;
    for ending in ["\n", "\r\n", "\r"] {
        let text = program.replace('\n', ending);
        let source = SourceFile::new(FileId::new(0), "lines.aivi", text.as_str());
        let lexed = lex_module(&source);
        assert!(!lexed.has_errors(), "{ending:?}: {:?}", lexed.diagnostics());
        assert_eq!(lexed.replay(&source), text);
        assert_eq!(
            lexed
                .tokens()
                .iter()
                .filter(|token| token.kind() == TokenKind::Newline)
                .count(),
            program.bytes().filter(|byte| *byte == b'\n').count()
        );
        let parsed = parse_module(&source);
        assert!(
            !parsed.has_errors(),
            "{ending:?}: {:?}",
            parsed.all_diagnostics().collect::<Vec<_>>()
        );
        let formatted = Formatter.format(&parsed.module);
        assert!(formatted.contains("value main"));
        if let Some(expected) = &expected {
            assert_eq!(&formatted, expected, "{ending:?}");
        } else {
            expected = Some(formatted);
        }
    }
}

#[test]
fn unterminated_literals_stop_at_each_line_ending() {
    for prefix in ["\"", "rx\""] {
        for ending in ["\n", "\r\n", "\r"] {
            let text = format!("value broken = {prefix}unfinished{ending}value next = 1{ending}");
            let source = SourceFile::new(FileId::new(0), "lines.aivi", text.as_str());
            let lexed = lex_module(&source);
            assert_eq!(lexed.diagnostics().len(), 1, "{prefix:?}, {ending:?}");
            assert_eq!(lexed.replay(&source), text);
            let next = lexed
                .tokens()
                .iter()
                .find(|token| source.slice(token.span()) == "next")
                .expect("the next declaration must survive recovery");
            assert_eq!(source.offset_to_lsp_position(next.span().start()).line, 1);
        }
    }
}

#[test]
fn incomplete_escapes_do_not_consume_the_next_declaration() {
    for prefix in ["\"", "rx\""] {
        for body in ["unfinished\\", "\\u{123"] {
            for ending in ["\n", "\r\n", "\r"] {
                let text = format!("value broken = {prefix}{body}{ending}value next = 1{ending}");
                let source = SourceFile::new(FileId::new(0), "lines.aivi", text.as_str());
                let lexed = lex_module(&source);
                assert!(lexed.has_errors());
                assert_eq!(lexed.replay(&source), text);
                assert!(
                    lexed
                        .tokens()
                        .iter()
                        .any(|token| source.slice(token.span()) == "next"),
                    "recovery swallowed next declaration: {text:?}"
                );
            }
        }
    }
}
