# Environment Sources

<!-- quick-info: {"kind":"topic","name":"environment sources"} -->
Environment boundaries support single-value reads and structured prefix decoding.
<!-- /quick-info -->

Environment sources are for configuration that comes from the process environment: ports, feature flags, API keys, deployment names, and similar values.

They let you keep the same typed source pattern you use for files or HTTP APIs.

## APIs

- `env.get : Text -> Source Env Text`
- `env.decode : Text -> Source Env A`

Use:

- `env.get` when you want one variable as raw `Text`
- `env.decode` when you want a whole typed value decoded from a prefix

## Capability mapping

Loading from `env.get` or `env.decode` requires `process.env.read`.

## Reading a single variable

```aivi
do Effect {
  mode <- load (env.get "AIVI_MODE")  -- reads exactly one environment variable
  pure mode
}
```

`env.get` is the simplest option when you want to read a single string and decode or interpret it yourself later.

## Decoding a typed config value

```aivi
AppConfig = { port: Int, debug: Bool }

do Effect {
  cfg <- load (env.decode "AIVI_APP")  -- looks for variables under the AIVI_APP prefix
  pure cfg
}
```

`env.decode "AIVI_APP"` collects matching variables such as:

- `AIVI_APP_PORT`
- `AIVI_APP_DEBUG`

Those values are then decoded into the expected result type. In the example above, `port` must decode as an `Int` and `debug` must decode as a `Bool`.

## When to use `env.decode`

`env.decode` is especially useful when:

- several configuration values belong together,
- you want type errors reported at the boundary,
- you want the same config record shape in development, tests, and production.

## Failure modes

Environment sources can fail in two common ways:

- the variable is missing or inaccessible,
- the text value exists but cannot be decoded into the expected type.

That second case is often where typed environment sources help most. Instead of sprinkling manual parsing logic throughout the program, you keep the decoding rule at the boundary.
