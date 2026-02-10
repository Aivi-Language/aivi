# External Sources

External data enters AIVI through typed **Sources**. A source represents a persistent connection or a one-off fetch to an external system, with full type safety enforced during decoding.

## 12.1 The Source Type

```aivi
Source K A
```

- `K` — the **kind** of source (File, Http, Db, etc.)
- `A` — the **decoded type** of the content

Sources are effectful. Loading a source performs I/O and returns an `Effect E A` (where `E` captures the possible source errors). All source interactions must occur within an `effect` block.

Typical API shape:

```aivi
load : Source K A -> Effect (SourceError K) A
```

To handle errors as values, use `attempt` (see [Effects](09_effects.md)):

```aivi
effect {
  res <- attempt (load (file.read "./README.md"))
  res ?
    | Ok txt => pure txt
    | Err _  => pure "(missing)"
}
```


## 12.2 File Sources

Used for local system access. Supports structured (JSON, CSV) and unstructured (Bytes, Text) data.

```aivi
// Read entire file as text
readme = file.read "./README.md"

// Stream bytes from a large file
blob = file.stream "./large.bin"

// Read structured CSV (the expected type drives decoding)
User = { id: Int, name: Text, email: Text }
users : Source File (List User)
users = file.csv "./users.csv"
```


## 12.3 HTTP Sources

Typed REST/API integration.

```aivi
User = { id: Int, name: Text }

// Typed GET request (inferred type)
users = http.get ~u(https://api.example.com/v1/users)

// Request with headers and body
req = http.request {
  method: Post
  url: ~u(https://api.example.com/v1/users)
  headers: [("Content-Type", "application/json")]
  body: Some (Json.encode { name: "New User" })
}
```


## 12.4 Environment Sources (Env)

Typed access to environment configuration. Values are decoded using the expected type and optional defaults.

```aivi
// Read a single environment variable as Text
appEnv : Source Env Text
appEnv = env.get "APP_ENV"

// Decode a typed configuration record with defaults
DbConfig = {
  driver: Text
  url: Text?
  host: Text?
  port: Int?
  user: Text?
  password: Text?
  database: Text?
}

defaultDbConfig : DbConfig
defaultDbConfig = {
  driver: "sqlite"
  url: Some "./local.db"
  host: None
  port: None
  user: None
  password: None
  database: None
}

dbConfig : Source Env DbConfig
dbConfig = env.decode defaultDbConfig
```

## 12.5 Database Sources (Db)

Integration with relational and document stores. Uses carrier-specific domains for querying.

```aivi
// SQLite connection
db = sqlite.open "./local.db"

User = { id: Int, name: Text }

// Typed query source (the expected type drives decoding)
activeUsers : Source Db (List User)
activeUsers = db.query "SELECT id, name FROM users WHERE active = 1"
```

See the [Database Domain](../05_stdlib/03_system/23_database.md) for table operations, deltas, and migrations.


## 12.6 Email Sources

Interacting with mail servers (IMAP/SMTP).

```aivi
// Fetch unread emails
inbox = email.imap {
  host: "imap.gmail.com"
  filter: "UNSEEN"
}

// Sending as a sink effect
sendWelcome = user => email.send {
  to: user.email
  subject: "Welcome!"
  body: "Glad to have you, {user.name}"
}
```


## 12.7 LLM Sources

AIVI treats Large Language Models as typed probabilistic sources. This is a core part of the AIVI vision for intelligent data pipelines.

```aivi
// Define expected output shape
Sentiment = Positive | Negative | Neutral

Analysis = {
  sentiment: Sentiment
  summary: Text
}

// LLM completion with strict schema enforcement
analyze input = llm.complete {
  model: "gpt-4o"
  prompt: "Analyze this feedback: {input}"
  schema: Analysis
}
```


## 12.8 Image Sources

Images are typed by their metadata and pixel data format.

```aivi
Image A = { width: Int, height: Int, format: ImageFormat, pixels: A }

// Load image metadata only
meta = file.imageMeta "./photo.jpg"

// Load full image with RGB pixel access
photo = file.image "./photo.jpg"
```


## 12.9 S3 / Cloud Storage Sources

Integration with object storage.

```aivi
// Bucket listings
images = s3.bucket "my-assets" |> s3.list "thumbnails/"

// Fetch object content
logo = s3.get "my-assets" "branding/logo.png"
```

> [!NOTE]
> Browser sources are part of the AIVI long-term vision for end-to-end automation but are considered **Experimental** and may not be fully available in the initial WASM-targeted phase.


## 12.10 Compile-Time Sources (@static)

Some sources are resolved at compile time and embedded into the binary. This ensures zero latency/failure at runtime.

```aivi
@static
version = file.read "./VERSION"

@static
locales = file.json "./i18n/en.json"
```
