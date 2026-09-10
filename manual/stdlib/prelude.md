# aivi.prelude

`aivi.prelude` is the carrier-agnostic foundation of AIVI programs. It names the built-in value
types, higher-kinded classes, and the generic ordering helpers that work for every `Ord` type.
Use ambient class operations such as `map`, `reduce`, `apply`, and `chain` across supported
carriers. Carrier-specific constructors and helpers live in their owning modules.

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
| `Apply F` | `apply` |
| `Applicative F` | `pure` |
| `Chain F` | `chain` |
| `Monad F` | `join` |
| `Foldable F` | `reduce` |

The complete hierarchy and current executable carrier support are documented in
[Typeclasses & Higher-Kinded Support](/guide/typeclasses).

Class members are ordinary callable values. Prefer them when an operation has the same meaning
across carriers; use names such as `mapRight`, `mapValues`, and `mapNel` when the carrier-specific
name makes the code clearer. Importing a carrier's module also brings its exported instances into
scope. Explicit imports of a same-named helper still take precedence over ambient class methods.

```aivi
use aivi.core.either (
    Either
    Right
)

type Int -> Int
func increment = n =>
    n + 1

value optional : Option Int = map increment (Some 2)
value right : Either Text Int = Right 2
value mapped : Either Text Int = map increment right
```

See [stdlib instances](/guide/typeclasses#standard-library-instances) for carrier behavior and
intentional exclusions.

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
