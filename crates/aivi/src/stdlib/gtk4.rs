pub const MODULE_NAME: &str = "aivi.ui.gtk4";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.ui.gtk4
export AppId, WindowId, WidgetId, BoxId, ButtonId, LabelId, EntryId, ScrollAreaId, DrawAreaId, MenuModelId, ImageId, GestureClickId, ActionId, OverlayId, SeparatorId, GtkError
export GtkBindingHandle
export GtkNode, GtkAttr
export GtkElement, GtkTextNode, GtkBoundText, GtkShowNode, GtkEachNode
export GtkStaticAttr, GtkBoundAttr, GtkStaticProp, GtkBoundProp, GtkEventProp, GtkEventSugarProp, GtkIdAttr, GtkRefAttr
export GtkSignalEvent, GtkClicked, GtkInputChanged, GtkActivated, GtkToggled, GtkValueChanged, GtkKeyPressed, GtkFocusIn, GtkFocusOut, GtkWindowClosed, GtkUnknownSignal, GtkTick
export init, appNew, appRun
export windowNew, windowSetTitle, windowSetTitlebar, windowSetChild, windowPresent, windowClose, windowOnClose, windowSetHideOnClose, windowSetDecorated, displayHeight
export mountAppWindow, runGtkApp
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
export menuModelNew, menuModelAppendItem, menuButtonSetMenuModel
export gestureClickNew, gestureClickLastButton, widgetAddController
export actionNew, actionSetEnabled, appAddAction
export osOpenUri
export gtkElement, gtkTextNode, gtkBoundText, gtkShow, gtkEach, gtkEachKeyed
export gtkStaticAttr, gtkBoundAttr, gtkStaticProp, gtkBoundProp, gtkEventAttr, gtkEventSugarAttr, gtkIdAttr, gtkRefAttr
export buildFromNode, buildWithIds, reconcileNode
export signalPoll, signalEmit, signalStream
export widgetById, widgetGetBoolProperty, widgetGetCalendarDate, widgetSetBoolProperty, widgetSetCalendarDate
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
MenuModelId = Int
ImageId = Int
GestureClickId = Int
ActionId = Int
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
  | GtkEventSugarProp Text Text GtkBindingHandle
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

gtkEventSugarAttr : Text -> Text -> a -> GtkAttr
gtkEventSugarAttr = name => source => value => GtkEventSugarProp name source (gtk4.captureBinding value)

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
signalEmit : WidgetId -> Text -> Text -> Text -> Effect GtkError Unit
signalEmit = gtk4.signalEmit

@deprecated "prefer direct callbacks, Event handles, or signalStream; gtkSetInterval is a low-level timer escape hatch"
gtkSetInterval : Int -> Effect GtkError Unit
gtkSetInterval = gtk4.setInterval

widgetById : Text -> Effect GtkError WidgetId
widgetById = gtk4.widgetById

widgetGetBoolProperty : WidgetId -> Text -> Effect GtkError Bool
widgetGetBoolProperty = gtk4.widgetGetBoolProperty

widgetGetCalendarDate : WidgetId -> Effect GtkError Text
widgetGetCalendarDate = gtk4.widgetGetCalendarDate

widgetSetBoolProperty : WidgetId -> Text -> Bool -> Effect GtkError Unit
widgetSetBoolProperty = gtk4.widgetSetBoolProperty

widgetSetCalendarDate : WidgetId -> Text -> Effect GtkError Unit
widgetSetCalendarDate = gtk4.widgetSetCalendarDate

init : Unit -> Effect GtkError Unit
init = gtk4.init

appNew : Text -> Effect GtkError AppId
appNew = gtk4.appNew

windowNew : AppId -> Text -> Int -> Int -> Effect GtkError WindowId
windowNew = gtk4.windowNew

mountAppWindow : AppId -> List GtkNode -> Effect GtkError WindowId
mountAppWindow = gtk4.mountAppWindow

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

displayHeight : Unit -> Effect GtkError Int
displayHeight = gtk4.displayHeight

appRun : AppId -> Effect GtkError Unit
appRun = gtk4.appRun

runGtkApp : { appId: Text, root: GtkNode, onStart: Effect GtkError Unit } -> Effect GtkError Unit
runGtkApp = config =>
  &|> init Unit
  &|> appNew config.appId #app
  &|> mountAppWindow app [config.root] #win
  &|> config.onStart
  &|> windowPresent win
  &|> appRun app

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

menuModelNew : Unit -> Effect GtkError MenuModelId
menuModelNew = gtk4.menuModelNew

menuModelAppendItem : MenuModelId -> Text -> Text -> Effect GtkError Unit
menuModelAppendItem = gtk4.menuModelAppendItem

menuButtonSetMenuModel : WidgetId -> MenuModelId -> Effect GtkError Unit
menuButtonSetMenuModel = gtk4.menuButtonSetMenuModel

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

gestureClickNew : WidgetId -> Effect GtkError GestureClickId
gestureClickNew = gtk4.gestureClickNew

gestureClickLastButton : GestureClickId -> Effect GtkError Int
gestureClickLastButton = gtk4.gestureClickLastButton

widgetAddController : WidgetId -> GestureClickId -> Effect GtkError Unit
widgetAddController = gtk4.widgetAddController

actionNew : Text -> Effect GtkError ActionId
actionNew = gtk4.actionNew

actionSetEnabled : ActionId -> Bool -> Effect GtkError Unit
actionSetEnabled = gtk4.actionSetEnabled

appAddAction : AppId -> ActionId -> Effect GtkError Unit
appAddAction = gtk4.appAddAction

osOpenUri : Text -> Effect GtkError Unit
osOpenUri = uri => gtk4.osOpenUri uri

"#;
