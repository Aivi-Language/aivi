# Values and Types

Every piece of data in AIVI has a type.
Types in AIVI are **closed**: the compiler knows every possible shape a value of that type can have.
There is no `any`, no `object`, no `null`.

## `val` — a named constant

```aivi
val answer = 42
val greeting = "Hello, world!"
val pi = 3.14159
```

`val` declares an immutable, named value. There are no variables in AIVI — once declared,
a value does not change. If something needs to change over time, that is a `sig` (covered in
[Chapter 05](/tour/05-signals)).

You can annotate the type explicitly:

```aivi
val answer:Int    = 42
val greeting:Text = "Hello, world!"
```

The compiler infers types when they are omitted, but annotations are welcome for documentation.

## `type` — defining your own types

AIVI has two flavours of `type`: **sum types** and **product types**.

### Sum types (variants)

A sum type lists all possible values (variants). If you have used TypeScript unions or Rust enums,
this is the same idea — but exhaustive and closed.

```aivi
type Direction = Up | Down | Left | Right

type Status = Running | Paused | GameOver

type Bool = True | False
```

Each variant is a constructor — a value of that type.
You can write `Up` or `GameOver` directly; they are ordinary values.

### Sum types with data

Variants can carry data:

```aivi
type Maybe A = Some A | None

type Result A = Ok A | Err Text

type Shape =
  | Circle Int          -- radius
  | Rectangle Int Int   -- width, height
```

`Maybe` and `Result` are **parametric types** — the type variable `A` is filled in at use sites:

```aivi
val found:Maybe Int    = Some 42
val missing:Maybe Int  = None
val success:Result Int = Ok 100
val failure:Result Int = Err "not found"
```

The type variable is always lowercase; type names and constructors are uppercase.

### No null

AIVI has no `null`, `nil`, or `undefined`. The absence of a value is always explicit:

```aivi
type Maybe A = Some A | None

val username:Maybe Text = None        -- not logged in
val username:Maybe Text = Some "ada"  -- logged in
```

Because you cannot ignore the `None` case (the compiler enforces it), null pointer bugs are
impossible by construction.

### Product types (records)

A product type groups multiple named fields into one value:

```aivi
type Point = {
    x: Int,
    y: Int
}

type User = {
    id:       Int,
    username: Text,
    email:    Text
}
```

Create a record by listing its fields:

```aivi
val origin:Point = { x: 0, y: 0 }
val user:User    = { id: 1, username: "ada", email: "ada@example.com" }
```

Access fields with dot projection:

```aivi
val name = user.username   -- "ada"
val x    = origin.x        -- 0
```

Records are immutable — you cannot update a field in place. Instead, create a new record:

```aivi
val movedUser:User = {
    id:       user.id,
    username: "lovelace",
    email:    user.email
}
```

### Combining sum and product types

Real programs combine both. Here is a snapshot from the Snake demo:

```aivi
type Vec2   = Vec2 Int Int
type Status = Running | GameOver

type Snake = {
    head:   Vec2,
    second: Vec2,
    length: Int
}

type Game = {
    snake:  Snake,
    status: Status,
    score:  Int
}
```

`Vec2` is a sum type with one variant that carries two `Int` values.
`Snake` and `Game` are product types whose fields reference other types.

## Built-in types

| Type | Description | Example |
|---|---|---|
| `Int` | Signed integer | `42`, `-7` |
| `Float` | Floating-point | `3.14` |
| `Bool` | `True` or `False` | `True` |
| `Text` | UTF-8 string | `"hello"` |
| `List A` | Homogeneous list | `[1, 2, 3]` |

`Bool` is a regular sum type under the hood: `type Bool = True | False`.
The operators `and`, `or`, and `not` work on it.

## Summary

- `val` = named constant.
- `type X = A | B` = sum type (pick one variant).
- `type X = { field: T }` = product type (all fields together).
- Type variables are lowercase; type names and constructors are uppercase.
- No null — use `Maybe A`.

[Next: Functions →](/tour/02-functions)
