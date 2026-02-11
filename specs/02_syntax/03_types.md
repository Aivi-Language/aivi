# The Type System

## 3.1 Primitive Types

AIVI distinguishes:

- **Compiler primitives**: types the compiler/runtime must know about to execute code.
- **Standard library types**: types defined in AIVI source (possibly with compiler-known representation in early implementations).

In v0.1, the recommended minimal set of **compiler primitives** is:

```aivi
Unit
Bool
Int
Float
```

Everything else below should be treated as a **standard library type** (even if an implementation chooses to represent it specially at first for performance/interop).

```aivi
Decimal
BigInt
Text
Bytes
Duration
Instant
Date
Time
TimeZone
ZonedDateTime
```

Numeric suffixes:

* `2024-05-21T12:00:00Z` → `Instant`
* `~d(2024-05-21)` → `Date`
* `~t(12:00:00)` → `Time`
* `~tz(Europe/Paris)` → `TimeZone`
* `~zdt(2024-05-21T12:00:00Z[Europe/Paris])` → `ZonedDateTime`


## 3.2 Algebraic Data Types

### `Bool`

`Bool` has exactly two values:

```aivi
True : Bool
False : Bool
```

`if ... then ... else ...` requires a `Bool` condition, and can be understood as desugaring to a `case` on `True`/`False`.

### Creating values (“objects”)

AIVI does not have “objects” in the OO sense. You create values using:

- **Constructors** for algebraic data types (ADTs)
- **Literals** for primitives and records
- **Domain-owned literals/operators** for domain types (e.g. `2w + 3d` for `Duration`)

```aivi
Option A = None | Some A
Result E A = Err E | Ok A
```

To create ADT values, apply constructors like ordinary functions:

```aivi
someCount = Some 123
okText    = Ok "done"
bad       = Err "nope"
```

Nullary constructors (like `None`, `True`, `False`) are values.

## 3.3 Open Records (Row Polymorphism)

Records are:

* structural
* open by default

```aivi
User = { id: Int, name: Text, email: Option Text }
```

To create a record value, use a record literal:

```aivi
alice : User
alice = { id: 1, name: "Alice", email: None }
```

Record literals can spread existing records:

```aivi
alice = { ...defaults, name: "Alice" }
```

Spreads merge fields left-to-right; later entries override earlier ones.

Functions specify **minimum required fields**, not exact shapes.

```aivi
getName : { name: Text } -> Text
getName = .name
```

## 3.4 Record Row Transforms

To avoid duplicating similar record shapes across layers, AIVI provides derived type operators
that transform record rows. These are type-level only and elaborate to plain record types.

Field lists are written as tuples of field labels, and rename maps use record-like syntax:

```aivi
Pick (id, name) User
Omit (isAdmin) User
Optional (email, name) User
Required (email, name) User
Rename { createdAt: created_at, updatedAt: updated_at } User
Defaulted { createdAt: Instant, updatedAt: Instant } User
```

Semantics:

- `Pick` keeps only the listed fields.
- `Omit` removes the listed fields.
- `Optional` wraps each listed field type in `Option` (if not already `Option`).
- `Required` unwraps `Option` for each listed field (if not `Option`, the type is unchanged).
- `Rename` renames fields; collisions are errors.
- `Defaulted` is equivalent to `Optional` at the type level and is reserved for codec/default derivation.

Errors:

- Selecting or renaming a field that does not exist in the source record is a type error.
- `Rename` collisions (two fields mapping to the same name, or a rename colliding with an existing field) are type errors.

Type-level piping mirrors expression piping and applies the left type as the final argument:

```aivi
User |> Omit (isAdmin) |> Rename { createdAt: created_at }
```

desugars to:

```aivi
Rename { createdAt: created_at } (Omit (isAdmin) User)
```


## 3.5 Classes and HKTs

```aivi
class Functor (F *) = {
  map: F A -> (A -> B) -> F B
}

// Tokens explained:
// - Functor: The class name
// - F: Generic type parameter
// - *: Denotes a higher-kinded type (F takes one type argument)
// - A, B: Type variables within the definition
```

```aivi
class Monad (M *) =
  Functor (M *) & { // The & operator denotes class inheritance/aggregation
    pure: A -> M A
    flatMap: M A -> (A -> M B) -> M B
  }
```

`A & B` in type position denotes **record/type composition** (an intersection-like merge). It is primarily used for class inheritance and trait aggregation in v0.1.

Instances:

```aivi
instance Monad (Option *) = { ... }
instance E: Monad (Result E *) = { ... } // E: binds the error parameter for the Result monad instance
```

> [!NOTE] Implementation Note: Kinds
> In the v0.1 compiler, kind annotations like `(F *)` were hints. The type checker now (planned) enforces kinds explicitly.

## 3.6 Implementation Details

> [!NOTE] Rust Codegen
> The current `rust_codegen` implementation relies on embedding the HIR and running it via the `aivi` interpreter runtime embedded in the binary. It does not yet produce native Rust code corresponding to AIVI logic.
