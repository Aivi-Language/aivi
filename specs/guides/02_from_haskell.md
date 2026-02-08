# AIVI for Haskell Developers

If you are coming from Haskell, AIVI will feel very comfortable. It is essentially a strict, impure-but-tracked, row-polymorphic Haskell.

## Key Differences

| Feature | Haskell | AIVI |
| :--- | :--- | :--- |
| **Evaluation** | Lazy (call-by-need) | Strict (call-by-value) |
| **Records** | Named fields (problematic names) | Anonymous row-polymorphic records |
| **Effects** | IO Monad / Transformers | `Effect` monad + algebraic effects (planned) |
| **Overloading** | Typeclasses | Classes (similar, but structural instances allowed) |
| **Syntax** | `main = do` | `main = effect { ... }` |

## Mappings

### Data Types

**Haskell:**
```haskell
data User = User { id :: Int, name :: String }
```

**AIVI:**
```aivi
User = { id: Int, name: Text }
// Just an alias, not a nominal type
```

### ADTs

**Haskell:**
```haskell
data Option a = None | Some a
```

**AIVI:**
```aivi
Option A = None | Some A
```

### Pattern Matching

**Haskell:**
```haskell
sum [] = 0
sum (x:xs) = x + sum xs
```

**AIVI:**
```aivi
sum =
  | [] => 0
  | [x, ...xs] => x + sum xs
```

### Functors/Monads

AIVI has HKTs and uses them for the standard hierarchy.

```aivi
class Functor (F *) = { map: ... }
```

However, AIVI is strict, so infinite lists are not default. Use `Generator` for lazy sequences.

## The "Missing" Features

* **GADT**: Currently no GADTs.
* **Type Families**: No type families yet.
* **Lazy IO**: Effects are strict and explicit.
