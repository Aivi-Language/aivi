pub const MODULE_NAME: &str = "aivi.url";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.url
export domain Url
export Url
export parse, toString, protocol, host, port, path, query, hash

use aivi

opaque Url = { protocol: Text, host: Text, port: Option Int, path: Text, query: List (Text, Text), hash: Option Text }

parse : Text -> Result Text Url
parse = value => url.parse value

toString : Url -> Text
toString = value => url.toString value

protocol : Url -> Text
protocol = value => value.protocol

host : Url -> Text
host = value => value.host

port : Url -> Option Int
port = value => value.port

path : Url -> Text
path = value => value.path

query : Url -> List (Text, Text)
query = value => value.query

hash : Url -> Option Text
hash = value => value.hash

filterKey : Text -> (Text, Text) -> Bool
filterKey = key pair => pair match
  | (k, _) => k != key

domain Url over Url = {
  (+) : Url -> (Text, Text) -> Url
  (+) = value (key, v) => { ...value, query: [...value.query, (key, v)] }

  (-) : Url -> Text -> Url
  (-) = value key => {
    ...value,
    query: List.filter (filterKey key) value.query
  }
}"#;
