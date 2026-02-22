pub const MODULE_NAME: &str = "aivi.ui.Gtk4";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.ui.Gtk4
export AppId, WindowId, GtkError
export init, appNew, windowNew, windowSetTitle, windowPresent, appRun

use aivi.gtk4 (AppId, WindowId, GtkError, init, appNew, windowNew, windowSetTitle, windowPresent, appRun)
"#;
