use std::collections::HashMap;
use std::sync::Arc;

use crate::runtime::{EffectValue, RuntimeError, Value};

/// Create a stub gtk4 builtin that returns an error effect.
fn gtk4_stub(name: &'static str, arity: usize) -> Value {
    let full_name = format!("gtk4.{name}");
    super::util::builtin(&full_name, arity, move |_args, _| {
        let msg = format!("gtk4.{name}: GTK4 runtime is not available");
        Ok(Value::Effect(Arc::new(EffectValue::Thunk {
            func: Arc::new(move |_| Err(RuntimeError::Error(Value::Text(msg.clone())))),
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
    let stubs: &[(&str, usize)] = &[
        ("buildFromNode", 1),
        ("signalPoll", 1),
        ("signalEmit", 4),
        ("signalStream", 1),
        ("init", 1),
        ("appNew", 1),
        ("appRun", 1),
        ("appSetCss", 2),
        ("windowNew", 4),
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
        ("boxNew", 2),
        ("boxAppend", 2),
        ("boxSetHomogeneous", 2),
        ("buttonNew", 1),
        ("buttonSetLabel", 2),
        ("buttonNewFromIconName", 1),
        ("buttonSetChild", 2),
        ("labelNew", 1),
        ("labelSetText", 2),
        ("labelSetWrap", 2),
        ("labelSetEllipsize", 2),
        ("labelSetXalign", 2),
        ("labelSetMaxWidthChars", 2),
        ("entryNew", 1),
        ("entrySetText", 2),
        ("entryText", 1),
        ("scrollAreaNew", 1),
        ("scrollAreaSetChild", 2),
        ("scrollAreaSetPolicy", 3),
        ("separatorNew", 1),
        ("overlayNew", 1),
        ("overlaySetChild", 2),
        ("overlayAddOverlay", 2),
        ("drawAreaNew", 2),
        ("drawAreaSetContentSize", 3),
        ("drawAreaQueueDraw", 1),
        ("trayIconNew", 2),
        ("trayIconSetTooltip", 2),
        ("trayIconSetVisible", 2),
        ("dragSourceNew", 1),
        ("dragSourceSetText", 2),
        ("dropTargetNew", 1),
        ("dropTargetLastText", 1),
        ("menuModelNew", 1),
        ("menuModelAppendItem", 3),
        ("menuButtonNew", 1),
        ("menuButtonSetMenuModel", 2),
        ("dialogNew", 1),
        ("dialogSetTitle", 2),
        ("dialogSetChild", 2),
        ("dialogPresent", 1),
        ("dialogClose", 1),
        ("fileDialogNew", 1),
        ("fileDialogSelectFile", 1),
        ("imageNewFromFile", 1),
        ("imageSetFile", 2),
        ("imageNewFromResource", 1),
        ("imageSetResource", 2),
        ("imageNewFromIconName", 1),
        ("imageSetPixelSize", 2),
        ("iconThemeAddSearchPath", 1),
        ("listStoreNew", 1),
        ("listStoreAppendText", 2),
        ("listStoreItems", 1),
        ("listViewNew", 1),
        ("listViewSetModel", 2),
        ("treeViewNew", 1),
        ("treeViewSetModel", 2),
        ("gestureClickNew", 1),
        ("gestureClickLastButton", 1),
        ("clipboardDefault", 1),
        ("clipboardSetText", 2),
        ("clipboardText", 1),
        ("actionNew", 1),
        ("actionSetEnabled", 2),
        ("appAddAction", 2),
        ("shortcutNew", 2),
        ("notificationNew", 2),
        ("notificationSetBody", 2),
        ("appSendNotification", 3),
        ("appWithdrawNotification", 2),
        ("layoutManagerNew", 1),
        ("osOpenUri", 2),
        ("osShowInFileManager", 1),
        ("osSetBadgeCount", 2),
        ("osThemePreference", 1),
        ("widgetById", 1),
        ("signalBindBoolProperty", 4),
        ("signalBindCssClass", 4),
        ("signalBindToggleBoolProperty", 3),
        ("signalToggleCssClass", 3),
        ("dialogNew", 1),
        ("dialogSetTitle", 2),
        ("dialogSetChild", 2),
        ("dialogPresent", 1),
        ("dialogClose", 1),
        ("signalBindDialogPresent", 3),
        ("signalBindStackPage", 3),
    ];

    let mut fields = HashMap::new();
    for &(name, arity) in stubs {
        fields.insert(name.to_string(), gtk4_stub(name, arity));
    }
    Value::Record(Arc::new(fields))
}
