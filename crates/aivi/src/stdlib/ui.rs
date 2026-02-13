pub const MODULE_NAME: &str = "aivi.ui";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.ui
export VNode, Attr, PatchOp, Event, LiveConfig, LiveError
export Element, TextNode, Keyed
export Class, Id, Style, OnClick, OnInput
export Replace, SetText, SetAttr, RemoveAttr
export Click, Input
export vElement, vText, vKeyed
export vClass, vId, vStyle, vAttr, vOnClick, vOnInput
export renderHtml, diff, patchToJson, eventFromJson
export live

use aivi

// A typed Virtual DOM. Rendering is backend/runtime-specific.
type VNode msg = Element Text (List (Attr msg)) (List (VNode msg)) | TextNode Text | Keyed Text (VNode msg)

type Attr msg = Class Text | Id Text | Style { } | OnClick msg | OnInput (Text -> msg) | Attr Text Text

// Helpers for tooling/lowerings. These avoid common names like `id` or `style`,
// which are likely to appear in user code and other stdlib modules.
vElement : Text -> List (Attr msg) -> List (VNode msg) -> VNode msg
vElement tag attrs children = Element tag attrs children

vText : Text -> VNode msg
vText t = TextNode t

vKeyed : Text -> VNode msg -> VNode msg
vKeyed key node = Keyed key node

vClass : Text -> Attr msg
vClass t = Class t

vId : Text -> Attr msg
vId t = Id t

vStyle : { } -> Attr msg
vStyle css = Style css

vAttr : Text -> Text -> Attr msg
vAttr k v = Attr k v

vOnClick : msg -> Attr msg
vOnClick msg = OnClick msg

vOnInput : (Text -> msg) -> Attr msg
vOnInput f = OnInput f

// Patch operations for LiveView-like updates.
type PatchOp = Replace Text Text | SetText Text Text | SetAttr Text Text Text | RemoveAttr Text Text

type Event = Click Int | Input Int Text

type LiveConfig = { address: Text, path: Text, title: Text }
type LiveError = { message: Text }

renderHtml : VNode msg -> Text
renderHtml node = ui.renderHtml node

diff : VNode msg -> VNode msg -> List PatchOp
diff old new = ui.diff old new

patchToJson : List PatchOp -> Text
patchToJson ops = ui.patchToJson ops

eventFromJson : Text -> Result LiveError Event
eventFromJson text = ui.eventFromJson text

// Live server: serves initial HTML and streams patches over WebSocket.
// The client protocol is implemented by the runtime's embedded JS snippet.
live : LiveConfig -> model -> (model -> VNode msg) -> (msg -> model -> model) -> Effect LiveError Server
live cfg initialModel view update = ui.live cfg initialModel view update
"#;
