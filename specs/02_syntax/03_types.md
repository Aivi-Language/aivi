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

* `42n` → `BigInt`
* `3.14dec` → `Decimal`


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

`A & B` in type position denotes **record/type composition** (an intersection-like merge). It is primarily used for class inheritance and trait aggregation in v0.1.

Instances:

```aivi
instance Monad (Option *) = { ... }
instance E: Monad (Result E *) = { ... } // E: binds the error parameter for the Result monad instance
```
