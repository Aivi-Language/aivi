# The Type System

## 3.1 Primitive Types

AIVI includes a comprehensive set of primitive types for high-integrity data handling. Type combinators (like `+` for record merging or domain-specific type transformations) are handled via the Domain system.

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
TimeZone
ZonedDateTime
```

Numeric suffixes:

* `42n` → `BigInt`
* `3.14d` → `Decimal`

---

## 3.2 Algebraic Data Types

```aivi
Option A = None | Some A
Result E A = Err E | Ok A
```

---

## 3.3 Open Records (Row Polymorphism)

Records are:

* structural
* open by default

```aivi
User = { id: Int, name: Text, email: Option Text }
```

Functions specify **minimum required fields**, not exact shapes.

```aivi
getName : { name: Text } -> Text
getName = .name
```

---

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
