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
            capabilities: Default::default(),
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
        ]
        .into_iter()
        .collect(),
    };
    env.insert(
        "system".to_string(),
        Scheme {
            vars: vec![system_env_decode_a, system_env_decode_arg],
            ty: system_record,
            capabilities: Default::default(),
            origin: None,
        },
    );
    let imap_a = checker.fresh_var_id();
    let mime_part_ty = Type::Record {
        fields: vec![
            ("contentType".to_string(), text_ty.clone()),
            ("body".to_string(), text_ty.clone()),
        ]
        .into_iter()
        .collect(),
    };
    let smtp_config_ty = Type::Record {
        fields: vec![
            ("host".to_string(), text_ty.clone()),
            ("user".to_string(), text_ty.clone()),
            ("password".to_string(), text_ty.clone()),
            ("from".to_string(), text_ty.clone()),
            ("to".to_string(), text_ty.clone()),
            ("subject".to_string(), text_ty.clone()),
            ("body".to_string(), text_ty.clone()),
        ]
        .into_iter()
        .collect(),
    };
    let email_record = Type::Record {
        fields: vec![
            (
                "imap".to_string(),
                Type::Func(
                    Box::new(Type::Record {
                        fields: vec![
                            ("host".to_string(), text_ty.clone()),
                            ("user".to_string(), text_ty.clone()),
                            ("password".to_string(), text_ty.clone()),
                            (
                                "mailbox".to_string(),
                                Type::con("Option").app(vec![text_ty.clone()]),
                            ),
                            (
                                "filter".to_string(),
                                Type::con("Option").app(vec![text_ty.clone()]),
                            ),
                            (
                                "limit".to_string(),
                                Type::con("Option").app(vec![int_ty.clone()]),
                            ),
                            (
                                "port".to_string(),
                                Type::con("Option").app(vec![int_ty.clone()]),
                            ),
                        ]
                        .into_iter()
                        .collect(),
                    }),
                    Box::new(Type::con("Source").app(vec![
                        Type::con("Imap"),
                        Type::con("List").app(vec![Type::Var(imap_a)]),
                    ])),
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
            capabilities: Default::default(),
            origin: None,
        },
    );
    let effect_text_unit = Type::con("Effect").app(vec![text_ty.clone(), Type::con("Unit")]);
    let effect_text_int = Type::con("Effect").app(vec![text_ty.clone(), int_ty.clone()]);
    let effect_text_text = Type::con("Effect").app(vec![text_ty.clone(), text_ty.clone()]);
    let css_record_ty = Type::Record {
        fields: vec![].into_iter().collect(),
    };
    let effect_text_list_text = Type::con("Effect").app(vec![
        text_ty.clone(),
        Type::con("List").app(vec![text_ty.clone()]),
    ]);
    let gtk4_model = checker.fresh_var_id();
    let gtk4_value = checker.fresh_var_id();
    let gtk4_signal_model = checker.fresh_var_id();
    let gtk4_signal_value = checker.fresh_var_id();
    let gtk4_attr_value = checker.fresh_var_id();
    let gtk4_each_source = checker.fresh_var_id();
    let gtk4_each_item = checker.fresh_var_id();
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
                "computed".to_string(),
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
                "signal".to_string(),
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
                "eachItems".to_string(),
                Type::Func(
                    Box::new(Type::Var(gtk4_each_source)),
                    Box::new(Type::Func(
                        Box::new(Type::Func(
                            Box::new(Type::Var(gtk4_each_item)),
                            Box::new(Type::con("GtkNode")),
                        )),
                        Box::new(Type::con("List").app(vec![Type::con("GtkNode")])),
                    )),
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
                "dragSourceNew".to_string(),
                Type::Func(Box::new(int_ty.clone()), Box::new(effect_text_int.clone())),
            ),
            (
                "dragSourceSetText".to_string(),
                Type::Func(
                    Box::new(int_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(effect_text_unit.clone()),
                    )),
                ),
            ),
            (
                "dropTargetNew".to_string(),
                Type::Func(Box::new(int_ty.clone()), Box::new(effect_text_int.clone())),
            ),
            (
                "dropTargetLastText".to_string(),
                Type::Func(Box::new(int_ty.clone()), Box::new(effect_text_text.clone())),
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
                "menuButtonNew".to_string(),
                Type::Func(Box::new(text_ty.clone()), Box::new(effect_text_int.clone())),
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
                "dialogNew".to_string(),
                Type::Func(Box::new(int_ty.clone()), Box::new(effect_text_int.clone())),
            ),
            (
                "dialogSetTitle".to_string(),
                Type::Func(
                    Box::new(int_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(effect_text_unit.clone()),
                    )),
                ),
            ),
            (
                "dialogSetChild".to_string(),
                Type::Func(
                    Box::new(int_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(int_ty.clone()),
                        Box::new(effect_text_unit.clone()),
                    )),
                ),
            ),
            (
                "dialogPresent".to_string(),
                Type::Func(Box::new(int_ty.clone()), Box::new(effect_text_unit.clone())),
            ),
            (
                "dialogClose".to_string(),
                Type::Func(Box::new(int_ty.clone()), Box::new(effect_text_unit.clone())),
            ),
            (
                "adwDialogPresent".to_string(),
                Type::Func(
                    Box::new(int_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(int_ty.clone()),
                        Box::new(effect_text_unit.clone()),
                    )),
                ),
            ),
            (
                "fileDialogNew".to_string(),
                Type::Func(
                    Box::new(Type::con("Unit")),
                    Box::new(effect_text_int.clone()),
                ),
            ),
            (
                "fileDialogSelectFile".to_string(),
                Type::Func(Box::new(int_ty.clone()), Box::new(effect_text_text.clone())),
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
                "listStoreNew".to_string(),
                Type::Func(
                    Box::new(Type::con("Unit")),
                    Box::new(effect_text_int.clone()),
                ),
            ),
            (
                "listStoreAppendText".to_string(),
                Type::Func(
                    Box::new(int_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(effect_text_unit.clone()),
                    )),
                ),
            ),
            (
                "listStoreItems".to_string(),
                Type::Func(
                    Box::new(int_ty.clone()),
                    Box::new(effect_text_list_text.clone()),
                ),
            ),
            (
                "listViewNew".to_string(),
                Type::Func(
                    Box::new(Type::con("Unit")),
                    Box::new(effect_text_int.clone()),
                ),
            ),
            (
                "listViewSetModel".to_string(),
                Type::Func(
                    Box::new(int_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(int_ty.clone()),
                        Box::new(effect_text_unit.clone()),
                    )),
                ),
            ),
            (
                "treeViewNew".to_string(),
                Type::Func(
                    Box::new(Type::con("Unit")),
                    Box::new(effect_text_int.clone()),
                ),
            ),
            (
                "treeViewSetModel".to_string(),
                Type::Func(
                    Box::new(int_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(int_ty.clone()),
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
                "clipboardDefault".to_string(),
                Type::Func(
                    Box::new(Type::con("Unit")),
                    Box::new(effect_text_int.clone()),
                ),
            ),
            (
                "clipboardSetText".to_string(),
                Type::Func(
                    Box::new(int_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(effect_text_unit.clone()),
                    )),
                ),
            ),
            (
                "clipboardText".to_string(),
                Type::Func(Box::new(int_ty.clone()), Box::new(effect_text_text.clone())),
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
                "shortcutNew".to_string(),
                Type::Func(
                    Box::new(text_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(effect_text_int.clone()),
                    )),
                ),
            ),
            (
                "widgetAddShortcut".to_string(),
                Type::Func(
                    Box::new(int_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(int_ty.clone()),
                        Box::new(effect_text_unit.clone()),
                    )),
                ),
            ),
            (
                "notificationNew".to_string(),
                Type::Func(
                    Box::new(text_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(effect_text_int.clone()),
                    )),
                ),
            ),
            (
                "notificationSetBody".to_string(),
                Type::Func(
                    Box::new(int_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(effect_text_unit.clone()),
                    )),
                ),
            ),
            (
                "appSendNotification".to_string(),
                Type::Func(
                    Box::new(int_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(int_ty.clone()),
                            Box::new(effect_text_unit.clone()),
                        )),
                    )),
                ),
            ),
            (
                "appWithdrawNotification".to_string(),
                Type::Func(
                    Box::new(int_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(effect_text_unit.clone()),
                    )),
                ),
            ),
            (
                "layoutManagerNew".to_string(),
                Type::Func(Box::new(text_ty.clone()), Box::new(effect_text_int.clone())),
            ),
            (
                "widgetSetLayoutManager".to_string(),
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
                "dbusServerStart".to_string(),
                Type::Func(
                    Box::new(Type::con("Unit")),
                    Box::new(effect_text_unit.clone()),
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
                    Box::new(int_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(effect_text_unit.clone()),
                    )),
                ),
            ),
            (
                "osShowInFileManager".to_string(),
                Type::Func(
                    Box::new(text_ty.clone()),
                    Box::new(effect_text_unit.clone()),
                ),
            ),
            (
                "osSetBadgeCount".to_string(),
                Type::Func(
                    Box::new(int_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(int_ty.clone()),
                        Box::new(effect_text_unit.clone()),
                    )),
                ),
            ),
            (
                "osThemePreference".to_string(),
                Type::Func(Box::new(Type::con("Unit")), Box::new(effect_text_text)),
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
            capabilities: Default::default(),
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
    let table_ty = Type::con("Table").app(vec![Type::Var(db_row)]);
    let pred_ty = Type::con("Pred").app(vec![Type::Var(db_row)]);
    let patch_ty = Type::con("Patch").app(vec![Type::Var(db_row)]);
    let delta_ty = Type::con("Delta").app(vec![Type::Var(db_row)]);
    let list_table_ty = Type::con("List").app(vec![table_ty.clone()]);
    let list_row_ty = Type::con("List").app(vec![Type::Var(db_row)]);
    let list_column_ty = Type::con("List").app(vec![Type::con("Column")]);
    let list_text_ty = Type::con("List").app(vec![text_ty.clone()]);
    let db_effect_table_ty = Type::con("Effect").app(vec![db_error_ty.clone(), table_ty.clone()]);
    let db_effect_rows_ty = Type::con("Effect").app(vec![db_error_ty.clone(), list_row_ty.clone()]);
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
                "load".to_string(),
                Type::Func(
                    Box::new(table_ty.clone()),
                    Box::new(db_effect_rows_ty.clone()),
                ),
            ),
            (
                "loadOn".to_string(),
                Type::Func(
                    Box::new(db_connection_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(table_ty.clone()),
                        Box::new(db_effect_rows_ty.clone()),
                    )),
                ),
            ),
            (
                "runQuery".to_string(),
                Type::Func(
                    Box::new(Type::con("Query").app(vec![Type::Var(db_row)])),
                    Box::new(db_effect_rows_ty.clone()),
                ),
            ),
            (
                "applyDelta".to_string(),
                Type::Func(
                    Box::new(table_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(delta_ty.clone()),
                        Box::new(db_effect_table_ty.clone()),
                    )),
                ),
            ),
            (
                "applyDeltaOn".to_string(),
                Type::Func(
                    Box::new(db_connection_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(table_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(delta_ty.clone()),
                            Box::new(db_effect_table_ty.clone()),
                        )),
                    )),
                ),
            ),
            (
                "runMigrations".to_string(),
                Type::Func(Box::new(list_table_ty), Box::new(db_effect_unit_ty.clone())),
            ),
            (
                "runMigrationsOn".to_string(),
                Type::Func(
                    Box::new(db_connection_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(Type::con("List").app(vec![table_ty.clone()])),
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
                Type::Func(Box::new(pred_ty.clone()), Box::new(delta_ty.clone())),
            ),
            (
                "ups".to_string(),
                Type::Func(
                    Box::new(pred_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(Type::Var(db_row)),
                        Box::new(Type::Func(
                            Box::new(Type::Func(
                                Box::new(Type::Var(db_row)),
                                Box::new(Type::Var(db_row)),
                            )),
                            Box::new(delta_ty.clone()),
                        )),
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
