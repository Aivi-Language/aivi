pub const MODULE_NAME: &str = "aivi.ui.Gtk4";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.ui.Gtk4
export AppId, WindowId, WidgetId, BoxId, ButtonId, LabelId, EntryId, ScrollAreaId, TrayIconId, DragSourceId, DropTargetId, MenuModelId, MenuButtonId, DialogId, FileDialogId, ImageId, ListStoreId, ListViewId, TreeViewId, GestureClickId, ClipboardId, ActionId, ShortcutId, GtkError
export init, appNew, appRun
export windowNew, windowSetTitle, windowSetChild, windowPresent
export widgetShow, widgetHide
export boxNew, boxAppend
export buttonNew, buttonSetLabel
export labelNew, labelSetText
export entryNew, entrySetText, entryText
export scrollAreaNew, scrollAreaSetChild
export trayIconNew, trayIconSetTooltip, trayIconSetVisible
export dragSourceNew, dragSourceSetText
export dropTargetNew, dropTargetLastText
export menuModelNew, menuModelAppendItem, menuButtonNew, menuButtonSetMenuModel
export dialogNew, dialogSetTitle, dialogSetChild, dialogPresent, dialogClose
export fileDialogNew, fileDialogSelectFile
export imageNewFromFile, imageSetFile
export listStoreNew, listStoreAppendText, listStoreItems
export listViewNew, listViewSetModel
export treeViewNew, treeViewSetModel
export gestureClickNew, gestureClickLastButton, widgetAddController
export clipboardDefault, clipboardSetText, clipboardText
export actionNew, actionSetEnabled, appAddAction
export shortcutNew, widgetAddShortcut

use aivi.gtk4 (AppId, WindowId, WidgetId, BoxId, ButtonId, LabelId, EntryId, ScrollAreaId, TrayIconId, DragSourceId, DropTargetId, MenuModelId, MenuButtonId, DialogId, FileDialogId, ImageId, ListStoreId, ListViewId, TreeViewId, GestureClickId, ClipboardId, ActionId, ShortcutId, GtkError, init, appNew, appRun, windowNew, windowSetTitle, windowSetChild, windowPresent, widgetShow, widgetHide, boxNew, boxAppend, buttonNew, buttonSetLabel, labelNew, labelSetText, entryNew, entrySetText, entryText, scrollAreaNew, scrollAreaSetChild, trayIconNew, trayIconSetTooltip, trayIconSetVisible, dragSourceNew, dragSourceSetText, dropTargetNew, dropTargetLastText, menuModelNew, menuModelAppendItem, menuButtonNew, menuButtonSetMenuModel, dialogNew, dialogSetTitle, dialogSetChild, dialogPresent, dialogClose, fileDialogNew, fileDialogSelectFile, imageNewFromFile, imageSetFile, listStoreNew, listStoreAppendText, listStoreItems, listViewNew, listViewSetModel, treeViewNew, treeViewSetModel, gestureClickNew, gestureClickLastButton, widgetAddController, clipboardDefault, clipboardSetText, clipboardText, actionNew, actionSetEnabled, appAddAction, shortcutNew, widgetAddShortcut)
"#;
