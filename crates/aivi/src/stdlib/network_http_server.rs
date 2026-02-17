pub const MODULE_NAME: &str = "aivi.net.http_server";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.net.http_server
export Header, Request, Response, ServerConfig
export HttpError, WsError, WsMessage, ServerReply
export TextMsg, BinaryMsg, Ping, Pong, Close
export Server, WebSocket
export Http, Ws
export listen, stop, wsRecv, wsSend, wsClose

use aivi

Header = { name: Text, value: Text }
Request = { method: Text, path: Text, headers: List Header, body: List Int, remoteAddr: Option Text }
Response = { status: Int, headers: List Header, body: List Int }
ServerConfig = { address: Text }
HttpError = { message: Text }
WsError = { message: Text }

WsMessage = TextMsg Text | BinaryMsg (List Int) | Ping | Pong | Close
ServerReply = Http Response | Ws (WebSocket -> Effect WsError Unit)

listen : ServerConfig -> (Request -> Effect HttpError ServerReply) -> Resource HttpError Server
listen = config handler => resource {
  server <- httpServer.listen config handler
  yield server
  _ <- httpServer.stop server
}

stop : Server -> Effect HttpError Unit
stop = server => httpServer.stop server

wsRecv : WebSocket -> Effect WsError WsMessage
wsRecv = socket => httpServer.ws_recv socket

wsSend : WebSocket -> WsMessage -> Effect WsError Unit
wsSend = socket msg => httpServer.ws_send socket msg

wsClose : WebSocket -> Effect WsError Unit
wsClose = socket => httpServer.ws_close socket
"#;
