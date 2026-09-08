# aivi.prelude

The `aivi.prelude` module is AIVI's convenience layer — it re-exports the most commonly used functions from across the standard library.

The compiler supplies an ambient prelude, and several core stdlib modules additionally
declare `hoist`. Common types and class members are therefore available without an
explicit import. This does not mean every export of every standard library module is
globally available; use explicit imports for module-specific helpers.

## At a glance

`aivi.prelude` is already a scan-first module page: the tables below list the types, classes, and
representative helper batteries that are available everywhere by default. Use this page as the
overview map; use the module pages like `aivi.option`, `aivi.result`, and `aivi.list` when you need
the full per-module battery tables.

## Helper export map

The examples below are representative, not an exhaustive list. Additional named wrappers
are grouped here; the linked module pages explain their underlying operations:

| Area | Additional exports |
| --- | --- |
| [Option](option.md) / [Result](result.md) | `isNone`, `isErr` |
| [Validation](validation.md) | `Errors`, `isInvalid`, `validationMapErr`, `validationFromResult`, `validationToOption`, `validationMap`, `validationAndThen`, `zipValidation`, `validationFold` |
| [List](list.md) | `at`, `last`, `tailOrEmpty`, `nonEmpty`, `flatten`, `reverse`, `replaceAt`, `take`, `drop`, `sum`, `contains`, `any`, `find`, `findMap` |
| [Order](order.md) | `maxOf`, `comparing` |
| [Text](text.md) | `textIsEmpty` (named wrapper for text emptiness) |
| [Math](math.md) | `sign`, `isOdd`, `square`, `divides` |
| [Bool](bool.md) | `implies`, `boolFromInt` (wrapper for `fromInt`) |
| [Pair](pair.md) | `mapFirst`, `mapSecond`, `mapPair` (wrapper for `mapBoth`), `duplicate` |

## Built-in types

These types are always available and can be imported from `aivi.prelude`:

| Type | Description |
|------|-------------|
| `Int` | 64-bit signed integer |
| `Float` | 64-bit floating point |
| `Decimal` | Fixed-precision decimal backed by `rust_decimal`; unlike `BigInt`, not arbitrary precision |
| `BigInt` | Arbitrary-precision integer |
| `Bool` | Boolean: `True` or `False` |
| `Text` | Unicode text string |
| `Unit` | The unit type, with one value: `Unit` |
| `Ordering` | Result of comparison: `Less`, `Equal`, or `Greater` |
| `List A` | Ordered collection |
| `Option A` | Optional value: `Some value` or `None` |
| `Result E A` | Success or failure: `Ok value` or `Err error` |
| `Validation E A` | Accumulating validation: `Valid value` or `Invalid errors` |
| `Signal A` | A reactive value that changes over time |
| `Task E A` | A one-shot async computation description |

## Type Classes

| Class | Description |
|-------|-------------|
| `Eq A` | Equality comparison |
| `Ord A` | Ordering and comparison via `compare : A -> A -> Ordering`; ordinary `<`, `>`, `<=`, and `>=` derive from this member |
| `Default A` | A default value |
| `Functor F` | Mappable container |
| `Apply F` | Effectful function application |
| `Applicative F` | Applicative functor |
| `Chain M` | Dependent sequencing without changing the carrier family |
| `Monad F` | Monadic sequencing (`List`, `Option`, `Result`, and `Task` are the builtin executable carriers today) |
| `Foldable F` | Foldable container |
| `Traversable F` | Traversable container |
| `Filterable F` | Filterable container |
| `Semigroup A` | Associative combination |
| `Monoid A` | Semigroup with identity |
| `Bifunctor F` | Mappable over both type parameters |

For the current higher-kinded hierarchy, the canonical executable support reference, and the current
unary imported-instance slice for user-authored higher-kinded classes and instances, see
[Typeclasses & Higher-Kinded Support](/guide/typeclasses). For the law contract behind those classes,
see [Class Laws & Design Boundaries](/guide/class-laws). Parser or checker acceptance alone does not
imply executable runtime support.

## Option Functions

```aivi
use aivi.prelude (
    Option
    Text
    Bool
    getOrElse
    isSome
    isSomeAnd
    mapOr
    textNonEmpty
)

value name : Option Text = Some "Ada"
value displayName : Text = getOrElse "guest" name
value hasName : Bool = isSome name

type Text -> Text
func punctuate = name =>
    append name "!"

value foldedName : Text = mapOr "guest" punctuate name
value checkedName : Bool = isSomeAnd textNonEmpty name
```

## Result Functions

```aivi
value age : Result Text Int = Ok 30
value ageValue : Int = withDefault 0 age
value succeeded : Bool = isOk age
```

## Validation Functions

```aivi
use aivi.prelude (
    Text
    Validation
    isValid
    validationToResult
    validationGetOrElse
)

value checked : Validation Text Text = Valid "Ada"
value passed : Bool = isValid checked
value checkedText : Text = validationGetOrElse "guest" checked
value checkedResult : Result Text Text = validationToResult checked
```

## List Functions

```aivi
use aivi.prelude (
    Int
    Text
    Bool
    List
    length
    head
    isEmpty
    indexed
)

value items : List Text = [
    "Ada",
    "Grace",
    "Hedy"
]

value count : Int = length items
value first : Option Text = head items
value empty : Bool = isEmpty []
value indexedItems : List (Int, Text) = indexed items
```

```aivi
use aivi.prelude (
    Int
    mapWithIndex
    reduceWithIndex
)

type Int -> Int -> Int
func addIndex = index item =>
    index + item

type Int -> Int -> Int -> Int
func addIndexed = total index item =>
    total + index + item

value adjusted : List Int =
    mapWithIndex addIndex [
        10,
        20,
        30
    ]

value indexedTotal : Int =
    reduceWithIndex addIndexed 0 [
        10,
        20,
        30
    ]
```

## Order Functions

`Ord.compare` is the primitive ordering member in the prelude. Any type with an `Ord` instance can use `min`, `max`, `minOf`, and the ordinary ordering operators directly.

```aivi
value smallest : Int = min 5 3
value greatest : Int = max 5 3

value leastOf : Int =
    minOf 10 [
        7,
        4,
        9
    ]
```

## Text Functions

Prelude keeps bare `join` for the generic `Monad.join` member. Text joining stays on
`aivi.text.join`, which you can import locally when needed.

```aivi
use aivi.text (join as textJoin)

value csv : Text =
    textJoin ", " [
        "Ada",
        "Grace",
        "Hedy"
    ]

value combined : Text =
    textJoin "" [
        "Hello",
        " ",
        "World"
    ]

value wrapped : Text = surround "(" ")" "AIVI"
```

## Math Functions

```aivi
value absolute : Int = abs (-5)
value flipped : Int = negate 7
value even : Bool = isEven 4
value clamped : Int = clamp 0 100 150
value inRange : Bool = between 1 10 5
```

## Bool Functions

```aivi
value inverted : Bool = not True
value exclusive : Bool = xor True False
```

## Pair Functions

```aivi
value pair : (Int, Text) = (
    42,
    "hello"
)

value firstValue : Int = first pair
value secondValue : Text = second pair
value swapped : (Text, Int) = swap pair
```
