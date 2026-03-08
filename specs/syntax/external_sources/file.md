# File Sources

<!-- quick-info: {"kind":"topic","name":"file sources"} -->
Typed file sources remove boilerplate parsing by letting the expected type drive decoding at the boundary.
<!-- /quick-info -->

File sources are the most direct way to bring local data into an AIVI program.

They work well for:

- JSON configuration files,
- CSV imports,
- checked-in fixtures for tests,
- plain-text templates or content.

For image-specific file-backed reads such as metadata inspection or decoded pixel payloads, see [Image Sources](image.md).

## APIs

- `file.read : Text -> Source File Text`
- `file.json : Text -> Source File A`
- `file.csv : Text -> Source File (List A)`

## Choosing the right file source

- use `file.read` when you want raw text
- use `file.json` when the file should decode into a typed value
- use `file.csv` when each row should decode into a typed record

The examples on this page use the compact path-only form. When you want a reusable declaration with an explicit schema contract, see [Schema-First Source Definitions](schema_first.md).

## Capability mapping

Loading `file.read`, `file.json`, or `file.csv` requires `file.read` (or the broader `file` family shorthand). Image-backed file sources also require `file.read`; see [Image Sources](image.md).

## Example

<<< ../../snippets/from_md/syntax/external_sources/file/block_01.aivi{aivi}


This style keeps parsing rules close to the file boundary instead of spreading ad hoc decoding logic through the rest of the program.

## Raw text example

<<< ../../snippets/from_md/syntax/external_sources/file/block_01.aivi{aivi}


## Error experience

At the type level, file-source failures surface as [`SourceError File`](../external_sources.md#1211-sourceerror). The decode branch carries path-aware [`aivi.validation.DecodeError`](../../stdlib/core/validation.md#5-decodeerror-adt) values, which format into messages that point to both:

- the file that was read,
- the place inside the decoded structure where the mismatch happened.

For example:

```text
failed to parse source [File] at $.users[1].enabled
expected Bool but received "yes"
```

That kind of message makes it much easier to fix bad input data quickly.
