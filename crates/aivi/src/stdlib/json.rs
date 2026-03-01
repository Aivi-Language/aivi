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
export renderSchemaIssue, renderSchemaIssues, renderJsonError
export logSchemaIssues, logJsonError

use aivi
use aivi.text
use aivi.console

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
  | JsonString s   => "\"" ++ jsonEscapeText s ++ "\""
  | JsonArray items  => "[" ++ joinJsonValues items ++ "]"
  | JsonObject pairs => "\{" ++ joinJsonPairs pairs ++ "\}"

jsonEscapeText : Text -> Text
jsonEscapeText = s =>
  s
    |> text.replace "\\" "\\\\"
    |> text.replace "\"" "\\\""
    |> text.replace "\n" "\\n"
    |> text.replace "\r" "\\r"
    |> text.replace "\t" "\\t"

joinJsonValues : List JsonValue -> Text
joinJsonValues = items => items match
  | []         => ""
  | [x]        => jsonToText x
  | [x, ...xs] => jsonToText x ++ "," ++ joinJsonValues xs

joinJsonPairs : List (Text, JsonValue) -> Text
joinJsonPairs = pairs => pairs match
  | []              => ""
  | [(k, v)]        => "\"" ++ jsonEscapeText k ++ "\":" ++ jsonToText v
  | [(k, v), ...ps] => "\"" ++ jsonEscapeText k ++ "\":" ++ jsonToText v ++ "," ++ joinJsonPairs ps

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

// ── Coloured error rendering ──────────────────────────────────────────────────

// Renders a single SchemaIssue as a numbered line with ANSI colour.
//   1. at $.user.age — expected Int, got String
renderSchemaIssue : Int -> SchemaIssue -> Text
renderSchemaIssue = index issue => {
  num  = console.color Yellow (text.toText index ++ ".")
  path = console.color Cyan issue.path
  "  " ++ num ++ " at " ++ path ++ " — " ++ issue.message
}

renderSchemaIssueLines : List SchemaIssue -> Int -> List Text -> List Text
renderSchemaIssueLines = issues index acc => issues match
  | []           => List.reverse acc
  | [x, ...rest] => renderSchemaIssueLines rest (index + 1) [renderSchemaIssue index x, ...acc]

joinLinesJson : List Text -> Text
joinLinesJson = lines => lines match
  | []         => ""
  | [x]        => x
  | [x, ...xs] => x ++ "\n" ++ joinLinesJson xs

// Renders a list of SchemaIssues as a compiler-style error block with ANSI colour.
//
//   error[decode]: 2 issue(s) found
//     1. at $.user.age — expected Int, got String
//     2. at $.user.email — missing required field
renderSchemaIssues : List SchemaIssue -> Text
renderSchemaIssues = issues => {
  count  = List.length issues
  label  = console.color Yellow "error[decode]"
  header = label ++ ": " ++ text.toText count ++ " issue(s) found"
  lines  = renderSchemaIssueLines issues 1 []
  joinLinesJson [header, ...lines]
}

// Renders a single JsonError at a given JSON path with ANSI colour.
//
//   error[decode] at $.user.id — expected Int
renderJsonError : Text -> JsonError -> Text
renderJsonError = context err => {
  label = console.color Yellow "error[decode]"
  path  = console.color Cyan context
  label ++ " at " ++ path ++ " — " ++ err.message
}

// Logs all SchemaIssues to stderr with ANSI colour.
logSchemaIssues : List SchemaIssue -> Effect Text Unit
logSchemaIssues = issues => console.error (renderSchemaIssues issues)

// Logs a single JsonError to stderr with ANSI colour.
logJsonError : Text -> JsonError -> Effect Text Unit
logJsonError = context err => console.error (renderJsonError context err)
"#;
