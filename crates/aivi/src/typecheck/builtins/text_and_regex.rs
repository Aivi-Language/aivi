use super::TypeChecker;
use crate::typecheck::types::{Scheme, Type, TypeEnv};

pub(super) fn register(checker: &mut TypeChecker, env: &mut TypeEnv) {
    let text_ty = Type::con("Text");
    let char_ty = Type::con("Char");
    let int_ty = Type::con("Int");
    let float_ty = Type::con("Float");
    let bool_ty = Type::con("Bool");
    let bytes_ty = Type::con("Bytes");
    let encoding_ty = Type::con("Encoding");
    let text_error_ty = Type::con("TextError");
    let list_text_ty = Type::con("List").app(vec![text_ty.clone()]);
    let option_int_ty = Type::con("Option").app(vec![int_ty.clone()]);
    let option_float_ty = Type::con("Option").app(vec![float_ty.clone()]);
    let result_text_error_text_ty =
        Type::con("Result").app(vec![text_error_ty.clone(), text_ty.clone()]);

    let text_record = Type::Record {
        fields: vec![
            (
                "length".to_string(),
                Type::Func(Box::new(text_ty.clone()), Box::new(int_ty.clone())),
            ),
            (
                "isEmpty".to_string(),
                Type::Func(Box::new(text_ty.clone()), Box::new(bool_ty.clone())),
            ),
            (
                "isDigit".to_string(),
                Type::Func(Box::new(char_ty.clone()), Box::new(bool_ty.clone())),
            ),
            (
                "isAlpha".to_string(),
                Type::Func(Box::new(char_ty.clone()), Box::new(bool_ty.clone())),
            ),
            (
                "isAlnum".to_string(),
                Type::Func(Box::new(char_ty.clone()), Box::new(bool_ty.clone())),
            ),
            (
                "isSpace".to_string(),
                Type::Func(Box::new(char_ty.clone()), Box::new(bool_ty.clone())),
            ),
            (
                "isUpper".to_string(),
                Type::Func(Box::new(char_ty.clone()), Box::new(bool_ty.clone())),
            ),
            (
                "isLower".to_string(),
                Type::Func(Box::new(char_ty.clone()), Box::new(bool_ty.clone())),
            ),
            (
                "contains".to_string(),
                Type::Func(
                    Box::new(text_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(bool_ty.clone()),
                    )),
                ),
            ),
            (
                "startsWith".to_string(),
                Type::Func(
                    Box::new(text_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(bool_ty.clone()),
                    )),
                ),
            ),
            (
                "endsWith".to_string(),
                Type::Func(
                    Box::new(text_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(bool_ty.clone()),
                    )),
                ),
            ),
            (
                "indexOf".to_string(),
                Type::Func(
                    Box::new(text_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(option_int_ty.clone()),
                    )),
                ),
            ),
            (
                "lastIndexOf".to_string(),
                Type::Func(
                    Box::new(text_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(option_int_ty.clone()),
                    )),
                ),
            ),
            (
                "count".to_string(),
                Type::Func(
                    Box::new(text_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(int_ty.clone()),
                    )),
                ),
            ),
            (
                "compare".to_string(),
                Type::Func(
                    Box::new(text_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(int_ty.clone()),
                    )),
                ),
            ),
            (
                "slice".to_string(),
                Type::Func(
                    Box::new(int_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(int_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(text_ty.clone()),
                            Box::new(text_ty.clone()),
                        )),
                    )),
                ),
            ),
            (
                "split".to_string(),
                Type::Func(
                    Box::new(text_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(list_text_ty.clone()),
                    )),
                ),
            ),
            (
                "splitLines".to_string(),
                Type::Func(Box::new(text_ty.clone()), Box::new(list_text_ty.clone())),
            ),
            (
                "chunk".to_string(),
                Type::Func(
                    Box::new(int_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(list_text_ty.clone()),
                    )),
                ),
            ),
            (
                "trim".to_string(),
                Type::Func(Box::new(text_ty.clone()), Box::new(text_ty.clone())),
            ),
            (
                "trimStart".to_string(),
                Type::Func(Box::new(text_ty.clone()), Box::new(text_ty.clone())),
            ),
            (
                "trimEnd".to_string(),
                Type::Func(Box::new(text_ty.clone()), Box::new(text_ty.clone())),
            ),
            (
                "padStart".to_string(),
                Type::Func(
                    Box::new(int_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(text_ty.clone()),
                            Box::new(text_ty.clone()),
                        )),
                    )),
                ),
            ),
            (
                "padEnd".to_string(),
                Type::Func(
                    Box::new(int_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(text_ty.clone()),
                            Box::new(text_ty.clone()),
                        )),
                    )),
                ),
            ),
            (
                "replace".to_string(),
                Type::Func(
                    Box::new(text_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(text_ty.clone()),
                            Box::new(text_ty.clone()),
                        )),
                    )),
                ),
            ),
            (
                "replaceAll".to_string(),
                Type::Func(
                    Box::new(text_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(text_ty.clone()),
                            Box::new(text_ty.clone()),
                        )),
                    )),
                ),
            ),
            (
                "remove".to_string(),
                Type::Func(
                    Box::new(text_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(text_ty.clone()),
                    )),
                ),
            ),
            (
                "repeat".to_string(),
                Type::Func(
                    Box::new(int_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(text_ty.clone()),
                    )),
                ),
            ),
            (
                "reverse".to_string(),
                Type::Func(Box::new(text_ty.clone()), Box::new(text_ty.clone())),
            ),
            (
                "concat".to_string(),
                Type::Func(Box::new(list_text_ty.clone()), Box::new(text_ty.clone())),
            ),
            (
                "toLower".to_string(),
                Type::Func(Box::new(text_ty.clone()), Box::new(text_ty.clone())),
            ),
            (
                "toUpper".to_string(),
                Type::Func(Box::new(text_ty.clone()), Box::new(text_ty.clone())),
            ),
            (
                "capitalize".to_string(),
                Type::Func(Box::new(text_ty.clone()), Box::new(text_ty.clone())),
            ),
            (
                "titleCase".to_string(),
                Type::Func(Box::new(text_ty.clone()), Box::new(text_ty.clone())),
            ),
            (
                "caseFold".to_string(),
                Type::Func(Box::new(text_ty.clone()), Box::new(text_ty.clone())),
            ),
            (
                "normalizeNFC".to_string(),
                Type::Func(Box::new(text_ty.clone()), Box::new(text_ty.clone())),
            ),
            (
                "normalizeNFD".to_string(),
                Type::Func(Box::new(text_ty.clone()), Box::new(text_ty.clone())),
            ),
            (
                "normalizeNFKC".to_string(),
                Type::Func(Box::new(text_ty.clone()), Box::new(text_ty.clone())),
            ),
            (
                "normalizeNFKD".to_string(),
                Type::Func(Box::new(text_ty.clone()), Box::new(text_ty.clone())),
            ),
            (
                "toBytes".to_string(),
                Type::Func(
                    Box::new(encoding_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(bytes_ty.clone()),
                    )),
                ),
            ),
            (
                "fromBytes".to_string(),
                Type::Func(
                    Box::new(encoding_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(bytes_ty.clone()),
                        Box::new(result_text_error_text_ty.clone()),
                    )),
                ),
            ),
            (
                "toText".to_string(),
                Type::Func(
                    Box::new(Type::Var(checker.fresh_var_id())),
                    Box::new(text_ty.clone()),
                ),
            ),
            (
                "parseInt".to_string(),
                Type::Func(Box::new(text_ty.clone()), Box::new(option_int_ty.clone())),
            ),
            (
                "parseFloat".to_string(),
                Type::Func(Box::new(text_ty.clone()), Box::new(option_float_ty.clone())),
            ),
        ]
        .into_iter()
        .collect(),
        open: true,
        row_tail: None,
    };
    env.insert("text".to_string(), Scheme::mono(text_record));

    let regex_ty = Type::con("Regex");
    let regex_error_ty = Type::con("RegexError");
    let match_ty = Type::con("Match");
    let option_match_ty = Type::con("Option").app(vec![match_ty.clone()]);
    let list_match_ty = Type::con("List").app(vec![match_ty.clone()]);
    let tuple_int_int_ty = Type::Tuple(vec![int_ty.clone(), int_ty.clone()]);
    let option_tuple_int_int_ty = Type::con("Option").app(vec![tuple_int_int_ty.clone()]);
    let list_tuple_int_int_ty = Type::con("List").app(vec![tuple_int_int_ty.clone()]);

    let regex_record = Type::Record {
        fields: vec![
            (
                "compile".to_string(),
                Type::Func(
                    Box::new(text_ty.clone()),
                    Box::new(
                        Type::con("Result").app(vec![regex_error_ty.clone(), regex_ty.clone()]),
                    ),
                ),
            ),
            (
                "test".to_string(),
                Type::Func(
                    Box::new(regex_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(bool_ty.clone()),
                    )),
                ),
            ),
            (
                "match".to_string(),
                Type::Func(
                    Box::new(regex_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(option_match_ty.clone()),
                    )),
                ),
            ),
            (
                "matches".to_string(),
                Type::Func(
                    Box::new(regex_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(list_match_ty.clone()),
                    )),
                ),
            ),
            (
                "find".to_string(),
                Type::Func(
                    Box::new(regex_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(option_tuple_int_int_ty.clone()),
                    )),
                ),
            ),
            (
                "findAll".to_string(),
                Type::Func(
                    Box::new(regex_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(list_tuple_int_int_ty.clone()),
                    )),
                ),
            ),
            (
                "split".to_string(),
                Type::Func(
                    Box::new(regex_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(list_text_ty.clone()),
                    )),
                ),
            ),
            (
                "replace".to_string(),
                Type::Func(
                    Box::new(regex_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(text_ty.clone()),
                            Box::new(text_ty.clone()),
                        )),
                    )),
                ),
            ),
            (
                "replaceAll".to_string(),
                Type::Func(
                    Box::new(regex_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(text_ty.clone()),
                            Box::new(text_ty.clone()),
                        )),
                    )),
                ),
            ),
        ]
        .into_iter()
        .collect(),
        open: true,
        row_tail: None,
    };
    env.insert("regex".to_string(), Scheme::mono(regex_record));
}
