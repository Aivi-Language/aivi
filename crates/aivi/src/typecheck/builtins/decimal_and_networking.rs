use super::TypeChecker;
use crate::typecheck::types::{Scheme, Type, TypeEnv};

pub(super) fn register(checker: &mut TypeChecker, env: &mut TypeEnv) {
    let int_ty = Type::con("Int");
    let float_ty = Type::con("Float");
    let text_ty = Type::con("Text");
    let decimal_ty = Type::con("Decimal");

    let decimal_record = Type::Record {
        fields: vec![
            (
                "fromFloat".to_string(),
                Type::Func(Box::new(float_ty.clone()), Box::new(decimal_ty.clone())),
            ),
            (
                "toFloat".to_string(),
                Type::Func(Box::new(decimal_ty.clone()), Box::new(float_ty.clone())),
            ),
            (
                "round".to_string(),
                Type::Func(
                    Box::new(decimal_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(int_ty.clone()),
                        Box::new(decimal_ty.clone()),
                    )),
                ),
            ),
            (
                "add".to_string(),
                Type::Func(
                    Box::new(decimal_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(decimal_ty.clone()),
                        Box::new(decimal_ty.clone()),
                    )),
                ),
            ),
            (
                "sub".to_string(),
                Type::Func(
                    Box::new(decimal_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(decimal_ty.clone()),
                        Box::new(decimal_ty.clone()),
                    )),
                ),
            ),
            (
                "mul".to_string(),
                Type::Func(
                    Box::new(decimal_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(decimal_ty.clone()),
                        Box::new(decimal_ty.clone()),
                    )),
                ),
            ),
            (
                "div".to_string(),
                Type::Func(
                    Box::new(decimal_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(decimal_ty.clone()),
                        Box::new(decimal_ty.clone()),
                    )),
                ),
            ),
        ]
        .into_iter()
        .collect(),
    };
    env.insert("decimal".to_string(), Scheme::mono(decimal_record));

    let url_ty = Type::con("Url");
    let url_record = Type::Record {
        fields: vec![
            (
                "parse".to_string(),
                Type::Func(
                    Box::new(text_ty.clone()),
                    Box::new(Type::con("Result").app(vec![text_ty.clone(), url_ty.clone()])),
                ),
            ),
            (
                "toString".to_string(),
                Type::Func(Box::new(url_ty.clone()), Box::new(text_ty.clone())),
            ),
        ]
        .into_iter()
        .collect(),
    };
    env.insert("url".to_string(), Scheme::mono(url_record));

    let request_ty = Type::con("Request");
    let response_ty = Type::con("Response");
    let error_ty = Type::con("Error");
    let http_result_ty = Type::con("Result").app(vec![error_ty.clone(), response_ty.clone()]);
    let http_source_ty = Type::con("Source").app(vec![Type::con("Http"), http_result_ty.clone()]);
    let http_record = Type::Record {
        fields: vec![
            (
                "get".to_string(),
                Type::Func(Box::new(url_ty.clone()), Box::new(http_source_ty.clone())),
            ),
            (
                "post".to_string(),
                Type::Func(
                    Box::new(url_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(http_source_ty.clone()),
                    )),
                ),
            ),
            (
                "fetch".to_string(),
                Type::Func(
                    Box::new(request_ty.clone()),
                    Box::new(http_source_ty.clone()),
                ),
            ),
        ]
        .into_iter()
        .collect(),
    };
    env.insert("http".to_string(), Scheme::mono(http_record));

    let https_source_ty = Type::con("Source").app(vec![Type::con("Https"), http_result_ty.clone()]);
    let https_record = Type::Record {
        fields: vec![
            (
                "get".to_string(),
                Type::Func(Box::new(url_ty.clone()), Box::new(https_source_ty.clone())),
            ),
            (
                "post".to_string(),
                Type::Func(
                    Box::new(url_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(https_source_ty.clone()),
                    )),
                ),
            ),
            (
                "fetch".to_string(),
                Type::Func(
                    Box::new(request_ty.clone()),
                    Box::new(https_source_ty.clone()),
                ),
            ),
        ]
        .into_iter()
        .collect(),
    };
    env.insert("https".to_string(), Scheme::mono(https_record));

    let rest_a = checker.fresh_var_id();
    let rest_source_ty = Type::con("Source").app(vec![Type::con("RestApi"), Type::Var(rest_a)]);
    let rest_record = Type::Record {
        fields: vec![
            (
                "get".to_string(),
                Type::Func(Box::new(url_ty.clone()), Box::new(rest_source_ty.clone())),
            ),
            (
                "post".to_string(),
                Type::Func(
                    Box::new(url_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(rest_source_ty.clone()),
                    )),
                ),
            ),
            (
                "fetch".to_string(),
                Type::Func(Box::new(request_ty), Box::new(rest_source_ty.clone())),
            ),
        ]
        .into_iter()
        .collect(),
    };
    env.insert(
        "rest".to_string(),
        Scheme {
            vars: vec![rest_a],
            ty: rest_record,
            origin: None,
        },
    );

    let address_ty = Type::Record {
        fields: vec![
            ("host".to_string(), Type::con("Text")),
            ("port".to_string(), Type::con("Int")),
        ]
        .into_iter()
        .collect(),
    };
    let socket_error_ty = Type::Record {
        fields: vec![("message".to_string(), Type::con("Text"))]
            .into_iter()
            .collect(),
    };
    let sockets_record = Type::Record {
        fields: vec![
            (
                "listen".to_string(),
                Type::Func(
                    Box::new(address_ty.clone()),
                    Box::new(
                        Type::con("Effect")
                            .app(vec![socket_error_ty.clone(), Type::con("Listener")]),
                    ),
                ),
            ),
            (
                "accept".to_string(),
                Type::Func(
                    Box::new(Type::con("Listener")),
                    Box::new(
                        Type::con("Effect")
                            .app(vec![socket_error_ty.clone(), Type::con("Connection")]),
                    ),
                ),
            ),
            (
                "connect".to_string(),
                Type::Func(
                    Box::new(address_ty.clone()),
                    Box::new(
                        Type::con("Effect")
                            .app(vec![socket_error_ty.clone(), Type::con("Connection")]),
                    ),
                ),
            ),
            (
                "send".to_string(),
                Type::Func(
                    Box::new(Type::con("Connection")),
                    Box::new(Type::Func(
                        Box::new(Type::con("List").app(vec![Type::con("Int")])),
                        Box::new(
                            Type::con("Effect")
                                .app(vec![socket_error_ty.clone(), Type::con("Unit")]),
                        ),
                    )),
                ),
            ),
            (
                "recv".to_string(),
                Type::Func(
                    Box::new(Type::con("Connection")),
                    Box::new(Type::con("Effect").app(vec![
                        socket_error_ty.clone(),
                        Type::con("List").app(vec![Type::con("Int")]),
                    ])),
                ),
            ),
            (
                "close".to_string(),
                Type::Func(
                    Box::new(Type::con("Connection")),
                    Box::new(
                        Type::con("Effect").app(vec![socket_error_ty.clone(), Type::con("Unit")]),
                    ),
                ),
            ),
            (
                "closeListener".to_string(),
                Type::Func(
                    Box::new(Type::con("Listener")),
                    Box::new(
                        Type::con("Effect").app(vec![socket_error_ty.clone(), Type::con("Unit")]),
                    ),
                ),
            ),
        ]
        .into_iter()
        .collect(),
    };
    env.insert("sockets".to_string(), Scheme::mono(sockets_record));

    let stream_error_ty = Type::Record {
        fields: vec![("message".to_string(), Type::con("Text"))]
            .into_iter()
            .collect(),
    };
    let stream_bytes_ty =
        Type::con("Stream").app(vec![Type::con("List").app(vec![Type::con("Int")])]);

    // Type vars for polymorphic combinators
    let a_map = checker.fresh_var_id();
    let b_map = checker.fresh_var_id();
    let a_filter = checker.fresh_var_id();
    let a_take = checker.fresh_var_id();
    let a_drop = checker.fresh_var_id();
    let a_flatmap = checker.fresh_var_id();
    let b_flatmap = checker.fresh_var_id();
    let a_merge = checker.fresh_var_id();
    let a_fold = checker.fresh_var_id();
    let b_fold = checker.fresh_var_id();
    let e_fold = checker.fresh_var_id(); // polymorphic error type
    let a_fromlist = checker.fresh_var_id();

    let stream_a_map = Type::con("Stream").app(vec![Type::Var(a_map)]);
    let stream_b_map = Type::con("Stream").app(vec![Type::Var(b_map)]);
    let stream_a_filter = Type::con("Stream").app(vec![Type::Var(a_filter)]);
    let stream_a_take = Type::con("Stream").app(vec![Type::Var(a_take)]);
    let stream_a_drop = Type::con("Stream").app(vec![Type::Var(a_drop)]);
    let stream_a_flatmap = Type::con("Stream").app(vec![Type::Var(a_flatmap)]);
    let stream_b_flatmap = Type::con("Stream").app(vec![Type::Var(b_flatmap)]);
    let stream_a_merge = Type::con("Stream").app(vec![Type::Var(a_merge)]);
    let stream_a_fold = Type::con("Stream").app(vec![Type::Var(a_fold)]);
    let stream_a_fromlist = Type::con("Stream").app(vec![Type::Var(a_fromlist)]);
    let list_a_fromlist = Type::con("List").app(vec![Type::Var(a_fromlist)]);

    let streams_record = Type::Record {
        fields: vec![
            (
                "fromSocket".to_string(),
                Type::Func(
                    Box::new(Type::con("Connection")),
                    Box::new(stream_bytes_ty.clone()),
                ),
            ),
            (
                "toSocket".to_string(),
                Type::Func(
                    Box::new(Type::con("Connection")),
                    Box::new(Type::Func(
                        Box::new(stream_bytes_ty.clone()),
                        Box::new(
                            Type::con("Effect")
                                .app(vec![stream_error_ty.clone(), Type::con("Unit")]),
                        ),
                    )),
                ),
            ),
            (
                "chunks".to_string(),
                Type::Func(
                    Box::new(Type::con("Int")),
                    Box::new(Type::Func(
                        Box::new(stream_bytes_ty.clone()),
                        Box::new(stream_bytes_ty.clone()),
                    )),
                ),
            ),
            // fromList : List A -> Stream A
            (
                "fromList".to_string(),
                Type::Func(
                    Box::new(list_a_fromlist),
                    Box::new(stream_a_fromlist),
                ),
            ),
            // map : (A -> B) -> Stream A -> Stream B
            (
                "map".to_string(),
                Type::Func(
                    Box::new(Type::Func(
                        Box::new(Type::Var(a_map)),
                        Box::new(Type::Var(b_map)),
                    )),
                    Box::new(Type::Func(
                        Box::new(stream_a_map),
                        Box::new(stream_b_map),
                    )),
                ),
            ),
            // filter : (A -> Bool) -> Stream A -> Stream A
            (
                "filter".to_string(),
                Type::Func(
                    Box::new(Type::Func(
                        Box::new(Type::Var(a_filter)),
                        Box::new(Type::con("Bool")),
                    )),
                    Box::new(Type::Func(
                        Box::new(stream_a_filter.clone()),
                        Box::new(stream_a_filter),
                    )),
                ),
            ),
            // take : Int -> Stream A -> Stream A
            (
                "take".to_string(),
                Type::Func(
                    Box::new(Type::con("Int")),
                    Box::new(Type::Func(
                        Box::new(stream_a_take.clone()),
                        Box::new(stream_a_take),
                    )),
                ),
            ),
            // drop : Int -> Stream A -> Stream A
            (
                "drop".to_string(),
                Type::Func(
                    Box::new(Type::con("Int")),
                    Box::new(Type::Func(
                        Box::new(stream_a_drop.clone()),
                        Box::new(stream_a_drop),
                    )),
                ),
            ),
            // flatMap : (A -> Stream B) -> Stream A -> Stream B
            (
                "flatMap".to_string(),
                Type::Func(
                    Box::new(Type::Func(
                        Box::new(Type::Var(a_flatmap)),
                        Box::new(stream_b_flatmap.clone()),
                    )),
                    Box::new(Type::Func(
                        Box::new(stream_a_flatmap),
                        Box::new(stream_b_flatmap),
                    )),
                ),
            ),
            // merge : Stream A -> Stream A -> Stream A
            (
                "merge".to_string(),
                Type::Func(
                    Box::new(stream_a_merge.clone()),
                    Box::new(Type::Func(
                        Box::new(stream_a_merge.clone()),
                        Box::new(stream_a_merge),
                    )),
                ),
            ),
            // fold : (B -> A -> B) -> B -> Stream A -> Effect e B
            (
                "fold".to_string(),
                Type::Func(
                    Box::new(Type::Func(
                        Box::new(Type::Var(b_fold)),
                        Box::new(Type::Func(
                            Box::new(Type::Var(a_fold)),
                            Box::new(Type::Var(b_fold)),
                        )),
                    )),
                    Box::new(Type::Func(
                        Box::new(Type::Var(b_fold)),
                        Box::new(Type::Func(
                            Box::new(stream_a_fold),
                            Box::new(
                                Type::con("Effect")
                                    .app(vec![Type::Var(e_fold), Type::Var(b_fold)]),
                            ),
                        )),
                    )),
                ),
            ),
        ]
        .into_iter()
        .collect(),
    };
    env.insert(
        "streams".to_string(),
        Scheme {
            vars: vec![
                a_map, b_map, a_filter, a_take, a_drop, a_flatmap, b_flatmap, a_merge, a_fold,
                b_fold, e_fold, a_fromlist,
            ],
            ty: streams_record,
            origin: None,
        },
    );
}
