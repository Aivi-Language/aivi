pub const MODULE_NAME: &str = "aivi.ui.gtk4";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.ui.gtk4
export AppId, WindowId, WidgetId, BoxId, ButtonId, LabelId, EntryId, ScrollAreaId, DrawAreaId, DragSourceId, DropTargetId, MenuModelId, MenuButtonId, DialogId, FileDialogId, ImageId, ListStoreId, ListViewId, TreeViewId, GestureClickId, ClipboardId, ActionId, ShortcutId, NotificationId, LayoutManagerId, OverlayId, SeparatorId, GtkError
export GtkNode, GtkAttr, GtkElement, GtkTextNode, GtkAttribute
export GtkSignalEvent, GtkClicked, GtkInputChanged, GtkActivated, GtkToggled, GtkValueChanged, GtkKeyPressed, GtkFocusIn, GtkFocusOut, GtkUnknownSignal, GtkTick
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
export dialogNew, dialogSetTitle, dialogSetChild, dialogPresent, dialogClose, adwDialogPresent
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
export gtkElement, gtkTextNode, gtkAttr, gtkSignalAttr, gtkEachItems
export buildFromNode, buildWithIds, reconcileNode
export signalPoll, signalEmit, signalStream, dbusServerStart
export widgetById, widgetSetBoolProperty, signalBindBoolProperty, signalBindCssClass, signalBindToggleBoolProperty, signalToggleCssClass
export signalBindDialogPresent, signalBindStackPage
export trayNotifyPersonalEmail, traySetEmailSuggestions
export CommandKey, SubscriptionKey, AppStep
export Command, CommandNone, CommandBatch, CommandEmit, CommandPerform, CommandAfter, CommandCancel
export Subscription, SubscriptionNone, SubscriptionBatch, SubscriptionEvery, SubscriptionSource
export appStep, appStepWith, noSubscriptions, liftAppUpdate, auto
export commandNone, commandBatch, commandEmit, commandPerform, commandAfter, commandCancel
export subscriptionNone, subscriptionBatch, subscriptionEvery, subscriptionSource
export gtkApp
export gtkSetInterval
export derive, memo, readDerived

use aivi
use aivi.concurrency as concurrent

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

GtkNode = GtkElement Text (List GtkAttr) (List GtkNode) | GtkTextNode Text

GtkAttr = GtkAttribute Text Text

GtkSignalEvent =
  | GtkClicked       WidgetId Text
  | GtkInputChanged  WidgetId Text Text
  | GtkActivated     WidgetId Text
  | GtkToggled       WidgetId Text Bool
  | GtkValueChanged  WidgetId Text Float
  | GtkKeyPressed    WidgetId Text Text Text
  | GtkFocusIn       WidgetId Text
  | GtkFocusOut      WidgetId Text
  | GtkUnknownSignal WidgetId Text Text Text Text
  | GtkTick

CommandKey = Text
SubscriptionKey = Text

HostedCommand = {
  cancel: Effect GtkError Unit
}

HostedSubscription = {
  cancel: Effect GtkError Unit
  signature: Text
  reusable: Bool
}

HostedEvent msg
  = HostedMsg msg
  | HostedCloseRequest

Command msg
  = CommandNone
  | CommandBatch (List (Command msg))
  | CommandEmit msg
  | CommandPerform {
      run: Effect GtkError msg
      onError: Option (GtkError -> msg)
    }
  | CommandAfter {
      key: CommandKey
      millis: Int
      msg: msg
    }
  | CommandCancel CommandKey

Subscription msg
  = SubscriptionNone
  | SubscriptionBatch (List (Subscription msg))
  | SubscriptionEvery {
      key: SubscriptionKey
      millis: Int
      tag: msg
    }
  | SubscriptionSource {
      key: SubscriptionKey
      open: Resource GtkError (Recv msg)
      onError: Option (GtkError -> msg)
      onClosed: Option msg
    }

AppStep model msg = {
  model: model
  commands: List (Command msg)
}

emptyHostedCommands : Map CommandKey HostedCommand
emptyHostedCommands = Map.empty

emptyHostedSubscriptions : Map SubscriptionKey HostedSubscription
emptyHostedSubscriptions = Map.empty

appStep : s -> AppStep s msg
appStep = model => { model, commands: [] }

appStepWith : s -> List (Command msg) -> AppStep s msg
appStepWith = model commands => { model, commands }

noSubscriptions : s -> List (Subscription msg)
noSubscriptions = _ => []

auto : GtkSignalEvent -> Option msg
auto = event => gtk4.autoToMsg event

memo : Text -> (model -> a) -> model -> a
memo = key deriveFn => gtk4.memo key deriveFn

derive : (model -> a) -> model -> a
derive = deriveFn => gtk4.derive deriveFn

readDerived : (model -> a) -> model -> a
readDerived = derivedValue => model => derivedValue model

commandNone : Command msg
commandNone = CommandNone

commandBatch : List (Command msg) -> Command msg
commandBatch = commands => CommandBatch commands

commandEmit : msg -> Command msg
commandEmit = msg => CommandEmit msg

commandPerform : { run: Effect GtkError msg, onError: Option (GtkError -> msg) } -> Command msg
commandPerform = spec => CommandPerform spec

commandAfter : { key: CommandKey, millis: Int, msg: msg } -> Command msg
commandAfter = spec => CommandAfter spec

commandCancel : CommandKey -> Command msg
commandCancel = key => CommandCancel key

subscriptionNone : Subscription msg
subscriptionNone = SubscriptionNone

subscriptionBatch : List (Subscription msg) -> Subscription msg
subscriptionBatch = subscriptions => SubscriptionBatch subscriptions

subscriptionEvery : { key: SubscriptionKey, millis: Int, tag: msg } -> Subscription msg
subscriptionEvery = spec => SubscriptionEvery spec

subscriptionSource : {
  key: SubscriptionKey
  open: Resource GtkError (Recv msg)
  onError: Option (GtkError -> msg)
  onClosed: Option msg
} -> Subscription msg
subscriptionSource = spec => SubscriptionSource spec

liftAppUpdate : (msg -> s -> Effect GtkError s) -> msg -> s -> Effect GtkError (AppStep s msg)
liftAppUpdate = update => msg => state => do Effect {
  next <- update msg state
  pure (appStep next)
}

emitIfSome : Sender msg -> Option msg -> Effect GtkError Unit
emitIfSome = sender maybeMsg =>
  maybeMsg match
    | None     => pure Unit
    | Some msg => concurrent.send sender msg

emitHosted : Sender (HostedEvent msg) -> msg -> Effect GtkError Unit
emitHosted = sender => msg => concurrent.send sender (HostedMsg msg)

emitIfSomeHosted : Sender (HostedEvent msg) -> Option msg -> Effect GtkError Unit
emitIfSomeHosted = sender => maybeMsg =>
  maybeMsg match
    | None     => pure Unit
    | Some msg => emitHosted sender msg

emitMappedError : Sender msg -> Option (GtkError -> msg) -> GtkError -> Effect GtkError Unit
emitMappedError = sender onError err =>
  onError match
    | None        => pure Unit
    | Some mkMsg  => concurrent.send sender (mkMsg err)

emitMappedHostedError : Sender (HostedEvent msg) -> Option (GtkError -> msg) -> GtkError -> Effect GtkError Unit
emitMappedHostedError = sender => onError => err =>
  onError match
    | None       => pure Unit
    | Some mkMsg => emitHosted sender (mkMsg err)

flattenCommands : List (Command msg) -> List (Command msg)
flattenCommands = commands =>
  commands match
    | [] => []
    | [command, ...rest] =>
        command match
          | CommandNone         => flattenCommands rest
          | CommandBatch nested => [...flattenCommands nested, ...flattenCommands rest]
          | _                   => [command, ...flattenCommands rest]

flattenSubscriptions : List (Subscription msg) -> List (Subscription msg)
flattenSubscriptions = subscriptions =>
  subscriptions match
    | [] => []
    | [subscription, ...rest] =>
        subscription match
          | SubscriptionNone         => flattenSubscriptions rest
          | SubscriptionBatch nested => [...flattenSubscriptions nested, ...flattenSubscriptions rest]
          | _                        => [subscription, ...flattenSubscriptions rest]

cancelHostedCommand : CommandKey -> Map CommandKey HostedCommand -> Effect GtkError (Map CommandKey HostedCommand)
cancelHostedCommand = key handles =>
  Map.get key handles match
    | None => pure handles
    | Some handle => do Effect {
        _ <- handle.cancel
        pure (Map.remove key handles)
      }

replaceHostedCommand : CommandKey -> HostedCommand -> Map CommandKey HostedCommand -> Effect GtkError (Map CommandKey HostedCommand)
replaceHostedCommand = key handle handles => do Effect {
  cleared <- cancelHostedCommand key handles
  pure (Map.insert key handle cleared)
}

launchCommands : Sender (HostedEvent msg) -> List (Command msg) -> Map CommandKey HostedCommand -> Effect GtkError (Map CommandKey HostedCommand)
launchCommands = sender => commands => handles =>
  commands match
    | [] => pure handles
    | [command, ...rest] => do Effect {
        nextHandles <- launchCommand sender command handles
        launchCommands sender rest nextHandles
      }

launchCommand : Sender (HostedEvent msg) -> Command msg -> Map CommandKey HostedCommand -> Effect GtkError (Map CommandKey HostedCommand)
launchCommand = sender => command => handles =>
  command match
    | CommandNone => pure handles
    | CommandBatch nested => launchCommands sender (flattenCommands nested) handles
    | CommandEmit msg => do Effect {
        _ <- emitHosted sender msg
        pure handles
      }
    | CommandPerform spec => do Effect {
        _ <- concurrent.spawn (do Effect {
          result <- attempt spec.run
          result match
            | Ok msg  => emitHosted sender msg
            | Err err => emitMappedHostedError sender spec.onError err
        })
        pure handles
      }
    | CommandAfter { key, millis, msg } => do Effect {
        task <- concurrent.spawn (do Effect {
          _ <- concurrent.sleep millis
          emitHosted sender msg
        })
        replaceHostedCommand key { cancel: task.cancel } handles
      }
    | CommandCancel key => cancelHostedCommand key handles

startEverySubscription : Sender (HostedEvent msg) -> Int -> msg -> Effect GtkError HostedSubscription
startEverySubscription = sender => millis => tag => do Effect {
  task <- concurrent.spawn (do Effect {
    loop _ = Unit => {
      _ <- concurrent.sleep millis
      _ <- emitHosted sender tag
      recurse Unit
    }
  })
  pure {
    cancel: task.cancel
    signature: "every:{millis}"
    reusable: True
  }
}

startSourceSubscription : Sender (HostedEvent msg) -> Resource GtkError (Recv msg) -> Option (GtkError -> msg) -> Option msg -> Effect GtkError HostedSubscription
startSourceSubscription = sender => open => onError => onClosed => do Effect {
  task <- concurrent.spawn (do Effect {
    receiver <- open
    result <- attempt (concurrent.forEach receiver (msg => emitHosted sender msg))
    result match
      | Ok _    => emitIfSomeHosted sender onClosed
      | Err err => emitMappedHostedError sender onError err
  })
  pure {
    cancel: task.cancel
    signature: "source"
    reusable: False
  }
}

cancelSubscriptionMap : Map SubscriptionKey HostedSubscription -> Effect GtkError Unit
cancelSubscriptionMap = subscriptions =>
  cancelSubscriptionEntries (Map.entries subscriptions)

cancelSubscriptionEntries : List (SubscriptionKey, HostedSubscription) -> Effect GtkError Unit
cancelSubscriptionEntries = entries =>
  entries match
    | [] => pure Unit
    | [(_, handle), ...rest] => do Effect {
        _ <- handle.cancel
        cancelSubscriptionEntries rest
      }

syncSubscriptions : Sender (HostedEvent msg) -> List (Subscription msg) -> Map SubscriptionKey HostedSubscription -> Effect GtkError (Map SubscriptionKey HostedSubscription)
syncSubscriptions = sender => subscriptions => current =>
  syncSubscriptionList sender (flattenSubscriptions subscriptions) current emptyHostedSubscriptions

syncSubscriptionList : Sender (HostedEvent msg) -> List (Subscription msg) -> Map SubscriptionKey HostedSubscription -> Map SubscriptionKey HostedSubscription -> Effect GtkError (Map SubscriptionKey HostedSubscription)
syncSubscriptionList = sender => subscriptions => current => next =>
  subscriptions match
    | [] => do Effect {
        _ <- cancelSubscriptionMap current
        pure next
      }
    | [subscription, ...rest] =>
        subscription match
          | SubscriptionEvery { key, millis, tag } =>
              Map.get key current match
                | None => do Effect {
                    handle <- startEverySubscription sender millis tag
                    syncSubscriptionList sender rest current (Map.insert key handle next)
                  }
                | Some handle =>
                    if handle.reusable && handle.signature == "every:{millis}"
                      then
                        syncSubscriptionList sender rest (Map.remove key current) (Map.insert key handle next)
                      else
                        do Effect {
                          _ <- handle.cancel
                          replacement <- startEverySubscription sender millis tag
                          syncSubscriptionList sender rest (Map.remove key current) (Map.insert key replacement next)
                        }
          | SubscriptionSource { key, open, onError, onClosed } =>
              Map.get key current match
                | None => do Effect {
                    handle <- startSourceSubscription sender open onError onClosed
                    syncSubscriptionList sender rest current (Map.insert key handle next)
                  }
                | Some handle =>
                    if handle.reusable && handle.signature == "source"
                      then
                        syncSubscriptionList sender rest (Map.remove key current) (Map.insert key handle next)
                      else
                        do Effect {
                          _ <- handle.cancel
                          replacement <- startSourceSubscription sender open onError onClosed
                          syncSubscriptionList sender rest (Map.remove key current) (Map.insert key replacement next)
                        }
          | SubscriptionNone =>
              syncSubscriptionList sender rest current next
          | SubscriptionBatch nested =>
              syncSubscriptionList sender [...flattenSubscriptions nested, ...rest] current next

gtkAppCloseRequested : GtkSignalEvent -> Bool
gtkAppCloseRequested = event =>
  event match
    | GtkUnknownSignal _ _ "close-request" _ _ => True
    | _                                        => False

forwardGtkMessages : Sender (HostedEvent msg) -> Recv GtkSignalEvent -> (GtkSignalEvent -> Option msg) -> Effect GtkError Unit
forwardGtkMessages = msgTx => signalRx => toMsgFn =>
  concurrent.forEach signalRx (event =>
    if gtkAppCloseRequested event
      then concurrent.send msgTx HostedCloseRequest
      else
        toMsgFn event match
          | None     => pure Unit
          | Some msg => emitHosted msgTx msg
  )

runGtkAppLoop : Sender (HostedEvent msg) -> Recv (HostedEvent msg) -> AppId -> WindowId -> s -> WidgetId -> Map CommandKey HostedCommand -> Map SubscriptionKey HostedSubscription -> (s -> GtkNode) -> (s -> List (Subscription msg)) -> (AppId -> WindowId -> msg -> s -> Effect GtkError (AppStep s msg)) -> Effect GtkError Unit
runGtkAppLoop = msgTx => msgRx => appId => win => currentModel => currentRoot => currentCommands => currentSubscriptions => viewFn => subscriptionsFn => updateFn => do Effect {
  loop state = {
    model: currentModel
    root: currentRoot
    commands: currentCommands
    subscriptions: currentSubscriptions
  } => {
    result <- concurrent.recv msgRx
    result match
      | Err _ => pure Unit
      | Ok HostedCloseRequest =>
          do Effect {
            hideOnClose <- widgetGetBoolProperty win "hide-on-close"
            if hideOnClose
              then recurse state
              else pure Unit
          }
      | Ok (HostedMsg msg) => do Effect {
          step <- updateFn appId win msg state.model
          nextCommands <- launchCommands msgTx (flattenCommands step.commands) state.commands
          if step.model == state.model
            then recurse (state <| { commands: nextCommands })
            else do Effect {
              _ <- gtk4.reactiveCommit state.model step.model
              newView = viewFn step.model
              _ <- gtk4.autoBindingsSet newView
              newRoot <- reconcileNode state.root newView
              _ <- if newRoot == state.root then pure Unit else windowSetChild win newRoot
              nextSubscriptions <- syncSubscriptions msgTx (subscriptionsFn step.model) state.subscriptions
              recurse {
                model: step.model
                root: newRoot
                commands: nextCommands
                subscriptions: nextSubscriptions
              }
            }
        }
  }
}

gtkElement : Text -> List GtkAttr -> List GtkNode -> GtkNode
gtkElement = tag attrs children => GtkElement tag attrs children

gtkTextNode : Text -> GtkNode
gtkTextNode = t => GtkTextNode t

gtkAttr : Text -> a -> GtkAttr
gtkAttr = name value => GtkAttribute name (gtk4.serializeAttr value)

gtkSignalAttr : Text -> A -> GtkAttr
gtkSignalAttr = name value => GtkAttribute name (gtk4.serializeSignal value)

gtkEachItems : a -> (b -> GtkNode) -> List GtkNode
gtkEachItems = items template => gtk4.eachItems items template

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

@deprecated "use subscriptionEvery inside gtkApp; gtkSetInterval is a low-level escape hatch"
gtkSetInterval : Int -> Effect GtkError Unit
gtkSetInterval = gtk4.setInterval

widgetById : Text -> Effect GtkError WidgetId
widgetById = gtk4.widgetById

widgetGetBoolProperty : WidgetId -> Text -> Effect GtkError Bool
widgetGetBoolProperty = gtk4.widgetGetBoolProperty

widgetSetBoolProperty : WidgetId -> Text -> Bool -> Effect GtkError Unit
widgetSetBoolProperty = gtk4.widgetSetBoolProperty

signalBindBoolProperty : Text -> WidgetId -> Text -> Bool -> Effect GtkError Unit
signalBindBoolProperty = gtk4.signalBindBoolProperty

signalBindCssClass : Text -> WidgetId -> Text -> Bool -> Effect GtkError Unit
signalBindCssClass = gtk4.signalBindCssClass

signalBindToggleBoolProperty : Text -> WidgetId -> Text -> Effect GtkError Unit
signalBindToggleBoolProperty = gtk4.signalBindToggleBoolProperty

signalToggleCssClass : Text -> WidgetId -> Text -> Effect GtkError Unit
signalToggleCssClass = gtk4.signalToggleCssClass

signalBindDialogPresent : Text -> DialogId -> WindowId -> Effect GtkError Unit
signalBindDialogPresent = gtk4.signalBindDialogPresent

signalBindStackPage : Text -> WidgetId -> Text -> Effect GtkError Unit
signalBindStackPage = gtk4.signalBindStackPage

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

runGtkAppHost : {
  id: Text
  title: Text
  size: (Int, Int)
  decorated: Bool
  hideOnClose: Bool
  model: s
  onStart: AppId -> WindowId -> Effect GtkError Unit
  subscriptions: s -> List (Subscription msg)
  view: s -> GtkNode
  toMsg: GtkSignalEvent -> Option msg
  update: AppId -> WindowId -> msg -> s -> Effect GtkError (AppStep s msg)
} -> Effect GtkError Unit
runGtkAppHost = config =>
  concurrent.scope (_ => do Effect {
    subscriptionsFn = config.subscriptions
    viewFn = config.view
    toMsgFn = config.toMsg
    updateFn = config.update
    _ <- init Unit
    appId <- appNew config.id
    (w, h) = config.size
    win <- windowNew appId config.title w h
    _ <- windowSetDecorated win config.decorated
    _ <- windowSetHideOnClose win config.hideOnClose
    _ <- config.onStart appId win
    _ <- windowOnClose win "__gtkAppClose"
    _ <- gtk4.reactiveInit config.model
    initialView = viewFn config.model
    _ <- gtk4.autoBindingsSet initialView
    root <- buildFromNode initialView
    _ <- windowSetChild win root
    (msgTx, msgRx) <- concurrent.make Unit
    signalRx <- signalStream {}
    _ <- concurrent.spawn (forwardGtkMessages msgTx signalRx toMsgFn)
    activeSubscriptions <- syncSubscriptions msgTx (subscriptionsFn config.model) emptyHostedSubscriptions
    _ <- windowPresent win
    runGtkAppLoop msgTx msgRx appId win config.model root emptyHostedCommands activeSubscriptions viewFn subscriptionsFn updateFn
  })

gtkApp : {
  id: Text
  title: Text
  size: (Int, Int)
  model: s
  onStart: AppId -> WindowId -> Effect GtkError Unit
  subscriptions: s -> List (Subscription msg)
  view: s -> GtkNode
  toMsg: GtkSignalEvent -> Option msg
  update: msg -> s -> Effect GtkError (AppStep s msg)
} -> Effect GtkError Unit
gtkApp = config =>
  runGtkAppHost {
    id: config.id
    title: config.title
    size: config.size
    decorated: True
    hideOnClose: False
    model: config.model
    onStart: config.onStart
    subscriptions: config.subscriptions
    view: config.view
    toMsg: config.toMsg
    update: _ _ msg state => config.update msg state
  }
"#;
