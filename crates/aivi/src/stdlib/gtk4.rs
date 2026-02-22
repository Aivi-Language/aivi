pub const MODULE_NAME: &str = "aivi.gtk4";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.gtk4
export AppId, WindowId, GtkError
export init, appNew, windowNew, windowSetTitle, windowPresent, appRun

use aivi

AppId = Int
WindowId = Int
GtkError = Text

@native "gtk4.init"
init : Unit -> Effect GtkError Unit
init = unit => unit

@native "gtk4.appNew"
appNew : Text -> Effect GtkError AppId
appNew = applicationId => applicationId

@native "gtk4.windowNew"
windowNew : AppId -> Text -> Int -> Int -> Effect GtkError WindowId
windowNew = appId title width height => appId

@native "gtk4.windowSetTitle"
windowSetTitle : WindowId -> Text -> Effect GtkError Unit
windowSetTitle = windowId title => Unit

@native "gtk4.windowPresent"
windowPresent : WindowId -> Effect GtkError Unit
windowPresent = windowId => Unit

@native "gtk4.appRun"
appRun : AppId -> Effect GtkError Unit
appRun = appId => Unit
"#;
