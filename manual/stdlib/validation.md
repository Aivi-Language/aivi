# aivi.validation

Pure helpers for `Validation E A`. Use `Validation` when several independent checks should report
their errors together. The `&|>` applicative pipe performs that accumulation; dependent checks use
`!|>` and stop after a failure.

```aivi
use aivi.validation (
    Errors
    isValid
    isInvalid
    getOrElse
    mapErr
    toResult
    fromResult
    toOption
    map
    andThen
    fold
)
```

`Errors E` is an alias for `NonEmptyList E`, the standard non-empty error carrier.

| Function | Type | Behavior |
| --- | --- | --- |
| `isValid` | `Validation E A -> Bool` | Test for `Valid` |
| `isInvalid` | `Validation E A -> Bool` | Test for `Invalid` |
| `getOrElse` | `A -> Validation E A -> A` | Extract a valid value or use a fallback |
| `mapErr` | `(E1 -> E2) -> Validation E1 A -> Validation E2 A` | Transform the error |
| `toResult` | `Validation E A -> Result E A` | Convert `Valid`/`Invalid` to `Ok`/`Err` |
| `fromResult` | `Result E A -> Validation E A` | Convert `Ok`/`Err` to `Valid`/`Invalid` |
| `toOption` | `Validation E A -> Option A` | Keep only a valid value |
| `map` | `(A -> B) -> Validation E A -> Validation E B` | Transform a valid value |
| `andThen` | `(A -> Validation E B) -> Validation E A -> Validation E B` | Run a dependent validation |
| `fold` | `(E -> B) -> (A -> B) -> Validation E A -> B` | Handle both constructors |

```aivi
use aivi.validation (
    getOrElse
    map
)

type Int -> Int
func increment = n =>
    n + 1

value checked : Validation Text Int = Valid 2
value next : Validation Text Int = map increment checked
value displayed : Int = getOrElse 0 next
```

See [Pipelines & Operators](/guide/pipes) for the `&|>` and `!|>` validation rules.

## Error-list alias

`Errors E` is an alias for `NonEmptyList E`, ensuring an invalid validation carries at least one error.
