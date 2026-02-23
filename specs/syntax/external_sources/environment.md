# Environment Sources

<!-- quick-info: {"kind":"topic","name":"environment sources"} -->
Environment boundaries support single-value reads and structured prefix decoding.
<!-- /quick-info -->

## APIs (v0.1)

- `env.get : Text -> Source Env Text`
- `env.decode : Text -> Source Env A`

## Example

```aivi
AppConfig = { port: Int, debug: Bool }

do Effect {
  cfg <- load (env.decode "AIVI_APP")
  pure cfg
}
```

`env.decode` collects variables under the prefix (for example `AIVI_APP_PORT`, `AIVI_APP_DEBUG`) and decodes them into the expected type.
