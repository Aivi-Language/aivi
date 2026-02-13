pub const MODULE_NAME: &str = "aivi.ui";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.ui
export VNode, Attr, PatchOp, Event, LiveConfig, LiveError
export element, text, keyed
export className, id, style, attr, onClick, onInput
export renderHtml, diff, patchToJson, eventFromJson
export live

use aivi

// A typed Virtual DOM. Rendering is backend/runtime-specific.
VNode msg = Element Text (List (Attr msg)) (List (VNode msg)) | TextNode Text | Keyed Text (VNode msg)

Attr msg = Class Text | Id Text | Style { } | OnClick msg | OnInput (Text -> msg) | Attr Text Text

element : Text -> List (Attr msg) -> List (VNode msg) -> VNode msg
element tag attrs children = Element tag attrs children

text : Text -> VNode msg
text t = TextNode t

keyed : Text -> VNode msg -> VNode msg
keyed key node = Keyed key node

className : Text -> Attr msg
className t = Class t

id : Text -> Attr msg
id t = Id t

style : { } -> Attr msg
style css = Style css

attr : Text -> Text -> Attr msg
attr k v = Attr k v

onClick : msg -> Attr msg
onClick msg = OnClick msg

onInput : (Text -> msg) -> Attr msg
onInput f = OnInput f

// Patch operations for LiveView-like updates.
PatchOp = Replace Text Text | SetText Text Text | SetAttr Text Text Text | RemoveAttr Text Text

Event = Click Int | Input Int Text

LiveConfig = { address: Text, path: Text, title: Text }
LiveError = { message: Text }

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
