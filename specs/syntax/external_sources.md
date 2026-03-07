# External Sources

External sources are AIVI's typed way to read data that lives outside your program: files, environment variables, HTTP APIs, email, images, databases, and more.

If you are not used to functional programming, a helpful mental model is this:

- a `Source K A` is a **recipe** for getting some external data,
- `K` says **where it comes from**,
- `A` says **what type you want after decoding**,
- nothing is actually read until you call `load`.

That separation keeps I/O explicit. You can define a source once, pass it around like any other value, and decide later where the actual read should happen.

## Source Guides

- [Schema-First Source Definitions](external_sources/schema_first.md)
- [Source Composition](external_sources/composition.md)
- [File Sources](external_sources/file.md)
- [REST / HTTP Sources](external_sources/rest_http.md)
- [Environment Sources](external_sources/environment.md)
- [IMAP Email Sources](external_sources/imap_email.md)
- [Image Sources](external_sources/image.md)
- [Compile-Time Sources](external_sources/compile_time.md)

## 12.1 The Source Type

<<< ../snippets/from_md/syntax/external_sources/the_source_type.aivi{aivi}

- `K` is the **kind** of source, such as `File`, `RestApi`, `Env`, or `Imap`
- `A` is the **decoded result type**

In day-to-day code, a source gives you a clear boundary between "outside data" and "trusted, typed values inside the program".

### A small example

```aivi
User = { id: Int, name: Text }

usersSource : Source File (List User)
usersSource = file.json "./users.json"  -- describe the boundary

do Effect {
  users <- load usersSource             -- perform the actual read
  pure users
}
```

### SourceError

A `Source K A` is loaded through an effect:

```aivi
load : Source K A -> Effect (SourceError K) A
```

`SourceError K` separates transport problems from data-shape problems:

```aivi
SourceError K =
  | IOError Text
  | DecodeError (List aivi.validation.DecodeError)
```

- `IOError` means the program could not reach or read the external system
- `DecodeError` means the read succeeded, but the payload did not match the expected shape

Typical API shape:

<<< ../snippets/from_md/syntax/external_sources/sourceerror_01.aivi{aivi}

If you want to handle failures as ordinary data, use `attempt`:

<<< ../snippets/from_md/syntax/external_sources/sourceerror_02.aivi{aivi}

### Capability mapping

Defining a source is pure. The capability requirement appears when the source is loaded:

- `load (file.*)` / `load (file.image*)` → `file.read`
- `load (rest.*)` / `load (http.*)` / `load (https.*)` → `network.http`
- `load (env.*)` → `process.env.read`
- `load (email.imap ...)` and other mail/network connectors → `network`
- database-backed source reads → `db.query`
- `@static` embedded sources → no runtime capability after compilation

See [Capabilities](capabilities.md) for the standard vocabulary.

### A practical workflow

Most source-heavy programs follow the same pattern:

1. define the data type you want,
2. build a source that can decode into that type,
3. call `load` inside `do Effect { ... }`,
4. handle `SourceError` at the boundary.

That keeps parsing, validation, and I/O at the edge of the program instead of spread throughout the business logic.

## 12.2 File Sources

Use file sources for local configuration, checked-in fixtures, imports, and one-off data processing jobs.

- `file.read` gives you raw `Text`
- `file.json` decodes JSON into the type you ask for
- `file.csv` decodes rows into a typed list

<<< ../snippets/from_md/syntax/external_sources/file_sources.aivi{aivi}

For a practical guide, see [File Sources](external_sources/file.md).

## 12.3 HTTP Sources

Use HTTP or REST sources when your program reads typed data from a web service.

- `http.*` and `https.*` expose the lower-level HTTP boundary
- `rest.*` adds a REST-oriented surface for typed API work

<<< ../snippets/from_md/syntax/external_sources/http_sources.aivi{aivi}

For request options and examples, see [REST / HTTP Sources](external_sources/rest_http.md).

## 12.4 Environment Sources (Env)

Use environment sources for deployment-time configuration such as ports, feature flags, secrets, and connection strings.

- `env.get` reads a single variable as `Text`
- `env.decode` reads a prefixed group of variables and decodes them into a record or other type

<<< ../snippets/from_md/syntax/external_sources/environment_sources_env.aivi{aivi}

For practical patterns, see [Environment Sources](external_sources/environment.md).

## 12.5 Database Sources (Db)

Database sources apply the same idea to tables, projections, and query results: describe the boundary once, then decode rows into a typed result.

<<< ../snippets/from_md/syntax/external_sources/database_sources_db.aivi{aivi}

For table operations, deltas, and migrations, see the [Database Domain](../stdlib/system/database.md).

## 12.6 Email Sources

Email sources are useful when you want to treat a mailbox read as typed input, for example reading unseen support requests or extracting structured messages from an inbox.

<<< ../snippets/from_md/syntax/external_sources/email_sources.aivi{aivi}

For IMAP-specific guidance, see [IMAP Email Sources](external_sources/imap_email.md).

## 12.7 LLM Sources

AIVI models LLM boundaries as typed external inputs: you describe the shape you want back, then decode the response into that shape at the boundary.

<<< ../snippets/from_md/syntax/external_sources/llm_sources.aivi{aivi}

## 12.8 Image Sources

Image sources let you read metadata and pixel-oriented image data through the same typed source model.

<<< ../snippets/from_md/syntax/external_sources/image_sources.aivi{aivi}

For practical examples, see [Image Sources](external_sources/image.md).

## 12.9 S3 / Cloud Storage Sources

Object storage fits naturally into the source model: define the external object boundary, then load and decode it like any other source.

<<< ../snippets/from_md/syntax/external_sources/s3_cloud_storage_sources.aivi{aivi}

When portability matters, check the runtime-specific documentation for the storage connectors available in your target environment.

## 12.10 Compile-Time Sources (`@static`)

Some sources are read during compilation and embedded into the program. This is useful for build metadata, checked-in schemas, generated clients, and other inputs that should be fixed when the binary is built.

<<< ../snippets/from_md/syntax/external_sources/compile_time_sources_static.aivi{aivi}

For supported patterns and caveats, see [Compile-Time Sources](external_sources/compile_time.md).

## 12.11 Source Composition

Sometimes "read and decode this value" is not enough. You may also want to:

- normalize the payload,
- run semantic validation,
- retry network acquisition,
- cache successful results,
- attach provenance or observability metadata.

That is what source composition is for. It keeps those policies attached to the source definition so `load` can execute them in a predictable order.

See [Source Composition](external_sources/composition.md) for the full stage model and examples.
