# External Sources

External data enters AIVI through typed **Sources**. A source represents a persistent connection or a one-off fetch to an external system, with full type safety enforced during decoding.

## 12.1 The Source Type

```aivi
Source K A
```

- `K` — the **kind** of source (File, Http, Db, etc.)
- `A` — the **decoded type** of the content

Sources are effectful. Loading a source performs I/O and returns a `Result Error A`. All source interactions must occur within an `effect` block.

---

## 12.2 File Sources

Used for local system access. Supports structured (JSON, CSV) and unstructured (Bytes, Text) data.

```aivi
// Read entire file as text
readme : Source File Text
readme = file.read "./README.md"

// Stream bytes from a large file
blob : Source File (Generator Bytes)
blob = file.stream "./large.bin"

// Read structured CSV with schema
@schema "id:Int,name:Text,email:Text"
users : Source File (List User)
users = file.csv "./users.csv"
```

---

## 12.3 HTTP Sources

Typed REST/API integration.

```aivi
User = { id: Int, name: Text }

// Typed GET request (inferred type)
users : Source Http (List User)
users = http.get "https://api.example.com/v1/users"

// Request with headers and body
req = http.request {
  method: Post
  url: "https://api.example.com/v1/users"
  headers: [("Content-Type", "application/json")]
  body: Some (Json.encode { name: "New User" })
}
```

---

## 12.4 Database Sources (Db)

Integration with relational and document stores. Uses carrier-specific domains for querying.

```aivi
// SQLite connection
db = sqlite.open "./local.db"

// Typed query source
@sql "SELECT id, name FROM users WHERE active = 1"
activeUsers : Source Db (List User)
activeUsers = db.query
```

---

## 12.5 Email Sources

Interacting with mail servers (IMAP/SMTP).

```aivi
// Fetch unread emails
inbox : Source Email (List Message)
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

---

## 12.6 LLM Sources

AIVI treats Large Language Models as typed probabilistic sources. This is a core part of the AIVI vision for intelligent data pipelines.

```aivi
// Define expected output shape
Sentiment = Positive | Negative | Neutral

Analysis = {
  sentiment: Sentiment
  summary: Text
}

// LLM completion with strict schema enforcement
@model "gpt-4o"
analyze : Text -> Source Llm Analysis
analyze input = llm.complete {
  prompt: "Analyze this feedback: {input}"
  schema: Analysis
}
```

---

## 12.7 Image Sources

Images are typed by their metadata and pixel data format.

```aivi
Image A = { width: Int, height: Int, format: ImageFormat, pixels: A }

// Load image metadata only
meta : Source File ImageMeta
meta = file.imageMeta "./photo.jpg"

// Load full image with RGB pixel access
photo : Source File (Image ImageData)
photo = file.image "./photo.jpg"
```

---

## 12.8 S3 / Cloud Storage Sources

Integration with object storage.

```aivi
// Bucket listings
images : Source S3 (List S3Object)
images = s3.bucket "my-assets" |> s3.list "thumbnails/"

// Fetch object content
logo : Source S3 Bytes
logo = s3.get "my-assets" "branding/logo.png"
```

---
> [!NOTE]
> Browser sources are part of the AIVI long-term vision for end-to-end automation but are considered **Experimental** and may not be fully available in the initial WASM-targeted phase.

---

## 12.10 Compile-Time Sources (@static)

Some sources are resolved at compile time and embedded into the binary. This ensures zero latency/failure at runtime.

```aivi
@static
version : Text
version = file.read "./VERSION"

@static
locales : Json
locales = file.json "./i18n/en.json"
```
