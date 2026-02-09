# The Type System

## 3.1 Primitive Types

AIVI includes a comprehensive set of primitive types for high-integrity data handling. Type combinators (like `&` for record/type composition, or domain-specific type transformations) are handled via the Domain system.

```aivi
Unit
Bool
Int
Float
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

* `42n` → `BigInt`
* `3.14d` → `Decimal`


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

Functions specify **minimum required fields**, not exact shapes.

```aivi
getName : { name: Text } -> Text
getName = .name
```


## 3.4 Classes and HKTs

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

Instances:

```aivi
instance Monad (Option *) = { ... }
instance E: Monad (Result E *) = { ... } // E: binds the error parameter for the Result monad instance
```
