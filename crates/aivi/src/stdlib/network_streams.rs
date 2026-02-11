pub const MODULE_NAME: &str = "aivi.net.streams";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.net.streams = {
  export Stream, StreamError
  export fromSocket, toSocket, chunks

  use aivi

  StreamError = { message: Text }

  fromSocket : Connection -> Stream (List Int)
  fromSocket conn = streams.fromSocket conn

  toSocket : Connection -> Stream (List Int) -> Effect StreamError Unit
  toSocket conn stream = streams.toSocket conn stream

  chunks : Int -> Stream (List Int) -> Stream (List Int)
  chunks size stream = streams.chunks size stream
}
"#;
