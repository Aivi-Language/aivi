pub const MODULE_NAME: &str = "aivi.url";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.url = {
  export domain Url
  export Url
  export parse, toString

  use aivi

  Url = { protocol: Text, host: Text, port: Option Int, path: Text, query: List (Text, Text), hash: Option Text }

  parse : Text -> Result Text Url
  parse value = url.parse value

  toString : Url -> Text
  toString value = url.toString value

  filter : (A -> Bool) -> List A -> List A
  filter predicate items = items ?
    | [] => []
    | [x, ...xs] => if predicate x then [x, ...filter predicate xs] else filter predicate xs

  append : List A -> List A -> List A
  append left right = left ?
    | [] => right
    | [x, ...xs] => [x, ...append xs right]

  filterKey : Text -> (Text, Text) -> Bool
  filterKey key pair = pair ?
    | (k, _) => k != key

  domain Url over Url = {
    (+) : Url -> (Text, Text) -> Url
    (+) value (key, v) = { ...value, query: append value.query [(key, v)] }

    (-) : Url -> Text -> Url
    (-) value key = {
      ...value,
      query: filter (filterKey key) value.query
    }
  }
}
"#;
