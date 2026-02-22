pub const MODULE_NAME: &str = "aivi.gtk4";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.gtk4
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

use aivi

AppId = Int
WindowId = Int
WidgetId = Int
BoxId = Int
ButtonId = Int
LabelId = Int
EntryId = Int
ScrollAreaId = Int
TrayIconId = Int
DragSourceId = Int
DropTargetId = Int
MenuModelId = Int
MenuButtonId = Int
DialogId = Int
FileDialogId = Int
ImageId = Int
ListStoreId = Int
ListViewId = Int
TreeViewId = Int
GestureClickId = Int
ClipboardId = Int
ActionId = Int
ShortcutId = Int
GtkError = Text

init : Unit -> Effect GtkError Unit
init = gtk4.init

appNew : Text -> Effect GtkError AppId
appNew = gtk4.appNew

windowNew : AppId -> Text -> Int -> Int -> Effect GtkError WindowId
windowNew = gtk4.windowNew

windowSetTitle : WindowId -> Text -> Effect GtkError Unit
windowSetTitle = gtk4.windowSetTitle

windowSetChild : WindowId -> WidgetId -> Effect GtkError Unit
windowSetChild = gtk4.windowSetChild

windowPresent : WindowId -> Effect GtkError Unit
windowPresent = gtk4.windowPresent

appRun : AppId -> Effect GtkError Unit
appRun = gtk4.appRun

widgetShow : WidgetId -> Effect GtkError Unit
widgetShow = gtk4.widgetShow

widgetHide : WidgetId -> Effect GtkError Unit
widgetHide = gtk4.widgetHide

boxNew : Int -> Int -> Effect GtkError BoxId
boxNew = gtk4.boxNew

boxAppend : BoxId -> WidgetId -> Effect GtkError Unit
boxAppend = gtk4.boxAppend

buttonNew : Text -> Effect GtkError ButtonId
buttonNew = gtk4.buttonNew

buttonSetLabel : ButtonId -> Text -> Effect GtkError Unit
buttonSetLabel = gtk4.buttonSetLabel

labelNew : Text -> Effect GtkError LabelId
labelNew = gtk4.labelNew

labelSetText : LabelId -> Text -> Effect GtkError Unit
labelSetText = gtk4.labelSetText

entryNew : Unit -> Effect GtkError EntryId
entryNew = gtk4.entryNew

entrySetText : EntryId -> Text -> Effect GtkError Unit
entrySetText = gtk4.entrySetText

entryText : EntryId -> Effect GtkError Text
entryText = gtk4.entryText

scrollAreaNew : Unit -> Effect GtkError ScrollAreaId
scrollAreaNew = gtk4.scrollAreaNew

scrollAreaSetChild : ScrollAreaId -> WidgetId -> Effect GtkError Unit
scrollAreaSetChild = gtk4.scrollAreaSetChild

trayIconNew : Text -> Text -> Effect GtkError TrayIconId
trayIconNew = gtk4.trayIconNew

trayIconSetTooltip : TrayIconId -> Text -> Effect GtkError Unit
trayIconSetTooltip = gtk4.trayIconSetTooltip

trayIconSetVisible : TrayIconId -> Bool -> Effect GtkError Unit
trayIconSetVisible = gtk4.trayIconSetVisible

dragSourceNew : WidgetId -> Effect GtkError DragSourceId
dragSourceNew = gtk4.dragSourceNew

dragSourceSetText : DragSourceId -> Text -> Effect GtkError Unit
dragSourceSetText = gtk4.dragSourceSetText

dropTargetNew : WidgetId -> Effect GtkError DropTargetId
dropTargetNew = gtk4.dropTargetNew

dropTargetLastText : DropTargetId -> Effect GtkError Text
dropTargetLastText = gtk4.dropTargetLastText

menuModelNew : Unit -> Effect GtkError MenuModelId
menuModelNew = gtk4.menuModelNew

menuModelAppendItem : MenuModelId -> Text -> Text -> Effect GtkError Unit
menuModelAppendItem = gtk4.menuModelAppendItem

menuButtonNew : Text -> Effect GtkError MenuButtonId
menuButtonNew = gtk4.menuButtonNew

menuButtonSetMenuModel : MenuButtonId -> MenuModelId -> Effect GtkError Unit
menuButtonSetMenuModel = gtk4.menuButtonSetMenuModel

dialogNew : AppId -> Effect GtkError DialogId
dialogNew = gtk4.dialogNew

dialogSetTitle : DialogId -> Text -> Effect GtkError Unit
dialogSetTitle = gtk4.dialogSetTitle

dialogSetChild : DialogId -> WidgetId -> Effect GtkError Unit
dialogSetChild = gtk4.dialogSetChild

dialogPresent : DialogId -> Effect GtkError Unit
dialogPresent = gtk4.dialogPresent

dialogClose : DialogId -> Effect GtkError Unit
dialogClose = gtk4.dialogClose

fileDialogNew : Unit -> Effect GtkError FileDialogId
fileDialogNew = gtk4.fileDialogNew

fileDialogSelectFile : FileDialogId -> Effect GtkError Text
fileDialogSelectFile = gtk4.fileDialogSelectFile

imageNewFromFile : Text -> Effect GtkError ImageId
imageNewFromFile = gtk4.imageNewFromFile

imageSetFile : ImageId -> Text -> Effect GtkError Unit
imageSetFile = gtk4.imageSetFile

listStoreNew : Unit -> Effect GtkError ListStoreId
listStoreNew = gtk4.listStoreNew

listStoreAppendText : ListStoreId -> Text -> Effect GtkError Unit
listStoreAppendText = gtk4.listStoreAppendText

listStoreItems : ListStoreId -> Effect GtkError (List Text)
listStoreItems = gtk4.listStoreItems

listViewNew : Unit -> Effect GtkError ListViewId
listViewNew = gtk4.listViewNew

listViewSetModel : ListViewId -> ListStoreId -> Effect GtkError Unit
listViewSetModel = gtk4.listViewSetModel

treeViewNew : Unit -> Effect GtkError TreeViewId
treeViewNew = gtk4.treeViewNew

treeViewSetModel : TreeViewId -> ListStoreId -> Effect GtkError Unit
treeViewSetModel = gtk4.treeViewSetModel

gestureClickNew : WidgetId -> Effect GtkError GestureClickId
gestureClickNew = gtk4.gestureClickNew

gestureClickLastButton : GestureClickId -> Effect GtkError Int
gestureClickLastButton = gtk4.gestureClickLastButton

widgetAddController : WidgetId -> GestureClickId -> Effect GtkError Unit
widgetAddController = gtk4.widgetAddController

clipboardDefault : Unit -> Effect GtkError ClipboardId
clipboardDefault = gtk4.clipboardDefault

clipboardSetText : ClipboardId -> Text -> Effect GtkError Unit
clipboardSetText = gtk4.clipboardSetText

clipboardText : ClipboardId -> Effect GtkError Text
clipboardText = gtk4.clipboardText

actionNew : Text -> Effect GtkError ActionId
actionNew = gtk4.actionNew

actionSetEnabled : ActionId -> Bool -> Effect GtkError Unit
actionSetEnabled = gtk4.actionSetEnabled

appAddAction : AppId -> ActionId -> Effect GtkError Unit
appAddAction = gtk4.appAddAction

shortcutNew : Text -> Text -> Effect GtkError ShortcutId
shortcutNew = gtk4.shortcutNew

widgetAddShortcut : WidgetId -> ShortcutId -> Effect GtkError Unit
widgetAddShortcut = gtk4.widgetAddShortcut
"#;
