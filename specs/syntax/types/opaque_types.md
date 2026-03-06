# Opaque Types

An **opaque type** hides its internal representation outside the defining module, preventing direct construction, field access, record update, and pattern matching on its structure. Only functions and domain operators exported by the defining module can manipulate the type.

## Motivation

Without opaque types, a record type alias like `Url` is fully transparent everywhere:

```aivi
// anyone can write this — no validation, no guarantee
badUrl = { protocol: "http", host: "", port: None, path: "", query: [], hash: None }
```

This bypasses `Url.parse` and its validation, allowing invalid values to propagate silently. The `opaque` keyword fixes this by restricting who can construct and destructure the type.

## Syntax

The `opaque` keyword can precede any type definition form:

```aivi
// Opaque record (most common use case)
opaque Url = {
  protocol: Text
  host: Text
  port: Option Int
  path: Text
  query: List (Text, Text)
  hash: Option Text
}

// Opaque ADT — hides constructors outside the module
opaque Color = Red | Green | Blue

// Opaque branded type
opaque Email = Text!

// Opaque plain alias
opaque UserId = Int
```

## Semantics

### Inside the defining module (transparent)

Inside the module that declares the `opaque` type, the type behaves as if `opaque` were absent. All operations work normally: construction, field access, record update, and pattern matching.

```aivi
module aivi.url

opaque Url = {
  protocol: Text
  host: Text
  port: Option Int
  path: Text
  query: List (Text, Text)
  hash: Option Text
}

// ✅ All of these work inside the defining module:
example = { protocol: "https", host: "example.com", port: None, path: "/", query: [], hash: None }
p = example.protocol
updated = example <| { host: "other.com" }
f = url => url match
  | { protocol: "https" } => True
  | _                      => False
```

### Outside the defining module (opaque)

Outside the module, only the *type name* is visible — its structure is hidden.

| Operation | Allowed? | Notes |
|---|---|---|
| Type name in signatures | ✅ | `f : Url -> Text` is fine |
| Exported functions | ✅ | `parse`, `toString`, accessor functions |
| Domain operators | ✅ | `url + ("key", "val")` via `domain Url` |
| Class instances | ✅ | `Eq`, `Show`, etc. work normally |
| Record literal construction | ❌ | Compile error |
| Field access (`url.host`) | ❌ | Compile error |
| Record update (`url <| { ... }`) | ❌ | Compile error |
| Pattern match on structure | ❌ | Compile error |
| ADT constructor (`Red`, `Green`) | ❌ | Compile error (for opaque ADTs) |

### Providing an API surface

The defining module exports whatever operations it wants to expose:

```aivi
module aivi.url

export Url, parse, toString, protocol, host, port, path, query, hash

opaque Url = { ... }

// Smart constructor (the only way to create a Url from outside)
parse : Text -> Result UrlError Url
parse = text => ...

// Accessor functions
protocol : Url -> Text
protocol = url => url.protocol

host : Url -> Text
host = url => url.host

// ... etc.
```

Outside code uses the exported API:

```aivi
use aivi.url (Url, parse, protocol)

myUrl = parse "https://example.com" or panic "bad url"
p = protocol myUrl   // ✅ via exported accessor
```

## Interaction with domains

Domain operators declared `over` an opaque type work normally from outside the module, because the domain operator implementations live inside the defining module (or are explicitly granted access):

```aivi
use aivi.url
use aivi.url (domain Url)

url = Url.parse "https://example.com" or panic "bad"
url2 = url + ("q", "search")  // ✅ domain operator works
```

## Interaction with classes

Class instances for opaque types work normally. Instance declarations must be in the defining module (since they need access to internals), but instance usage works everywhere:

```aivi
url1 == url2        // ✅ Eq instance works
toText url1         // ✅ Show/ToText instance works
```

## Diagnostics

When code outside the defining module tries to violate opacity, the compiler emits a clear error:

```
error[E4100]: cannot construct opaque type `Url` outside module `aivi.url`
  --> app.aivi:5:1
   |
 5 | bad = { protocol: "http", host: "" ... }
   |       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ opaque type cannot be constructed here
   |
   = help: use `Url.parse` or another exported constructor from `aivi.url`

error[E4101]: cannot access field `host` on opaque type `Url` outside module `aivi.url`
  --> app.aivi:8:7
   |
 8 | h = url.host
   |         ^^^^ field access on opaque type
   |
   = help: use an exported accessor function from `aivi.url`
```

## Existing handle-based opaque types

AIVI already has types that are opaque at the runtime level — `FileHandle`, `Listener`, `Connection`, `DbConnection`, `Server`, `WebSocket`, etc. These are declared without a right-hand side:

```aivi
FileHandle
```

These have **no** AIVI-level structure at all (they wrap Rust values). The `opaque` keyword is different: the type *has* a definition, but that definition is hidden outside the module.

| Form | Has definition? | Visible outside? |
|---|---|---|
| `FileHandle` (runtime opaque) | No | Only as a type name |
| `opaque Url = { ... }` (module opaque) | Yes | Only as a type name |
| `Url = { ... }` (transparent) | Yes | Fully visible |
