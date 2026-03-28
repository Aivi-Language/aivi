# aivi.core.either

Disjoint union type for values that can be one of two alternatives. By convention `Left` holds an error or secondary value and `Right` holds the primary or success value.

```aivi
use aivi.core.either (Either, mapRight, mapLeft, mapBoth, fold, isLeft, isRight,
                      fromLeft, fromRight, swap, toOption, fromOption,
                      rightFromResult, rightFromOption, partitionEithers)
```

---

## Either

```
type Either L R =
  | Left L
  | Right R
```

A value of type `Either L R` is either a `Left L` or a `Right R`. Use `||>` to branch on which case you have:

```aivi
use aivi.core.either (Either)

fun describeResult:Text result:(Either Text Int) =>
    result
     ||> Left msg  -> "Error: {msg}"
     ||> Right n   -> "Got {n}"
```

---

## mapRight

Transforms the `Right` value, leaving `Left` unchanged.

```
mapRight : (R -> R2) -> Either L R -> Either L R2
```

```aivi
use aivi.core.either (Either, mapRight)

fun doubleRight:Either Text Int result:(Either Text Int) =>
    mapRight (fun n:Int x:Int => x * 2) result
```

---

## mapLeft

Transforms the `Left` value, leaving `Right` unchanged.

```
mapLeft : (L -> L2) -> Either L R -> Either L2 R
```

```aivi
use aivi.core.either (Either, mapLeft)

fun wrapError:(Either Text Int) result:(Either Int Int) =>
    mapLeft (fun msg:Text code:Int => "Error code: {code}") result
```

---

## mapBoth

Transforms both sides independently.

```
mapBoth : (L -> L2) -> (R -> R2) -> Either L R -> Either L2 R2
```

```aivi
use aivi.core.either (Either, mapBoth)
use aivi.math (negate)
use aivi.text (surround)

fun transformBoth:(Either Text Int) e:(Either Text Int) =>
    mapBoth (surround "[" "]") negate e
```

---

## fold

Reduces an `Either` to a single value by applying the appropriate function.

```
fold : (L -> C) -> (R -> C) -> Either L R -> C
```

```aivi
use aivi.core.either (Either, fold)

fun toLength:Int e:(Either Text Text) =>
    fold (fun n:Int s:Text => 0 - 1) (fun n:Int s:Text => 1) e
```

---

## isLeft / isRight

Predicates that test which case an `Either` holds.

```
isLeft  : Either L R -> Bool
isRight : Either L R -> Bool
```

```aivi
use aivi.core.either (Either, isLeft, isRight)

fun hasError:Bool e:(Either Text Int) =>
    isLeft e
```

---

## fromLeft / fromRight

Extract the value from the expected case, returning a default if the other case is held.

```
fromLeft  : L -> Either L R -> L
fromRight : R -> Either L R -> R
```

```aivi
use aivi.core.either (Either, fromRight)

fun getValueOrZero:Int e:(Either Text Int) =>
    fromRight 0 e
```

---

## swap

Swaps the `Left` and `Right` cases.

```
swap : Either L R -> Either R L
```

```aivi
use aivi.core.either (Either, swap)

fun flipEither:(Either Int Text) e:(Either Text Int) =>
    swap e
```

---

## toOption

Converts to `Option`, keeping only `Right` values.

```
toOption : Either L R -> Option R
```

```aivi
use aivi.core.either (Either, toOption)

fun rightOrNone:(Option Int) e:(Either Text Int) =>
    toOption e
```

---

## fromOption

Wraps an `Option` into `Either`, using the given default for the `Left` case when the option is `None`.

```
fromOption : L -> Option R -> Either L R
```

```aivi
use aivi.core.either (Either, fromOption)

fun optToEither:(Either Text Int) opt:(Option Int) =>
    fromOption "missing" opt
```

---

## partitionEithers

Splits a list of `Either` values into two lists: lefts and rights.

```
partitionEithers : List (Either L R) -> { lefts: List L, rights: List R }
```

```aivi
use aivi.core.either (Either, partitionEithers)

fun splitResults:(List Text) items:(List (Either Text Int)) =>
    (partitionEithers items).lefts
```
