# Opaque Types

An **opaque type** exposes a public type name while hiding its representation outside the module that defines it.
That gives you a practical way to enforce invariants: callers can store and pass the type, but they cannot construct invalid values or depend on internal details.

## Why use an opaque type?

Without `opaque`, a plain type alias is transparent everywhere.
For example, if `Url` were just a record alias, any module could create a malformed value directly:

<<< ../../snippets/from_md/syntax/types/opaque_types/block_01.aivi{aivi}


That bypasses your parsing and validation logic.
`opaque` closes that escape hatch and forces callers through the constructors, accessors, and domains you choose to expose.

## Syntax

Use `opaque` when you want a record, ADT, branded alias, or ordinary alias to stay abstract outside its defining module:

<<< ../../snippets/from_md/syntax/types/opaque_types/block_02.aivi{aivi}


## What changes inside and outside the module

### Inside the defining module

Inside the module that defines the type, `opaque` behaves as if it were not there.
You can construct values, inspect fields, update records, and pattern match normally.

<<< ../../snippets/from_md/syntax/types/opaque_types/block_03.aivi{aivi}


### Outside the defining module

Outside the defining module, only the type name is visible.
The structure is hidden.

| Operation | Allowed? | Notes |
| --- | --- | --- |
| Type name in signatures | ✅ | `f : Url -> Text` is fine |
| Exported functions | ✅ | Use smart constructors, accessors, and helpers |
| Exported domain operators | ✅ | Operators still work when the module exports `domain Url` and callers import that domain |
| Class instances | ✅ | `Eq`, `ToText`, and similar instances work normally |
| Record literal construction | ❌ | Callers cannot build the hidden representation directly |
| Field access (`url.host`) | ❌ | Hidden fields stay hidden |
| Record update (`url <| { ... }`) | ❌ | Updates would depend on the hidden layout |
| Pattern match on structure | ❌ | Matching would reveal internals |
| ADT constructor (`Red`, `Green`) | ❌ | For opaque ADTs, constructors are hidden too |

If you want operator syntax or suffix literals to be part of the public API, export the matching domain too. See [Domains](../domains.md) and [Modules](../modules.md) for the import and export rules.

## Designing the public API

The defining module decides what operations to export.
A common pattern is:

- export the type name
- export one or more validated constructors or parsers
- export read-only accessor functions
- export `domain ...` only when operator or literal syntax is part of the public API
- export class instances as needed

<<< ../../snippets/from_md/syntax/types/opaque_types/block_04.aivi{aivi}


Callers then work through the exported surface instead of the hidden representation:

<<< ../../snippets/from_md/syntax/types/opaque_types/block_01.aivi{aivi}


If the module also exports a domain, callers must import that domain explicitly with `use some.module (domain Name)`. A plain `use some.module` does not activate domain-owned operators or suffixes.

## Interaction with domains

Opaque types and domains fit together naturally.
The domain implementation lives in the same module as the opaque type, so it can work with the hidden representation while callers only see the exported behavior.

<<< ../../snippets/from_md/syntax/types/opaque_types/block_02.aivi{aivi}


For the full domain syntax, see [Domains](../domains.md). For the module-level `export domain ...` / `use ... (domain ...)` rules, see [Modules](../modules.md).

## Interaction with classes

Class instances for opaque types work normally.
Define the instance in code that can see the representation; using the instance does not reveal any internals.
See [Classes and Higher-Kinded Types](classes_and_hkts.md) for the instance syntax.

<<< ../../snippets/from_md/syntax/types/opaque_types/block_03.aivi{aivi}


## Diagnostics

When outside code tries to break opacity, the compiler reports the problem directly.
The current compile-fail suite explicitly covers field access and record update errors, for example:

```text
cannot update opaque type `Wrapper` outside module `testOpaqueUpdate.definer`
cannot access field `inner` on opaque type `Wrapper` outside module `testOpaque.definer`
```

Structural pattern matches are rejected for the same reason: destructuring would reveal the hidden representation.

## Existing runtime-opaque handles

AIVI also has runtime-managed handle types such as [`FileHandle`](../../stdlib/system/file.md), [`Listener`](../../stdlib/network/sockets.md), [`Connection`](../../stdlib/network/sockets.md), [`DbConnection`](../../stdlib/system/database.md), [`Server`](../../stdlib/network/http_server.md), and [`WebSocket`](../../stdlib/network/http_server.md).
They are opaque for a different reason: they wrap host-language values, so AIVI code never sees an AIVI-level definition for their internals.

<<< ../../snippets/from_md/syntax/types/opaque_types/block_04.aivi{aivi}


By contrast, `opaque Url = { ... }` does have an AIVI-level definition, but that definition is hidden outside the module.

| Form | Has AIVI-level definition? | Visible outside? |
| --- | --- | --- |
| `FileHandle` (runtime opaque) | No | Only as a type name |
| `opaque Url = { ... }` (module opaque) | Yes | Only as a type name |
| `Url = { ... }` (transparent) | Yes | Fully visible |

## See also

- [The Type System](../types.md) for where opaque types fit among the other type forms
- [Domains](../domains.md) for domain definitions and imports
- [Modules](../modules.md) for `export domain` and import-list syntax
- [`aivi.url`](../../stdlib/system/url.md) for a concrete opaque-type API in the standard library
