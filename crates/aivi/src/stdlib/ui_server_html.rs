pub const MODULE_NAME: &str = "aivi.ui.ServerHtml";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.ui.ServerHtml
export ViewId, UrlInfo, InitContext
export PlatformEvent
export ClipboardError, Effect
export ServerHtmlApp
export Route, serve
export serveHttp, serveWs

use aivi
use aivi.net.http_server (Request, Response, WebSocket, ServerConfig, Server, ServerReply, HttpError, WsError, Http, Ws)
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

Route model msg =
  { path: Text
  , app: ServerHtmlApp model msg
  }

// Render initial HTML + boot data, allocating a fresh ViewId.
serveHttp : ServerHtmlApp model msg -> Request -> Response
serveHttp = app req => ui.ServerHtml.serveHttp app req

// WebSocket session handler (expects a `hello` message first).
serveWs : ServerHtmlApp model msg -> WebSocket -> Effect WsError Unit
serveWs = app socket => ui.ServerHtml.serveWs app socket

trimTrailingSlashes : Text -> Text
trimTrailingSlashes = path =>
  if path == "/" then "/" else
  if text.endsWith "/" path then
    trimTrailingSlashes (text.slice 0 (text.length path - 1) path)
  else
    path

normalizePath : Text -> Text
normalizePath = raw => {
  p = text.trim raw
  p = if p == "" then "/" else if text.startsWith "/" p then p else text.concat ["/", p]
  trimTrailingSlashes p
}

wsPathFor : Text -> Text
wsPathFor = routePath => {
  p = normalizePath routePath
  if p == "/" then "/ws" else text.concat [p, "/ws"]
}

findAppForHttpPath : Text -> List (Route model msg) -> Option (ServerHtmlApp model msg)
findAppForHttpPath = path routes => routes ?
  | [] => None
  | [r, ...rest] => if normalizePath r.path == path then Some r.app else findAppForHttpPath path rest

findAppForWsPath : Text -> List (Route model msg) -> Option (ServerHtmlApp model msg)
findAppForWsPath = path routes => routes ?
  | [] => None
  | [r, ...rest] => if wsPathFor r.path == path then Some r.app else findAppForWsPath path rest

notFound : Response
notFound =
  { status: 404
  , headers: [{ name: "content-type", value: "text/plain; charset=utf-8" }]
  , body: []
  }

dispatch : List (Route model msg) -> Request -> ServerReply
dispatch = routes req => {
  path = normalizePath req.path
  findAppForHttpPath path routes ?
    | Some app => Http (serveHttp app req)
    | None =>
        findAppForWsPath path routes ?
          | Some app => Ws (socket => serveWs app socket)
          | None => Http notFound
}

serve : ServerConfig -> List (Route model msg) -> Resource HttpError Server
serve = config routes => httpServer.listen config (req => pure (dispatch routes req))
"#;
