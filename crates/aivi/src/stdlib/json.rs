pub const MODULE_NAME: &str = "aivi.json";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.json
export JsonValue, JsonError, JsonSchema, SchemaIssue
export decode, jsonToText
export encodeText, decodeText
export encodeInt, decodeInt
export encodeFloat, decodeFloat
export encodeBool, decodeBool
export encodeObject, encodeArray
export decodeField, decodeList
export requiredField, strictFields, validateSchema, migrateObject

use aivi
use aivi.text

JsonValue =
  | JsonNull
  | JsonBool Bool
  | JsonInt Int
  | JsonFloat Float
  | JsonString Text
  | JsonArray (List JsonValue)
  | JsonObject (List (Text, JsonValue))

JsonError = { message: Text }
SchemaIssue = { path: Text, message: Text }
JsonSchema = {
  required: List Text
  strict: Bool
}

decode : Text -> Result JsonError JsonValue
decode = raw => Err { message: "json.decode: native JSON parsing not yet available" }

jsonToText : JsonValue -> Text
jsonToText = value => value match
  | JsonNull       => "null"
  | JsonBool b     => b match | True => "true" | False => "false"
  | JsonInt n      => text.toText n
  | JsonFloat f    => text.toText f
  | JsonString s   => "\"" ++ s ++ "\""
  | JsonArray _    => "[...]"
  | JsonObject _   => "\{...\}"

encodeText : Text -> JsonValue
encodeText = t => JsonString t

decodeText : JsonValue -> Result JsonError Text
decodeText = v => v match
  | JsonString s => Ok s
  | _            => Err { message: "expected Text" }

encodeInt : Int -> JsonValue
encodeInt = n => JsonInt n

decodeInt : JsonValue -> Result JsonError Int
decodeInt = v => v match
  | JsonInt n => Ok n
  | _         => Err { message: "expected Int" }

encodeFloat : Float -> JsonValue
encodeFloat = f => JsonFloat f

decodeFloat : JsonValue -> Result JsonError Float
decodeFloat = v => v match
  | JsonFloat f => Ok f
  | JsonInt n   => Ok (n * 1.0)
  | _           => Err { message: "expected Float" }

encodeBool : Bool -> JsonValue
encodeBool = b => JsonBool b

decodeBool : JsonValue -> Result JsonError Bool
decodeBool = v => v match
  | JsonBool b => Ok b
  | _          => Err { message: "expected Bool" }

encodeObject : List (Text, JsonValue) -> JsonValue
encodeObject = entries => JsonObject entries

encodeArray : List JsonValue -> JsonValue
encodeArray = items => JsonArray items

decodeField : Text -> JsonValue -> Result JsonError JsonValue
decodeField = name obj => obj match
  | JsonObject entries => findField name entries
  | _                  => Err { message: "expected Object" }

requiredField : Text -> JsonValue -> Result JsonError JsonValue
requiredField = name obj => decodeField name obj

findField : Text -> List (Text, JsonValue) -> Result JsonError JsonValue
findField = name entries => entries match
  | []              => Err { message: "missing field: " ++ name }
  | [(k, v), ...es] => k == name match
    | True  => Ok v
    | False => findField name es

decodeList : (JsonValue -> Result JsonError A) -> JsonValue -> Result JsonError (List A)
decodeList = decoder arr => arr match
  | JsonArray items => decodeListLoop decoder items []
  | _               => Err { message: "expected Array" }

decodeListLoop : (JsonValue -> Result JsonError A) -> List JsonValue -> List A -> Result JsonError (List A)
decodeListLoop = decoder items acc => items match
  | []         => Ok (List.reverse acc)
  | [x, ...xs] => decoder x match
    | Ok v  => decodeListLoop decoder xs [v, ...acc]
    | Err e => Err e

hasKey : Text -> List (Text, JsonValue) -> Bool
hasKey = name entries => entries match
  | [] => False
  | [(k, _), ...rest] => if k == name then True else hasKey name rest

strictFields : List Text -> JsonValue -> Result JsonError JsonValue
strictFields = allowed obj => obj match
  | JsonObject entries =>
      allAllowed allowed entries match
        | True => Ok obj
        | False => Err { message: "json.strictFields: unknown key" }
  | _ => Err { message: "expected Object" }

allAllowed : List Text -> List (Text, JsonValue) -> Bool
allAllowed = allowed entries => entries match
  | [] => True
  | [(k, _), ...rest] =>
      if containsText k allowed
      then allAllowed allowed rest
      else False

containsText : Text -> List Text -> Bool
containsText = needle values => values match
  | [] => False
  | [x, ...xs] => if x == needle then True else containsText needle xs

validateSchema : JsonSchema -> JsonValue -> List SchemaIssue
validateSchema = schema value =>
  value match
    | JsonObject entries => validateRequired schema.required entries []
    | _ => [{ path: "$", message: "expected object" }]

validateRequired : List Text -> List (Text, JsonValue) -> List SchemaIssue -> List SchemaIssue
validateRequired = keys entries acc => keys match
  | [] => List.reverse acc
  | [k, ...rest] =>
      if hasKey k entries
      then validateRequired rest entries acc
      else validateRequired rest entries [{ path: text.concat ["$.", k], message: "missing required field" }, ...acc]

migrateObject : (List (Text, JsonValue) -> List (Text, JsonValue)) -> JsonValue -> JsonValue
migrateObject = patchFn value => value match
  | JsonObject entries => JsonObject (patchFn entries)
  | _ => value
"#;
