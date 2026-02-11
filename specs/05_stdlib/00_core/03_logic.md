# Standard Library: Logic (Algebraic Hierarchy)

The `aivi.logic` module defines the standard algebraic hierarchy for AIVI, based on the **Fantasy Land Specification**. These classes provide a universal language for data transformation, equality, and composition.

```aivi
module aivi.logic
```

## 1. Equality and Ordering

### Setoid
A `Setoid` has an equivalence relation.
```aivi
class Setoid A = {
  equals: A -> A -> Bool
}
```

### Ord
An `Ord` must be a `Setoid` and have a [total](https://en.wikipedia.org/wiki/Total_order) ordering.
```aivi
class Ord A = 
  Setoid A & {
    lte: A -> A -> Bool
  }
```

## 2. Monoids and Semigroups

### Semigroup
A `Semigroup` has an associative binary operation.
```aivi
class Semigroup A = {
  concat: A -> A -> A
}
```

### Monoid
A `Monoid` must be a `Semigroup` and have an `empty` value.
```aivi
class Monoid A = 
  Semigroup A & {
    empty: A
  }
```

### Group
A `Group` must be a `Monoid` and have an `invert` operation.
```aivi
class Group A = 
  Monoid A & {
    invert: A -> A
  }
```

## 3. Categories

### Semigroupoid
```aivi
class Semigroupoid (F * *) = {
  compose: F B C -> F A B -> F A C
}
```

### Category
```aivi
class Category (F * *) = 
  Semigroupoid F & {
    id: F A A
  }
```

## 4. Functional Mappings

### Functor
```aivi
class Functor (F *) = {
  map: F A -> (A -> B) -> F B
}
```

### Apply
```aivi
class Apply (F *) = 
  Functor F & {
    ap: F (A -> B) -> F A -> F B
  }
```

### Applicative
```aivi
class Applicative (F *) = 
  Apply F & {
    of: A -> F A
  }
```

### Chain
```aivi
class Chain (F *) = 
  Apply F & {
    chain: F A -> (A -> F B) -> F B
  }
```

### Monad
```aivi
class Monad (M *) = 
  Applicative M & Chain M
```

## 5. Folds and Traversals

### Foldable
```aivi
class Foldable (F *) = {
  reduce: (B -> A -> B) -> B -> F A -> B
}
```

### Traversable
```aivi
class Traversable (T *) = 
  Functor T & Foldable T & {
    traverse: (Applicative F) => (A -> F B) -> T A -> F (T B)
  }
```

## 6. Higher-Order Mappings

### Bifunctor
```aivi
class Bifunctor (F * *) = {
  bimap: (A -> C) -> (B -> D) -> F A B -> F C D
}
```

### Profunctor
```aivi
class Profunctor (F * *) = {
  promap: (A -> B) -> (C -> D) -> F B C -> F A D
}
```
