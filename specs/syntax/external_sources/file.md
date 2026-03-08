# File Sources

<!-- quick-info: {"kind":"topic","name":"file sources"} -->
Typed file sources remove boilerplate parsing by letting the expected type drive decoding at the boundary.
<!-- /quick-info -->

File sources are the most direct way to bring local data into an AIVI program.

They work well for:

- JSON configuration files,
- CSV imports,
- checked-in fixtures for tests,
- plain-text templates or content,
- image metadata and decoded image payloads.

## APIs

- `file.read : Text -> Source File Text`
- `file.json : Text -> Source File A`
- `file.csv : Text -> Source File (List A)`
- `file.imageMeta : Text -> Source Image A`
- `file.image : Text -> Source Image A`

## Choosing the right file source

- use `file.read` when you want raw text
- use `file.json` when the file should decode into a typed value
- use `file.csv` when each row should decode into a typed record
- use `file.imageMeta` when you only need image metadata such as width or format
- use `file.image` when you need the decoded image payload itself

## Capability mapping

Loading a file source requires `file.read` (or the broader `file` family shorthand). This applies to `file.read`, `file.json`, `file.csv`, `file.imageMeta`, and `file.image`.

## Example

<<< ../../snippets/from_md/syntax/external_sources/file/block_01.aivi{aivi}


This style keeps parsing rules close to the file boundary instead of spreading ad hoc decoding logic through the rest of the program.

## Raw text example

```aivi
do Effect {
  template <- load (file.read "./email-template.txt")  // no decoding beyond Text
  pure template
}
```

## Error experience

When decoding fails, the error should point to both:

- the file that was read,
- the place inside the decoded structure where the mismatch happened.

For example:

```text
failed to parse source [File] at $.users[1].enabled
expected Bool but received "yes"
```

That kind of message makes it much easier to fix bad input data quickly.
