#![cfg(feature = "proptest")]

use aivi::{file_diagnostics_have_errors, format_text, parse_modules};
use proptest::prelude::*;
use proptest::string::string_regex;
use std::path::Path;

fn leaf_expr() -> impl Strategy<Value = String> {
    let int_lit = (0u32..=10_000).prop_map(|n| n.to_string());
    let bool_lit = prop_oneof![Just("True".to_string()), Just("False".to_string())];
    let text_lit = string_regex("[a-z]{0,12}")
        .expect("regex")
        .prop_map(|s| format!("\"{}\"", s));
    let ident = Just("value0".to_string());

    prop_oneof![int_lit, bool_lit, text_lit, ident]
}

fn expr_strategy() -> impl Strategy<Value = String> {
    leaf_expr().prop_recursive(4, 64, 8, |inner| {
        prop_oneof![
            (inner.clone(), inner.clone()).prop_map(|(a, b)| format!("({a}, {b})")),
            (inner.clone(), inner.clone()).prop_map(|(a, b)| format!("{a} + {b}")),
            (inner.clone(), inner.clone())
                .prop_map(|(t, e)| format!("if True then {t} else {e}")),
        ]
    })
}

fn program_strategy() -> impl Strategy<Value = String> {
    prop::collection::vec(expr_strategy(), 1..8).prop_map(|exprs| {
        let mut out = String::from("module prop.generated\n\n");
        for (index, expr) in exprs.iter().enumerate() {
            out.push_str(&format!("value{} = {}\n", index, expr));
        }
        out
    })
}

proptest! {
    #[test]
    fn parser_never_panics_on_arbitrary_text(
        input in prop::collection::vec(any::<char>(), 0..2048)
            .prop_map(|chars| chars.into_iter().collect::<String>())
    ) {
        let _ = parse_modules(Path::new("prop_fuzz.aivi"), &input);
    }

    #[test]
    fn formatter_round_trip_idempotent_on_generated_programs(program in program_strategy()) {
        let formatted1 = format_text(&program);
        let (_modules, diags) = parse_modules(Path::new("prop_valid.aivi"), &formatted1);
        prop_assert!(
            !file_diagnostics_have_errors(&diags),
            "formatter output should parse without errors: {diags:?}"
        );

        let formatted2 = format_text(&formatted1);
        prop_assert_eq!(formatted1, formatted2);
    }
}
