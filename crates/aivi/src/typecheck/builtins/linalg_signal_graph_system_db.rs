use super::TypeChecker;
use crate::typecheck::types::{Scheme, Type, TypeEnv};

pub(super) fn register(checker: &mut TypeChecker, env: &mut TypeEnv) {
    let int_ty = Type::con("Int");
    let float_ty = Type::con("Float");
    let text_ty = Type::con("Text");

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
        row_tail: None,
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
        row_tail: None,
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
        row_tail: None,
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
        row_tail: None,
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
        open: true,
        row_tail: None,
    };
    env.insert("crypto".to_string(), Scheme::mono(crypto_record));

    let i18n_record = Type::Record {
        fields: vec![].into_iter().collect(),
        open: true,
        row_tail: None,
    };
    env.insert("i18n".to_string(), Scheme::mono(i18n_record));

    let option_text_ty = Type::con("Option").app(vec![text_ty.clone()]);
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
        ]
        .into_iter()
        .collect(),
        open: false,
        row_tail: None,
    };
    let env_source_record = Type::Record {
        fields: vec![(
            "get".to_string(),
            Type::Func(
                Box::new(text_ty.clone()),
                Box::new(Type::con("Source").app(vec![Type::con("Env"), text_ty.clone()])),
            ),
        )]
        .into_iter()
        .collect(),
        open: false,
        row_tail: None,
    };
    env.insert("env".to_string(), Scheme::mono(env_source_record));
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
        row_tail: None,
    };
    env.insert("system".to_string(), Scheme::mono(system_record));
    let effect_text_unit = Type::con("Effect").app(vec![text_ty.clone(), Type::con("Unit")]);
    let effect_text_int = Type::con("Effect").app(vec![text_ty.clone(), int_ty.clone()]);
    let effect_text_text = Type::con("Effect").app(vec![text_ty.clone(), text_ty.clone()]);
    let effect_text_list_text = Type::con("Effect").app(vec![
        text_ty.clone(),
        Type::con("List").app(vec![text_ty.clone()]),
    ]);
    let gtk4_record = Type::Record {
        fields: vec![
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
                "trayIconNew".to_string(),
                Type::Func(
                    Box::new(text_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(effect_text_int.clone()),
                    )),
                ),
            ),
            (
                "trayIconSetTooltip".to_string(),
                Type::Func(
                    Box::new(int_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(text_ty.clone()),
                        Box::new(effect_text_unit.clone()),
                    )),
                ),
            ),
            (
                "trayIconSetVisible".to_string(),
                Type::Func(
                    Box::new(int_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(Type::con("Bool")),
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
                        Box::new(effect_text_unit),
                    )),
                ),
            ),
        ]
        .into_iter()
        .collect(),
        open: true,
        row_tail: None,
    };
    env.insert("gtk4".to_string(), Scheme::mono(gtk4_record));

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
        row_tail: None,
    };
    env.insert("logger".to_string(), Scheme::mono(logger_record));

    let db_row = checker.fresh_var_id();
    let db_error_ty = Type::con("DbError");
    let db_config_ty = Type::con("DbConfig");
    let table_ty = Type::con("Table").app(vec![Type::Var(db_row)]);
    let pred_ty = Type::con("Pred").app(vec![Type::Var(db_row)]);
    let patch_ty = Type::con("Patch").app(vec![Type::Var(db_row)]);
    let delta_ty = Type::con("Delta").app(vec![Type::Var(db_row)]);
    let list_table_ty = Type::con("List").app(vec![table_ty.clone()]);
    let list_row_ty = Type::con("List").app(vec![Type::Var(db_row)]);
    let list_column_ty = Type::con("List").app(vec![Type::con("Column")]);
    let db_effect_table_ty = Type::con("Effect").app(vec![db_error_ty.clone(), table_ty.clone()]);
    let db_effect_rows_ty = Type::con("Effect").app(vec![db_error_ty.clone(), list_row_ty.clone()]);
    let db_effect_unit_ty = Type::con("Effect").app(vec![db_error_ty.clone(), Type::con("Unit")]);
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
        row_tail: None,
    };
    env.insert("database".to_string(), Scheme::mono(database_record));
}
