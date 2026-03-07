# Opaque Types

An **opaque type** exposes a public type name while hiding its representation outside the module that defines it.
That gives you a practical way to enforce invariants: callers can use the type, but they cannot construct invalid values or depend on internal details.

## Why use an opaque type?

Without `opaque`, a plain type alias is transparent everywhere.
For example, if `Url` were just a record alias, any module could create a malformed value directly:

```aivi
badUrl = {
  protocol: "http",
  host: "",
  port: None,
  path: "",
  query: [],
  hash: None
}   // no validation happens here
```

That bypasses your parsing and validation logic.
`opaque` closes that escape hatch and forces callers through the API you choose to expose.

## Syntax

The `opaque` keyword can be used with any type definition form:

```aivi
opaque Url = {
  protocol: Text,
  host: Text,
  port: Option Int,
  path: Text,
  query: List (Text, Text),
  hash: Option Text
}

opaque Color = Red | Green | Blue   // constructors are hidden outside the module
opaque Email = Text!                // opaque branded type
opaque UserId = Int                 // opaque alias
```

## What changes inside and outside the module

### Inside the defining module

Inside the module that defines the type, `opaque` behaves as if it were not there.
You can construct values, inspect fields, update records, and pattern match normally.

```aivi
module aivi.url

opaque Url = {
  protocol: Text,
  host: Text,
  port: Option Int,
  path: Text,
  query: List (Text, Text),
  hash: Option Text
}

example = {
  protocol: "https",
  host: "example.com",
  port: None,
  path: "/",
  query: [],
  hash: None
}

p = example.protocol                  // field access is allowed here
updated = example <| { host: "other.com" }

usesHttps = url => url match
  | { protocol: "https" } => True   // pattern matching is also allowed here
  | _                      => False
```

### Outside the defining module

Outside the defining module, only the type name is visible.
The structure is hidden.

| Operation | Allowed? | Notes |
| --- | --- | --- |
| Type name in signatures | ✅ | `f : Url -> Text` is fine |
| Exported functions | ✅ | Use smart constructors, accessors, and helpers |
| Domain operators | ✅ | Domain methods can still work through exported APIs |
| Class instances | ✅ | `Eq`, `Show`, `ToText`, and similar instances work normally |
| Record literal construction | ❌ | Callers cannot build the hidden representation directly |
| Field access (`url.host`) | ❌ | Hidden fields stay hidden |
| Record update (`url <| { ... }`) | ❌ | Updates would depend on the hidden layout |
| Pattern match on structure | ❌ | Matching would reveal internals |
| ADT constructor (`Red`, `Green`) | ❌ | For opaque ADTs, constructors are hidden too |

## Designing the public API

The defining module decides what operations to export.
A common pattern is:

- export the type name
- export one or more validated constructors
- export read-only accessor functions
- export domain operations or class instances as needed

```aivi
module aivi.url

export Url, parse, toString, protocol, host, port, path, query, hash

opaque Url = { ... }

parse : Text -> Result UrlError Url
parse = text => ...                 // validated constructor

protocol : Url -> Text
protocol = url => url.protocol      // safe accessor

host : Url -> Text
host = url => url.host
```

Callers then work through the exported surface instead of the hidden representation:

```aivi
use aivi.url (Url, parse, protocol)

myUrl = parse "https://example.com" or panic "bad url"
p = protocol myUrl                  // use the exported accessor
```

## Interaction with domains

Domain operators declared over an opaque type still work outside the module because their implementations live in code that is allowed to see the internals.

```aivi
use aivi.url
use aivi.url (domain Url)

url = Url.parse "https://example.com" or panic "bad"
url2 = url + ("q", "search")      // operator works through the exported domain API
```

## Interaction with classes

Class instances for opaque types work normally.
The instance definition belongs in code that can see the representation, but using the instance does not reveal any internals.

```aivi
url1 == url2        // `Eq` works
toText url1         // `ToText` or similar conversion works
```

## Diagnostics

When outside code tries to break opacity, the compiler reports the problem directly:

```text
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

## Existing runtime-opaque handles

AIVI also has handle-like types such as `FileHandle`, `Listener`, `Connection`, `DbConnection`, `Server`, and `WebSocket` that are opaque for a different reason: they do not have an AIVI-level definition at all.

```aivi
FileHandle
```

Those runtime handles wrap host-language values.
By contrast, `opaque Url = { ... }` has a definition, but that definition is hidden outside the module.

| Form | Has definition? | Visible outside? |
| --- | --- | --- |
| `FileHandle` (runtime opaque) | No | Only as a type name |
| `opaque Url = { ... }` (module opaque) | Yes | Only as a type name |
| `Url = { ... }` (transparent) | Yes | Fully visible |
