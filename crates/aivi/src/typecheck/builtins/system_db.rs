use super::TypeChecker;
use crate::typecheck::types::{Scheme, Type, TypeEnv};

pub(super) fn register(checker: &mut TypeChecker, env: &mut TypeEnv) {
    let int_ty = Type::con("Int");
    let text_ty = Type::con("Text");

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
    };
    env.insert("console".to_string(), Scheme::mono(console_record));

    // Builtin source records that are exported from `aivi` but implemented by the runtime.
    // Keep these lightweight: they primarily exist so user code can reference them and the
    // embedded stdlib wrappers can typecheck against field access.
    let crypto_record = Type::Record {
        fields: vec![
            (
                "sha256".to_string(),
                Type::Func(Box::new(text_ty.clone()), Box::new(text_ty.clone())),
            ),
            (
                "randomUuid".to_string(),
                Type::Func(
                    Box::new(Type::con("Unit")),
                    Box::new(Type::con("Effect").app(vec![text_ty.clone(), text_ty.clone()])),
                ),
            ),
            (
                "randomBytes".to_string(),
                Type::Func(
                    Box::new(int_ty.clone()),
                    Box::new(Type::con("Effect").app(vec![text_ty.clone(), Type::con("Bytes")])),
                ),
            ),
        ]
        .into_iter()
        .collect(),
    };
    env.insert("crypto".to_string(), Scheme::mono(crypto_record));

    let i18n_record = Type::Record {
        fields: vec![].into_iter().collect(),
    };
    env.insert("i18n".to_string(), Scheme::mono(i18n_record));

    let option_text_ty = Type::con("Option").app(vec![text_ty.clone()]);
    let command_output_ty = Type::Record {
        fields: vec![
            ("status".to_string(), int_ty.clone()),
            ("stdout".to_string(), text_ty.clone()),
            ("stderr".to_string(), text_ty.clone()),
        ]
        .into_iter()
        .collect(),
    };
    let system_env_decode_a = checker.fresh_var_id();
    let system_env_decode_arg = checker.fresh_var_id();
    let env_record = Type::Record {
        fields: vec![
            (
                "get".to_string(),
                Type::Func(
                    Box::new(text_ty.clone()),
                    Box::new(
                        Type::con("Source").app(vec![Type::con("Env"), option_text_ty.clone()]),
                    ),
                ),
            ),
            (
                "set".to_string(),
                Type::Func(
                    Box::new(text_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(Type::con("Effect").app(vec![text_ty.clone(), Type::con("Unit")])),
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
            (
                "decode".to_string(),
                Type::Func(
                    Box::new(Type::Var(system_env_decode_arg)),
                    Box::new(
                        Type::con("Source")
                            .app(vec![Type::con("Env"), Type::Var(system_env_decode_a)]),
                    ),
                ),
            ),
        ]
        .into_iter()
        .collect(),
    };
    let env_decode_a = checker.fresh_var_id();
    let env_decode_arg = checker.fresh_var_id();
    let env_source_record = Type::Record {
        fields: vec![
            (
                "get".to_string(),
                Type::Func(
                    Box::new(text_ty.clone()),
                    Box::new(Type::con("Source").app(vec![Type::con("Env"), text_ty.clone()])),
                ),
            ),
            (
                "set".to_string(),
                Type::Func(
                    Box::new(text_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(Type::con("Effect").app(vec![text_ty.clone(), Type::con("Unit")])),
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
            (
                "decode".to_string(),
                Type::Func(
                    Box::new(Type::Var(env_decode_arg)),
                    Box::new(
                        Type::con("Source").app(vec![Type::con("Env"), Type::Var(env_decode_a)]),
                    ),
                ),
            ),
        ]
        .into_iter()
        .collect(),
    };
    env.insert(
        "env".to_string(),
        Scheme {
            vars: vec![env_decode_a, env_decode_arg],
            ty: env_source_record,
            origin: None,
        },
    );
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
            (
                "localeTag".to_string(),
                Type::Func(
                    Box::new(Type::con("Unit")),
                    Box::new(
                        Type::con("Effect").app(vec![text_ty.clone(), option_text_ty.clone()]),
                    ),
                ),
            ),
            (
                "run".to_string(),
                Type::Func(
                    Box::new(text_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(Type::con("List").app(vec![text_ty.clone()])),
                        Box::new(Type::con("Effect").app(vec![text_ty.clone(), command_output_ty])),
                    )),
                ),
            ),
        ]
        .into_iter()
        .collect(),
    };
    env.insert(
        "system".to_string(),
        Scheme {
            vars: vec![system_env_decode_a, system_env_decode_arg],
            ty: system_record,
            origin: None,
        },
    );
    let imap_a = checker.fresh_var_id();
    let option_int_ty = Type::con("Option").app(vec![int_ty.clone()]);
    let option_bool_ty = Type::con("Option").app(vec![Type::con("Bool")]);
    let list_text_ty = Type::con("List").app(vec![text_ty.clone()]);
    let email_auth_ty = Type::con("EmailAuth");
    let imap_session_ty = Type::con("ImapSession");
    let mime_part_ty = Type::Record {
        fields: vec![
            ("contentType".to_string(), text_ty.clone()),
            ("body".to_string(), text_ty.clone()),
        ]
        .into_iter()
        .collect(),
    };
    let mailbox_info_ty = Type::Record {
        fields: vec![
            ("name".to_string(), text_ty.clone()),
            ("separator".to_string(), option_text_ty.clone()),
            ("attributes".to_string(), list_text_ty.clone()),
        ]
        .into_iter()
        .collect(),
    };
    let imap_config_ty = Type::Record {
        fields: vec![
            ("host".to_string(), text_ty.clone()),
            ("user".to_string(), text_ty.clone()),
            ("auth".to_string(), email_auth_ty.clone()),
            ("port".to_string(), option_int_ty.clone()),
            ("starttls".to_string(), option_bool_ty.clone()),
            ("mailbox".to_string(), option_text_ty.clone()),
            ("filter".to_string(), option_text_ty.clone()),
            ("limit".to_string(), option_int_ty.clone()),
        ]
        .into_iter()
        .collect(),
    };
    let smtp_config_ty = Type::Record {
        fields: vec![
            ("host".to_string(), text_ty.clone()),
            ("user".to_string(), text_ty.clone()),
            ("auth".to_string(), email_auth_ty.clone()),
            ("from".to_string(), text_ty.clone()),
            ("to".to_string(), list_text_ty.clone()),
            (
                "cc".to_string(),
                Type::con("Option").app(vec![list_text_ty.clone()]),
            ),
            (
                "bcc".to_string(),
                Type::con("Option").app(vec![list_text_ty.clone()]),
            ),
            ("subject".to_string(), text_ty.clone()),
            ("body".to_string(), text_ty.clone()),
            ("port".to_string(), option_int_ty.clone()),
            ("starttls".to_string(), option_bool_ty.clone()),
        ]
        .into_iter()
        .collect(),
    };
    let email_record = Type::Record {
        fields: vec![
            (
                "imap".to_string(),
                Type::Func(
                    Box::new(imap_config_ty.clone()),
                    Box::new(Type::con("Source").app(vec![
                        Type::con("Imap"),
                        Type::con("List").app(vec![Type::Var(imap_a)]),
                    ])),
                ),
            ),
            (
                "imapOpen".to_string(),
                Type::Func(
                    Box::new(imap_config_ty.clone()),
                    Box::new(
                        Type::con("Effect").app(vec![text_ty.clone(), imap_session_ty.clone()]),
                    ),
                ),
            ),
            (
                "imapClose".to_string(),
                Type::Func(
                    Box::new(imap_session_ty.clone()),
                    Box::new(Type::con("Effect").app(vec![text_ty.clone(), Type::con("Unit")])),
                ),
            ),
            (
                "imapSelect".to_string(),
                Type::Func(
                    Box::new(text_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(imap_session_ty.clone()),
                        Box::new(
                            Type::con("Effect").app(vec![text_ty.clone(), mailbox_info_ty.clone()]),
                        ),
                    )),
                ),
            ),
            (
                "imapExamine".to_string(),
                Type::Func(
                    Box::new(text_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(imap_session_ty.clone()),
                        Box::new(
                            Type::con("Effect").app(vec![text_ty.clone(), mailbox_info_ty.clone()]),
                        ),
                    )),
                ),
            ),
            (
                "imapSearch".to_string(),
                Type::Func(
                    Box::new(text_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(imap_session_ty.clone()),
                        Box::new(Type::con("Effect").app(vec![
                            text_ty.clone(),
                            Type::con("List").app(vec![int_ty.clone()]),
                        ])),
                    )),
                ),
            ),
            (
                "imapFetch".to_string(),
                Type::Func(
                    Box::new(Type::con("List").app(vec![int_ty.clone()])),
                    Box::new(Type::Func(
                        Box::new(imap_session_ty.clone()),
                        Box::new(Type::con("Effect").app(vec![
                            text_ty.clone(),
                            Type::con("List").app(vec![Type::Var(imap_a)]),
                        ])),
                    )),
                ),
            ),
            (
                "imapSetFlags".to_string(),
                Type::Func(
                    Box::new(Type::con("List").app(vec![int_ty.clone()])),
                    Box::new(Type::Func(
                        Box::new(list_text_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(imap_session_ty.clone()),
                            Box::new(
                                Type::con("Effect").app(vec![text_ty.clone(), Type::con("Unit")]),
                            ),
                        )),
                    )),
                ),
            ),
            (
                "imapAddFlags".to_string(),
                Type::Func(
                    Box::new(Type::con("List").app(vec![int_ty.clone()])),
                    Box::new(Type::Func(
                        Box::new(list_text_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(imap_session_ty.clone()),
                            Box::new(
                                Type::con("Effect").app(vec![text_ty.clone(), Type::con("Unit")]),
                            ),
                        )),
                    )),
                ),
            ),
            (
                "imapRemoveFlags".to_string(),
                Type::Func(
                    Box::new(Type::con("List").app(vec![int_ty.clone()])),
                    Box::new(Type::Func(
                        Box::new(list_text_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(imap_session_ty.clone()),
                            Box::new(
                                Type::con("Effect").app(vec![text_ty.clone(), Type::con("Unit")]),
                            ),
                        )),
                    )),
                ),
            ),
            (
                "imapExpunge".to_string(),
                Type::Func(
                    Box::new(imap_session_ty.clone()),
                    Box::new(Type::con("Effect").app(vec![text_ty.clone(), Type::con("Unit")])),
                ),
            ),
            (
                "imapCopy".to_string(),
                Type::Func(
                    Box::new(Type::con("List").app(vec![int_ty.clone()])),
                    Box::new(Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(imap_session_ty.clone()),
                            Box::new(
                                Type::con("Effect").app(vec![text_ty.clone(), Type::con("Unit")]),
                            ),
                        )),
                    )),
                ),
            ),
            (
                "imapMove".to_string(),
                Type::Func(
                    Box::new(Type::con("List").app(vec![int_ty.clone()])),
                    Box::new(Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(imap_session_ty.clone()),
                            Box::new(
                                Type::con("Effect").app(vec![text_ty.clone(), Type::con("Unit")]),
                            ),
                        )),
                    )),
                ),
            ),
            (
                "imapListMailboxes".to_string(),
                Type::Func(
                    Box::new(imap_session_ty.clone()),
                    Box::new(Type::con("Effect").app(vec![
                        text_ty.clone(),
                        Type::con("List").app(vec![mailbox_info_ty.clone()]),
                    ])),
                ),
            ),
            (
                "imapCreateMailbox".to_string(),
                Type::Func(
                    Box::new(text_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(imap_session_ty.clone()),
                        Box::new(Type::con("Effect").app(vec![text_ty.clone(), Type::con("Unit")])),
                    )),
                ),
            ),
            (
                "imapDeleteMailbox".to_string(),
                Type::Func(
                    Box::new(text_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(imap_session_ty.clone()),
                        Box::new(Type::con("Effect").app(vec![text_ty.clone(), Type::con("Unit")])),
                    )),
                ),
            ),
            (
                "imapRenameMailbox".to_string(),
                Type::Func(
                    Box::new(text_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(imap_session_ty.clone()),
                            Box::new(
                                Type::con("Effect").app(vec![text_ty.clone(), Type::con("Unit")]),
                            ),
                        )),
                    )),
                ),
            ),
            (
                "imapAppend".to_string(),
                Type::Func(
                    Box::new(text_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(imap_session_ty.clone()),
                            Box::new(
                                Type::con("Effect").app(vec![text_ty.clone(), Type::con("Unit")]),
                            ),
                        )),
                    )),
                ),
            ),
            (
                "imapIdle".to_string(),
                Type::Func(
                    Box::new(int_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(imap_session_ty.clone()),
                        Box::new(
                            Type::con("Effect").app(vec![text_ty.clone(), Type::con("IdleResult")]),
                        ),
                    )),
                ),
            ),
            (
                "smtpSend".to_string(),
                Type::Func(
                    Box::new(smtp_config_ty),
                    Box::new(Type::con("Effect").app(vec![text_ty.clone(), Type::con("Unit")])),
                ),
            ),
            (
                "mimeParts".to_string(),
                Type::Func(
                    Box::new(text_ty.clone()),
                    Box::new(Type::con("List").app(vec![mime_part_ty])),
                ),
            ),
        ]
        .into_iter()
        .collect(),
    };
    env.insert(
        "email".to_string(),
        Scheme {
            vars: vec![imap_a],
            ty: email_record,
            origin: None,
        },
    );
    let goa_record = Type::Record {
        fields: vec![
            (
                "listMailAccounts".to_string(),
                Type::Func(
                    Box::new(Type::con("Unit")),
                    Box::new(Type::con("Effect").app(vec![
                        Type::con("GoaError"),
                        Type::con("List").app(vec![Type::con("GoaMailAccount")]),
                    ])),
                ),
            ),
            (
                "ensureCredentials".to_string(),
                Type::Func(
                    Box::new(text_ty.clone()),
                    Box::new(
                        Type::con("Effect").app(vec![Type::con("GoaError"), Type::con("Unit")]),
                    ),
                ),
            ),
            (
                "imapConfig".to_string(),
                Type::Func(
                    Box::new(text_ty.clone()),
                    Box::new(
                        Type::con("Effect")
                            .app(vec![Type::con("GoaError"), Type::con("GoaImapConfig")]),
                    ),
                ),
            ),
            (
                "smtpConfig".to_string(),
                Type::Func(
                    Box::new(text_ty.clone()),
                    Box::new(
                        Type::con("Effect")
                            .app(vec![Type::con("GoaError"), Type::con("GoaSmtpConfig")]),
                    ),
                ),
            ),
        ]
        .into_iter()
        .collect(),
    };
    env.insert("gnomeOnlineAccounts".to_string(), Scheme::mono(goa_record));
    let effect_text_unit = Type::con("Effect").app(vec![text_ty.clone(), Type::con("Unit")]);
    let effect_text_int = Type::con("Effect").app(vec![text_ty.clone(), int_ty.clone()]);
    let effect_text_bool = Type::con("Effect").app(vec![text_ty.clone(), Type::con("Bool")]);
    let effect_text_text = Type::con("Effect").app(vec![text_ty.clone(), text_ty.clone()]);
    let css_record_ty = Type::Record {
        fields: vec![].into_iter().collect(),
    };
    let reactive_signal_a = checker.fresh_var_id();
    let reactive_signal_b = checker.fresh_var_id();
    let reactive_signal_c = checker.fresh_var_id();
    let reactive_watch_r = checker.fresh_var_id();
    let reactive_batch_a = checker.fresh_var_id();
    let reactive_event_e = checker.fresh_var_id();
    let reactive_event_a = checker.fresh_var_id();
    let reactive_record = Type::Record {
        fields: vec![
            (
                "signal".to_string(),
                Type::Func(
                    Box::new(Type::Var(reactive_signal_a)),
                    Box::new(Type::con("Signal").app(vec![Type::Var(reactive_signal_a)])),
                ),
            ),
            (
                "get".to_string(),
                Type::Func(
                    Box::new(Type::con("Signal").app(vec![Type::Var(reactive_signal_a)])),
                    Box::new(Type::Var(reactive_signal_a)),
                ),
            ),
            (
                "peek".to_string(),
                Type::Func(
                    Box::new(Type::con("Signal").app(vec![Type::Var(reactive_signal_a)])),
                    Box::new(Type::Var(reactive_signal_a)),
                ),
            ),
            (
                "set".to_string(),
                Type::Func(
                    Box::new(Type::con("Signal").app(vec![Type::Var(reactive_signal_a)])),
                    Box::new(Type::Func(
                        Box::new(Type::Var(reactive_signal_a)),
                        Box::new(Type::con("Unit")),
                    )),
                ),
            ),
            (
                "update".to_string(),
                Type::Func(
                    Box::new(Type::con("Signal").app(vec![Type::Var(reactive_signal_a)])),
                    Box::new(Type::Func(
                        Box::new(Type::Func(
                            Box::new(Type::Var(reactive_signal_a)),
                            Box::new(Type::Var(reactive_signal_a)),
                        )),
                        Box::new(Type::con("Unit")),
                    )),
                ),
            ),
            (
                "derive".to_string(),
                Type::Func(
                    Box::new(Type::con("Signal").app(vec![Type::Var(reactive_signal_a)])),
                    Box::new(Type::Func(
                        Box::new(Type::Func(
                            Box::new(Type::Var(reactive_signal_a)),
                            Box::new(Type::Var(reactive_signal_b)),
                        )),
                        Box::new(Type::con("Signal").app(vec![Type::Var(reactive_signal_b)])),
                    )),
                ),
            ),
            (
                "combineAll".to_string(),
                Type::Func(
                    Box::new(Type::Var(reactive_signal_a)),
                    Box::new(Type::Func(
                        Box::new(Type::Func(
                            Box::new(Type::Var(reactive_signal_b)),
                            Box::new(Type::Var(reactive_signal_c)),
                        )),
                        Box::new(Type::con("Signal").app(vec![Type::Var(reactive_signal_c)])),
                    )),
                ),
            ),
            (
                "watch".to_string(),
                Type::Func(
                    Box::new(Type::con("Signal").app(vec![Type::Var(reactive_signal_a)])),
                    Box::new(Type::Func(
                        Box::new(Type::Func(
                            Box::new(Type::Var(reactive_signal_a)),
                            Box::new(Type::Var(reactive_watch_r)),
                        )),
                        Box::new(Type::con("Disposable")),
                    )),
                ),
            ),
            (
                "batch".to_string(),
                Type::Func(
                    Box::new(Type::Func(
                        Box::new(Type::con("Unit")),
                        Box::new(Type::Var(reactive_batch_a)),
                    )),
                    Box::new(Type::Var(reactive_batch_a)),
                ),
            ),
            (
                "eventFrom".to_string(),
                Type::Func(
                    Box::new(Type::con("Effect").app(vec![
                        Type::Var(reactive_event_e),
                        Type::Var(reactive_event_a),
                    ])),
                    Box::new(Type::con("EventHandle").app(vec![
                        Type::Var(reactive_event_e),
                        Type::Var(reactive_event_a),
                    ])),
                ),
            ),
        ]
        .into_iter()
        .collect(),
    };
    env.insert(
        "reactive".to_string(),
        Scheme {
            vars: vec![
                reactive_signal_a,
                reactive_signal_b,
                reactive_signal_c,
                reactive_watch_r,
                reactive_batch_a,
                reactive_event_e,
                reactive_event_a,
            ],
            ty: reactive_record,
            origin: None,
        },
    );
    let gtk4_model = checker.fresh_var_id();
    let gtk4_value = checker.fresh_var_id();
    let gtk4_signal_model = checker.fresh_var_id();
    let gtk4_signal_value = checker.fresh_var_id();
    let gtk4_attr_value = checker.fresh_var_id();
    let gtk4_record = Type::Record {
        fields: vec![
            (
                "reactiveInit".to_string(),
                Type::Func(
                    Box::new(Type::Var(gtk4_model)),
                    Box::new(effect_text_unit.clone()),
                ),
            ),
            (
                "reactiveCommit".to_string(),
                Type::Func(
                    Box::new(Type::Var(gtk4_model)),
                    Box::new(Type::Func(
                        Box::new(Type::Var(gtk4_model)),
                        Box::new(effect_text_unit.clone()),
                    )),
                ),
            ),
            (
                "memo".to_string(),
                Type::Func(
                    Box::new(text_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(Type::Func(
                            Box::new(Type::Var(gtk4_model)),
                            Box::new(Type::Var(gtk4_value)),
                        )),
                        Box::new(Type::Func(
                            Box::new(Type::Var(gtk4_model)),
                            Box::new(Type::Var(gtk4_value)),
                        )),
                    )),
                ),
            ),
            (
                "derive".to_string(),
                Type::Func(
                    Box::new(Type::Func(
                        Box::new(Type::Var(gtk4_signal_model)),
                        Box::new(Type::Var(gtk4_signal_value)),
                    )),
                    Box::new(Type::Func(
                        Box::new(Type::Var(gtk4_signal_model)),
                        Box::new(Type::Var(gtk4_signal_value)),
                    )),
                ),
            ),
            (
                "serializeAttr".to_string(),
                Type::Func(
                    Box::new(Type::Var(gtk4_attr_value)),
                    Box::new(text_ty.clone()),
                ),
            ),
            (
                "captureBinding".to_string(),
                Type::Func(
                    Box::new(Type::Var(gtk4_attr_value)),
                    Box::new(int_ty.clone()),
                ),
            ),
            (
                "init".to_string(),
                Type::Func(
                    Box::new(Type::con("Unit")),
                    Box::new(effect_text_unit.clone()),
                ),
            ),
            (
                "appNew".to_string(),
                Type::Func(Box::new(text_ty.clone()), Box::new(effect_text_int.clone())),
            ),
            (
                "windowNew".to_string(),
                Type::Func(
                    Box::new(int_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(int_ty.clone()),
                            Box::new(Type::Func(
                                Box::new(int_ty.clone()),
                                Box::new(effect_text_int.clone()),
                            )),
                        )),
                    )),
                ),
            ),
            (
                "mountAppWindow".to_string(),
                Type::Func(
                    Box::new(int_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(Type::con("List").app(vec![Type::con("GtkNode")])),
                        Box::new(effect_text_int.clone()),
                    )),
                ),
            ),
            (
                "displayHeight".to_string(),
                Type::Func(
                    Box::new(Type::con("Unit")),
                    Box::new(effect_text_int.clone()),
                ),
            ),
            (
                "windowSetTitle".to_string(),
                Type::Func(
                    Box::new(int_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(effect_text_unit.clone()),
                    )),
                ),
            ),
            (
                "windowSetChild".to_string(),
                Type::Func(
                    Box::new(int_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(int_ty.clone()),
                        Box::new(effect_text_unit.clone()),
                    )),
                ),
            ),
            (
                "windowPresent".to_string(),
                Type::Func(Box::new(int_ty.clone()), Box::new(effect_text_unit.clone())),
            ),
            (
                "windowSetHideOnClose".to_string(),
                Type::Func(
                    Box::new(int_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(Type::con("Bool")),
                        Box::new(effect_text_unit.clone()),
                    )),
                ),
            ),
            (
                "widgetGetBoolProperty".to_string(),
                Type::Func(
                    Box::new(int_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(effect_text_bool.clone()),
                    )),
                ),
            ),
            (
                "widgetGetCalendarDate".to_string(),
                Type::Func(Box::new(int_ty.clone()), Box::new(effect_text_text.clone())),
            ),
            (
                "appRun".to_string(),
                Type::Func(Box::new(int_ty.clone()), Box::new(effect_text_unit.clone())),
            ),
            (
                "widgetShow".to_string(),
                Type::Func(Box::new(int_ty.clone()), Box::new(effect_text_unit.clone())),
            ),
            (
                "widgetHide".to_string(),
                Type::Func(Box::new(int_ty.clone()), Box::new(effect_text_unit.clone())),
            ),
            (
                "boxNew".to_string(),
                Type::Func(
                    Box::new(int_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(int_ty.clone()),
                        Box::new(effect_text_int.clone()),
                    )),
                ),
            ),
            (
                "boxAppend".to_string(),
                Type::Func(
                    Box::new(int_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(int_ty.clone()),
                        Box::new(effect_text_unit.clone()),
                    )),
                ),
            ),
            (
                "buttonNew".to_string(),
                Type::Func(Box::new(text_ty.clone()), Box::new(effect_text_int.clone())),
            ),
            (
                "buttonSetLabel".to_string(),
                Type::Func(
                    Box::new(int_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(effect_text_unit.clone()),
                    )),
                ),
            ),
            (
                "labelNew".to_string(),
                Type::Func(Box::new(text_ty.clone()), Box::new(effect_text_int.clone())),
            ),
            (
                "labelSetText".to_string(),
                Type::Func(
                    Box::new(int_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(effect_text_unit.clone()),
                    )),
                ),
            ),
            (
                "entryNew".to_string(),
                Type::Func(
                    Box::new(Type::con("Unit")),
                    Box::new(effect_text_int.clone()),
                ),
            ),
            (
                "entrySetText".to_string(),
                Type::Func(
                    Box::new(int_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(effect_text_unit.clone()),
                    )),
                ),
            ),
            (
                "entryText".to_string(),
                Type::Func(Box::new(int_ty.clone()), Box::new(effect_text_text.clone())),
            ),
            (
                "scrollAreaNew".to_string(),
                Type::Func(
                    Box::new(Type::con("Unit")),
                    Box::new(effect_text_int.clone()),
                ),
            ),
            (
                "scrollAreaSetChild".to_string(),
                Type::Func(
                    Box::new(int_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(int_ty.clone()),
                        Box::new(effect_text_unit.clone()),
                    )),
                ),
            ),
            (
                "drawAreaNew".to_string(),
                Type::Func(
                    Box::new(int_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(int_ty.clone()),
                        Box::new(effect_text_int.clone()),
                    )),
                ),
            ),
            (
                "drawAreaSetContentSize".to_string(),
                Type::Func(
                    Box::new(int_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(int_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(int_ty.clone()),
                            Box::new(effect_text_unit.clone()),
                        )),
                    )),
                ),
            ),
            (
                "drawAreaQueueDraw".to_string(),
                Type::Func(Box::new(int_ty.clone()), Box::new(effect_text_unit.clone())),
            ),
            (
                "widgetSetCss".to_string(),
                Type::Func(
                    Box::new(int_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(css_record_ty.clone()),
                        Box::new(effect_text_unit.clone()),
                    )),
                ),
            ),
            (
                "appSetCss".to_string(),
                Type::Func(
                    Box::new(int_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(css_record_ty),
                        Box::new(effect_text_unit.clone()),
                    )),
                ),
            ),
            (
                "menuModelNew".to_string(),
                Type::Func(
                    Box::new(Type::con("Unit")),
                    Box::new(effect_text_int.clone()),
                ),
            ),
            (
                "menuModelAppendItem".to_string(),
                Type::Func(
                    Box::new(int_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(text_ty.clone()),
                            Box::new(effect_text_unit.clone()),
                        )),
                    )),
                ),
            ),
            (
                "menuButtonSetMenuModel".to_string(),
                Type::Func(
                    Box::new(int_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(int_ty.clone()),
                        Box::new(effect_text_unit.clone()),
                    )),
                ),
            ),
            (
                "imageNewFromFile".to_string(),
                Type::Func(Box::new(text_ty.clone()), Box::new(effect_text_int.clone())),
            ),
            (
                "imageSetFile".to_string(),
                Type::Func(
                    Box::new(int_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(effect_text_unit.clone()),
                    )),
                ),
            ),
            (
                "imageNewFromResource".to_string(),
                Type::Func(Box::new(text_ty.clone()), Box::new(effect_text_int.clone())),
            ),
            (
                "imageSetResource".to_string(),
                Type::Func(
                    Box::new(int_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(effect_text_unit.clone()),
                    )),
                ),
            ),
            (
                "gestureClickNew".to_string(),
                Type::Func(Box::new(int_ty.clone()), Box::new(effect_text_int.clone())),
            ),
            (
                "gestureClickLastButton".to_string(),
                Type::Func(Box::new(int_ty.clone()), Box::new(effect_text_int.clone())),
            ),
            (
                "widgetAddController".to_string(),
                Type::Func(
                    Box::new(int_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(int_ty.clone()),
                        Box::new(effect_text_unit.clone()),
                    )),
                ),
            ),
            (
                "actionNew".to_string(),
                Type::Func(Box::new(text_ty.clone()), Box::new(effect_text_int.clone())),
            ),
            (
                "actionSetEnabled".to_string(),
                Type::Func(
                    Box::new(int_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(Type::con("Bool")),
                        Box::new(effect_text_unit.clone()),
                    )),
                ),
            ),
            (
                "appAddAction".to_string(),
                Type::Func(
                    Box::new(int_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(int_ty.clone()),
                        Box::new(effect_text_unit.clone()),
                    )),
                ),
            ),
            (
                "buildFromNode".to_string(),
                Type::Func(
                    Box::new(Type::con("GtkNode")),
                    Box::new(effect_text_int.clone()),
                ),
            ),
            (
                "reconcileNode".to_string(),
                Type::Func(
                    Box::new(int_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(Type::con("GtkNode")),
                        Box::new(Type::Func(
                            Box::new(Type::con("GtkNode")),
                            Box::new(effect_text_unit.clone()),
                        )),
                    )),
                ),
            ),
            (
                "signalPoll".to_string(),
                Type::Func(
                    Box::new(Type::con("Unit")),
                    Box::new(Type::con("Effect").app(vec![
                        text_ty.clone(),
                        Type::con("Option").app(vec![Type::con("GtkSignalEvent")]),
                    ])),
                ),
            ),
            (
                "signalStream".to_string(),
                Type::Func(
                    Box::new(Type::con("Unit")),
                    Box::new(Type::con("Effect").app(vec![
                        text_ty.clone(),
                        Type::con("Recv").app(vec![Type::con("GtkSignalEvent")]),
                    ])),
                ),
            ),
            (
                "signalEmit".to_string(),
                Type::Func(
                    Box::new(int_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(text_ty.clone()),
                            Box::new(Type::Func(
                                Box::new(text_ty.clone()),
                                Box::new(effect_text_unit.clone()),
                            )),
                        )),
                    )),
                ),
            ),
            (
                "setInterval".to_string(),
                Type::Func(Box::new(int_ty.clone()), Box::new(effect_text_unit.clone())),
            ),
            (
                "osOpenUri".to_string(),
                Type::Func(
                    Box::new(text_ty.clone()),
                    Box::new(effect_text_unit.clone()),
                ),
            ),
            (
                "widgetSetCss".to_string(),
                Type::Func(
                    Box::new(int_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(Type::Record {
                            fields: vec![].into_iter().collect(),
                        }),
                        Box::new(effect_text_unit.clone()),
                    )),
                ),
            ),
            (
                "appSetCss".to_string(),
                Type::Func(
                    Box::new(int_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(Type::Record {
                            fields: vec![].into_iter().collect(),
                        }),
                        Box::new(effect_text_unit.clone()),
                    )),
                ),
            ),
            (
                "widgetById".to_string(),
                Type::Func(Box::new(text_ty.clone()), Box::new(effect_text_int.clone())),
            ),
            (
                "widgetSetBoolProperty".to_string(),
                Type::Func(
                    Box::new(int_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(Type::con("Bool")),
                            Box::new(effect_text_unit.clone()),
                        )),
                    )),
                ),
            ),
            (
                "widgetSetCalendarDate".to_string(),
                Type::Func(
                    Box::new(int_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(effect_text_unit.clone()),
                    )),
                ),
            ),
            (
                "signalBindBoolProperty".to_string(),
                Type::Func(
                    Box::new(text_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(int_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(text_ty.clone()),
                            Box::new(Type::Func(
                                Box::new(Type::con("Bool")),
                                Box::new(effect_text_unit.clone()),
                            )),
                        )),
                    )),
                ),
            ),
            (
                "signalBindCssClass".to_string(),
                Type::Func(
                    Box::new(text_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(int_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(text_ty.clone()),
                            Box::new(Type::Func(
                                Box::new(Type::con("Bool")),
                                Box::new(effect_text_unit.clone()),
                            )),
                        )),
                    )),
                ),
            ),
            (
                "signalBindToggleBoolProperty".to_string(),
                Type::Func(
                    Box::new(text_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(int_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(text_ty.clone()),
                            Box::new(effect_text_unit.clone()),
                        )),
                    )),
                ),
            ),
            (
                "signalToggleCssClass".to_string(),
                Type::Func(
                    Box::new(text_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(int_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(text_ty.clone()),
                            Box::new(effect_text_unit),
                        )),
                    )),
                ),
            ),
        ]
        .into_iter()
        .collect(),
    };
    env.insert(
        "gtk4".to_string(),
        Scheme {
            vars: vec![gtk4_model, gtk4_value],
            ty: gtk4_record,
            origin: None,
        },
    );

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
    };
    env.insert("logger".to_string(), Scheme::mono(logger_record));

    let db_row = checker.fresh_var_id();
    let db_error_ty = Type::con("DbError");
    let db_config_ty = Type::con("DbConfig");
    let db_connection_ty = Type::con("DbConnection");
    let relation_ty = Type::con("Relation").app(vec![Type::Var(db_row)]);
    let list_relation_ty = Type::con("List").app(vec![relation_ty.clone()]);
    let list_row_ty = Type::con("List").app(vec![Type::Var(db_row)]);
    let list_text_ty = Type::con("List").app(vec![text_ty.clone()]);
    let db_effect_relation_ty =
        Type::con("Effect").app(vec![db_error_ty.clone(), relation_ty.clone()]);
    let db_effect_rows_ty = Type::con("Effect").app(vec![db_error_ty.clone(), list_row_ty.clone()]);
    let db_effect_int_ty = Type::con("Effect").app(vec![db_error_ty.clone(), Type::con("Int")]);
    let db_effect_bool_ty = Type::con("Effect").app(vec![db_error_ty.clone(), Type::con("Bool")]);
    let db_effect_unit_ty = Type::con("Effect").app(vec![db_error_ty.clone(), Type::con("Unit")]);
    let sqlite_tuning_ty = Type::Record {
        fields: vec![
            ("wal".to_string(), Type::con("Bool")),
            ("busyTimeoutMs".to_string(), Type::con("Int")),
        ]
        .into_iter()
        .collect(),
    };
    let database_record = Type::Record {
        fields: vec![
            (
                "configure".to_string(),
                Type::Func(Box::new(db_config_ty), Box::new(db_effect_unit_ty.clone())),
            ),
            (
                "connect".to_string(),
                Type::Func(
                    Box::new(Type::con("DbConfig")),
                    Box::new(
                        Type::con("Effect")
                            .app(vec![db_error_ty.clone(), db_connection_ty.clone()]),
                    ),
                ),
            ),
            (
                "close".to_string(),
                Type::Func(
                    Box::new(db_connection_ty.clone()),
                    Box::new(db_effect_unit_ty.clone()),
                ),
            ),
            (
                "loadRelation".to_string(),
                Type::Func(
                    Box::new(relation_ty.clone()),
                    Box::new(db_effect_rows_ty.clone()),
                ),
            ),
            (
                "loadRelationOn".to_string(),
                Type::Func(
                    Box::new(db_connection_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(relation_ty.clone()),
                        Box::new(db_effect_rows_ty.clone()),
                    )),
                ),
            ),
            (
                "insert".to_string(),
                Type::Func(
                    Box::new(relation_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(Type::Var(db_row)),
                        Box::new(db_effect_relation_ty.clone()),
                    )),
                ),
            ),
            (
                "insertOn".to_string(),
                Type::Func(
                    Box::new(db_connection_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(relation_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(Type::Var(db_row)),
                            Box::new(db_effect_relation_ty.clone()),
                        )),
                    )),
                ),
            ),
            (
                "count".to_string(),
                Type::Func(
                    Box::new(Type::con("Query").app(vec![Type::Var(db_row)])),
                    Box::new(db_effect_int_ty.clone()),
                ),
            ),
            (
                "countOn".to_string(),
                Type::Func(
                    Box::new(db_connection_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(Type::con("Query").app(vec![Type::Var(db_row)])),
                        Box::new(db_effect_int_ty.clone()),
                    )),
                ),
            ),
            (
                "exists".to_string(),
                Type::Func(
                    Box::new(Type::con("Query").app(vec![Type::Var(db_row)])),
                    Box::new(db_effect_bool_ty.clone()),
                ),
            ),
            (
                "existsOn".to_string(),
                Type::Func(
                    Box::new(db_connection_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(Type::con("Query").app(vec![Type::Var(db_row)])),
                        Box::new(db_effect_bool_ty.clone()),
                    )),
                ),
            ),
            (
                "runMigrations".to_string(),
                Type::Func(
                    Box::new(list_relation_ty),
                    Box::new(db_effect_unit_ty.clone()),
                ),
            ),
            (
                "runMigrationsOn".to_string(),
                Type::Func(
                    Box::new(db_connection_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(Type::con("List").app(vec![relation_ty.clone()])),
                        Box::new(db_effect_unit_ty.clone()),
                    )),
                ),
            ),
            (
                "configureSqlite".to_string(),
                Type::Func(
                    Box::new(sqlite_tuning_ty),
                    Box::new(db_effect_unit_ty.clone()),
                ),
            ),
            (
                "configureSqliteOn".to_string(),
                Type::Func(
                    Box::new(db_connection_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(Type::Record {
                            fields: vec![
                                ("wal".to_string(), Type::con("Bool")),
                                ("busyTimeoutMs".to_string(), Type::con("Int")),
                            ]
                            .into_iter()
                            .collect(),
                        }),
                        Box::new(db_effect_unit_ty.clone()),
                    )),
                ),
            ),
            ("beginTx".to_string(), db_effect_unit_ty.clone()),
            ("commitTx".to_string(), db_effect_unit_ty.clone()),
            ("rollbackTx".to_string(), db_effect_unit_ty.clone()),
            (
                "beginTxOn".to_string(),
                Type::Func(
                    Box::new(db_connection_ty.clone()),
                    Box::new(db_effect_unit_ty.clone()),
                ),
            ),
            (
                "commitTxOn".to_string(),
                Type::Func(
                    Box::new(db_connection_ty.clone()),
                    Box::new(db_effect_unit_ty.clone()),
                ),
            ),
            (
                "rollbackTxOn".to_string(),
                Type::Func(
                    Box::new(db_connection_ty.clone()),
                    Box::new(db_effect_unit_ty.clone()),
                ),
            ),
            (
                "savepoint".to_string(),
                Type::Func(
                    Box::new(text_ty.clone()),
                    Box::new(db_effect_unit_ty.clone()),
                ),
            ),
            (
                "savepointOn".to_string(),
                Type::Func(
                    Box::new(db_connection_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(db_effect_unit_ty.clone()),
                    )),
                ),
            ),
            (
                "releaseSavepoint".to_string(),
                Type::Func(
                    Box::new(text_ty.clone()),
                    Box::new(db_effect_unit_ty.clone()),
                ),
            ),
            (
                "releaseSavepointOn".to_string(),
                Type::Func(
                    Box::new(db_connection_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(db_effect_unit_ty.clone()),
                    )),
                ),
            ),
            (
                "rollbackToSavepoint".to_string(),
                Type::Func(
                    Box::new(text_ty.clone()),
                    Box::new(db_effect_unit_ty.clone()),
                ),
            ),
            (
                "rollbackToSavepointOn".to_string(),
                Type::Func(
                    Box::new(db_connection_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(db_effect_unit_ty.clone()),
                    )),
                ),
            ),
            (
                "runMigrationSql".to_string(),
                Type::Func(Box::new(list_text_ty), Box::new(db_effect_unit_ty.clone())),
            ),
            (
                "runMigrationSqlOn".to_string(),
                Type::Func(
                    Box::new(db_connection_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(Type::con("List").app(vec![text_ty.clone()])),
                        Box::new(db_effect_unit_ty.clone()),
                    )),
                ),
            ),
        ]
        .into_iter()
        .collect(),
    };
    env.insert("database".to_string(), Scheme::mono(database_record));

    let encrypted_blob_ty = Type::Record {
        fields: vec![
            ("keyId".to_string(), text_ty.clone()),
            ("algorithm".to_string(), text_ty.clone()),
            ("ciphertext".to_string(), Type::con("Bytes")),
        ]
        .into_iter()
        .collect(),
    };
    let option_encrypted_blob_ty = Type::con("Option").app(vec![encrypted_blob_ty.clone()]);
    let secrets_record = Type::Record {
        fields: vec![
            (
                "put".to_string(),
                Type::Func(
                    Box::new(text_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(encrypted_blob_ty.clone()),
                        Box::new(
                            Type::con("Effect").app(vec![Type::con("Text"), Type::con("Unit")]),
                        ),
                    )),
                ),
            ),
            (
                "get".to_string(),
                Type::Func(
                    Box::new(text_ty.clone()),
                    Box::new(
                        Type::con("Effect").app(vec![Type::con("Text"), option_encrypted_blob_ty]),
                    ),
                ),
            ),
            (
                "delete".to_string(),
                Type::Func(
                    Box::new(text_ty.clone()),
                    Box::new(Type::con("Effect").app(vec![Type::con("Text"), Type::con("Unit")])),
                ),
            ),
            (
                "makeBlob".to_string(),
                Type::Func(
                    Box::new(text_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(Type::con("Bytes")),
                            Box::new(encrypted_blob_ty.clone()),
                        )),
                    )),
                ),
            ),
            (
                "validateBlob".to_string(),
                Type::Func(Box::new(encrypted_blob_ty), Box::new(Type::con("Bool"))),
            ),
        ]
        .into_iter()
        .collect(),
    };
    env.insert("secrets".to_string(), Scheme::mono(secrets_record));
}
