use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use crate::runtime::{EffectValue, Value};

static MOCK_ID: AtomicI64 = AtomicI64::new(1);

/// Create a stub gtk4 builtin that returns a mock value (Int id or Unit).
fn gtk4_stub(name: &'static str, arity: usize) -> Value {
    let full_name = format!("gtk4.{name}");
    super::util::builtin(&full_name, arity, move |_args, _| {
        Ok(Value::Effect(Arc::new(EffectValue::Thunk {
            func: Arc::new(move |_| {
                Ok(Value::Int(MOCK_ID.fetch_add(1, Ordering::Relaxed)))
            }),
        })))
    })
}

/// Create a stub that returns Unit (for void-like operations).
fn gtk4_stub_unit(name: &'static str, arity: usize) -> Value {
    let full_name = format!("gtk4.{name}");
    super::util::builtin(&full_name, arity, move |_args, _| {
        Ok(Value::Effect(Arc::new(EffectValue::Thunk {
            func: Arc::new(move |_| Ok(Value::Unit)),
        })))
    })
}

pub(super) fn build_gtk4_record() -> Value {
    if let Some(real) = super::gtk4_real::build_gtk4_record_real(build_gtk4_stubs) {
        return real;
    }
    build_gtk4_stubs()
}

fn build_gtk4_stubs() -> Value {
    // Stubs that return a widget/object ID (Int)
    let id_stubs: &[(&str, usize)] = &[
        ("buildFromNode", 1),
        ("appNew", 1),
        ("windowNew", 4),
        ("boxNew", 2),
        ("buttonNew", 1),
        ("buttonNewFromIconName", 1),
        ("labelNew", 1),
        ("entryNew", 1),
        ("scrollAreaNew", 1),
        ("separatorNew", 1),
        ("overlayNew", 1),
        ("drawAreaNew", 2),
        ("trayIconNew", 2),
        ("dragSourceNew", 1),
        ("dropTargetNew", 1),
        ("menuModelNew", 1),
        ("menuButtonNew", 1),
        ("dialogNew", 1),
        ("fileDialogNew", 1),
        ("imageNewFromFile", 1),
        ("imageNewFromResource", 1),
        ("imageNewFromIconName", 1),
        ("listStoreNew", 1),
        ("listViewNew", 1),
        ("treeViewNew", 1),
        ("gestureClickNew", 1),
        ("clipboardDefault", 1),
        ("actionNew", 1),
        ("shortcutNew", 2),
        ("notificationNew", 2),
        ("layoutManagerNew", 1),
        ("init", 1),
    ];

    // Stubs that return Unit (setters, presenters, void ops)
    let unit_stubs: &[(&str, usize)] = &[
        ("signalPoll", 1),
        ("signalEmit", 4),
        ("appRun", 1),
        ("appSetCss", 2),
        ("windowSetTitle", 2),
        ("windowSetTitlebar", 2),
        ("windowSetChild", 2),
        ("windowPresent", 1),
        ("widgetShow", 1),
        ("widgetHide", 1),
        ("widgetSetSizeRequest", 3),
        ("widgetSetHexpand", 2),
        ("widgetSetVexpand", 2),
        ("widgetSetHalign", 2),
        ("widgetSetValign", 2),
        ("widgetSetMarginStart", 2),
        ("widgetSetMarginEnd", 2),
        ("widgetSetMarginTop", 2),
        ("widgetSetMarginBottom", 2),
        ("widgetAddCssClass", 2),
        ("widgetRemoveCssClass", 2),
        ("widgetSetTooltipText", 2),
        ("widgetSetOpacity", 2),
        ("widgetSetCss", 2),
        ("widgetAddController", 2),
        ("widgetAddShortcut", 2),
        ("widgetSetLayoutManager", 2),
        ("boxAppend", 2),
        ("boxSetHomogeneous", 2),
        ("buttonSetLabel", 2),
        ("buttonSetChild", 2),
        ("labelSetText", 2),
        ("labelSetWrap", 2),
        ("labelSetEllipsize", 2),
        ("labelSetXalign", 2),
        ("labelSetMaxWidthChars", 2),
        ("entrySetText", 2),
        ("scrollAreaSetChild", 2),
        ("scrollAreaSetPolicy", 3),
        ("overlaySetChild", 2),
        ("overlayAddOverlay", 2),
        ("drawAreaSetContentSize", 3),
        ("drawAreaQueueDraw", 1),
        ("trayIconSetTooltip", 2),
        ("trayIconSetVisible", 2),
        ("dragSourceSetText", 2),
        ("menuModelAppendItem", 3),
        ("menuButtonSetMenuModel", 2),
        ("dialogSetTitle", 2),
        ("dialogSetChild", 2),
        ("dialogPresent", 1),
        ("dialogClose", 1),
        ("fileDialogSelectFile", 1),
        ("imageSetFile", 2),
        ("imageSetResource", 2),
        ("imageSetPixelSize", 2),
        ("iconThemeAddSearchPath", 1),
        ("listStoreAppendText", 2),
        ("listViewSetModel", 2),
        ("treeViewSetModel", 2),
        ("clipboardSetText", 2),
        ("actionSetEnabled", 2),
        ("appAddAction", 2),
        ("notificationSetBody", 2),
        ("appSendNotification", 3),
        ("appWithdrawNotification", 2),
        ("osOpenUri", 2),
        ("osShowInFileManager", 1),
        ("osSetBadgeCount", 2),
    ];

    // Stubs that return Text
    let text_stubs: &[(&str, usize)] = &[
        ("entryText", 1),
        ("dropTargetLastText", 1),
        ("clipboardText", 1),
        ("osThemePreference", 1),
    ];

    // List-returning stubs
    let list_stubs: &[(&str, usize)] = &[
        ("listStoreItems", 1),
        ("gestureClickLastButton", 1),
    ];

    let mut fields = HashMap::new();
    for &(name, arity) in id_stubs {
        fields.insert(name.to_string(), gtk4_stub(name, arity));
    }
    for &(name, arity) in unit_stubs {
        fields.insert(name.to_string(), gtk4_stub_unit(name, arity));
    }
    for &(name, arity) in text_stubs {
        let full_name = format!("gtk4.{name}");
        fields.insert(name.to_string(), super::util::builtin(&full_name, arity, move |_args, _| {
            Ok(Value::Effect(Arc::new(EffectValue::Thunk {
                func: Arc::new(move |_| Ok(Value::Text(String::new()))),
            })))
        }));
    }
    for &(name, arity) in list_stubs {
        let full_name = format!("gtk4.{name}");
        fields.insert(name.to_string(), super::util::builtin(&full_name, arity, move |_args, _| {
            Ok(Value::Effect(Arc::new(EffectValue::Thunk {
                func: Arc::new(move |_| Ok(Value::List(Arc::new(vec![])))),
            })))
        }));
    }
    Value::Record(Arc::new(fields))
}
