pub const MODULE_NAME: &str = "aivi.ui";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.ui
export VNode, Attr, PatchOp, Event, LiveConfig, LiveError
export Element, TextNode, Keyed
export Class, Id, Style, OnClick, OnInput
export ClickEvent, InputEvent, KeyboardEvent, PointerEvent
export OnClickE, OnInputE, OnKeyDown, OnKeyUp
export OnPointerDown, OnPointerUp, OnPointerMove
export OnFocus, OnBlur
export Replace, SetText, SetAttr, RemoveAttr
export Click, Input
export vElement, vText, vKeyed
export vClass, vId, vStyle, vAttr, vOnClick, vOnInput
export vOnClickE, vOnInputE, vOnKeyDown, vOnKeyUp
export vOnPointerDown, vOnPointerUp, vOnPointerMove
export vOnFocus, vOnBlur
export renderHtml, diff, patchToJson, eventFromJson
export live

use aivi

// A typed Virtual DOM. Rendering is backend/runtime-specific.
VNode msg = Element Text (List (Attr msg)) (List (VNode msg)) | TextNode Text | Keyed Text (VNode msg)

// Typed UI event payload records (used by server-driven runtimes like `aivi.ui.ServerHtml`).
ClickEvent = { button: Int, alt: Bool, ctrl: Bool, shift: Bool, meta: Bool }

InputEvent = { value: Text }

KeyboardEvent =
  { key: Text, code: Text, alt: Bool, ctrl: Bool, shift: Bool, meta: Bool, repeat: Bool, isComposing: Bool }

PointerEvent =
  { pointerId: Int, pointerType: Text, button: Int, buttons: Int, clientX: Float, clientY: Float, alt: Bool, ctrl: Bool, shift: Bool, meta: Bool }

Attr msg =
  Class Text
  | Id Text
  | Style { }
  | OnClick msg
  | OnInput (Text -> msg)
  | OnClickE (ClickEvent -> msg)
  | OnInputE (InputEvent -> msg)
  | OnKeyDown (KeyboardEvent -> msg)
  | OnKeyUp (KeyboardEvent -> msg)
  | OnPointerDown (PointerEvent -> msg)
  | OnPointerUp (PointerEvent -> msg)
  | OnPointerMove (PointerEvent -> msg)
  | OnFocus msg
  | OnBlur msg
  | Attr Text Text

// Helpers for tooling/lowerings. These avoid common names like `id` or `style`,
// which are likely to appear in user code and other stdlib modules.
vElement : Text -> List (Attr msg) -> List (VNode msg) -> VNode msg
vElement = tag attrs children => Element tag attrs children

vText : Text -> VNode msg
vText = t => TextNode t

vKeyed : Text -> VNode msg -> VNode msg
vKeyed = key node => Keyed key node

vClass : Text -> Attr msg
vClass = t => Class t

vId : Text -> Attr msg
vId = t => Id t

vStyle : { } -> Attr msg
vStyle = css => Style css

vAttr : Text -> Text -> Attr msg
vAttr = k v => Attr k v

vOnClick : msg -> Attr msg
vOnClick = msg => OnClick msg

vOnInput : (Text -> msg) -> Attr msg
vOnInput = f => OnInput f

vOnClickE : (ClickEvent -> msg) -> Attr msg
vOnClickE = f => OnClickE f

vOnInputE : (InputEvent -> msg) -> Attr msg
vOnInputE = f => OnInputE f

vOnKeyDown : (KeyboardEvent -> msg) -> Attr msg
vOnKeyDown = f => OnKeyDown f

vOnKeyUp : (KeyboardEvent -> msg) -> Attr msg
vOnKeyUp = f => OnKeyUp f

vOnPointerDown : (PointerEvent -> msg) -> Attr msg
vOnPointerDown = f => OnPointerDown f

vOnPointerUp : (PointerEvent -> msg) -> Attr msg
vOnPointerUp = f => OnPointerUp f

vOnPointerMove : (PointerEvent -> msg) -> Attr msg
vOnPointerMove = f => OnPointerMove f

vOnFocus : msg -> Attr msg
vOnFocus = msg => OnFocus msg

vOnBlur : msg -> Attr msg
vOnBlur = msg => OnBlur msg

// Patch operations for LiveView-like updates.
PatchOp = Replace Text Text | SetText Text Text | SetAttr Text Text Text | RemoveAttr Text Text

Event = Click Int | Input Int Text

LiveConfig = { address: Text, path: Text, title: Text }
LiveError = { message: Text }

renderHtml : VNode msg -> Text
renderHtml = node => ui.renderHtml node

diff : VNode msg -> VNode msg -> List PatchOp
diff = old new => ui.diff old new

patchToJson : List PatchOp -> Text
patchToJson = ops => ui.patchToJson ops

eventFromJson : Text -> Result LiveError Event
eventFromJson = text => ui.eventFromJson text

// Live server: serves initial HTML and streams patches over WebSocket.
// The client protocol is implemented by the runtime's embedded JS snippet.
live : LiveConfig -> model -> (model -> VNode msg) -> (msg -> model -> model) -> Effect LiveError Server
live = cfg initialModel view update => ui.live cfg initialModel view update
"#;
