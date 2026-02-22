# `aivi.ui.gtk4`
## Native GTK4 Runtime Bindings

<!-- quick-info: {"kind":"module","name":"aivi.ui.gtk4"} -->
`aivi.ui.gtk4` is the convenience module for GTK4-oriented native UI effects.
It exposes AIVI types/functions mapped directly to runtime native bindings.
<!-- /quick-info -->

<div class="import-badge">use aivi.ui.gtk4</div>

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
DrawAreaId = Int
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
NotificationId = Int
LayoutManagerId = Int
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

drawAreaNew : Int -> Int -> Effect GtkError DrawAreaId
drawAreaSetContentSize : DrawAreaId -> Int -> Int -> Effect GtkError Unit
drawAreaQueueDraw : DrawAreaId -> Effect GtkError Unit

widgetSetCss : WidgetId -> { } -> Effect GtkError Unit
appSetCss : AppId -> { } -> Effect GtkError Unit

notificationNew : Text -> Text -> Effect GtkError NotificationId
notificationSetBody : NotificationId -> Text -> Effect GtkError Unit
appSendNotification : AppId -> Text -> NotificationId -> Effect GtkError Unit
appWithdrawNotification : AppId -> Text -> Effect GtkError Unit

layoutManagerNew : Text -> Effect GtkError LayoutManagerId
widgetSetLayoutManager : WidgetId -> LayoutManagerId -> Effect GtkError Unit

osOpenUri : AppId -> Text -> Effect GtkError Unit
osShowInFileManager : Text -> Effect GtkError Unit
osSetBadgeCount : AppId -> Int -> Effect GtkError Unit
osThemePreference : Unit -> Effect GtkError Text
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
| `drawAreaNew` | `gtk4.drawAreaNew` |
| `drawAreaSetContentSize` | `gtk4.drawAreaSetContentSize` |
| `drawAreaQueueDraw` | `gtk4.drawAreaQueueDraw` |
| `widgetSetCss` | `gtk4.widgetSetCss` |
| `appSetCss` | `gtk4.appSetCss` |
| `notificationNew` | `gtk4.notificationNew` |
| `notificationSetBody` | `gtk4.notificationSetBody` |
| `appSendNotification` | `gtk4.appSendNotification` |
| `appWithdrawNotification` | `gtk4.appWithdrawNotification` |
| `layoutManagerNew` | `gtk4.layoutManagerNew` |
| `widgetSetLayoutManager` | `gtk4.widgetSetLayoutManager` |
| `osOpenUri` | `gtk4.osOpenUri` |
| `osShowInFileManager` | `gtk4.osShowInFileManager` |
| `osSetBadgeCount` | `gtk4.osSetBadgeCount` |
| `osThemePreference` | `gtk4.osThemePreference` |

## Example

```aivi
use aivi
use aivi.ui.gtk4

main = do Effect {
  init Unit
  appId <- appNew "com.example.counter"
  win <- windowNew appId "Counter" 640 480
  root <- boxNew 1 8
  title <- labelNew "Mailfox"
  boxAppend root title
  windowSetChild win root
  windowPresent win
  appRun appId
}
```

## UI update pattern (state machine + events + repaint)

You can drive GTK updates from an AIVI model/update loop:

1. represent UI state as a model value,
2. model valid transitions with `machine`,
3. convert GTK input into `Msg`,
4. call `drawAreaQueueDraw` (or widget setters) when state changes.

```aivi
module user.mailfox

use aivi
use aivi.mutableMap
use aivi.ui.gtk4

export main

main = do Effect {
  init Unit
  appId <- appNew "com.example.counter"
  win   <- windowNew appId "Example" 800 600
  root  <- boxNew 1 8
  title <- labelNew "Example"
  boxAppend root title
  windowSetChild win root
  windowPresent win
  appRun appId
}
```

For non-canvas widgets, do the same model/update step but call setters directly (`labelSetText`, `entrySetText`, `widgetSetCss`, etc.) instead of `drawAreaQueueDraw`.

## Compatibility

`widgetSetCss` and `appSetCss` accept AIVI style records (`{ }`) so your existing `aivi.ui`/`aivi.ui.layout` CSS-style values can be reused with GTK widgets/app styling.
