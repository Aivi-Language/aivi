# Environment Sources

<!-- quick-info: {"kind":"topic","name":"environment sources"} -->
Environment sources describe typed configuration reads from the process environment.
<!-- /quick-info -->

Environment sources are for configuration that comes from the process environment: ports, feature flags, API keys, deployment names, and similar values.

They follow the same source pattern used for files or HTTP APIs: define a `Source Env A` first, then call `load` inside `do Effect { ... }` when you actually want to read from the host environment.

## APIs

- `env.get : Text -> Source Env Text`
- `env.decode : Text -> Source Env A`

Choose the API by the shape of configuration you need:

- `env.get` when one required variable should come through as raw `Text`
- `env.decode` when several related variables should decode into one typed value

## Capability mapping

Defining either source is pure. The capability requirement appears when you load it:

- `load (env.get ...)` requires `process.env.read`
- `load (env.decode ...)` requires `process.env.read`

See [Capabilities](../capabilities.md) for the standard capability vocabulary.

## Reading a single variable

```aivi
do Effect {
  mode <- load (env.get "AIVI_MODE") // reads exactly one environment variable
  pure mode
}
```

`env.get` is the simplest option when you want one required string and plan to interpret it yourself later.

If `AIVI_MODE` is missing, `load (env.get "AIVI_MODE")` fails at the source boundary. Use `attempt` at the load site when you want to recover from a missing variable instead of letting the effect fail.

## Decoding a typed config value

<<< ../../snippets/from_md/syntax/external_sources/environment/block_02.aivi{aivi}


`env.decode "AIVI_APP"` looks for variables that share the prefix, such as:

- `AIVI_APP_PORT`
- `AIVI_APP_DEBUG`

After the shared prefix is removed, the remaining suffix becomes the field name in lowercase. In the example above, `PORT` maps to `port` and `DEBUG` maps to `debug`, so the expected result type must be compatible with fields like:

- `port : Int`
- `debug : Bool`

## When to use `env.decode`

`env.decode` is especially useful when:

- several configuration values belong together,
- you want type errors reported at the boundary,
- you want the same config record shape in development, tests, and production.

## Failure modes and diagnostics

Environment sources can fail in three common ways:

- `env.get` fails when the named variable is missing or inaccessible,
- `env.decode` fails when no variables match the requested prefix,
- either source can fail when the text values exist but cannot be decoded into the expected type.

Typical diagnostics include messages such as `env var not found: AIVI_MODE` or `no environment variables found for prefix \`AIVI_APP\``. Decode failures keep the source boundary attached as well, so you can tell whether the problem came from acquisition or from type-directed decoding.

For the general source error model, see [External Sources](../external_sources.md#1211-sourceerror).
