# aivi.prelude

`aivi.prelude` is the carrier-agnostic foundation of AIVI programs. It names the built-in value
types, higher-kinded classes, and the generic ordering helpers that work for every `Ord` type.
Collection, option, result, validation, text, boolean, and pair operations live in their owning
modules and should be imported from there.

The compiler also makes the same core types and class members available ambiently. Explicit
prelude imports remain useful when a file wants to show those dependencies.

## Built-in types

| Type | Description |
| --- | --- |
| `Int` | 64-bit signed integer |
| `Float` | 64-bit floating-point number |
| `Decimal` | Fixed-precision decimal |
| `BigInt` | Arbitrary-precision integer |
| `Bool` | `True` or `False` |
| `Text` | Unicode text |
| `Unit` | Unit type |
| `Ordering` | `Less`, `Equal`, or `Greater` |
| `List A` | Ordered collection |
| `Option A` | Optional value |
| `Result E A` | Success or failure |
| `Validation E A` | Validation that can accumulate errors |
| `Signal A` | Reactive value |
| `Task E A` | Description of asynchronous work |

## Type classes

| Class | Main operation |
| --- | --- |
| `Eq A` | `(==)` and `(!=)` |
| `Default A` | `default` |
| `Functor F` | `map` |
| `Ord A` | `compare` |
| `Semigroup A` | `append` |
| `Monoid A` | `empty` |
| `Bifunctor F` | `bimap` |
| `Traversable F` | `traverse` |
| `Filterable F` | `filterMap` |
| `Applicative F` | `pure` and `apply` |
| `Monad F` | `chain` and `join` |
| `Foldable F` | `reduce` |

The complete hierarchy and current executable carrier support are documented in
[Typeclasses & Higher-Kinded Support](/guide/typeclasses).

## Generic ordering helpers

| Function | Type | Description |
| --- | --- | --- |
| `min` | `Ord A => A -> A -> A` | Return the lesser value |
| `max` | `Ord A => A -> A -> A` | Return the greater value |
| `minOf` | `Ord A => A -> List A -> A` | Fold a list from an explicit first value using `min` |
| `maxOf` | `Ord A => A -> List A -> A` | Fold a list from an explicit first value using `max` |
| `clamp` | `Ord A => A -> A -> A -> A` | Restrict a value to the inclusive bounds |

```aivi
use aivi.prelude (
    Int
    Ord
    min
    max
    clamp
)

value smallest : Int = min 5 3
value greatest : Int = max 5 3
value bounded : Int = clamp 0 100 140
```

## Owning modules

Import carrier-specific functions from these modules:

| Values | Module |
| --- | --- |
| `Option A` | [`aivi.option`](option.md) |
| `Result E A` | [`aivi.result`](result.md) |
| `Validation E A` | [`aivi.validation`](validation.md) |
| `List A` | [`aivi.list`](list.md) |
| pairs | [`aivi.pair`](pair.md) |
| text | [`aivi.text`](text.md) |
| booleans | [`aivi.bool`](bool.md) |
| integer helpers | [`aivi.math`](math.md) |

This keeps each public function tied to one implementation and one reference page.

## Built-in carriers and classes

The carrier types re-exported by the prelude are `List`, `Option`, `Result`, `Validation`, `Signal`,
and `Task`. Its class interfaces are `Eq`, `Default`, `Functor`, `Semigroup`, `Monoid`,
`Bifunctor`, `Traversable`, `Filterable`, `Applicative`, `Monad`, and `Foldable`.
