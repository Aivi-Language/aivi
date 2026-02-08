# The Type System

## 3.1 Primitive Types

```aivi
Unit
Bool
Int
Float
Decimal
BigInt
String
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

### From Haskell
Equivalent to `data Option a = None | Some a`.

### From TypeScript
Equivalent to discriminated unions: `type Option<A> = { tag: 'None' } | { tag: 'Some', value: A }`.

---

## 3.3 Open Records (Row Polymorphism)

Records are:

* structural
* open by default

```aivi
User = { id: Int, name: String, email: Option String }
```

Functions specify **minimum required fields**, not exact shapes.

```aivi
getName = u => u.name
```

### From TypeScript
Similar to interfaces or types, but structural typing is "exact" unless specified otherwise in some contexts; here it's polymorphic on the row.

---

## 3.4 Classes and HKTs

```aivi
class Functor (F *) = {
  map: F A, (A => B) => F B
}
```

```aivi
class Monad (M *) =
  Functor (M *) & {
    pure: A => M A
    flatMap: M A, (A => M B) => M B
  }
```

Instances:

```aivi
instance Monad (Option *) = { ... }
instance E: Monad (Result E) = { ... }
```
