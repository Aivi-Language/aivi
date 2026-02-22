# `aivi.gtk4`
## Native GTK4 Runtime Bindings

<!-- quick-info: {"kind":"module","name":"aivi.gtk4"} -->
`aivi.gtk4` is the convenience module for GTK4-oriented native UI effects.
It exposes AIVI types/functions mapped directly to runtime native bindings.
<!-- /quick-info -->

<div class="import-badge">use aivi.gtk4</div>

## Public API

```aivi
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
appNew : Text -> Effect GtkError AppId
appRun : AppId -> Effect GtkError Unit

windowNew : AppId -> Text -> Int -> Int -> Effect GtkError WindowId
windowSetTitle : WindowId -> Text -> Effect GtkError Unit
windowSetChild : WindowId -> WidgetId -> Effect GtkError Unit
windowPresent : WindowId -> Effect GtkError Unit

widgetShow : WidgetId -> Effect GtkError Unit
widgetHide : WidgetId -> Effect GtkError Unit

boxNew : Int -> Int -> Effect GtkError BoxId
boxAppend : BoxId -> WidgetId -> Effect GtkError Unit

buttonNew : Text -> Effect GtkError ButtonId
buttonSetLabel : ButtonId -> Text -> Effect GtkError Unit

labelNew : Text -> Effect GtkError LabelId
labelSetText : LabelId -> Text -> Effect GtkError Unit

entryNew : Unit -> Effect GtkError EntryId
entrySetText : EntryId -> Text -> Effect GtkError Unit
entryText : EntryId -> Effect GtkError Text

scrollAreaNew : Unit -> Effect GtkError ScrollAreaId
scrollAreaSetChild : ScrollAreaId -> WidgetId -> Effect GtkError Unit

trayIconNew : Text -> Text -> Effect GtkError TrayIconId
trayIconSetTooltip : TrayIconId -> Text -> Effect GtkError Unit
trayIconSetVisible : TrayIconId -> Bool -> Effect GtkError Unit

dragSourceNew : WidgetId -> Effect GtkError DragSourceId
dragSourceSetText : DragSourceId -> Text -> Effect GtkError Unit
dropTargetNew : WidgetId -> Effect GtkError DropTargetId
dropTargetLastText : DropTargetId -> Effect GtkError Text

menuModelNew : Unit -> Effect GtkError MenuModelId
menuModelAppendItem : MenuModelId -> Text -> Text -> Effect GtkError Unit
menuButtonNew : Text -> Effect GtkError MenuButtonId
menuButtonSetMenuModel : MenuButtonId -> MenuModelId -> Effect GtkError Unit

dialogNew : AppId -> Effect GtkError DialogId
dialogSetTitle : DialogId -> Text -> Effect GtkError Unit
dialogSetChild : DialogId -> WidgetId -> Effect GtkError Unit
dialogPresent : DialogId -> Effect GtkError Unit
dialogClose : DialogId -> Effect GtkError Unit

fileDialogNew : Unit -> Effect GtkError FileDialogId
fileDialogSelectFile : FileDialogId -> Effect GtkError Text

imageNewFromFile : Text -> Effect GtkError ImageId
imageSetFile : ImageId -> Text -> Effect GtkError Unit

listStoreNew : Unit -> Effect GtkError ListStoreId
listStoreAppendText : ListStoreId -> Text -> Effect GtkError Unit
listStoreItems : ListStoreId -> Effect GtkError (List Text)
listViewNew : Unit -> Effect GtkError ListViewId
listViewSetModel : ListViewId -> ListStoreId -> Effect GtkError Unit
treeViewNew : Unit -> Effect GtkError TreeViewId
treeViewSetModel : TreeViewId -> ListStoreId -> Effect GtkError Unit

gestureClickNew : WidgetId -> Effect GtkError GestureClickId
gestureClickLastButton : GestureClickId -> Effect GtkError Int
widgetAddController : WidgetId -> GestureClickId -> Effect GtkError Unit

clipboardDefault : Unit -> Effect GtkError ClipboardId
clipboardSetText : ClipboardId -> Text -> Effect GtkError Unit
clipboardText : ClipboardId -> Effect GtkError Text

actionNew : Text -> Effect GtkError ActionId
actionSetEnabled : ActionId -> Bool -> Effect GtkError Unit
appAddAction : AppId -> ActionId -> Effect GtkError Unit

shortcutNew : Text -> Text -> Effect GtkError ShortcutId
widgetAddShortcut : WidgetId -> ShortcutId -> Effect GtkError Unit
```

## Native Mapping Table

| AIVI function | Native target |
| --- | --- |
| `init` | `gtk4.init` |
| `appNew` | `gtk4.appNew` |
| `windowNew` | `gtk4.windowNew` |
| `windowSetTitle` | `gtk4.windowSetTitle` |
| `windowSetChild` | `gtk4.windowSetChild` |
| `windowPresent` | `gtk4.windowPresent` |
| `appRun` | `gtk4.appRun` |
| `widgetShow` | `gtk4.widgetShow` |
| `widgetHide` | `gtk4.widgetHide` |
| `boxNew` | `gtk4.boxNew` |
| `boxAppend` | `gtk4.boxAppend` |
| `buttonNew` | `gtk4.buttonNew` |
| `buttonSetLabel` | `gtk4.buttonSetLabel` |
| `labelNew` | `gtk4.labelNew` |
| `labelSetText` | `gtk4.labelSetText` |
| `entryNew` | `gtk4.entryNew` |
| `entrySetText` | `gtk4.entrySetText` |
| `entryText` | `gtk4.entryText` |
| `scrollAreaNew` | `gtk4.scrollAreaNew` |
| `scrollAreaSetChild` | `gtk4.scrollAreaSetChild` |
| `trayIconNew` | `gtk4.trayIconNew` |
| `trayIconSetTooltip` | `gtk4.trayIconSetTooltip` |
| `trayIconSetVisible` | `gtk4.trayIconSetVisible` |
| `dragSourceNew` | `gtk4.dragSourceNew` |
| `dragSourceSetText` | `gtk4.dragSourceSetText` |
| `dropTargetNew` | `gtk4.dropTargetNew` |
| `dropTargetLastText` | `gtk4.dropTargetLastText` |
| `menuModelNew` | `gtk4.menuModelNew` |
| `menuModelAppendItem` | `gtk4.menuModelAppendItem` |
| `menuButtonNew` | `gtk4.menuButtonNew` |
| `menuButtonSetMenuModel` | `gtk4.menuButtonSetMenuModel` |
| `dialogNew` | `gtk4.dialogNew` |
| `dialogSetTitle` | `gtk4.dialogSetTitle` |
| `dialogSetChild` | `gtk4.dialogSetChild` |
| `dialogPresent` | `gtk4.dialogPresent` |
| `dialogClose` | `gtk4.dialogClose` |
| `fileDialogNew` | `gtk4.fileDialogNew` |
| `fileDialogSelectFile` | `gtk4.fileDialogSelectFile` |
| `imageNewFromFile` | `gtk4.imageNewFromFile` |
| `imageSetFile` | `gtk4.imageSetFile` |
| `listStoreNew` | `gtk4.listStoreNew` |
| `listStoreAppendText` | `gtk4.listStoreAppendText` |
| `listStoreItems` | `gtk4.listStoreItems` |
| `listViewNew` | `gtk4.listViewNew` |
| `listViewSetModel` | `gtk4.listViewSetModel` |
| `treeViewNew` | `gtk4.treeViewNew` |
| `treeViewSetModel` | `gtk4.treeViewSetModel` |
| `gestureClickNew` | `gtk4.gestureClickNew` |
| `gestureClickLastButton` | `gtk4.gestureClickLastButton` |
| `widgetAddController` | `gtk4.widgetAddController` |
| `clipboardDefault` | `gtk4.clipboardDefault` |
| `clipboardSetText` | `gtk4.clipboardSetText` |
| `clipboardText` | `gtk4.clipboardText` |
| `actionNew` | `gtk4.actionNew` |
| `actionSetEnabled` | `gtk4.actionSetEnabled` |
| `appAddAction` | `gtk4.appAddAction` |
| `shortcutNew` | `gtk4.shortcutNew` |
| `widgetAddShortcut` | `gtk4.widgetAddShortcut` |

## Example

```aivi
use aivi
use aivi.gtk4

main = do Effect {
  _ <- init Unit
  appId <- appNew "com.example.demo"
  winId <- windowNew appId "AIVI GTK4" 800 600
  root <- boxNew 1 8
  button <- buttonNew "Click me"
  _ <- boxAppend root button
  _ <- windowSetChild winId root
  _ <- windowPresent winId
  _ <- appRun appId
  pure Unit
}
```

## Compatibility

`aivi.ui.Gtk4` is still available and re-exports `aivi.gtk4` for compatibility.
