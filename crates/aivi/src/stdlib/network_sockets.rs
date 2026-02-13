pub const MODULE_NAME: &str = "aivi.net.sockets";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.net.sockets
export Address, SocketError, Listener, Connection
export listen, accept, connect, send, recv, close

use aivi

Address = { host: Text, port: Int }
SocketError = { message: Text }

listen : Address -> Resource SocketError Listener
listen address = resource {
  listener <- sockets.listen address
  yield listener
  _ <- sockets.closeListener listener
}

accept : Listener -> Effect SocketError Connection
accept listener = sockets.accept listener

connect : Address -> Effect SocketError Connection
connect address = sockets.connect address

send : Connection -> List Int -> Effect SocketError Unit
send conn bytes = sockets.send conn bytes

recv : Connection -> Effect SocketError (List Int)
recv conn = sockets.recv conn

close : Connection -> Effect SocketError Unit
close conn = sockets.close conn"#;
