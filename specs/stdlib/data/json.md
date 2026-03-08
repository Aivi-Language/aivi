# Standard Library: JSON Parsing & Validation

<!-- quick-info: {"kind":"module","name":"aivi.json"} -->
The `aivi.json` module provides low-level JSON values, helper functions for inspecting and building them, and small schema utilities for object-shaped data.
<!-- /quick-info -->

`aivi.json` is the JSON toolbox you reach for when you need to work with JSON-shaped data directly inside a program. In current AIVI, fully typed JSON decoding usually happens at source boundaries such as [`file.json`](../../syntax/external_sources/file.md), while `aivi.json` itself focuses on `JsonValue`, field extraction, schema checks, and error formatting.

Use it to:

- build or inspect `JsonValue` values,
- turn `JsonValue` back into JSON text,
- pull fields and lists out of existing JSON values,
- check simple object requirements with `JsonSchema`,
- format JSON-specific failures for logs and diagnostics.

If you are new to [`Validation`](../core/validation.md), the practical idea is simple: source decoding can report a list of problems instead of stopping at the first one. In AIVI today, that accumulated error story appears through `SourceError.DecodeError (List aivi.validation.DecodeError)` at structured source boundaries and through `source.validate` for extra semantic checks.

## Import

<div class="import-badge">use aivi.json</div>

If you are adding `Validation`-based checks on top of decoded source data, also import [`aivi.validation`](../core/validation.md).

## Start here

For most apps today, the shortest useful path is:

1. decode JSON into a typed value at the source boundary with [`file.json`](../../syntax/external_sources/file.md) or another structured source,
2. handle `SourceError.DecodeError` if the payload shape is wrong,
3. add `source.validate` only when structure alone is not enough,
4. use `JsonValue` helpers when you need manual inspection, migration, or rendering,
5. use `JsonSchema` only when you must match an external contract.

## Decode JSON into the type you want at the source boundary

In current AIVI, the practical typed-decoding path is a structured source such as `file.json`, not a direct `json.parse` helper.

<<< ../../snippets/from_md/stdlib/data/json/block_01.aivi{aivi}


Here `loadUser` succeeds with a normal `User` value or fails with `SourceError File`. If the file can be read but the JSON does not match `User`, the failure is `DecodeError (List aivi.validation.DecodeError)`, so callers get a list of path-aware problems rather than a partially decoded value.

See also:

- [`file sources`](../../syntax/external_sources/file.md) for the boundary-decoding workflow,
- [`Validation`](../core/validation.md) for accumulated error handling,
- [`source composition`](../../syntax/external_sources/composition.md#validate) for semantic checks after decoding.

## Use JSON with external data sources

When you want to inspect success and failure as ordinary data, wrap the load with `attempt`:

<<< ../../snippets/from_md/stdlib/data/json/block_02.aivi{aivi}


`readUsers` lets you distinguish:

- `Ok data` when the file existed and matched `List User`,
- `Err (IOError message)` when the file could not be read,
- `Err (DecodeError errors)` when the file was readable but the JSON shape was wrong.

This keeps raw I/O and JSON-shape problems at the program boundary, so the rest of your code can work with normal typed values.

## Add custom decoders for values that need special rules

Some values cannot be accepted from structure alone. Enums, constrained strings, and tagged formats often need one extra step that explains the allowed cases.

When you already have a `JsonValue`, a small custom decoder can build on the primitive helpers in `aivi.json`:

<<< ../../snippets/from_md/stdlib/data/json/block_03.aivi{aivi}


This style is a good fit when you are manually walking a `JsonValue`. If the value came from a structured source and you want accumulated, path-aware errors, keep the structural decoding at the source boundary and add the domain rule with [`source.validate`](../../syntax/external_sources/composition.md#validate) and [`aivi.validation.DecodeError`](../core/validation.md#5-decodeerror-adt).

## Validate against a schema when you need a contract

Use `JsonSchema` when you want a small, explicit contract for object-shaped data that already exists as a `JsonValue`.

<<< ../../snippets/from_md/stdlib/data/json/block_04.aivi{aivi}


In the example above, `issues` contains one `SchemaIssue` for the missing `name` field. In the current implementation, `validateSchema` checks that the input is an object and that every `required` field exists. The `strict` flag is part of `JsonSchema`, but `validateSchema` does not yet reject extra keys on its own; use `strictFields` when you need unknown-key rejection today.

If you want to attach a JSON contract to a source declaration and have the compiler compare it with the declared result type, see [`schema-first source definitions`](../../syntax/external_sources/schema_first.md).

## Core types

### `JsonValue`

`JsonValue` represents any JSON value. It is the low-level format you work with before decoding into application-specific types.

<<< ../../snippets/from_md/stdlib/data/json/block_08.aivi{aivi}


For example, `JsonObject [("name", JsonString "Ada")]` is a `JsonValue`.

### `JsonError`

`JsonError` is the one-off error type returned by helpers such as `decode`, `decodeText`, `decodeField`, and `decodeList`.

<<< ../../snippets/from_md/stdlib/data/json/block_05.aivi{aivi}


### `JsonSchema`

`JsonSchema` describes the small schema record understood by `validateSchema`.

<<< ../../snippets/from_md/stdlib/data/json/block_06.aivi{aivi}


`required` lists fields that must exist on an object. `strict` records whether extra keys should be treated as unexpected, but the current `validateSchema` helper does not yet enforce that flag directly.

### `SchemaIssue`

`SchemaIssue` describes one schema-validation problem, including where it happened.

<<< ../../snippets/from_md/stdlib/data/json/block_07.aivi{aivi}


### How these types relate to `DecodeError`

Do not confuse the low-level JSON helper types above with [`aivi.validation.DecodeError`](../core/validation.md#5-decodeerror-adt):

- `JsonError` is for one-off helper failures such as “expected Text” or “missing field: name”,
- `SchemaIssue` is for `validateSchema`,
- `DecodeError` is the path-aware error type used by structured source decoding and `source.validate`.

## API reference

Use the tables below by workflow:

- parse or render raw JSON,
- build `JsonValue` values manually,
- extract fields from existing JSON,
- validate or migrate objects,
- format errors for people.

### Parse and serialise JSON

| Function | Type | What it does |
| --- | --- | --- |
| `decode` | `Text -> Result JsonError JsonValue` | Reserved for parsing raw JSON text into a `JsonValue`. In the current implementation, native text-to-JSON parsing is not yet available, so this function currently returns `Err`. For production decoding today, prefer typed sources such as `file.json`. |
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
| `decodeList` | `(JsonValue -> Result JsonError A) -> JsonValue -> Result JsonError (List A)` | Applies a decoder to each element of a `JsonArray`, stopping at the first element-level failure. |

### Validate and migrate objects

| Function | Type | What it does |
| --- | --- | --- |
| `requiredField` | `Text -> JsonValue -> Result JsonError JsonValue` | Alias for `decodeField` when you want the call site to clearly say that a field must exist. |
| `strictFields` | `List Text -> JsonValue -> Result JsonError JsonValue` | Fails if an object contains keys outside the allowed list. |
| `validateSchema` | `JsonSchema -> JsonValue -> List SchemaIssue` | Checks that the value is an object and that every required field exists. The current implementation does not yet use the `strict` flag; combine it with `strictFields` if you also need unknown-key rejection. |
| `migrateObject` | `(List (Text, JsonValue) -> List (Text, JsonValue)) -> JsonValue -> JsonValue` | Applies a patch function to the entries of a `JsonObject`. Non-object values pass through unchanged. |

### Render and log errors

| Function | Type | What it does |
| --- | --- | --- |
| `renderSchemaIssue` | `Int -> SchemaIssue -> Text` | Formats one `SchemaIssue` as a numbered, ANSI-coloured line. |
| `renderSchemaIssues` | `List SchemaIssue -> Text` | Formats many schema issues as a compiler-style error block. |
| `renderJsonError` | `Text -> JsonError -> Text` | Formats a `JsonError` at a given JSON path. |
| `logSchemaIssues` | `List SchemaIssue -> Effect Text Unit` | Writes schema issues to stderr using `renderSchemaIssues`. |
| `logJsonError` | `Text -> JsonError -> Effect Text Unit` | Writes a JSON error to stderr using `renderJsonError`. |
