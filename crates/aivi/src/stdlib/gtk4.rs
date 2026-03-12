pub const MODULE_NAME: &str = "aivi.ui.gtk4";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.ui.gtk4
export AppId, WindowId, WidgetId, BoxId, ButtonId, LabelId, EntryId, ScrollAreaId, DrawAreaId, DragSourceId, DropTargetId, MenuModelId, MenuButtonId, DialogId, FileDialogId, ImageId, ListStoreId, ListViewId, TreeViewId, GestureClickId, ClipboardId, ActionId, ShortcutId, NotificationId, LayoutManagerId, OverlayId, SeparatorId, GtkError
export GtkBindingHandle
export GtkNode, GtkAttr
export GtkElement, GtkTextNode, GtkBoundText, GtkShowNode, GtkEachNode
export GtkStaticAttr, GtkBoundAttr, GtkStaticProp, GtkBoundProp, GtkEventProp, GtkIdAttr, GtkRefAttr
export GtkSignalEvent, GtkClicked, GtkInputChanged, GtkActivated, GtkToggled, GtkValueChanged, GtkKeyPressed, GtkFocusIn, GtkFocusOut, GtkWindowClosed, GtkUnknownSignal, GtkTick
export init, appNew, appRun
export windowNew, windowSetTitle, windowSetTitlebar, windowSetChild, windowPresent, windowClose, windowOnClose, windowSetHideOnClose, windowSetDecorated
export widgetShow, widgetHide
export widgetSetSizeRequest, widgetSetHexpand, widgetSetVexpand
export widgetSetHalign, widgetSetValign
export widgetSetMarginStart, widgetSetMarginEnd, widgetSetMarginTop, widgetSetMarginBottom
export widgetAddCssClass, widgetRemoveCssClass, widgetSetTooltipText, widgetSetOpacity
export boxNew, boxAppend, boxSetHomogeneous
export buttonNew, buttonSetLabel, buttonNewFromIconName, buttonSetChild
export labelNew, labelSetText, labelSetWrap, labelSetEllipsize, labelSetXalign, labelSetMaxWidthChars
export entryNew, entrySetText, entryText
export scrollAreaNew, scrollAreaSetChild, scrollAreaSetPolicy
export separatorNew
export overlayNew, overlaySetChild, overlayAddOverlay
export drawAreaNew, drawAreaSetContentSize, drawAreaQueueDraw
export widgetSetCss, appSetCss
export imageNewFromFile, imageSetFile, imageNewFromResource, imageSetResource, imageNewFromIconName, imageSetPixelSize
export iconThemeAddSearchPath
export TrayIconId, trayIconNew, trayIconSetTooltip, trayIconSetVisible, trayIconSetMenuItems
export dragSourceNew, dragSourceSetText
export dropTargetNew, dropTargetLastText
export menuModelNew, menuModelAppendItem, menuButtonNew, menuButtonSetMenuModel
export dialogNew, dialogSetTitle, dialogSetChild, dialogPresent, dialogClose, adwDialogPresent, signalBindDialogPresent
export fileDialogNew, fileDialogSelectFile
export listStoreNew, listStoreAppendText, listStoreItems
export listViewNew, listViewSetModel
export treeViewNew, treeViewSetModel
export gestureClickNew, gestureClickLastButton, widgetAddController
export clipboardDefault, clipboardSetText, clipboardText
export actionNew, actionSetEnabled, appAddAction
export shortcutNew, widgetAddShortcut
export notificationNew, notificationSetBody, appSendNotification, appWithdrawNotification
export layoutManagerNew, widgetSetLayoutManager
export osOpenUri, osShowInFileManager, osSetBadgeCount, osThemePreference
export gtkElement, gtkTextNode, gtkBoundText, gtkShow, gtkEach, gtkEachKeyed
export gtkStaticAttr, gtkBoundAttr, gtkStaticProp, gtkBoundProp, gtkEventAttr, gtkIdAttr, gtkRefAttr
export buildFromNode, buildWithIds, reconcileNode
export signalPoll, signalEmit, signalStream, dbusServerStart
export widgetById, widgetGetBoolProperty, widgetSetBoolProperty
export trayNotifyPersonalEmail, traySetEmailSuggestions
export gtkSetInterval

use aivi

AppId = Int
WindowId = Int
WidgetId = Int
BoxId = Int
ButtonId = Int
LabelId = Int
EntryId = Int
ScrollAreaId = Int
DrawAreaId = Int
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
OverlayId = Int
SeparatorId = Int
GtkError = Text

GtkBindingHandle = Int

GtkNode =
  | GtkElement Text (List GtkAttr) (List GtkNode)
  | GtkTextNode Text
  | GtkBoundText GtkBindingHandle
  | GtkShowNode GtkBindingHandle GtkNode
  | GtkEachNode GtkBindingHandle GtkBindingHandle (Option GtkBindingHandle)

GtkAttr =
  | GtkStaticAttr Text Text
  | GtkBoundAttr Text GtkBindingHandle
  | GtkStaticProp Text Text
  | GtkBoundProp Text GtkBindingHandle
  | GtkEventProp Text GtkBindingHandle
  | GtkIdAttr Text
  | GtkRefAttr Text

GtkSignalEvent =
  | GtkClicked       WidgetId Text
  | GtkInputChanged  WidgetId Text Text
  | GtkActivated     WidgetId Text
  | GtkToggled       WidgetId Text Bool
  | GtkValueChanged  WidgetId Text Float
  | GtkKeyPressed    WidgetId Text Text Text
  | GtkFocusIn       WidgetId Text
  | GtkFocusOut      WidgetId Text
  | GtkWindowClosed  WidgetId Text
  | GtkUnknownSignal WidgetId Text Text Text Text
  | GtkTick

gtkElement : Text -> List GtkAttr -> List GtkNode -> GtkNode
gtkElement = tag => attrs => children => GtkElement tag attrs children

gtkTextNode : Text -> GtkNode
gtkTextNode = t => GtkTextNode t

gtkBoundText : a -> GtkNode
gtkBoundText = value => GtkBoundText (gtk4.captureBinding value)

gtkShow : a -> GtkNode -> GtkNode
gtkShow = condition => childNode => GtkShowNode (gtk4.captureBinding condition) childNode

gtkEach : a -> (b -> GtkNode) -> GtkNode
gtkEach = items => template =>
  GtkEachNode (gtk4.captureBinding items) (gtk4.captureBinding template) None

gtkEachKeyed : a -> (b -> key) -> (b -> GtkNode) -> GtkNode
gtkEachKeyed = items => keyFn => template => {
  base = gtkEach items template
  base match
    | GtkEachNode itemsHandle templateHandle _ =>
        GtkEachNode itemsHandle templateHandle (Some (gtk4.captureBinding keyFn))
    | _ => base
}

gtkStaticAttr : Text -> a -> GtkAttr
gtkStaticAttr = name => value => GtkStaticAttr name (gtk4.serializeAttr value)

gtkBoundAttr : Text -> a -> GtkAttr
gtkBoundAttr = name => value => GtkBoundAttr name (gtk4.captureBinding value)

gtkStaticProp : Text -> a -> GtkAttr
gtkStaticProp = name => value => GtkStaticProp name (gtk4.serializeAttr value)

gtkBoundProp : Text -> a -> GtkAttr
gtkBoundProp = name => value => GtkBoundProp name (gtk4.captureBinding value)

gtkEventAttr : Text -> a -> GtkAttr
gtkEventAttr = name => value => GtkEventProp name (gtk4.captureBinding value)

gtkIdAttr : Text -> GtkAttr
gtkIdAttr = name => GtkIdAttr name

gtkRefAttr : Text -> GtkAttr
gtkRefAttr = name => GtkRefAttr name

buildFromNode : GtkNode -> Effect GtkError WidgetId
buildFromNode = gtk4.buildFromNode

buildWithIds : GtkNode -> Effect GtkError { root: WidgetId, widgets: Map Text WidgetId }
buildWithIds = gtk4.buildWithIds

reconcileNode : WidgetId -> GtkNode -> Effect GtkError WidgetId
reconcileNode = gtk4.reconcileNode

signalPoll : Unit -> Effect GtkError (Option GtkSignalEvent)
signalPoll = gtk4.signalPoll

signalStream : Unit -> Effect GtkError (Recv GtkSignalEvent)
signalStream = gtk4.signalStream

dbusServerStart : Unit -> Effect GtkError Unit
dbusServerStart = gtk4.dbusServerStart

signalEmit : WidgetId -> Text -> Text -> Text -> Effect GtkError Unit
signalEmit = gtk4.signalEmit

@deprecated "prefer direct callbacks, Event handles, or signalStream; gtkSetInterval is a low-level timer escape hatch"
gtkSetInterval : Int -> Effect GtkError Unit
gtkSetInterval = gtk4.setInterval

widgetById : Text -> Effect GtkError WidgetId
widgetById = gtk4.widgetById

widgetGetBoolProperty : WidgetId -> Text -> Effect GtkError Bool
widgetGetBoolProperty = gtk4.widgetGetBoolProperty

widgetSetBoolProperty : WidgetId -> Text -> Bool -> Effect GtkError Unit
widgetSetBoolProperty = gtk4.widgetSetBoolProperty

init : Unit -> Effect GtkError Unit
init = gtk4.init

appNew : Text -> Effect GtkError AppId
appNew = gtk4.appNew

windowNew : AppId -> Text -> Int -> Int -> Effect GtkError WindowId
windowNew = gtk4.windowNew

windowSetTitle : WindowId -> Text -> Effect GtkError Unit
windowSetTitle = gtk4.windowSetTitle

windowSetTitlebar : WindowId -> WidgetId -> Effect GtkError Unit
windowSetTitlebar = gtk4.windowSetTitlebar

windowSetChild : WindowId -> WidgetId -> Effect GtkError Unit
windowSetChild = gtk4.windowSetChild

windowPresent : WindowId -> Effect GtkError Unit
windowPresent = gtk4.windowPresent

windowClose : WindowId -> Effect GtkError Unit
windowClose = gtk4.windowClose

windowOnClose : WindowId -> Text -> Effect GtkError Unit
windowOnClose = gtk4.windowOnClose

windowSetHideOnClose : WindowId -> Bool -> Effect GtkError Unit
windowSetHideOnClose = gtk4.windowSetHideOnClose

windowSetDecorated : WindowId -> Bool -> Effect GtkError Unit
windowSetDecorated = gtk4.windowSetDecorated

appRun : AppId -> Effect GtkError Unit
appRun = gtk4.appRun

widgetShow : WidgetId -> Effect GtkError Unit
widgetShow = gtk4.widgetShow

widgetHide : WidgetId -> Effect GtkError Unit
widgetHide = gtk4.widgetHide

widgetSetSizeRequest : WidgetId -> Int -> Int -> Effect GtkError Unit
widgetSetSizeRequest = gtk4.widgetSetSizeRequest

widgetSetHexpand : WidgetId -> Bool -> Effect GtkError Unit
widgetSetHexpand = gtk4.widgetSetHexpand

widgetSetVexpand : WidgetId -> Bool -> Effect GtkError Unit
widgetSetVexpand = gtk4.widgetSetVexpand

widgetSetHalign : WidgetId -> Int -> Effect GtkError Unit
widgetSetHalign = gtk4.widgetSetHalign

widgetSetValign : WidgetId -> Int -> Effect GtkError Unit
widgetSetValign = gtk4.widgetSetValign

widgetSetMarginStart : WidgetId -> Int -> Effect GtkError Unit
widgetSetMarginStart = gtk4.widgetSetMarginStart

widgetSetMarginEnd : WidgetId -> Int -> Effect GtkError Unit
widgetSetMarginEnd = gtk4.widgetSetMarginEnd

widgetSetMarginTop : WidgetId -> Int -> Effect GtkError Unit
widgetSetMarginTop = gtk4.widgetSetMarginTop

widgetSetMarginBottom : WidgetId -> Int -> Effect GtkError Unit
widgetSetMarginBottom = gtk4.widgetSetMarginBottom

widgetAddCssClass : WidgetId -> Text -> Effect GtkError Unit
widgetAddCssClass = gtk4.widgetAddCssClass

widgetRemoveCssClass : WidgetId -> Text -> Effect GtkError Unit
widgetRemoveCssClass = gtk4.widgetRemoveCssClass

widgetSetTooltipText : WidgetId -> Text -> Effect GtkError Unit
widgetSetTooltipText = gtk4.widgetSetTooltipText

widgetSetOpacity : WidgetId -> Int -> Effect GtkError Unit
widgetSetOpacity = gtk4.widgetSetOpacity

boxNew : Int -> Int -> Effect GtkError BoxId
boxNew = gtk4.boxNew

boxAppend : BoxId -> WidgetId -> Effect GtkError Unit
boxAppend = gtk4.boxAppend

boxSetHomogeneous : BoxId -> Bool -> Effect GtkError Unit
boxSetHomogeneous = gtk4.boxSetHomogeneous

buttonNew : Text -> Effect GtkError ButtonId
buttonNew = gtk4.buttonNew

buttonSetLabel : ButtonId -> Text -> Effect GtkError Unit
buttonSetLabel = gtk4.buttonSetLabel

buttonNewFromIconName : Text -> Effect GtkError ButtonId
buttonNewFromIconName = gtk4.buttonNewFromIconName

buttonSetChild : ButtonId -> WidgetId -> Effect GtkError Unit
buttonSetChild = gtk4.buttonSetChild

labelNew : Text -> Effect GtkError LabelId
labelNew = gtk4.labelNew

labelSetText : LabelId -> Text -> Effect GtkError Unit
labelSetText = gtk4.labelSetText

labelSetWrap : LabelId -> Bool -> Effect GtkError Unit
labelSetWrap = gtk4.labelSetWrap

labelSetEllipsize : LabelId -> Int -> Effect GtkError Unit
labelSetEllipsize = gtk4.labelSetEllipsize

labelSetXalign : LabelId -> Int -> Effect GtkError Unit
labelSetXalign = gtk4.labelSetXalign

labelSetMaxWidthChars : LabelId -> Int -> Effect GtkError Unit
labelSetMaxWidthChars = gtk4.labelSetMaxWidthChars

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

scrollAreaSetPolicy : ScrollAreaId -> Int -> Int -> Effect GtkError Unit
scrollAreaSetPolicy = gtk4.scrollAreaSetPolicy

separatorNew : Int -> Effect GtkError SeparatorId
separatorNew = gtk4.separatorNew

overlayNew : Unit -> Effect GtkError OverlayId
overlayNew = gtk4.overlayNew

overlaySetChild : OverlayId -> WidgetId -> Effect GtkError Unit
overlaySetChild = gtk4.overlaySetChild

overlayAddOverlay : OverlayId -> WidgetId -> Effect GtkError Unit
overlayAddOverlay = gtk4.overlayAddOverlay

drawAreaNew : Int -> Int -> Effect GtkError DrawAreaId
drawAreaNew = gtk4.drawAreaNew

drawAreaSetContentSize : DrawAreaId -> Int -> Int -> Effect GtkError Unit
drawAreaSetContentSize = gtk4.drawAreaSetContentSize

drawAreaQueueDraw : DrawAreaId -> Effect GtkError Unit
drawAreaQueueDraw = gtk4.drawAreaQueueDraw

widgetSetCss : WidgetId -> { } -> Effect GtkError Unit
widgetSetCss = gtk4.widgetSetCss

appSetCss : AppId -> Text -> Effect GtkError Unit
appSetCss = gtk4.appSetCss

TrayIconId = Int

trayIconNew : Text -> Text -> Effect GtkError TrayIconId
trayIconNew = gtk4.trayIconNew

trayIconSetTooltip : TrayIconId -> Text -> Effect GtkError Unit
trayIconSetTooltip = gtk4.trayIconSetTooltip

trayIconSetVisible : TrayIconId -> Bool -> Effect GtkError Unit
trayIconSetVisible = gtk4.trayIconSetVisible

trayIconSetMenuItems : TrayIconId -> List { label: Text, action: Text } -> Effect GtkError Unit
trayIconSetMenuItems = gtk4.trayIconSetMenuItems

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

dialogPresent : DialogId -> WindowId -> Effect GtkError Unit
dialogPresent = gtk4.dialogPresent

dialogClose : DialogId -> Effect GtkError Unit
dialogClose = gtk4.dialogClose

adwDialogPresent : WidgetId -> WindowId -> Effect GtkError Unit
adwDialogPresent = gtk4.adwDialogPresent

signalBindDialogPresent : Text -> WidgetId -> WindowId -> Effect GtkError Unit
signalBindDialogPresent = gtk4.signalBindDialogPresent

fileDialogNew : Unit -> Effect GtkError FileDialogId
fileDialogNew = gtk4.fileDialogNew

fileDialogSelectFile : FileDialogId -> Effect GtkError Text
fileDialogSelectFile = gtk4.fileDialogSelectFile

imageNewFromFile : Text -> Effect GtkError ImageId
imageNewFromFile = gtk4.imageNewFromFile

imageSetFile : ImageId -> Text -> Effect GtkError Unit
imageSetFile = gtk4.imageSetFile

imageNewFromResource : Text -> Effect GtkError ImageId
imageNewFromResource = gtk4.imageNewFromResource

imageSetResource : ImageId -> Text -> Effect GtkError Unit
imageSetResource = gtk4.imageSetResource

imageNewFromIconName : Text -> Effect GtkError ImageId
imageNewFromIconName = gtk4.imageNewFromIconName

imageSetPixelSize : ImageId -> Int -> Effect GtkError Unit
imageSetPixelSize = gtk4.imageSetPixelSize

iconThemeAddSearchPath : Text -> Effect GtkError Unit
iconThemeAddSearchPath = gtk4.iconThemeAddSearchPath

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

notificationNew : Text -> Text -> Effect GtkError NotificationId
notificationNew = gtk4.notificationNew

notificationSetBody : NotificationId -> Text -> Effect GtkError Unit
notificationSetBody = gtk4.notificationSetBody

appSendNotification : AppId -> Text -> NotificationId -> Effect GtkError Unit
appSendNotification = gtk4.appSendNotification

appWithdrawNotification : AppId -> Text -> Effect GtkError Unit
appWithdrawNotification = gtk4.appWithdrawNotification

layoutManagerNew : Text -> Effect GtkError LayoutManagerId
layoutManagerNew = gtk4.layoutManagerNew

widgetSetLayoutManager : WidgetId -> LayoutManagerId -> Effect GtkError Unit
widgetSetLayoutManager = gtk4.widgetSetLayoutManager

osOpenUri : AppId -> Text -> Effect GtkError Unit
osOpenUri = gtk4.osOpenUri

osShowInFileManager : Text -> Effect GtkError Unit
osShowInFileManager = gtk4.osShowInFileManager

osSetBadgeCount : AppId -> Int -> Effect GtkError Unit
osSetBadgeCount = gtk4.osSetBadgeCount

osThemePreference : Unit -> Effect GtkError Text
osThemePreference = gtk4.osThemePreference

trayNotifyPersonalEmail : Text -> Text -> Text -> Text -> Effect GtkError Unit
trayNotifyPersonalEmail = gtk4.trayNotifyPersonalEmail

traySetEmailSuggestions : List Text -> Effect GtkError Unit
traySetEmailSuggestions = gtk4.traySetEmailSuggestions

"#;
