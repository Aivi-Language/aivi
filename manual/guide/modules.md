# Modules

Each `.aivi` file is a module. Modules import names with `use` and expose names with `export`.

## Importing with `use`

```aivi
use aivi.network (
    http
    socket
    Request
    Channel
)

type PrimaryRequest = (Request Text)

type ProviderChannel = (Channel Text Text)
```

Imported names become available to the rest of the file.

## Import aliases

Use `as` when you want a local name that differs from the exported one:

```aivi
use aivi.network (
    http as primaryHttp
    Request as HttpRequest
)

type RequestPayload = (HttpRequest Text)

value selectedProvider = primaryHttp
```

## Exporting names

You can export one name:

```aivi
value greeting = "hello"

export greeting
```

Or several names together:

```aivi
type Direction =
  | Up
  | Down
  | Left
  | Right

type Direction -> Direction
func opposite = .
 ||> Up    -> Down
 ||> Down  -> Up
 ||> Left  -> Right
 ||> Right -> Left

value startDirection : Direction = Right

export (Direction, opposite, startDirection)
```

## A small complete module

```aivi
use aivi.network (
    http
    socket
)

type Text -> Text -> Text
func joinProviders = left right =>
    "{left}/{right}"

value primaryProvider = http
value fallbackProvider = socket
value providerPair = joinProviders primaryProvider fallbackProvider

export providerPair
```

## Typical module layout

A practical order is:

1. `use`
2. `type` / `domain` / `class`
3. `func` and `value`
4. `signal`
5. `export`

That ordering is not required by the language, but it keeps modules easy to scan.

## Making names globally available with `hoist`

`hoist` makes an entire module's exports available project-wide, without needing `use` in every file.

```aivi
hoist aivi.list
```

After this declaration (typically placed in a shared module like `aivi.aivi`), every file in the project can use `map`, `filter`, `length`, etc. from `aivi.list` without importing them explicitly.

### Kind filters

Hoist only specific kinds of exports:

```aivi
hoist aivi.list (func, value)
```

Valid kind filters: `func`, `value`, `signal`, `type`, `domain`, `class`.

### Hiding specific names

Suppress individual names from the hoist:

```aivi
hoist aivi.list hiding (head, tail)
```

Combine kind filters and hiding:

```aivi
hoist aivi.list (func) hiding (foldr, foldl)
```

### Name disambiguation

When two hoisted modules export the same name (e.g. `map` from both `aivi.list` and `aivi.option`), the compiler resolves the correct one from type context:

```aivi
# <unparseable item>

value doubled
```

If the type context is insufficient, the compiler reports an error and suggests using `hiding` to exclude one of the conflicting names.

### Priority order

```
local definitions > use imports > hoisted globals > ambient prelude
```

`use` imports always win over `hoist` for the same name, so you can always override a hoisted name locally.

## Summary

| Form | Meaning |
| --- | --- |
| `use module (names)` | Import selected names |
| `use module (name as localName)` | Import one name under a local alias |
| `export name` | Export one name |
| `export (a, b, c)` | Export several names |
| `hoist module` | Make all module exports globally available |
| `hoist module (func, value)` | Hoist only selected kinds |
| `hoist module hiding (a, b)` | Hoist all except named items |
