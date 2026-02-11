pub const MODULE_NAME: &str = "aivi.net";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.net = {
  export http, https, httpServer, sockets, streams

  use aivi
}
"#;
