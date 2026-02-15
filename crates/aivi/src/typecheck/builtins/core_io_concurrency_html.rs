use super::TypeChecker;
use crate::typecheck::types::{Scheme, Type, TypeEnv};

pub(super) fn register(checker: &mut TypeChecker, env: &mut TypeEnv) {
    env.insert("Unit".to_string(), Scheme::mono(Type::con("Unit")));
    env.insert("True".to_string(), Scheme::mono(Type::con("Bool")));
    env.insert("False".to_string(), Scheme::mono(Type::con("Bool")));

    let a = checker.fresh_var_id();
    env.insert(
        "None".to_string(),
        Scheme {
            vars: vec![a],
            ty: Type::con("Option").app(vec![Type::Var(a)]),
        },
    );
    let a = checker.fresh_var_id();
    env.insert(
        "Some".to_string(),
        Scheme {
            vars: vec![a],
            ty: Type::Func(
                Box::new(Type::Var(a)),
                Box::new(Type::con("Option").app(vec![Type::Var(a)])),
            ),
        },
    );

    let e = checker.fresh_var_id();
    let a = checker.fresh_var_id();
    env.insert(
        "Ok".to_string(),
        Scheme {
            vars: vec![e, a],
            ty: Type::Func(
                Box::new(Type::Var(a)),
                Box::new(Type::con("Result").app(vec![Type::Var(e), Type::Var(a)])),
            ),
        },
    );
    let e = checker.fresh_var_id();
    let a = checker.fresh_var_id();
    env.insert(
        "Err".to_string(),
        Scheme {
            vars: vec![e, a],
            ty: Type::Func(
                Box::new(Type::Var(e)),
                Box::new(Type::con("Result").app(vec![Type::Var(e), Type::Var(a)])),
            ),
        },
    );
    env.insert("Closed".to_string(), Scheme::mono(Type::con("Closed")));

    let a = checker.fresh_var_id();
    let b = checker.fresh_var_id();
    env.insert(
        "foldGen".to_string(),
        Scheme {
            vars: vec![a, b],
            ty: Type::Func(
                Box::new(Type::con("Generator").app(vec![Type::Var(a)])),
                Box::new(Type::Func(
                    Box::new(Type::Func(
                        Box::new(Type::Var(b)),
                        Box::new(Type::Func(Box::new(Type::Var(a)), Box::new(Type::Var(b)))),
                    )),
                    Box::new(Type::Func(Box::new(Type::Var(b)), Box::new(Type::Var(b)))),
                )),
            ),
        },
    );

    let a = checker.fresh_var_id();
    let e = checker.fresh_var_id();
    env.insert(
        "pure".to_string(),
        Scheme {
            vars: vec![e, a],
            ty: Type::Func(
                Box::new(Type::Var(a)),
                Box::new(Type::con("Effect").app(vec![Type::Var(e), Type::Var(a)])),
            ),
        },
    );
    let a = checker.fresh_var_id();
    let e = checker.fresh_var_id();
    env.insert(
        "fail".to_string(),
        Scheme {
            vars: vec![e, a],
            ty: Type::Func(
                Box::new(Type::Var(e)),
                Box::new(Type::con("Effect").app(vec![Type::Var(e), Type::Var(a)])),
            ),
        },
    );
    let a = checker.fresh_var_id();
    let e = checker.fresh_var_id();
    env.insert(
        "attempt".to_string(),
        Scheme {
            vars: vec![e, a],
            ty: Type::Func(
                Box::new(Type::con("Effect").app(vec![Type::Var(e), Type::Var(a)])),
                Box::new(Type::con("Effect").app(vec![
                    Type::Var(e),
                    Type::con("Result").app(vec![Type::Var(e), Type::Var(a)]),
                ])),
            ),
        },
    );

    env.insert(
        "print".to_string(),
        Scheme::mono(Type::Func(
            Box::new(Type::con("Text")),
            Box::new(Type::con("Effect").app(vec![Type::con("Text"), Type::con("Unit")])),
        )),
    );
    env.insert(
        "println".to_string(),
        Scheme::mono(Type::Func(
            Box::new(Type::con("Text")),
            Box::new(Type::con("Effect").app(vec![Type::con("Text"), Type::con("Unit")])),
        )),
    );

    let a = checker.fresh_var_id();
    let k = checker.fresh_var_id();
    env.insert(
        "load".to_string(),
        Scheme {
            vars: vec![k, a],
            ty: Type::Func(
                Box::new(Type::con("Source").app(vec![Type::Var(k), Type::Var(a)])),
                Box::new(Type::con("Effect").app(vec![
                    Type::con("SourceError").app(vec![Type::Var(k)]),
                    Type::Var(a),
                ])),
            ),
        },
    );

    let file_record = Type::Record {
        fields: vec![
            (
                "read".to_string(),
                Type::Func(
                    Box::new(Type::con("Text")),
                    Box::new(Type::con("Source").app(vec![Type::con("File"), Type::con("Text")])),
                ),
            ),
            (
                "open".to_string(),
                Type::Func(
                    Box::new(Type::con("Text")),
                    Box::new(
                        Type::con("Effect").app(vec![Type::con("Text"), Type::con("FileHandle")]),
                    ),
                ),
            ),
            (
                "close".to_string(),
                Type::Func(
                    Box::new(Type::con("FileHandle")),
                    Box::new(Type::con("Effect").app(vec![Type::con("Text"), Type::con("Unit")])),
                ),
            ),
            (
                "readAll".to_string(),
                Type::Func(
                    Box::new(Type::con("FileHandle")),
                    Box::new(Type::con("Effect").app(vec![Type::con("Text"), Type::con("Text")])),
                ),
            ),
            (
                "write_text".to_string(),
                Type::Func(
                    Box::new(Type::con("Text")),
                    Box::new(Type::Func(
                        Box::new(Type::con("Text")),
                        Box::new(
                            Type::con("Effect").app(vec![Type::con("Text"), Type::con("Unit")]),
                        ),
                    )),
                ),
            ),
            (
                "exists".to_string(),
                Type::Func(
                    Box::new(Type::con("Text")),
                    Box::new(Type::con("Effect").app(vec![Type::con("Text"), Type::con("Bool")])),
                ),
            ),
            (
                "stat".to_string(),
                Type::Func(
                    Box::new(Type::con("Text")),
                    Box::new(
                        Type::con("Effect").app(vec![Type::con("Text"), Type::con("FileStats")]),
                    ),
                ),
            ),
            (
                "delete".to_string(),
                Type::Func(
                    Box::new(Type::con("Text")),
                    Box::new(Type::con("Effect").app(vec![Type::con("Text"), Type::con("Unit")])),
                ),
            ),
        ]
        .into_iter()
        .collect(),
        open: true,
    };
    env.insert("file".to_string(), Scheme::mono(file_record));

    let a = checker.fresh_var_id();
    let send_ty = Type::con("Send").app(vec![Type::Var(a)]);
    let recv_ty = Type::con("Recv").app(vec![Type::Var(a)]);
    let channel_record = Type::Record {
        fields: vec![
            (
                "make".to_string(),
                Type::Func(
                    Box::new(Type::con("Unit")),
                    Box::new(Type::con("Effect").app(vec![
                        Type::con("Text"),
                        Type::Tuple(vec![send_ty.clone(), recv_ty.clone()]),
                    ])),
                ),
            ),
            (
                "send".to_string(),
                Type::Func(
                    Box::new(send_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(Type::Var(a)),
                        Box::new(
                            Type::con("Effect").app(vec![Type::con("Text"), Type::con("Unit")]),
                        ),
                    )),
                ),
            ),
            (
                "recv".to_string(),
                Type::Func(
                    Box::new(recv_ty.clone()),
                    Box::new(Type::con("Effect").app(vec![
                        Type::con("Text"),
                        Type::con("Result").app(vec![Type::con("Closed"), Type::Var(a)]),
                    ])),
                ),
            ),
            (
                "close".to_string(),
                Type::Func(
                    Box::new(send_ty),
                    Box::new(Type::con("Effect").app(vec![Type::con("Text"), Type::con("Unit")])),
                ),
            ),
        ]
        .into_iter()
        .collect(),
        open: true,
    };
    env.insert("channel".to_string(), Scheme::mono(channel_record));

    let e = checker.fresh_var_id();
    let a = checker.fresh_var_id();
    let b = checker.fresh_var_id();
    let concurrent_record = Type::Record {
        fields: vec![
            (
                "scope".to_string(),
                Type::Func(
                    Box::new(Type::con("Effect").app(vec![Type::Var(e), Type::Var(a)])),
                    Box::new(Type::con("Effect").app(vec![Type::Var(e), Type::Var(a)])),
                ),
            ),
            (
                "par".to_string(),
                Type::Func(
                    Box::new(Type::con("Effect").app(vec![Type::Var(e), Type::Var(a)])),
                    Box::new(Type::Func(
                        Box::new(Type::con("Effect").app(vec![Type::Var(e), Type::Var(b)])),
                        Box::new(Type::con("Effect").app(vec![
                            Type::Var(e),
                            Type::Tuple(vec![Type::Var(a), Type::Var(b)]),
                        ])),
                    )),
                ),
            ),
            (
                "race".to_string(),
                Type::Func(
                    Box::new(Type::con("Effect").app(vec![Type::Var(e), Type::Var(a)])),
                    Box::new(Type::Func(
                        Box::new(Type::con("Effect").app(vec![Type::Var(e), Type::Var(a)])),
                        Box::new(Type::con("Effect").app(vec![Type::Var(e), Type::Var(a)])),
                    )),
                ),
            ),
            (
                "spawnDetached".to_string(),
                Type::Func(
                    Box::new(Type::con("Effect").app(vec![Type::Var(e), Type::Var(a)])),
                    Box::new(Type::con("Effect").app(vec![Type::Var(e), Type::con("Unit")])),
                ),
            ),
        ]
        .into_iter()
        .collect(),
        open: true,
    };
    env.insert("concurrent".to_string(), Scheme::mono(concurrent_record));

    let clock_record = Type::Record {
        fields: vec![(
            "now".to_string(),
            Type::Func(
                Box::new(Type::con("Unit")),
                Box::new(Type::con("Effect").app(vec![Type::con("Text"), Type::con("DateTime")])),
            ),
        )]
        .into_iter()
        .collect(),
        open: true,
    };
    env.insert("clock".to_string(), Scheme::mono(clock_record));

    let random_record = Type::Record {
        fields: vec![(
            "int".to_string(),
            Type::Func(
                Box::new(Type::con("Int")),
                Box::new(Type::Func(
                    Box::new(Type::con("Int")),
                    Box::new(Type::con("Effect").app(vec![Type::con("Text"), Type::con("Int")])),
                )),
            ),
        )]
        .into_iter()
        .collect(),
        open: true,
    };
    env.insert("random".to_string(), Scheme::mono(random_record));

    let header_ty = Type::Record {
        fields: vec![
            ("name".to_string(), Type::con("Text")),
            ("value".to_string(), Type::con("Text")),
        ]
        .into_iter()
        .collect(),
        open: false,
    };
    let request_ty = Type::Record {
        fields: vec![
            ("method".to_string(), Type::con("Text")),
            ("path".to_string(), Type::con("Text")),
            (
                "headers".to_string(),
                Type::con("List").app(vec![header_ty.clone()]),
            ),
            (
                "body".to_string(),
                Type::con("List").app(vec![Type::con("Int")]),
            ),
            (
                "remote_addr".to_string(),
                Type::con("Option").app(vec![Type::con("Text")]),
            ),
        ]
        .into_iter()
        .collect(),
        open: false,
    };
    let _response_ty = Type::Record {
        fields: vec![
            ("status".to_string(), Type::con("Int")),
            (
                "headers".to_string(),
                Type::con("List").app(vec![header_ty]),
            ),
            (
                "body".to_string(),
                Type::con("List").app(vec![Type::con("Int")]),
            ),
        ]
        .into_iter()
        .collect(),
        open: false,
    };
    let server_config_ty = Type::Record {
        fields: vec![("address".to_string(), Type::con("Text"))]
            .into_iter()
            .collect(),
        open: false,
    };
    let server_ty = Type::con("Server");
    let ws_ty = Type::con("WebSocket");
    let http_error_ty = Type::con("HttpError");
    let ws_error_ty = Type::con("WsError");
    let reply_ty = Type::con("ServerReply");
    let ws_message_ty = Type::con("WsMessage");
    let http_server_record = Type::Record {
        fields: vec![
            (
                "listen".to_string(),
                Type::Func(
                    Box::new(server_config_ty),
                    Box::new(Type::Func(
                        Box::new(Type::Func(
                            Box::new(request_ty),
                            Box::new(
                                Type::con("Effect").app(vec![http_error_ty.clone(), reply_ty]),
                            ),
                        )),
                        Box::new(Type::con("Effect").app(vec![http_error_ty, server_ty.clone()])),
                    )),
                ),
            ),
            (
                "stop".to_string(),
                Type::Func(
                    Box::new(server_ty),
                    Box::new(
                        Type::con("Effect").app(vec![Type::con("HttpError"), Type::con("Unit")]),
                    ),
                ),
            ),
            (
                "ws_recv".to_string(),
                Type::Func(
                    Box::new(ws_ty.clone()),
                    Box::new(Type::con("Effect").app(vec![ws_error_ty.clone(), ws_message_ty])),
                ),
            ),
            (
                "ws_send".to_string(),
                Type::Func(
                    Box::new(ws_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(Type::con("WsMessage")),
                        Box::new(
                            Type::con("Effect").app(vec![ws_error_ty.clone(), Type::con("Unit")]),
                        ),
                    )),
                ),
            ),
            (
                "ws_close".to_string(),
                Type::Func(
                    Box::new(ws_ty),
                    Box::new(Type::con("Effect").app(vec![ws_error_ty, Type::con("Unit")])),
                ),
            ),
        ]
        .into_iter()
        .collect(),
        open: true,
    };
    env.insert("httpServer".to_string(), Scheme::mono(http_server_record));

    let html_record = Type::Record {
        fields: vec![(
            "render".to_string(),
            Type::Func(Box::new(Type::con("Html")), Box::new(Type::con("Text"))),
        )]
        .into_iter()
        .collect(),
        open: true,
    };
    env.insert("html".to_string(), Scheme::mono(html_record));

    // UI runtime helpers (VDOM rendering + LiveView loop).
    let msg = checker.fresh_var_id();
    let model = checker.fresh_var_id();
    let vnode_msg = Type::con("VNode").app(vec![Type::Var(msg)]);
    let patch_op = Type::con("PatchOp");
    let list_patch_op = Type::con("List").app(vec![patch_op.clone()]);
    let event_ty = Type::con("Event");
    let live_error_ty = Type::con("LiveError");
    let live_config_ty = Type::con("LiveConfig");

    let ui_record = Type::Record {
        fields: vec![
            (
                "renderHtml".to_string(),
                Type::Func(Box::new(vnode_msg.clone()), Box::new(Type::con("Text"))),
            ),
            (
                "diff".to_string(),
                Type::Func(
                    Box::new(vnode_msg.clone()),
                    Box::new(Type::Func(
                        Box::new(vnode_msg.clone()),
                        Box::new(list_patch_op.clone()),
                    )),
                ),
            ),
            (
                "patchToJson".to_string(),
                Type::Func(Box::new(list_patch_op.clone()), Box::new(Type::con("Text"))),
            ),
            (
                "eventFromJson".to_string(),
                Type::Func(
                    Box::new(Type::con("Text")),
                    Box::new(Type::con("Result").app(vec![live_error_ty.clone(), event_ty])),
                ),
            ),
            (
                "live".to_string(),
                Type::Func(
                    Box::new(live_config_ty),
                    Box::new(Type::Func(
                        Box::new(Type::Var(model)),
                        Box::new(Type::Func(
                            Box::new(Type::Func(
                                Box::new(Type::Var(model)),
                                Box::new(vnode_msg.clone()),
                            )),
                            Box::new(Type::Func(
                                Box::new(Type::Func(
                                    Box::new(Type::Var(msg)),
                                    Box::new(Type::Func(
                                        Box::new(Type::Var(model)),
                                        Box::new(Type::Var(model)),
                                    )),
                                )),
                                Box::new(
                                    Type::con("Effect")
                                        .app(vec![live_error_ty, Type::con("Server")]),
                                ),
                            )),
                        )),
                    )),
                ),
            ),
        ]
        .into_iter()
        .collect(),
        open: true,
    };
    env.insert(
        "ui".to_string(),
        Scheme {
            vars: vec![msg, model],
            ty: ui_record,
        },
    );
}
