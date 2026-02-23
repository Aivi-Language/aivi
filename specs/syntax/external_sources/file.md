# File Sources

<!-- quick-info: {"kind":"topic","name":"file sources"} -->
Typed file sources remove boilerplate parsing by letting the expected type drive decoding at the boundary.
<!-- /quick-info -->

## APIs (v0.1)

- `file.read : Text -> Source File Text`
- `file.json : Text -> Source File A`
- `file.csv : Text -> Source File (List A)`
- `file.imageMeta : Text -> Source Image A`
- `file.image : Text -> Source Image A`

## Example

```aivi
User = { id: Int, name: Text, enabled: Bool }

do Effect {
  users <- load (file.csv "./users.csv")
  cfg   <- load (file.json "./config.json")
  pure { users, cfg }
}
```

## Error UX

Decode failures should surface source path + expected/received shape:

```text
failed to parse source [File] at $.users[1].enabled
expected Bool but received "yes"
```
