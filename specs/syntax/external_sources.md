# External Sources

External sources are AIVI’s typed way to read data that lives outside your program: files, environment variables, HTTP APIs, email, images, and other external systems.

## Start with the mental model

A `Source K A` is a **recipe** for obtaining outside data, not a read that has already happened.

- `K` says **where the data comes from**
- `A` says **what typed value you want after decoding**
- `load` is the moment where the program actually touches the outside world

That split keeps I/O explicit. You can define a source once, pass it around like any other value, and decide later where the actual read should happen.

## A two-step example

Most source-heavy code follows the same simple pattern:

1. **Describe the boundary.**
2. **Load it inside `do Effect { ... }`.**

<<< ../snippets/from_md/syntax/external_sources/block_01.aivi{aivi}


Nothing is read when `usersFromFile` is defined. The read happens when `load usersFromFile` runs.

For new code, prefer schema-first record forms such as `file.json { path: "...", schema: source.schema.derive }` and `env.decode { prefix: "...", schema: source.schema.derive }`. The shorter string forms still work and appear in a few overview examples because they are the smallest way to show the model.

## Source Guides

- [Schema-First Source Definitions](external_sources/schema_first.md)
- [Source Composition](external_sources/composition.md)
- [File Sources](external_sources/file.md)
- [REST / HTTP Sources](external_sources/rest_http.md)
- [Environment Sources](external_sources/environment.md)
- [IMAP Email Sources](external_sources/imap_email.md)
- [Image Sources](external_sources/image.md)
- [Compile-Time Sources](external_sources/compile_time.md)

If you already know which connector you need, jump straight to the guide above. If you want the big picture first, keep reading this page in order: start with `Source K A`, skim the common source kinds, then finish with error handling and composition.

This overview focuses on the source constructors currently documented end-to-end for v0.1: file, HTTP/REST, environment, IMAP email, image, and `@static` compile-time sources. Related domains such as [`aivi.database`](../stdlib/system/database.md) still matter for external data, but they are documented in their own module specs rather than as stable `Source` constructors on this overview page.

## 12.1 The Source Type

<<< ../snippets/from_md/syntax/external_sources/the_source_type.aivi{aivi}

Read `Source K A` as “a source of kind `K` that should decode into `A`”.

- `K` is the **kind** of source, such as `File`, `RestApi`, `Env`, or `Imap`
- `A` is the **decoded result type**
- in most code, you only mention `K` in type signatures or `SourceError K`; the constructor usually determines it for you

In everyday code, a source gives you a clear boundary between “outside data” and “trusted, typed values inside the program”.

### A practical workflow

Most source-based programs follow the same four steps:

1. define the data type you want,
2. build a source that can decode into that type,
3. call `load` inside `do Effect { ... }`,
4. handle failures at the boundary.

That keeps parsing, validation, and I/O at the edge of the program instead of spreading them throughout business logic.

## 12.2 File Sources

Use file sources for local configuration, checked-in fixtures, imports, and one-off data processing jobs.

- `file.read` gives you raw `Text`
- `file.json` decodes JSON into the type you ask for
- `file.csv` decodes rows into a typed list

```aivi
User = { id: Int, name: Text, enabled: Bool }

usersSource : Source File (List User)
usersSource =
  file.csv "./users.csv"

configSource : Source File { port: Int, debug: Bool }
configSource =
  file.json {
    path: "./config.json"
    schema: source.schema.derive
  }
```

For a practical guide, see [File Sources](external_sources/file.md).

## 12.3 HTTP Sources

Use HTTP or REST sources when your program reads typed data from a web service.

- `rest.*` is usually the best starting point when you want typed JSON-style API reads
- `http.*` and `https.*` expose the lower-level HTTP boundary when you need raw request/response control

```aivi
User = { name: Text, age: Int, gender: Text }

usersSource : Source RestApi (List User)
usersSource =
  rest.fetch {
    method: "GET"
    url: ~u(https://api.example.com/users)
    headers: []
    body: None
    timeoutMs: Some 5_000
    retryCount: Some 2
    bearerToken: Some apiToken
    strictStatus: Some True
  }
```

Here `apiToken` stands for a bearer token your program obtained elsewhere. If the endpoint is public, leave `bearerToken` out entirely.

For request options and examples, see [REST / HTTP Sources](external_sources/rest_http.md).

## 12.4 Environment Sources (Env)

Use environment sources for deployment-time configuration such as ports, feature flags, secrets, and connection strings.

- `env.get` reads a single variable as `Text`
- `env.decode` reads a prefixed group of variables and decodes them into a record or other type

```aivi
AppConfig = { port: Int, debug: Bool }

appConfig : Source Env AppConfig
appConfig =
  env.decode {
    prefix: "AIVI_APP"
    schema: source.schema.derive
  }
```

For practical patterns, see [Environment Sources](external_sources/environment.md). The short form `env.decode "AIVI_APP"` still works when you want the smallest possible example.

## 12.5 Email Sources

Email sources are useful when you want to treat a mailbox read as typed input, for example reading unseen support requests or extracting structured messages from an inbox.

<<< ../snippets/from_md/syntax/external_sources/imap_email/block_02.aivi{aivi}

The host, username, token, and mailbox values in examples are placeholders. Replace them with your provider's settings and load secrets from environment variables or another secure boundary rather than hard-coding them in real programs.

For IMAP-specific guidance, see [IMAP Email Sources](external_sources/imap_email.md).

## 12.6 Image Sources

Image sources let you read metadata and pixel-oriented image data through the same typed source model.

<<< ../snippets/from_md/syntax/external_sources/image/block_01.aivi{aivi}

For practical examples, see [Image Sources](external_sources/image.md).

## 12.7 Compile-Time Sources (`@static`)

Some sources are read during compilation and embedded into the program. This is useful for build metadata, checked-in schemas, generated clients, and other inputs that should be fixed when the binary is built.

<<< ../snippets/from_md/syntax/external_sources/compile_time_sources_static.aivi{aivi}

For supported patterns and caveats, see [Compile-Time Sources](external_sources/compile_time.md).

## 12.8 SourceError

A source is loaded through an effect:

```aivi
load : Source K A -> Effect (SourceError K) A
```

Here `K` is still the source kind from earlier sections, such as `File`, `RestApi`, `Env`, or `Imap`.

`SourceError K` tells you whether the failure happened while reaching the source or while decoding its data:

```aivi
SourceError K =
  | IOError Text
  | DecodeError (List aivi.validation.DecodeError)
```

- `IOError` means the program could not reach or read the external system
- `DecodeError` means the read succeeded, but the payload did not match the expected shape
- `source.decodeErrors` extracts just the structured decode/validation problems when you want to display or test them separately from transport failures

Typical API shape:

<<< ../snippets/from_md/syntax/external_sources/sourceerror_01.aivi{aivi}

If you want to handle failures as ordinary data, use `attempt`:

<<< ../snippets/from_md/syntax/external_sources/sourceerror_02.aivi{aivi}

## 12.9 Capability mapping

Defining a source is pure. The capability requirement appears when the source is loaded:

- `load (file.*)` / `load (file.image*)` → `file.read`
- `load (rest.*)` / `load (http.*)` / `load (https.*)` → `network.http`
- `load (env.*)` → `process.env.read`
- `load (email.imap ...)` and other mail/network connectors → `network`
- `@static` embedded sources → no runtime capability after compilation

See [Capabilities](capabilities.md) for the standard vocabulary.

## 12.10 Source Composition

Sometimes “read and decode this value” is not enough. You may also want to:

- normalize the payload,
- run semantic validation,
- retry network acquisition,
- cache successful results,
- attach provenance or observability metadata.

Source composition keeps those policies attached to the source definition so `load` can execute them in a predictable order.

See [Source Composition](external_sources/composition.md) for the full stage model and examples.
