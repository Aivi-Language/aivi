# External Sources

External data enters AIVI through typed **Sources**.

---

## 12.1 Source Types

```aivi
Source K A
```

- `K` — the **kind** of source (File, Http, Db, etc.)
- `A` — the **decoded type** of the content

Sources are effectful but typed at compile time.

---

## 12.2 JSON Sources

### Schema Inference

```aivi
users : Source Http (List User)
users = http.get `https://api.example.com/users`
```

The compiler infers/validates that the response matches `List User`.

### Inline Schema

```aivi
config : Source File { port: Int, host: Text }
config = file.json `./config.json`
```

Type mismatch at runtime produces a typed error:

```aivi
Result (SourceError File) { port: Int, host: Text }
```

### Dynamic JSON

When schema is unknown:

```aivi
raw : Source File Json
raw = file.json `./unknown.json`

-- Access dynamically
port = raw.get `port` |> Json.asInt
```

---

## 12.3 Image Sources

Images are typed by their metadata:

```aivi
ImageMeta = { width: Int, height: Int, format: ImageFormat, channels: Int }
ImageFormat = Png | Jpeg | Webp | Gif

logo : Source File (Image { width: 512, height: 512, format: Png })
logo = file.image `./logo.png`
```

The type encodes expected dimensions/format. Mismatches are compile-time errors when knowable, runtime errors otherwise.

### Pixel Access

```aivi
domain Image over ImageData = {
  (!) : ImageData -> (Int, Int) -> Rgba
  (!) img (x, y) = getPixel img x y
}

pixel = logo ! (10, 20)
```

---

## 12.4 HTTP Sources

```aivi
Http = { method: Method, url: Url, headers: Headers, body: Option Body }
Method = Get | Post | Put | Delete | Patch

api : Source Http (Result ApiError User)
api = http.request {
  method: Get
  url: `https://api.example.com/user/1`
  headers: [ (`Authorization`, `Bearer {token}`) ]
}
```

### Response Typing

```aivi
HttpResponse A = {
  status: Int
  headers: Headers
  body: A
}

-- Explicit response wrapper
fullResponse : Source Http (HttpResponse User)
fullResponse = http.getWithMeta `https://api.example.com/user/1`
```

---

## 12.5 File Streams

For large files, stream instead of loading:

```aivi
lines : Source File (Generator Text)
lines = file.lines `./large.csv`

-- Process lazily
processed = lines |> map parseCsvRow |> filter _.valid
```

Generators integrate with Source for backpressure-aware processing.

---

## 12.6 Binary Sources

```aivi
wasm : Source File Bytes
wasm = file.bytes `./module.wasm`

-- Typed binary parsing
header : WasmHeader
header = wasm |> Bytes.take 8 |> parseWasmHeader
```

---

## 12.7 Source Composition

Sources can be chained:

```aivi
-- Fetch config, then use it to fetch data
pipeline = do
  cfg <- file.json `./config.json`
  users <- http.get cfg.apiUrl
  pure users
```

---

## 12.8 Compile-Time Sources

Some sources resolve at compile time:

```aivi
@static
version : Text
version = file.read `./VERSION`
```

The `@static` decorator embeds the file content into the compiled WASM.
