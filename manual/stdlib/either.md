# aivi.core.either

`Either L R` is a disjoint union with `Left L` and `Right R` branches. Use it when both branches are meaningful alternatives. For ordinary success and failure, prefer `Result E A`.

## Type class operations

`Either E` implements `Functor`, `Apply`, `Applicative`, `Chain`, `Monad`, and `Foldable`. Ambient
`map` is equivalent to `mapRight`; `pure` constructs `Right`; `chain` and `join` short-circuit on
`Left`. `apply` checks the function's `Either` first, preserving its `Left` when both inputs fail.
`reduce` returns the seed for `Left` and visits the payload once for `Right`.

`Bifunctor Either` supplies `bimap`, equivalent to `mapBoth`. Import this module's type or helpers
to make its instances available.

## API

| Export | Type | Behavior |
| --- | --- | --- |
| `Either` | `Either L R` | The two-branch type |
| `Left` | `L -> Either L R` | Construct the left branch |
| `Right` | `R -> Either L R` | Construct the right branch |
| `isLeft` | `Either L R -> Bool` | Test for `Left` |
| `isRight` | `Either L R -> Bool` | Test for `Right` |
| `fromLeft` | `Either L R -> Option L` | Extract a left value when present |
| `fromRight` | `Either L R -> Option R` | Extract a right value when present |
| `mapLeft` | `(L1 -> L2) -> Either L1 R -> Either L2 R` | Transform the left branch |
| `mapRight` | `(R1 -> R2) -> Either L R1 -> Either L R2` | Transform the right branch |
| `mapBoth` | `(L1 -> L2) -> (R1 -> R2) -> Either L1 R1 -> Either L2 R2` | Transform either branch |
| `fold` | `(L -> C) -> (R -> C) -> Either L R -> C` | Eliminate an `Either` into one result type |
| `swap` | `Either L R -> Either R L` | Exchange the branches |
| `toOption` | `Either L R -> Option R` | Keep `Right`; discard `Left` |
| `toResult` | `Either L R -> Result L R` | Map `Left` to `Err` and `Right` to `Ok` |
| `fromResult` | `Result E A -> Either E A` | Map `Err` to `Left` and `Ok` to `Right` |
| `partitionEithers` | `List (Either L R) -> (List L, List R)` | Split values into left and right lists while preserving order |

```aivi
use aivi.core.either (
    Either
    Left
    Right
    fold
)

type Either Text Int -> Text
func describe = value =>
    fold (message => "error: {message}") (number => "value: {number}") value

value example : Text = describe (Right 3)
```
