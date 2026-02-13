use super::TypeChecker;
use crate::typecheck::types::{Kind, Scheme, Type, TypeEnv};

impl TypeChecker {
    pub(super) fn register_builtin_types(&mut self) {
        let star = Kind::Star;
        let arrow = |a, b| Kind::Arrow(Box::new(a), Box::new(b));

        for name in [
            "Unit",
            "Bool",
            "Int",
            "Float",
            "Text",
            "Char",
            "Bytes",
            "Patch",
            "Map",
            "Set",
            "Queue",
            "Deque",
            "Heap",
            "Vec",
            "Mat",
            "Signal",
            "Spectrum",
            "Graph",
            "Edge",
            "Generator",
            "Html",
            "DateTime",
            "Regex",
            "BigInt",
            "Rational",
            "Decimal",
            "FileHandle",
            "FileStats",
            "Listener",
            "Connection",
            "Stream",
            "Send",
            "Recv",
            "Closed",
            "Server",
            "WebSocket",
            "HttpError",
            "WsError",
            "ServerReply",
            "WsMessage",
        ] {
            self.builtin_types.insert(name.to_string(), star.clone());
        }

        // Higher kinded types
        self.builtin_types
            .insert("List".to_string(), arrow(star.clone(), star.clone()));
        self.builtin_types
            .insert("Option".to_string(), arrow(star.clone(), star.clone()));
        self.builtin_types
            .insert("Resource".to_string(), arrow(star.clone(), star.clone()));
        self.builtin_types.insert(
            "Result".to_string(),
            arrow(star.clone(), arrow(star.clone(), star.clone())),
        );
        self.builtin_types.insert(
            "Effect".to_string(),
            arrow(star.clone(), arrow(star.clone(), star.clone())),
        );

        self.type_constructors = self.builtin_types.clone();
    }

    pub(super) fn builtin_type_constructors(&self) -> std::collections::HashMap<String, Kind> {
        self.builtin_types.clone()
    }

    pub(super) fn register_builtin_values(&mut self) {
        let mut env = TypeEnv::default();
        env.insert("Unit".to_string(), Scheme::mono(Type::con("Unit")));
        env.insert("True".to_string(), Scheme::mono(Type::con("Bool")));
        env.insert("False".to_string(), Scheme::mono(Type::con("Bool")));

        let a = self.fresh_var_id();
        env.insert(
            "None".to_string(),
            Scheme {
                vars: vec![a],
                ty: Type::con("Option").app(vec![Type::Var(a)]),
            },
        );
        let a = self.fresh_var_id();
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

        let e = self.fresh_var_id();
        let a = self.fresh_var_id();
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
        let e = self.fresh_var_id();
        let a = self.fresh_var_id();
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

        let a = self.fresh_var_id();
        let b = self.fresh_var_id();
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

        let a = self.fresh_var_id();
        let e = self.fresh_var_id();
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
        let a = self.fresh_var_id();
        let e = self.fresh_var_id();
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
        let a = self.fresh_var_id();
        let e = self.fresh_var_id();
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

        let e = self.fresh_var_id();
        let a = self.fresh_var_id();
        env.insert(
            "load".to_string(),
            Scheme {
                vars: vec![e, a],
                ty: Type::Func(
                    Box::new(Type::con("Effect").app(vec![Type::Var(e), Type::Var(a)])),
                    Box::new(Type::con("Effect").app(vec![Type::Var(e), Type::Var(a)])),
                ),
            },
        );

        let file_record = Type::Record {
            fields: vec![
                (
                    "read".to_string(),
                    Type::Func(
                        Box::new(Type::con("Text")),
                        Box::new(
                            Type::con("Effect").app(vec![Type::con("Text"), Type::con("Text")]),
                        ),
                    ),
                ),
                (
                    "open".to_string(),
                    Type::Func(
                        Box::new(Type::con("Text")),
                        Box::new(
                            Type::con("Effect")
                                .app(vec![Type::con("Text"), Type::con("FileHandle")]),
                        ),
                    ),
                ),
                (
                    "close".to_string(),
                    Type::Func(
                        Box::new(Type::con("FileHandle")),
                        Box::new(
                            Type::con("Effect").app(vec![Type::con("Text"), Type::con("Unit")]),
                        ),
                    ),
                ),
                (
                    "readAll".to_string(),
                    Type::Func(
                        Box::new(Type::con("FileHandle")),
                        Box::new(
                            Type::con("Effect").app(vec![Type::con("Text"), Type::con("Text")]),
                        ),
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
                        Box::new(
                            Type::con("Effect").app(vec![Type::con("Text"), Type::con("Bool")]),
                        ),
                    ),
                ),
                (
                    "stat".to_string(),
                    Type::Func(
                        Box::new(Type::con("Text")),
                        Box::new(
                            Type::con("Effect")
                                .app(vec![Type::con("Text"), Type::con("FileStats")]),
                        ),
                    ),
                ),
                (
                    "delete".to_string(),
                    Type::Func(
                        Box::new(Type::con("Text")),
                        Box::new(
                            Type::con("Effect").app(vec![Type::con("Text"), Type::con("Unit")]),
                        ),
                    ),
                ),
            ]
            .into_iter()
            .collect(),
            open: true,
        };
        env.insert("file".to_string(), Scheme::mono(file_record));

        let a = self.fresh_var_id();
        let send_ty = Type::con("Send").app(vec![Type::Var(a)]);
        let recv_ty = Type::con("Recv").app(vec![Type::Var(a)]);
        let channel_record = Type::Record {
            fields: vec![
                (
                    "make".to_string(),
                    Type::Func(
                        Box::new(Type::con("Unit")),
                        Box::new(Type::con("Effect").app(vec![
                            Type::con("Closed"),
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
                                Type::con("Effect")
                                    .app(vec![Type::con("Closed"), Type::con("Unit")]),
                            ),
                        )),
                    ),
                ),
                (
                    "recv".to_string(),
                    Type::Func(
                        Box::new(recv_ty.clone()),
                        Box::new(Type::con("Effect").app(vec![
                            Type::con("Closed"),
                            Type::con("Result").app(vec![Type::con("Closed"), Type::Var(a)]),
                        ])),
                    ),
                ),
                (
                    "close".to_string(),
                    Type::Func(
                        Box::new(send_ty),
                        Box::new(
                            Type::con("Effect").app(vec![Type::con("Closed"), Type::con("Unit")]),
                        ),
                    ),
                ),
            ]
            .into_iter()
            .collect(),
            open: true,
        };
        env.insert("channel".to_string(), Scheme::mono(channel_record));

        let e = self.fresh_var_id();
        let a = self.fresh_var_id();
        let b = self.fresh_var_id();
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
                    Box::new(
                        Type::con("Effect").app(vec![Type::con("Text"), Type::con("DateTime")]),
                    ),
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
                        Box::new(
                            Type::con("Effect").app(vec![Type::con("Text"), Type::con("Int")]),
                        ),
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
                            Box::new(
                                Type::con("Effect").app(vec![http_error_ty, server_ty.clone()]),
                            ),
                        )),
                    ),
                ),
                (
                    "stop".to_string(),
                    Type::Func(
                        Box::new(server_ty),
                        Box::new(
                            Type::con("Effect")
                                .app(vec![Type::con("HttpError"), Type::con("Unit")]),
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
                                Type::con("Effect")
                                    .app(vec![ws_error_ty.clone(), Type::con("Unit")]),
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
        let msg = self.fresh_var_id();
        let model = self.fresh_var_id();
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

        let text_ty = Type::con("Text");
        let char_ty = Type::con("Char");
        let int_ty = Type::con("Int");
        let float_ty = Type::con("Float");
        let bool_ty = Type::con("Bool");
        let bytes_ty = Type::con("Bytes");
        let encoding_ty = Type::con("Encoding");
        let text_error_ty = Type::con("TextError");
        let option_int_ty = Type::con("Option").app(vec![int_ty.clone()]);
        let option_float_ty = Type::con("Option").app(vec![float_ty.clone()]);
        let result_text_error_text_ty =
            Type::con("Result").app(vec![text_error_ty.clone(), text_ty.clone()]);
        let list_text_ty = Type::con("List").app(vec![text_ty.clone()]);

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
                        Box::new(Type::Var(self.fresh_var_id())),
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
        };
        env.insert("regex".to_string(), Scheme::mono(regex_record));

        let angle_ty = Type::con("Angle");
        let bigint_ty = Type::con("BigInt");
        let abs_var = self.fresh_var_id();
        let math_record = Type::Record {
            fields: vec![
                ("pi".to_string(), float_ty.clone()),
                ("tau".to_string(), float_ty.clone()),
                ("e".to_string(), float_ty.clone()),
                ("inf".to_string(), float_ty.clone()),
                ("nan".to_string(), float_ty.clone()),
                ("phi".to_string(), float_ty.clone()),
                ("sqrt2".to_string(), float_ty.clone()),
                ("ln2".to_string(), float_ty.clone()),
                ("ln10".to_string(), float_ty.clone()),
                (
                    "abs".to_string(),
                    Type::Func(Box::new(Type::Var(abs_var)), Box::new(Type::Var(abs_var))),
                ),
                (
                    "sign".to_string(),
                    Type::Func(Box::new(float_ty.clone()), Box::new(float_ty.clone())),
                ),
                (
                    "copysign".to_string(),
                    Type::Func(
                        Box::new(float_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(float_ty.clone()),
                            Box::new(float_ty.clone()),
                        )),
                    ),
                ),
                (
                    "min".to_string(),
                    Type::Func(
                        Box::new(float_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(float_ty.clone()),
                            Box::new(float_ty.clone()),
                        )),
                    ),
                ),
                (
                    "max".to_string(),
                    Type::Func(
                        Box::new(float_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(float_ty.clone()),
                            Box::new(float_ty.clone()),
                        )),
                    ),
                ),
                (
                    "minAll".to_string(),
                    Type::Func(
                        Box::new(Type::con("List").app(vec![float_ty.clone()])),
                        Box::new(option_float_ty.clone()),
                    ),
                ),
                (
                    "maxAll".to_string(),
                    Type::Func(
                        Box::new(Type::con("List").app(vec![float_ty.clone()])),
                        Box::new(option_float_ty.clone()),
                    ),
                ),
                (
                    "clamp".to_string(),
                    Type::Func(
                        Box::new(float_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(float_ty.clone()),
                            Box::new(Type::Func(
                                Box::new(float_ty.clone()),
                                Box::new(float_ty.clone()),
                            )),
                        )),
                    ),
                ),
                (
                    "sum".to_string(),
                    Type::Func(
                        Box::new(Type::con("List").app(vec![float_ty.clone()])),
                        Box::new(float_ty.clone()),
                    ),
                ),
                (
                    "sumInt".to_string(),
                    Type::Func(
                        Box::new(Type::con("List").app(vec![int_ty.clone()])),
                        Box::new(int_ty.clone()),
                    ),
                ),
                (
                    "floor".to_string(),
                    Type::Func(Box::new(float_ty.clone()), Box::new(float_ty.clone())),
                ),
                (
                    "ceil".to_string(),
                    Type::Func(Box::new(float_ty.clone()), Box::new(float_ty.clone())),
                ),
                (
                    "trunc".to_string(),
                    Type::Func(Box::new(float_ty.clone()), Box::new(float_ty.clone())),
                ),
                (
                    "round".to_string(),
                    Type::Func(Box::new(float_ty.clone()), Box::new(float_ty.clone())),
                ),
                (
                    "fract".to_string(),
                    Type::Func(Box::new(float_ty.clone()), Box::new(float_ty.clone())),
                ),
                (
                    "modf".to_string(),
                    Type::Func(
                        Box::new(float_ty.clone()),
                        Box::new(Type::Tuple(vec![float_ty.clone(), float_ty.clone()])),
                    ),
                ),
                (
                    "frexp".to_string(),
                    Type::Func(
                        Box::new(float_ty.clone()),
                        Box::new(Type::Tuple(vec![float_ty.clone(), int_ty.clone()])),
                    ),
                ),
                (
                    "ldexp".to_string(),
                    Type::Func(
                        Box::new(float_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(int_ty.clone()),
                            Box::new(float_ty.clone()),
                        )),
                    ),
                ),
                (
                    "pow".to_string(),
                    Type::Func(
                        Box::new(float_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(float_ty.clone()),
                            Box::new(float_ty.clone()),
                        )),
                    ),
                ),
                (
                    "sqrt".to_string(),
                    Type::Func(Box::new(float_ty.clone()), Box::new(float_ty.clone())),
                ),
                (
                    "cbrt".to_string(),
                    Type::Func(Box::new(float_ty.clone()), Box::new(float_ty.clone())),
                ),
                (
                    "hypot".to_string(),
                    Type::Func(
                        Box::new(float_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(float_ty.clone()),
                            Box::new(float_ty.clone()),
                        )),
                    ),
                ),
                (
                    "exp".to_string(),
                    Type::Func(Box::new(float_ty.clone()), Box::new(float_ty.clone())),
                ),
                (
                    "exp2".to_string(),
                    Type::Func(Box::new(float_ty.clone()), Box::new(float_ty.clone())),
                ),
                (
                    "expm1".to_string(),
                    Type::Func(Box::new(float_ty.clone()), Box::new(float_ty.clone())),
                ),
                (
                    "log".to_string(),
                    Type::Func(Box::new(float_ty.clone()), Box::new(float_ty.clone())),
                ),
                (
                    "log10".to_string(),
                    Type::Func(Box::new(float_ty.clone()), Box::new(float_ty.clone())),
                ),
                (
                    "log2".to_string(),
                    Type::Func(Box::new(float_ty.clone()), Box::new(float_ty.clone())),
                ),
                (
                    "log1p".to_string(),
                    Type::Func(Box::new(float_ty.clone()), Box::new(float_ty.clone())),
                ),
                (
                    "sin".to_string(),
                    Type::Func(Box::new(angle_ty.clone()), Box::new(float_ty.clone())),
                ),
                (
                    "cos".to_string(),
                    Type::Func(Box::new(angle_ty.clone()), Box::new(float_ty.clone())),
                ),
                (
                    "tan".to_string(),
                    Type::Func(Box::new(angle_ty.clone()), Box::new(float_ty.clone())),
                ),
                (
                    "asin".to_string(),
                    Type::Func(Box::new(float_ty.clone()), Box::new(angle_ty.clone())),
                ),
                (
                    "acos".to_string(),
                    Type::Func(Box::new(float_ty.clone()), Box::new(angle_ty.clone())),
                ),
                (
                    "atan".to_string(),
                    Type::Func(Box::new(float_ty.clone()), Box::new(angle_ty.clone())),
                ),
                (
                    "atan2".to_string(),
                    Type::Func(
                        Box::new(float_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(float_ty.clone()),
                            Box::new(angle_ty.clone()),
                        )),
                    ),
                ),
                (
                    "sinh".to_string(),
                    Type::Func(Box::new(float_ty.clone()), Box::new(float_ty.clone())),
                ),
                (
                    "cosh".to_string(),
                    Type::Func(Box::new(float_ty.clone()), Box::new(float_ty.clone())),
                ),
                (
                    "tanh".to_string(),
                    Type::Func(Box::new(float_ty.clone()), Box::new(float_ty.clone())),
                ),
                (
                    "asinh".to_string(),
                    Type::Func(Box::new(float_ty.clone()), Box::new(float_ty.clone())),
                ),
                (
                    "acosh".to_string(),
                    Type::Func(Box::new(float_ty.clone()), Box::new(float_ty.clone())),
                ),
                (
                    "atanh".to_string(),
                    Type::Func(Box::new(float_ty.clone()), Box::new(float_ty.clone())),
                ),
                (
                    "gcd".to_string(),
                    Type::Func(
                        Box::new(int_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(int_ty.clone()),
                            Box::new(int_ty.clone()),
                        )),
                    ),
                ),
                (
                    "lcm".to_string(),
                    Type::Func(
                        Box::new(int_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(int_ty.clone()),
                            Box::new(int_ty.clone()),
                        )),
                    ),
                ),
                (
                    "gcdAll".to_string(),
                    Type::Func(
                        Box::new(Type::con("List").app(vec![int_ty.clone()])),
                        Box::new(Type::con("Option").app(vec![int_ty.clone()])),
                    ),
                ),
                (
                    "lcmAll".to_string(),
                    Type::Func(
                        Box::new(Type::con("List").app(vec![int_ty.clone()])),
                        Box::new(Type::con("Option").app(vec![int_ty.clone()])),
                    ),
                ),
                (
                    "factorial".to_string(),
                    Type::Func(Box::new(int_ty.clone()), Box::new(bigint_ty.clone())),
                ),
                (
                    "comb".to_string(),
                    Type::Func(
                        Box::new(int_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(int_ty.clone()),
                            Box::new(bigint_ty.clone()),
                        )),
                    ),
                ),
                (
                    "perm".to_string(),
                    Type::Func(
                        Box::new(int_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(int_ty.clone()),
                            Box::new(bigint_ty.clone()),
                        )),
                    ),
                ),
                (
                    "divmod".to_string(),
                    Type::Func(
                        Box::new(int_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(int_ty.clone()),
                            Box::new(Type::Tuple(vec![int_ty.clone(), int_ty.clone()])),
                        )),
                    ),
                ),
                (
                    "modPow".to_string(),
                    Type::Func(
                        Box::new(int_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(int_ty.clone()),
                            Box::new(Type::Func(
                                Box::new(int_ty.clone()),
                                Box::new(int_ty.clone()),
                            )),
                        )),
                    ),
                ),
                (
                    "isFinite".to_string(),
                    Type::Func(Box::new(float_ty.clone()), Box::new(bool_ty.clone())),
                ),
                (
                    "isInf".to_string(),
                    Type::Func(Box::new(float_ty.clone()), Box::new(bool_ty.clone())),
                ),
                (
                    "isNaN".to_string(),
                    Type::Func(Box::new(float_ty.clone()), Box::new(bool_ty.clone())),
                ),
                (
                    "nextAfter".to_string(),
                    Type::Func(
                        Box::new(float_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(float_ty.clone()),
                            Box::new(float_ty.clone()),
                        )),
                    ),
                ),
                (
                    "ulp".to_string(),
                    Type::Func(Box::new(float_ty.clone()), Box::new(float_ty.clone())),
                ),
                (
                    "fmod".to_string(),
                    Type::Func(
                        Box::new(float_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(float_ty.clone()),
                            Box::new(float_ty.clone()),
                        )),
                    ),
                ),
                (
                    "remainder".to_string(),
                    Type::Func(
                        Box::new(float_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(float_ty.clone()),
                            Box::new(float_ty.clone()),
                        )),
                    ),
                ),
            ]
            .into_iter()
            .collect(),
            open: true,
        };
        env.insert("math".to_string(), Scheme::mono(math_record));

        let date_ty = Type::con("Date");
        let calendar_record = Type::Record {
            fields: vec![
                (
                    "isLeapYear".to_string(),
                    Type::Func(Box::new(date_ty.clone()), Box::new(bool_ty.clone())),
                ),
                (
                    "daysInMonth".to_string(),
                    Type::Func(Box::new(date_ty.clone()), Box::new(int_ty.clone())),
                ),
                (
                    "endOfMonth".to_string(),
                    Type::Func(Box::new(date_ty.clone()), Box::new(date_ty.clone())),
                ),
                (
                    "addDays".to_string(),
                    Type::Func(
                        Box::new(date_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(int_ty.clone()),
                            Box::new(date_ty.clone()),
                        )),
                    ),
                ),
                (
                    "addMonths".to_string(),
                    Type::Func(
                        Box::new(date_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(int_ty.clone()),
                            Box::new(date_ty.clone()),
                        )),
                    ),
                ),
                (
                    "addYears".to_string(),
                    Type::Func(
                        Box::new(date_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(int_ty.clone()),
                            Box::new(date_ty.clone()),
                        )),
                    ),
                ),
            ]
            .into_iter()
            .collect(),
            open: true,
        };
        env.insert("calendar".to_string(), Scheme::mono(calendar_record));

        let rgb_ty = Type::con("Rgb");
        let hsl_ty = Type::con("Hsl");
        let hex_ty = Type::con("Hex");
        let color_record = Type::Record {
            fields: vec![
                (
                    "adjustLightness".to_string(),
                    Type::Func(
                        Box::new(rgb_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(int_ty.clone()),
                            Box::new(rgb_ty.clone()),
                        )),
                    ),
                ),
                (
                    "adjustSaturation".to_string(),
                    Type::Func(
                        Box::new(rgb_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(int_ty.clone()),
                            Box::new(rgb_ty.clone()),
                        )),
                    ),
                ),
                (
                    "adjustHue".to_string(),
                    Type::Func(
                        Box::new(rgb_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(int_ty.clone()),
                            Box::new(rgb_ty.clone()),
                        )),
                    ),
                ),
                (
                    "toRgb".to_string(),
                    Type::Func(Box::new(hsl_ty.clone()), Box::new(rgb_ty.clone())),
                ),
                (
                    "toHsl".to_string(),
                    Type::Func(Box::new(rgb_ty.clone()), Box::new(hsl_ty.clone())),
                ),
                (
                    "toHex".to_string(),
                    Type::Func(Box::new(rgb_ty.clone()), Box::new(hex_ty.clone())),
                ),
            ]
            .into_iter()
            .collect(),
            open: true,
        };
        env.insert("color".to_string(), Scheme::mono(color_record));

        let bigint_record = Type::Record {
            fields: vec![
                (
                    "fromInt".to_string(),
                    Type::Func(Box::new(int_ty.clone()), Box::new(bigint_ty.clone())),
                ),
                (
                    "toInt".to_string(),
                    Type::Func(Box::new(bigint_ty.clone()), Box::new(int_ty.clone())),
                ),
                (
                    "add".to_string(),
                    Type::Func(
                        Box::new(bigint_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(bigint_ty.clone()),
                            Box::new(bigint_ty.clone()),
                        )),
                    ),
                ),
                (
                    "sub".to_string(),
                    Type::Func(
                        Box::new(bigint_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(bigint_ty.clone()),
                            Box::new(bigint_ty.clone()),
                        )),
                    ),
                ),
                (
                    "mul".to_string(),
                    Type::Func(
                        Box::new(bigint_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(bigint_ty.clone()),
                            Box::new(bigint_ty.clone()),
                        )),
                    ),
                ),
            ]
            .into_iter()
            .collect(),
            open: true,
        };
        env.insert("bigint".to_string(), Scheme::mono(bigint_record));

        let rational_ty = Type::con("Rational");
        let rational_record = Type::Record {
            fields: vec![
                (
                    "fromBigInts".to_string(),
                    Type::Func(
                        Box::new(bigint_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(bigint_ty.clone()),
                            Box::new(rational_ty.clone()),
                        )),
                    ),
                ),
                (
                    "normalize".to_string(),
                    Type::Func(Box::new(rational_ty.clone()), Box::new(rational_ty.clone())),
                ),
                (
                    "numerator".to_string(),
                    Type::Func(Box::new(rational_ty.clone()), Box::new(bigint_ty.clone())),
                ),
                (
                    "denominator".to_string(),
                    Type::Func(Box::new(rational_ty.clone()), Box::new(bigint_ty.clone())),
                ),
                (
                    "add".to_string(),
                    Type::Func(
                        Box::new(rational_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(rational_ty.clone()),
                            Box::new(rational_ty.clone()),
                        )),
                    ),
                ),
                (
                    "sub".to_string(),
                    Type::Func(
                        Box::new(rational_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(rational_ty.clone()),
                            Box::new(rational_ty.clone()),
                        )),
                    ),
                ),
                (
                    "mul".to_string(),
                    Type::Func(
                        Box::new(rational_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(rational_ty.clone()),
                            Box::new(rational_ty.clone()),
                        )),
                    ),
                ),
                (
                    "div".to_string(),
                    Type::Func(
                        Box::new(rational_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(rational_ty.clone()),
                            Box::new(rational_ty.clone()),
                        )),
                    ),
                ),
            ]
            .into_iter()
            .collect(),
            open: true,
        };
        env.insert("rational".to_string(), Scheme::mono(rational_record));

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
            open: true,
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
            open: true,
        };
        env.insert("url".to_string(), Scheme::mono(url_record));

        let request_ty = Type::con("Request");
        let response_ty = Type::con("Response");
        let error_ty = Type::con("Error");
        let http_result_ty = Type::con("Result").app(vec![error_ty.clone(), response_ty.clone()]);
        let http_effect_ty =
            Type::con("Effect").app(vec![error_ty.clone(), http_result_ty.clone()]);
        let http_record = Type::Record {
            fields: vec![
                (
                    "get".to_string(),
                    Type::Func(Box::new(url_ty.clone()), Box::new(http_effect_ty.clone())),
                ),
                (
                    "post".to_string(),
                    Type::Func(
                        Box::new(url_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(text_ty.clone()),
                            Box::new(http_effect_ty.clone()),
                        )),
                    ),
                ),
                (
                    "fetch".to_string(),
                    Type::Func(
                        Box::new(request_ty.clone()),
                        Box::new(http_effect_ty.clone()),
                    ),
                ),
            ]
            .into_iter()
            .collect(),
            open: true,
        };
        env.insert("http".to_string(), Scheme::mono(http_record));

        let https_record = Type::Record {
            fields: vec![
                (
                    "get".to_string(),
                    Type::Func(Box::new(url_ty.clone()), Box::new(http_effect_ty.clone())),
                ),
                (
                    "post".to_string(),
                    Type::Func(
                        Box::new(url_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(text_ty.clone()),
                            Box::new(http_effect_ty.clone()),
                        )),
                    ),
                ),
                (
                    "fetch".to_string(),
                    Type::Func(
                        Box::new(request_ty.clone()),
                        Box::new(http_effect_ty.clone()),
                    ),
                ),
            ]
            .into_iter()
            .collect(),
            open: true,
        };
        env.insert("https".to_string(), Scheme::mono(https_record));

        let address_ty = Type::Record {
            fields: vec![
                ("host".to_string(), Type::con("Text")),
                ("port".to_string(), Type::con("Int")),
            ]
            .into_iter()
            .collect(),
            open: true,
        };
        let socket_error_ty = Type::Record {
            fields: vec![("message".to_string(), Type::con("Text"))]
                .into_iter()
                .collect(),
            open: true,
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
                            Type::con("Effect")
                                .app(vec![socket_error_ty.clone(), Type::con("Unit")]),
                        ),
                    ),
                ),
                (
                    "closeListener".to_string(),
                    Type::Func(
                        Box::new(Type::con("Listener")),
                        Box::new(
                            Type::con("Effect")
                                .app(vec![socket_error_ty.clone(), Type::con("Unit")]),
                        ),
                    ),
                ),
            ]
            .into_iter()
            .collect(),
            open: true,
        };
        env.insert("sockets".to_string(), Scheme::mono(sockets_record));

        let stream_error_ty = Type::Record {
            fields: vec![("message".to_string(), Type::con("Text"))]
                .into_iter()
                .collect(),
            open: true,
        };
        let stream_bytes_ty =
            Type::con("Stream").app(vec![Type::con("List").app(vec![Type::con("Int")])]);
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
            ]
            .into_iter()
            .collect(),
            open: true,
        };
        env.insert("streams".to_string(), Scheme::mono(streams_record));

        let map_k = self.fresh_var_id();
        let map_v = self.fresh_var_id();
        let map_v2 = self.fresh_var_id();
        let map_ty = Type::con("Map").app(vec![Type::Var(map_k), Type::Var(map_v)]);
        let map_ty_v2 = Type::con("Map").app(vec![Type::Var(map_k), Type::Var(map_v2)]);
        let map_tuple_ty = Type::Tuple(vec![Type::Var(map_k), Type::Var(map_v)]);
        let list_map_tuple_ty = Type::con("List").app(vec![map_tuple_ty.clone()]);
        let map_record = Type::Record {
            fields: vec![
                ("empty".to_string(), map_ty.clone()),
                (
                    "size".to_string(),
                    Type::Func(Box::new(map_ty.clone()), Box::new(int_ty.clone())),
                ),
                (
                    "has".to_string(),
                    Type::Func(
                        Box::new(Type::Var(map_k)),
                        Box::new(Type::Func(
                            Box::new(map_ty.clone()),
                            Box::new(bool_ty.clone()),
                        )),
                    ),
                ),
                (
                    "get".to_string(),
                    Type::Func(
                        Box::new(Type::Var(map_k)),
                        Box::new(Type::Func(
                            Box::new(map_ty.clone()),
                            Box::new(Type::con("Option").app(vec![Type::Var(map_v)])),
                        )),
                    ),
                ),
                (
                    "insert".to_string(),
                    Type::Func(
                        Box::new(Type::Var(map_k)),
                        Box::new(Type::Func(
                            Box::new(Type::Var(map_v)),
                            Box::new(Type::Func(
                                Box::new(map_ty.clone()),
                                Box::new(map_ty.clone()),
                            )),
                        )),
                    ),
                ),
                (
                    "update".to_string(),
                    Type::Func(
                        Box::new(Type::Var(map_k)),
                        Box::new(Type::Func(
                            Box::new(Type::Func(
                                Box::new(Type::Var(map_v)),
                                Box::new(Type::Var(map_v)),
                            )),
                            Box::new(Type::Func(
                                Box::new(map_ty.clone()),
                                Box::new(map_ty.clone()),
                            )),
                        )),
                    ),
                ),
                (
                    "remove".to_string(),
                    Type::Func(
                        Box::new(Type::Var(map_k)),
                        Box::new(Type::Func(
                            Box::new(map_ty.clone()),
                            Box::new(map_ty.clone()),
                        )),
                    ),
                ),
                (
                    "map".to_string(),
                    Type::Func(
                        Box::new(Type::Func(
                            Box::new(Type::Var(map_v)),
                            Box::new(Type::Var(map_v2)),
                        )),
                        Box::new(Type::Func(
                            Box::new(map_ty.clone()),
                            Box::new(map_ty_v2.clone()),
                        )),
                    ),
                ),
                (
                    "mapWithKey".to_string(),
                    Type::Func(
                        Box::new(Type::Func(
                            Box::new(Type::Var(map_k)),
                            Box::new(Type::Func(
                                Box::new(Type::Var(map_v)),
                                Box::new(Type::Var(map_v2)),
                            )),
                        )),
                        Box::new(Type::Func(
                            Box::new(map_ty.clone()),
                            Box::new(map_ty_v2.clone()),
                        )),
                    ),
                ),
                (
                    "keys".to_string(),
                    Type::Func(
                        Box::new(map_ty.clone()),
                        Box::new(Type::con("List").app(vec![Type::Var(map_k)])),
                    ),
                ),
                (
                    "values".to_string(),
                    Type::Func(
                        Box::new(map_ty.clone()),
                        Box::new(Type::con("List").app(vec![Type::Var(map_v)])),
                    ),
                ),
                (
                    "entries".to_string(),
                    Type::Func(
                        Box::new(map_ty.clone()),
                        Box::new(list_map_tuple_ty.clone()),
                    ),
                ),
                (
                    "fromList".to_string(),
                    Type::Func(
                        Box::new(list_map_tuple_ty.clone()),
                        Box::new(map_ty.clone()),
                    ),
                ),
                (
                    "toList".to_string(),
                    Type::Func(
                        Box::new(map_ty.clone()),
                        Box::new(list_map_tuple_ty.clone()),
                    ),
                ),
                (
                    "union".to_string(),
                    Type::Func(
                        Box::new(map_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(map_ty.clone()),
                            Box::new(map_ty.clone()),
                        )),
                    ),
                ),
            ]
            .into_iter()
            .collect(),
            open: true,
        };
        let map_record_value = map_record.clone();

        let set_a = self.fresh_var_id();
        let set_ty = Type::con("Set").app(vec![Type::Var(set_a)]);
        let set_record = Type::Record {
            fields: vec![
                ("empty".to_string(), set_ty.clone()),
                (
                    "size".to_string(),
                    Type::Func(Box::new(set_ty.clone()), Box::new(int_ty.clone())),
                ),
                (
                    "has".to_string(),
                    Type::Func(
                        Box::new(Type::Var(set_a)),
                        Box::new(Type::Func(
                            Box::new(set_ty.clone()),
                            Box::new(bool_ty.clone()),
                        )),
                    ),
                ),
                (
                    "insert".to_string(),
                    Type::Func(
                        Box::new(Type::Var(set_a)),
                        Box::new(Type::Func(
                            Box::new(set_ty.clone()),
                            Box::new(set_ty.clone()),
                        )),
                    ),
                ),
                (
                    "remove".to_string(),
                    Type::Func(
                        Box::new(Type::Var(set_a)),
                        Box::new(Type::Func(
                            Box::new(set_ty.clone()),
                            Box::new(set_ty.clone()),
                        )),
                    ),
                ),
                (
                    "union".to_string(),
                    Type::Func(
                        Box::new(set_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(set_ty.clone()),
                            Box::new(set_ty.clone()),
                        )),
                    ),
                ),
                (
                    "intersection".to_string(),
                    Type::Func(
                        Box::new(set_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(set_ty.clone()),
                            Box::new(set_ty.clone()),
                        )),
                    ),
                ),
                (
                    "difference".to_string(),
                    Type::Func(
                        Box::new(set_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(set_ty.clone()),
                            Box::new(set_ty.clone()),
                        )),
                    ),
                ),
                (
                    "fromList".to_string(),
                    Type::Func(
                        Box::new(Type::con("List").app(vec![Type::Var(set_a)])),
                        Box::new(set_ty.clone()),
                    ),
                ),
                (
                    "toList".to_string(),
                    Type::Func(
                        Box::new(set_ty.clone()),
                        Box::new(Type::con("List").app(vec![Type::Var(set_a)])),
                    ),
                ),
            ]
            .into_iter()
            .collect(),
            open: true,
        };
        let set_record_value = set_record.clone();

        let queue_a = self.fresh_var_id();
        let queue_ty = Type::con("Queue").app(vec![Type::Var(queue_a)]);
        let queue_tuple_ty = Type::Tuple(vec![Type::Var(queue_a), queue_ty.clone()]);
        let queue_record = Type::Record {
            fields: vec![
                ("empty".to_string(), queue_ty.clone()),
                (
                    "enqueue".to_string(),
                    Type::Func(
                        Box::new(Type::Var(queue_a)),
                        Box::new(Type::Func(
                            Box::new(queue_ty.clone()),
                            Box::new(queue_ty.clone()),
                        )),
                    ),
                ),
                (
                    "dequeue".to_string(),
                    Type::Func(
                        Box::new(queue_ty.clone()),
                        Box::new(Type::con("Option").app(vec![queue_tuple_ty.clone()])),
                    ),
                ),
                (
                    "peek".to_string(),
                    Type::Func(
                        Box::new(queue_ty.clone()),
                        Box::new(Type::con("Option").app(vec![Type::Var(queue_a)])),
                    ),
                ),
            ]
            .into_iter()
            .collect(),
            open: true,
        };
        let queue_record_value = queue_record.clone();

        let deque_a = self.fresh_var_id();
        let deque_ty = Type::con("Deque").app(vec![Type::Var(deque_a)]);
        let deque_tuple_ty = Type::Tuple(vec![Type::Var(deque_a), deque_ty.clone()]);
        let deque_record = Type::Record {
            fields: vec![
                ("empty".to_string(), deque_ty.clone()),
                (
                    "pushFront".to_string(),
                    Type::Func(
                        Box::new(Type::Var(deque_a)),
                        Box::new(Type::Func(
                            Box::new(deque_ty.clone()),
                            Box::new(deque_ty.clone()),
                        )),
                    ),
                ),
                (
                    "pushBack".to_string(),
                    Type::Func(
                        Box::new(Type::Var(deque_a)),
                        Box::new(Type::Func(
                            Box::new(deque_ty.clone()),
                            Box::new(deque_ty.clone()),
                        )),
                    ),
                ),
                (
                    "popFront".to_string(),
                    Type::Func(
                        Box::new(deque_ty.clone()),
                        Box::new(Type::con("Option").app(vec![deque_tuple_ty.clone()])),
                    ),
                ),
                (
                    "popBack".to_string(),
                    Type::Func(
                        Box::new(deque_ty.clone()),
                        Box::new(Type::con("Option").app(vec![deque_tuple_ty.clone()])),
                    ),
                ),
                (
                    "peekFront".to_string(),
                    Type::Func(
                        Box::new(deque_ty.clone()),
                        Box::new(Type::con("Option").app(vec![Type::Var(deque_a)])),
                    ),
                ),
                (
                    "peekBack".to_string(),
                    Type::Func(
                        Box::new(deque_ty.clone()),
                        Box::new(Type::con("Option").app(vec![Type::Var(deque_a)])),
                    ),
                ),
            ]
            .into_iter()
            .collect(),
            open: true,
        };
        let deque_record_value = deque_record.clone();

        let heap_a = self.fresh_var_id();
        let heap_ty = Type::con("Heap").app(vec![Type::Var(heap_a)]);
        let heap_tuple_ty = Type::Tuple(vec![Type::Var(heap_a), heap_ty.clone()]);
        let heap_record = Type::Record {
            fields: vec![
                ("empty".to_string(), heap_ty.clone()),
                (
                    "push".to_string(),
                    Type::Func(
                        Box::new(Type::Var(heap_a)),
                        Box::new(Type::Func(
                            Box::new(heap_ty.clone()),
                            Box::new(heap_ty.clone()),
                        )),
                    ),
                ),
                (
                    "popMin".to_string(),
                    Type::Func(
                        Box::new(heap_ty.clone()),
                        Box::new(Type::con("Option").app(vec![heap_tuple_ty.clone()])),
                    ),
                ),
                (
                    "peekMin".to_string(),
                    Type::Func(
                        Box::new(heap_ty.clone()),
                        Box::new(Type::con("Option").app(vec![Type::Var(heap_a)])),
                    ),
                ),
            ]
            .into_iter()
            .collect(),
            open: true,
        };
        let heap_record_value = heap_record.clone();

        let collections_record = Type::Record {
            fields: vec![
                ("map".to_string(), map_record),
                ("set".to_string(), set_record),
                ("queue".to_string(), queue_record),
                ("deque".to_string(), deque_record),
                ("heap".to_string(), heap_record),
            ]
            .into_iter()
            .collect(),
            open: true,
        };
        env.insert(
            "collections".to_string(),
            Scheme {
                vars: vec![map_k, map_v, map_v2, set_a, queue_a, deque_a, heap_a],
                ty: collections_record,
            },
        );
        env.insert(
            "Map".to_string(),
            Scheme {
                vars: vec![map_k, map_v, map_v2],
                ty: map_record_value,
            },
        );
        env.insert(
            "Set".to_string(),
            Scheme {
                vars: vec![set_a],
                ty: set_record_value,
            },
        );
        env.insert(
            "Queue".to_string(),
            Scheme {
                vars: vec![queue_a],
                ty: queue_record_value,
            },
        );
        env.insert(
            "Deque".to_string(),
            Scheme {
                vars: vec![deque_a],
                ty: deque_record_value,
            },
        );
        env.insert(
            "Heap".to_string(),
            Scheme {
                vars: vec![heap_a],
                ty: heap_record_value,
            },
        );

        let vec_ty = Type::con("Vec");
        let mat_ty = Type::con("Mat");
        let linalg_record = Type::Record {
            fields: vec![
                (
                    "dot".to_string(),
                    Type::Func(
                        Box::new(vec_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(vec_ty.clone()),
                            Box::new(float_ty.clone()),
                        )),
                    ),
                ),
                (
                    "matMul".to_string(),
                    Type::Func(
                        Box::new(mat_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(mat_ty.clone()),
                            Box::new(mat_ty.clone()),
                        )),
                    ),
                ),
                (
                    "solve2x2".to_string(),
                    Type::Func(
                        Box::new(mat_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(vec_ty.clone()),
                            Box::new(vec_ty.clone()),
                        )),
                    ),
                ),
            ]
            .into_iter()
            .collect(),
            open: true,
        };
        env.insert("linalg".to_string(), Scheme::mono(linalg_record));

        let signal_ty = Type::con("Signal");
        let spectrum_ty = Type::con("Spectrum");
        let signal_record = Type::Record {
            fields: vec![
                (
                    "fft".to_string(),
                    Type::Func(Box::new(signal_ty.clone()), Box::new(spectrum_ty.clone())),
                ),
                (
                    "ifft".to_string(),
                    Type::Func(Box::new(spectrum_ty.clone()), Box::new(signal_ty.clone())),
                ),
                (
                    "windowHann".to_string(),
                    Type::Func(Box::new(signal_ty.clone()), Box::new(signal_ty.clone())),
                ),
                (
                    "normalize".to_string(),
                    Type::Func(Box::new(signal_ty.clone()), Box::new(signal_ty.clone())),
                ),
            ]
            .into_iter()
            .collect(),
            open: true,
        };
        env.insert("signal".to_string(), Scheme::mono(signal_record));

        let graph_ty = Type::con("Graph");
        let edge_ty = Type::con("Edge");
        let list_node_ty = Type::con("List").app(vec![int_ty.clone()]);
        let graph_record = Type::Record {
            fields: vec![
                (
                    "addEdge".to_string(),
                    Type::Func(
                        Box::new(graph_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(edge_ty.clone()),
                            Box::new(graph_ty.clone()),
                        )),
                    ),
                ),
                (
                    "neighbors".to_string(),
                    Type::Func(
                        Box::new(graph_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(int_ty.clone()),
                            Box::new(list_node_ty.clone()),
                        )),
                    ),
                ),
                (
                    "shortestPath".to_string(),
                    Type::Func(
                        Box::new(graph_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(int_ty.clone()),
                            Box::new(Type::Func(
                                Box::new(int_ty.clone()),
                                Box::new(list_node_ty.clone()),
                            )),
                        )),
                    ),
                ),
            ]
            .into_iter()
            .collect(),
            open: true,
        };
        env.insert("graph".to_string(), Scheme::mono(graph_record));

        let ansi_color_ty = Type::con("AnsiColor");
        let ansi_style_ty = Type::con("AnsiStyle");
        let console_record = Type::Record {
            fields: vec![
                (
                    "log".to_string(),
                    Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(Type::con("Effect").app(vec![text_ty.clone(), Type::con("Unit")])),
                    ),
                ),
                (
                    "println".to_string(),
                    Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(Type::con("Effect").app(vec![text_ty.clone(), Type::con("Unit")])),
                    ),
                ),
                (
                    "print".to_string(),
                    Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(Type::con("Effect").app(vec![text_ty.clone(), Type::con("Unit")])),
                    ),
                ),
                (
                    "error".to_string(),
                    Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(Type::con("Effect").app(vec![text_ty.clone(), Type::con("Unit")])),
                    ),
                ),
                (
                    "readLine".to_string(),
                    Type::Func(
                        Box::new(Type::con("Unit")),
                        Box::new(Type::con("Effect").app(vec![
                            text_ty.clone(),
                            Type::con("Result").app(vec![text_ty.clone(), text_ty.clone()]),
                        ])),
                    ),
                ),
                (
                    "color".to_string(),
                    Type::Func(
                        Box::new(ansi_color_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(text_ty.clone()),
                            Box::new(text_ty.clone()),
                        )),
                    ),
                ),
                (
                    "bgColor".to_string(),
                    Type::Func(
                        Box::new(ansi_color_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(text_ty.clone()),
                            Box::new(text_ty.clone()),
                        )),
                    ),
                ),
                (
                    "style".to_string(),
                    Type::Func(
                        Box::new(ansi_style_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(text_ty.clone()),
                            Box::new(text_ty.clone()),
                        )),
                    ),
                ),
                (
                    "strip".to_string(),
                    Type::Func(Box::new(text_ty.clone()), Box::new(text_ty.clone())),
                ),
            ]
            .into_iter()
            .collect(),
            open: true,
        };
        env.insert("console".to_string(), Scheme::mono(console_record));

        let option_text_ty = Type::con("Option").app(vec![text_ty.clone()]);
        let env_record = Type::Record {
            fields: vec![
                (
                    "get".to_string(),
                    Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(
                            Type::con("Effect").app(vec![text_ty.clone(), option_text_ty.clone()]),
                        ),
                    ),
                ),
                (
                    "set".to_string(),
                    Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(text_ty.clone()),
                            Box::new(
                                Type::con("Effect").app(vec![text_ty.clone(), Type::con("Unit")]),
                            ),
                        )),
                    ),
                ),
                (
                    "remove".to_string(),
                    Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(Type::con("Effect").app(vec![text_ty.clone(), Type::con("Unit")])),
                    ),
                ),
            ]
            .into_iter()
            .collect(),
            open: false,
        };
        let system_record = Type::Record {
            fields: vec![
                ("env".to_string(), env_record),
                (
                    "args".to_string(),
                    Type::Func(
                        Box::new(Type::con("Unit")),
                        Box::new(Type::con("Effect").app(vec![
                            text_ty.clone(),
                            Type::con("List").app(vec![text_ty.clone()]),
                        ])),
                    ),
                ),
                (
                    "exit".to_string(),
                    Type::Func(
                        Box::new(int_ty.clone()),
                        Box::new(Type::con("Effect").app(vec![text_ty.clone(), Type::con("Unit")])),
                    ),
                ),
            ]
            .into_iter()
            .collect(),
            open: true,
        };
        env.insert("system".to_string(), Scheme::mono(system_record));

        let level_ty = Type::con("Level");
        let context_pair_ty = Type::Tuple(vec![text_ty.clone(), text_ty.clone()]);
        let context_ty = Type::con("List").app(vec![context_pair_ty]);
        let log_effect_ty = Type::con("Effect").app(vec![text_ty.clone(), Type::con("Unit")]);
        let logger_record = Type::Record {
            fields: vec![
                (
                    "log".to_string(),
                    Type::Func(
                        Box::new(level_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(text_ty.clone()),
                            Box::new(Type::Func(
                                Box::new(context_ty.clone()),
                                Box::new(log_effect_ty.clone()),
                            )),
                        )),
                    ),
                ),
                (
                    "trace".to_string(),
                    Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(context_ty.clone()),
                            Box::new(log_effect_ty.clone()),
                        )),
                    ),
                ),
                (
                    "debug".to_string(),
                    Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(context_ty.clone()),
                            Box::new(log_effect_ty.clone()),
                        )),
                    ),
                ),
                (
                    "info".to_string(),
                    Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(context_ty.clone()),
                            Box::new(log_effect_ty.clone()),
                        )),
                    ),
                ),
                (
                    "warn".to_string(),
                    Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(context_ty.clone()),
                            Box::new(log_effect_ty.clone()),
                        )),
                    ),
                ),
                (
                    "error".to_string(),
                    Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(context_ty.clone()),
                            Box::new(log_effect_ty.clone()),
                        )),
                    ),
                ),
            ]
            .into_iter()
            .collect(),
            open: true,
        };
        env.insert("logger".to_string(), Scheme::mono(logger_record));

        let db_row = self.fresh_var_id();
        let db_error_ty = Type::con("DbError");
        let db_config_ty = Type::con("DbConfig");
        let table_ty = Type::con("Table").app(vec![Type::Var(db_row)]);
        let pred_ty = Type::con("Pred").app(vec![Type::Var(db_row)]);
        let patch_ty = Type::con("Patch").app(vec![Type::Var(db_row)]);
        let delta_ty = Type::con("Delta").app(vec![Type::Var(db_row)]);
        let list_table_ty = Type::con("List").app(vec![table_ty.clone()]);
        let list_row_ty = Type::con("List").app(vec![Type::Var(db_row)]);
        let list_column_ty = Type::con("List").app(vec![Type::con("Column")]);
        let db_effect_table_ty =
            Type::con("Effect").app(vec![db_error_ty.clone(), table_ty.clone()]);
        let db_effect_rows_ty =
            Type::con("Effect").app(vec![db_error_ty.clone(), list_row_ty.clone()]);
        let db_effect_unit_ty =
            Type::con("Effect").app(vec![db_error_ty.clone(), Type::con("Unit")]);
        let database_record = Type::Record {
            fields: vec![
                (
                    "table".to_string(),
                    Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(list_column_ty),
                            Box::new(table_ty.clone()),
                        )),
                    ),
                ),
                (
                    "configure".to_string(),
                    Type::Func(Box::new(db_config_ty), Box::new(db_effect_unit_ty.clone())),
                ),
                (
                    "load".to_string(),
                    Type::Func(Box::new(table_ty.clone()), Box::new(db_effect_rows_ty)),
                ),
                (
                    "applyDelta".to_string(),
                    Type::Func(
                        Box::new(table_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(delta_ty.clone()),
                            Box::new(db_effect_table_ty),
                        )),
                    ),
                ),
                (
                    "runMigrations".to_string(),
                    Type::Func(Box::new(list_table_ty), Box::new(db_effect_unit_ty)),
                ),
                (
                    "ins".to_string(),
                    Type::Func(Box::new(Type::Var(db_row)), Box::new(delta_ty.clone())),
                ),
                (
                    "upd".to_string(),
                    Type::Func(
                        Box::new(pred_ty.clone()),
                        Box::new(Type::Func(Box::new(patch_ty), Box::new(delta_ty.clone()))),
                    ),
                ),
                (
                    "del".to_string(),
                    Type::Func(Box::new(pred_ty), Box::new(delta_ty.clone())),
                ),
            ]
            .into_iter()
            .collect(),
            open: true,
        };
        env.insert("database".to_string(), Scheme::mono(database_record));

        self.builtins = env;
    }
}
