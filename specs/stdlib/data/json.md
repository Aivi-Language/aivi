# Standard Library: JSON Parsing & Validation

<!-- quick-info: {"kind":"module","name":"aivi.json"} -->
The `aivi.json` module defines parsing strategies that connect raw data sources to typed structures via the `Validation` Applicative. 
<!-- /quick-info -->

<div class="import-badge">use aivi.json</div>

## 1. Type-Driven Parsing 

AIVI utilizes the type expected by the assignment to drive validation. Behind the scenes, `parse` takes a raw string (or dynamically parsed JSON Document) and checks it against an implicit AST dictionary of the expected shape.

<<< ../../snippets/from_md/stdlib/data/json/type_driven_parsing.aivi{aivi}

Because the output is `Validation (List DecodeError) A`, the caller never gets a malformed `User` structure that crashes deep within the logic phase. AIVI accumulates all structural errors instead of failing upon discovering the missing `age` key.

## 2. Integrating Decode with External Sources

A large part of the AIVI vision is that `Source` declarations automatically perform this validation when accessed via `<-` inside an `Effect` block. The implementation bridges `file.read` with `json.parse`.

<<< ../../snippets/from_md/stdlib/data/json/integrating_decode_with_external_sources.aivi{aivi}

## 3. Custom Decoders for Enums / Complex Types

Developers can supply custom decoders for types that cannot be structurally derived automatically. A decoder is any function returning a `Validation (List DecodeError) A`.

<<< ../../snippets/from_md/stdlib/data/json/custom_decoders_for_enums_complex_types.aivi{aivi}

## Types

### `JsonValue`

The sum type representing any JSON value.

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

Returned when parsing or decoding fails.

```aivi
JsonError = { message: Text }
```

### `JsonSchema`

Describes structural constraints used by `validateSchema`.

```aivi
JsonSchema = {
  required: List Text
  strict: Bool
}
```

### `SchemaIssue`

A single validation failure produced by `validateSchema`.

```aivi
SchemaIssue = { path: Text, message: Text }
```

## API Reference

### Parsing and serialisation

| Function | Explanation |
| --- | --- |
| **decode** raw<br><code>Text -> Result JsonError JsonValue</code> | Parses a JSON text string into a `JsonValue`. Returns `Err` on malformed input. |
| **jsonToText** value<br><code>JsonValue -> Text</code> | Converts a `JsonValue` back to a JSON text string. |

### Encoding

| Function | Explanation |
| --- | --- |
| **encodeText** t<br><code>Text -> JsonValue</code> | Wraps a `Text` value as `JsonString`. |
| **encodeInt** n<br><code>Int -> JsonValue</code> | Wraps an `Int` as `JsonInt`. |
| **encodeFloat** f<br><code>Float -> JsonValue</code> | Wraps a `Float` as `JsonFloat`. |
| **encodeBool** b<br><code>Bool -> JsonValue</code> | Wraps a `Bool` as `JsonBool`. |
| **encodeObject** entries<br><code>List (Text, JsonValue) -> JsonValue</code> | Builds a `JsonObject` from a key-value list. |
| **encodeArray** items<br><code>List JsonValue -> JsonValue</code> | Builds a `JsonArray` from a list. |

### Decoding

| Function | Explanation |
| --- | --- |
| **decodeText** value<br><code>JsonValue -> Result JsonError Text</code> | Extracts `Text` from a `JsonString`, or returns `Err`. |
| **decodeInt** value<br><code>JsonValue -> Result JsonError Int</code> | Extracts `Int` from a `JsonInt`, or returns `Err`. |
| **decodeFloat** value<br><code>JsonValue -> Result JsonError Float</code> | Extracts `Float` from a `JsonFloat` or `JsonInt`, or returns `Err`. |
| **decodeBool** value<br><code>JsonValue -> Result JsonError Bool</code> | Extracts `Bool` from a `JsonBool`, or returns `Err`. |
| **decodeField** name obj<br><code>Text -> JsonValue -> Result JsonError JsonValue</code> | Looks up a field by name in a `JsonObject`. Returns `Err` when missing or when `obj` is not an object. |
| **decodeList** decoder arr<br><code>(JsonValue -> Result JsonError A) -> JsonValue -> Result JsonError (List A)</code> | Decodes each element of a `JsonArray` using `decoder`. |

### Validation and migration

| Function | Explanation |
| --- | --- |
| **requiredField** name obj<br><code>Text -> JsonValue -> Result JsonError JsonValue</code> | Alias for `decodeField`; signals intent that the field is required. |
| **strictFields** allowed obj<br><code>List Text -> JsonValue -> Result JsonError JsonValue</code> | Fails if `obj` contains any key not in `allowed`. |
| **validateSchema** schema obj<br><code>JsonSchema -> JsonValue -> List SchemaIssue</code> | Validates `obj` against `schema`, returning all issues found. An empty list means the object is valid. |
| **migrateObject** patchFn value<br><code>(List (Text, JsonValue) -> List (Text, JsonValue)) -> JsonValue -> JsonValue</code> | Applies `patchFn` to the entries of a `JsonObject`; passes non-objects through unchanged. Useful for schema migrations. |
