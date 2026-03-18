use aivi::{embedded_stdlib_modules, ModuleItem};

#[test]
fn stdlib_ui_exports_v_element() {
    let modules = embedded_stdlib_modules();
    let ui = modules
        .iter()
        .find(|m| m.name.name == "aivi.ui")
        .expect("aivi.ui module exists");

    assert!(
        ui.exports.iter().any(|e| e.name.name == "vElement"),
        "expected aivi.ui to export vElement, exports={:?}",
        ui.exports
            .iter()
            .map(|e| e.name.name.as_str())
            .collect::<Vec<_>>()
    );

    let def_names: Vec<&str> = ui
        .items
        .iter()
        .filter_map(|item| match item {
            ModuleItem::Def(def) => Some(def.name.name.as_str()),
            _ => None,
        })
        .collect();

    let v_element_def = ui.items.iter().find_map(|item| match item {
        ModuleItem::Def(def) if def.name.name == "vElement" => Some(def),
        _ => None,
    });
    assert!(
        v_element_def.is_some(),
        "expected aivi.ui to define vElement; defs={def_names:?}"
    );

    for expected in ["vText", "vKeyed", "vClass", "vId", "vStyle", "vAttr"] {
        assert!(
            def_names.contains(&expected),
            "expected aivi.ui to define {expected}; defs={def_names:?}"
        );
    }

    let export_names: Vec<&str> = ui.exports.iter().map(|e| e.name.name.as_str()).collect();
    for expected in ["vText", "vKeyed", "vClass", "vId", "vStyle", "vAttr"] {
        assert!(
            export_names.contains(&expected),
            "expected aivi.ui to export {expected}; exports={export_names:?}"
        );
    }
    let mut actual_event_exports: Vec<&str> = export_names
        .iter()
        .copied()
        .filter(|name| {
            name.starts_with("On")
                || name.starts_with("vOn")
                || name.ends_with("Event")
                || matches!(*name, "Click" | "Input")
        })
        .collect();
    actual_event_exports.sort_unstable();
    assert_eq!(
        actual_event_exports,
        vec![
            "Click",
            "ClickEvent",
            "Input",
            "InputEvent",
            "KeyboardEvent",
            "OnBlur",
            "OnClick",
            "OnClickE",
            "OnFocus",
            "OnInput",
            "OnInputE",
            "OnKeyDown",
            "OnKeyUp",
            "OnPointerDown",
            "OnPointerMove",
            "OnPointerUp",
            "PointerEvent",
            "vOnBlur",
            "vOnClick",
            "vOnClickE",
            "vOnFocus",
            "vOnInput",
            "vOnInputE",
            "vOnKeyDown",
            "vOnKeyUp",
            "vOnPointerDown",
            "vOnPointerMove",
            "vOnPointerUp",
        ]
    );

    let _def = v_element_def.expect("vElement def");
}

#[test]
fn stdlib_gtk4_exports_signal_first_binding_surface() {
    let modules = embedded_stdlib_modules();
    let gtk4 = modules
        .iter()
        .find(|m| m.name.name == "aivi.ui.gtk4")
        .expect("aivi.ui.gtk4 module exists");

    let def_names: Vec<&str> = gtk4
        .items
        .iter()
        .filter_map(|item| match item {
            ModuleItem::Def(def) => Some(def.name.name.as_str()),
            _ => None,
        })
        .collect();
    let export_names: Vec<&str> = gtk4.exports.iter().map(|e| e.name.name.as_str()).collect();

    for expected in [
        "GtkStaticAttr",
        "GtkBoundAttr",
        "GtkStaticProp",
        "GtkBoundProp",
        "GtkEventProp",
        "GtkEventSugarProp",
        "GtkIdAttr",
        "GtkRefAttr",
    ] {
        assert!(
            export_names.contains(&expected),
            "expected aivi.ui.gtk4 to export {expected}, exports={export_names:?}"
        );
    }

    for expected in [
        "gtkBoundText",
        "gtkShow",
        "gtkEach",
        "gtkEachKeyed",
        "gtkStaticProp",
        "gtkBoundProp",
        "gtkEventAttr",
        "gtkEventSugarAttr",
        "gtkIdAttr",
        "gtkRefAttr",
        "buildFromNode",
        "buildWithIds",
        "reconcileNode",
        "mountAppWindow",
        "runGtkApp",
        "signalPoll",
        "signalStream",
        "signalEmit",
        "menuModelNew",
        "menuModelAppendItem",
        "menuButtonSetMenuModel",
        "osOpenUri",
        "gtkSetInterval",
    ] {
        assert!(
            export_names.contains(&expected),
            "expected aivi.ui.gtk4 to export {expected}, exports={export_names:?}"
        );
        assert!(
            def_names.contains(&expected),
            "expected aivi.ui.gtk4 to define {expected}; defs={def_names:?}"
        );
    }

    let mut actual_curated_imperative_exports: Vec<&str> = export_names
        .iter()
        .copied()
        .filter(|name| {
            matches!(
                *name,
                "MenuModelId"
                    | "MenuButtonId"
                    | "DialogId"
                    | "TrayIconId"
                    | "adwDialogPresent"
                    | "signalBindDialogPresent"
                    | "dbusServerStart"
                    | "gtkSetInterval"
                    | "trayNotifyPersonalEmail"
                    | "traySetEmailSuggestions"
            ) || name.starts_with("menu")
                || name.starts_with("dialog")
                || name.starts_with("tray")
                || name.starts_with("os")
        })
        .collect();
    actual_curated_imperative_exports.sort_unstable();
    assert_eq!(
        actual_curated_imperative_exports,
        vec![
            "MenuModelId",
            "gtkSetInterval",
            "menuButtonSetMenuModel",
            "menuModelAppendItem",
            "menuModelNew",
            "osOpenUri",
        ]
    );

    for removed in [
        "gtkApp",
        "AppStep",
        "Command",
        "Subscription",
        "auto",
        "derive",
        "memo",
        "readDerived",
        "signalBindBoolProperty",
        "signalBindCssClass",
        "signalBindToggleBoolProperty",
        "signalToggleCssClass",
    ] {
        assert!(
            !export_names.contains(&removed),
            "did not expect aivi.ui.gtk4 to export {removed}, exports={export_names:?}"
        );
    }

    for removed in [
        "MenuButtonId",
        "DialogId",
        "TrayIconId",
        "DragSourceId",
        "DropTargetId",
        "FileDialogId",
        "ListStoreId",
        "ListViewId",
        "TreeViewId",
        "ClipboardId",
        "ShortcutId",
        "NotificationId",
        "LayoutManagerId",
    ] {
        assert!(
            !export_names.contains(&removed),
            "did not expect aivi.ui.gtk4 to export {removed}, exports={export_names:?}"
        );
    }

    for removed in [
        "menuButtonNew",
        "dialogNew",
        "dialogSetTitle",
        "dialogSetChild",
        "dialogPresent",
        "dialogClose",
        "adwDialogPresent",
        "signalBindDialogPresent",
        "trayIconNew",
        "trayIconSetTooltip",
        "trayIconSetVisible",
        "trayIconSetMenuItems",
        "trayNotifyPersonalEmail",
        "traySetEmailSuggestions",
        "dbusServerStart",
        "osSetBadgeCount",
        "osThemePreference",
        "dragSourceNew",
        "dragSourceSetText",
        "dropTargetNew",
        "dropTargetLastText",
        "fileDialogNew",
        "fileDialogSelectFile",
        "listStoreNew",
        "listStoreAppendText",
        "listStoreItems",
        "listViewNew",
        "listViewSetModel",
        "treeViewNew",
        "treeViewSetModel",
        "clipboardDefault",
        "clipboardSetText",
        "clipboardText",
        "shortcutNew",
        "widgetAddShortcut",
        "notificationNew",
        "notificationSetBody",
        "appSendNotification",
        "appWithdrawNotification",
        "layoutManagerNew",
        "widgetSetLayoutManager",
        "osShowInFileManager",
    ] {
        assert!(
            !export_names.contains(&removed),
            "did not expect aivi.ui.gtk4 to export {removed}, exports={export_names:?}"
        );
    }
}

#[test]
fn stdlib_reactive_exports_core_primitives() {
    let modules = embedded_stdlib_modules();
    let reactive = modules
        .iter()
        .find(|m| m.name.name == "aivi.reactive")
        .expect("aivi.reactive module exists");

    let export_names: Vec<&str> = reactive
        .exports
        .iter()
        .map(|e| e.name.name.as_str())
        .collect();
    for expected in [
        "Signal",
        "Disposable",
        "EventHandle",
        "signal",
        "get",
        "peek",
        "set",
        "update",
        "derive",
        "combineAll",
        "watch",
        "on",
        "batch",
        "dispose",
    ] {
        assert!(
            export_names.contains(&expected),
            "expected aivi.reactive to export {expected}, exports={export_names:?}"
        );
    }
}
