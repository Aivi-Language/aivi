# Standard Library: JSON Parsing & Validation

<!-- quick-info: {"kind":"module","name":"aivi.json"} -->
The `aivi.json` module defines parsing strategies that connect raw data sources to typed structures via the `Validation` Applicative. 
<!-- /quick-info -->

`aivi.json` helps you move between raw JSON text and normal AIVI values. You can use it in a few different ways:

- parse JSON text into a generic `JsonValue`
- decode JSON into a concrete AIVI type such as `User`
- validate that an object has the shape you expect
- migrate older JSON objects into a newer layout

If you are new to `Validation`, the important idea is simple: decoding can return a valid value or a list of problems. That is especially useful for configuration files, API payloads, and imported data, because you can report several mistakes at once instead of stopping at the first one.

## Import

```aivi
use aivi.json
```

## Decode JSON directly into the type you want

A common pattern in AIVI is to state the type you want and let the JSON layer check the input against that type. This keeps the code close to the data model you actually care about.

<<< ../../snippets/from_md/stdlib/data/json/type_driven_parsing.aivi{aivi}

In the example above, the result is `Validation (List DecodeError) User`. That means:

- `Valid user` if the JSON matches the `User` type
- `Invalid errors` if the JSON shape does not match

The benefit is practical: the caller does not receive a partially decoded `User`. Instead, it receives a clear list of problems, such as a missing field or a field with the wrong type.

## Use JSON with external data sources

JSON decoding is also useful at the boundary of your program, especially when reading files or other external sources. In that setup, the source provides raw JSON, and AIVI checks it before your main logic sees the value.

<<< ../../snippets/from_md/stdlib/data/json/integrating_decode_with_external_sources.aivi{aivi}

This pattern is helpful when you want early failures with good error messages. Once the load succeeds, the rest of your code can work with a normal typed value instead of repeatedly checking raw JSON fields.

## Add custom decoders for values that need special rules

Some values cannot be derived from structure alone. Enums, constrained strings, and custom tagged formats often need a small decoder that explains the allowed cases.

<<< ../../snippets/from_md/stdlib/data/json/custom_decoders_for_enums_complex_types.aivi{aivi}

A custom decoder is just a function that turns a `JsonValue` into either a valid result or a list of `DecodeError`s. That keeps special-case rules close to the type they belong to.

## Validate against a schema when you need stricter contracts

If you already have a JSON schema, you can attach it to a source declaration or validate a decoded object against it. This is useful when the JSON must match an external contract shared with other systems.

```aivi
@static
userSchema : JsonSchema
userSchema = file.json "./schemas/users.schema.json"

usersSource : Source File (List User)
usersSource =
  file.json {
    path: "./users.json",
    // Reuse the checked schema when loading this source.
    schema: source.schema.json userSchema
  }
```

When the schema value is compile-time stable, the compiler can compare that schema with the source's result type before runtime. At runtime, `load` still uses the normal JSON decode pipeline and reports accumulated `DecodeError` values if the live data does not match.

Use `validateSchema` when you want a list of schema issues, and `migrateObject` when you need to reshape older JSON objects into the format your program expects today.

## Core types

### `JsonValue`

`JsonValue` represents any JSON value. It is the low-level format you work with before decoding into application-specific types.

```aivi
JsonValue
  = JsonNull
  | JsonBool Bool
  | JsonInt Int
  | JsonFloat Float
  | JsonString Text
  | JsonArray (List JsonValue)
  | JsonObject (List (Text, JsonValue))
```

### `JsonError`

`JsonError` describes a parsing or decoding failure.

```aivi
JsonError = { message: Text }
```

### `JsonSchema`

`JsonSchema` describes basic structural rules checked by `validateSchema`.

```aivi
JsonSchema = {
  required: List Text
  strict: Bool
}
```

### `SchemaIssue`

`SchemaIssue` describes one schema validation problem, including where it happened.

```aivi
SchemaIssue = { path: Text, message: Text }
```

## API reference

### Parse and serialise JSON

| Function | Type | What it does |
| --- | --- | --- |
| `decode` | `Text -> Result JsonError JsonValue` | Parses raw JSON text into a `JsonValue`. Returns `Err` when the text is not valid JSON. |
| `jsonToText` | `JsonValue -> Text` | Converts a `JsonValue` back into JSON text. |

### Build `JsonValue` values

| Function | Type | What it does |
| --- | --- | --- |
| `encodeText` | `Text -> JsonValue` | Wraps `Text` as `JsonString`. |
| `encodeInt` | `Int -> JsonValue` | Wraps an `Int` as `JsonInt`. |
| `encodeFloat` | `Float -> JsonValue` | Wraps a `Float` as `JsonFloat`. |
| `encodeBool` | `Bool -> JsonValue` | Wraps a `Bool` as `JsonBool`. |
| `encodeObject` | `List (Text, JsonValue) -> JsonValue` | Builds a `JsonObject` from key-value pairs. |
| `encodeArray` | `List JsonValue -> JsonValue` | Builds a `JsonArray` from a list of items. |

### Pull values back out of JSON

| Function | Type | What it does |
| --- | --- | --- |
| `decodeText` | `JsonValue -> Result JsonError Text` | Extracts `Text` from a `JsonString`, or returns `Err` if the value has a different shape. |
| `decodeInt` | `JsonValue -> Result JsonError Int` | Extracts `Int` from a `JsonInt`, or returns `Err`. |
| `decodeFloat` | `JsonValue -> Result JsonError Float` | Extracts `Float` from a `JsonFloat` or `JsonInt`, or returns `Err`. |
| `decodeBool` | `JsonValue -> Result JsonError Bool` | Extracts `Bool` from a `JsonBool`, or returns `Err`. |
| `decodeField` | `Text -> JsonValue -> Result JsonError JsonValue` | Looks up a field by name in a `JsonObject`. Returns `Err` if the field is missing or the input is not an object. |
| `decodeList` | `(JsonValue -> Result JsonError A) -> JsonValue -> Result JsonError (List A)` | Applies a decoder to each element of a `JsonArray`. |

### Validate and migrate objects

| Function | Type | What it does |
| --- | --- | --- |
| `requiredField` | `Text -> JsonValue -> Result JsonError JsonValue` | Alias for `decodeField` when you want the call site to clearly say that a field must exist. |
| `strictFields` | `List Text -> JsonValue -> Result JsonError JsonValue` | Fails if an object contains keys outside the allowed list. |
| `validateSchema` | `JsonSchema -> JsonValue -> List SchemaIssue` | Checks a `JsonValue` against a `JsonSchema` and returns every issue it finds. An empty list means the object passed validation. |
| `migrateObject` | `(List (Text, JsonValue) -> List (Text, JsonValue)) -> JsonValue -> JsonValue` | Applies a patch function to the entries of a `JsonObject`. Non-object values pass through unchanged. |

### Render and log errors

| Function | Type | What it does |
| --- | --- | --- |
| `renderSchemaIssue` | `Int -> SchemaIssue -> Text` | Formats one `SchemaIssue` as a numbered, ANSI-coloured line. |
| `renderSchemaIssues` | `List SchemaIssue -> Text` | Formats many schema issues as a compiler-style error block. |
| `renderJsonError` | `Text -> JsonError -> Text` | Formats a `JsonError` at a given JSON path. |
| `logSchemaIssues` | `List SchemaIssue -> Effect Text Unit` | Writes schema issues to stderr using `renderSchemaIssues`. |
| `logJsonError` | `Text -> JsonError -> Effect Text Unit` | Writes a JSON error to stderr using `renderJsonError`. |
