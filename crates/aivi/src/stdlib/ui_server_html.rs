pub const MODULE_NAME: &str = "aivi.ui.ServerHtml";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.ui.ServerHtml
export ViewId, UrlInfo, InitContext
export PlatformEvent
export ClipboardError, Effect
export ServerHtmlApp
export serveHttp, serveWs

use aivi
use aivi.net.http_server (Request, Response, WebSocket, WsError)
use aivi.ui (VNode)

ViewId = Text

UrlInfo = { url: Text, path: Text, query: Text, hash: Text }

InitContext = { viewId: ViewId, url: UrlInfo, online: Bool }

PlatformEvent =
  PopState UrlInfo
  | HashChange { old: Text, new: Text, hash: Text, url: UrlInfo }
  | Visibility { visibilityState: Text }
  | WindowFocus { focused: Bool }
  | Online { online: Bool }
  | Intersection { sid: Int, entries: List { tid: Int, isIntersecting: Bool, ratio: Float } }

ClipboardError = { name: Text }

Effect msg =
  ClipboardReadText (Result ClipboardError Text -> msg)
  | ClipboardWriteText Text (Result ClipboardError Unit -> msg)
  | SubscribeIntersection
      { sid: Int, rootMargin: Text, threshold: List Float, targets: List { tid: Int, nodeId: Text } }
  | UnsubscribeIntersection Int

// Server-driven app contract.
ServerHtmlApp model msg =
  { init: InitContext -> model
  , update: msg -> model -> (model, List (Effect msg))
  , view: model -> VNode msg
  , onPlatform: PlatformEvent -> Option msg
  }

// Render initial HTML + boot data, allocating a fresh ViewId.
serveHttp : ServerHtmlApp model msg -> Request -> Response
serveHttp = app req => ui.ServerHtml.serveHttp app req

// WebSocket session handler (expects a `hello` message first).
serveWs : ServerHtmlApp model msg -> WebSocket -> Effect WsError Unit
serveWs = app socket => ui.ServerHtml.serveWs app socket
"#;
