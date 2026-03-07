# Standard Library: Logic (Algebraic Hierarchy)

<!-- quick-info: {"kind":"module","name":"aivi.logic"} -->
The `aivi.logic` module defines the shared interface hierarchy used across the AIVI standard library. It follows the **Fantasy Land** naming conventions—a widely used community agreement about names and laws for functional-programming interfaces—so operations like `map`, `chain`, and `concat` mean the same thing across many types.
<!-- /quick-info -->
<div class="import-badge">use aivi.logic</div>

## Start here

If you opened this page because another module says “`Option` is a `Functor`” or “`Result` is a `Monad`”, you do **not** need a full category-theory background first.
Read the quick chooser below, then jump straight to the class you need.

Plain-language decoder:

- **Functor** = a wrapper or container you can `map` over
- **Applicative** = several wrapped values prepared independently, then combined
- **Monad** = wrapped step-by-step work where later steps may depend on earlier results
- **Monoid** = a type whose values can be combined, plus an `empty` identity value

<<< ../../snippets/from_md/stdlib/core/logic/standard_library_logic_algebraic_hierarchy.aivi{aivi}

See also:

- Syntax: classes and instances ([The Type System](../../syntax/types.md))
- Syntax: effects as monads ([Effects](../../syntax/effects.md))
- Fantasy Land upstream spec (naming + laws): https://github.com/fantasyland/fantasy-land

## How to read this page

If terms such as *Functor*, *Monad*, or *Monoid* are new to you, think of `aivi.logic` as a library of **shared interfaces**. These interfaces answer practical questions such as:

- Can two values be compared for equality?
- Can two values of this type be combined?
- Can I map over the values inside this container?
- Can I sequence several steps that may fail or have effects?
- Can I fold this structure into a summary value?

When a type implements one of these classes, you can use the same operation names across many different data types.

### Quick chooser

If you only need the everyday mental model, start here:

| If you want to... | Reach for... | Plain-language idea |
| --- | --- | --- |
| compare values | `Setoid`, `Ord` | equality and sorting |
| combine values of the same type | `Semigroup`, `Monoid` | append or merge, sometimes with an “empty” starting value |
| change values inside a wrapper | `Functor` | `map` over the inside without changing the wrapper |
| combine several independent wrapped steps | `Applicative` | prepare several inputs separately, then combine them |
| sequence wrapped steps that depend on earlier results | `Monad` | later steps can inspect earlier results |
| collapse many values into one summary | `Foldable` | reduce or fold |

### One problem, three levels of power

These three classes often appear together:

| Class | Use it when... | Plain-language question |
| --- | --- | --- |
| `Functor` | you already have the wrapped value and your function returns a plain value | “Can I transform the inside?” |
| `Applicative` | you have several wrapped inputs that do not depend on one another | “Can I combine these independent inputs?” |
| `Monad` | each next step may depend on the earlier successful value | “What should I do next based on what I just got?” |

For example, `Option` uses all three ideas:

- `map` changes `Some x` into `Some (f x)` and leaves `None` alone
- applicative combination can build a value from several optional inputs such as `host`, `port`, and `token`
- monadic sequencing powers `do Option { ... }`, where the second step can inspect the first step's result

## Why this matters in everyday code

`use aivi.logic` brings the class methods into scope. That means the same names like `map`, `filter`, `reduce`, `traverse`, `chain`, and `concat` work across multiple types when those types support the corresponding class.

This is one of the main ways AIVI keeps data pipelines readable without special-case APIs for every collection or effect type.

## 1. Equality and ordering

### Setoid

A `Setoid` says a type supports meaningful equality.

<<< ../../snippets/from_md/stdlib/core/logic/setoid.aivi{aivi}

### Ord

An `Ord` says values can be put into a consistent total order.

<<< ../../snippets/from_md/stdlib/core/logic/ord.aivi{aivi}

Use `Ord` when a type needs sorting, comparison operators, or ordered data structures.

## 2. Combining values

### Semigroup

A `Semigroup` provides an associative way to combine two values of the same type.

<<< ../../snippets/from_md/stdlib/core/logic/semigroup.aivi{aivi}

### Monoid

A `Monoid` is a `Semigroup` that also has an identity value called `empty`. In practice, `empty` is the “combine with nothing” value: `""` for text concatenation, `[]` for list concatenation, and similar neutral starting points.

<<< ../../snippets/from_md/stdlib/core/logic/monoid.aivi{aivi}

### Group

A `Group` adds an `invert` operation, so values can be combined and undone.

<<< ../../snippets/from_md/stdlib/core/logic/group.aivi{aivi}

## 3. Composing functions

### Semigroupoid

A `Semigroupoid` supports composition of compatible arrows.

<<< ../../snippets/from_md/stdlib/core/logic/semigroupoid.aivi{aivi}

### Category

A `Category` is a `Semigroupoid` with an identity arrow.

<<< ../../snippets/from_md/stdlib/core/logic/category.aivi{aivi}

These are more abstract than the collection-focused classes below, but they follow the same theme: small reusable interfaces with common laws.

## 4. Mapping and sequencing

### Functor

<!-- quick-info: {"kind":"class","name":"Functor","module":"aivi.logic"} -->
<<< ../../snippets/from_md/stdlib/core/logic/functor.aivi{aivi}
<!-- /quick-info -->

A `Functor` lets you transform values **inside** another structure without changing the structure itself. In everyday terms, `map` means “apply this function to each contained value.”

### Apply

`Apply` lets you apply wrapped functions to wrapped values. A useful mental model is: both the function and the input are already “inside the same kind of box,” and `Apply` knows how to line them up.

<<< ../../snippets/from_md/stdlib/core/logic/apply.aivi{aivi}

### Applicative

An `Applicative` can lift a plain value into the context with `of` and combine independent computations. In plainer terms: you can prepare several wrapped inputs separately, then combine them at the end.

<<< ../../snippets/from_md/stdlib/core/logic/applicative.aivi{aivi}

A good mental model is: use `Applicative` when several steps can be prepared independently and then combined.

### Chain

`Chain` lets one step decide what the next step should be based on the previous result.

<<< ../../snippets/from_md/stdlib/core/logic/chain.aivi{aivi}

### Monad

A `Monad` combines applicative setup with `Chain`. It models step-by-step computations where each step can depend on the previous one.

<<< ../../snippets/from_md/stdlib/core/logic/monad.aivi{aivi}

If you use `do Option { ... }`, `do Result { ... }`, or `do Effect { ... }`, you are already working with monadic sequencing.

## 5. Folding and traversing

### Foldable

A `Foldable` can be summarized into one value with operations such as `reduce`.

<<< ../../snippets/from_md/stdlib/core/logic/foldable.aivi{aivi}

### Traversable

A `Traversable` lets you map with an effect and collect the results in one pass.

<<< ../../snippets/from_md/stdlib/core/logic/traversable.aivi{aivi}

This is especially useful when you have a list of effectful steps and want either one collected success or one combined effectful computation. A common example is turning `List (Result Text Int)` into `Result Text (List Int)`.

## 5b. Filtering

### Filterable

<!-- quick-info: {"kind":"class","name":"Filterable","module":"aivi.logic"} -->
A `Filterable` can remove elements using a predicate. Requires `Functor`.
`filter` expands from `(A -> Bool) -> F A` to `(A -> Bool) -> F A -> F A`.
<!-- /quick-info -->

`Filterable` is the shared interface behind filtering values out of structures such as lists, maps, generators, and other containers that support removal.

<<< ../../snippets/from_md/stdlib/core/logic/filterable.aivi{aivi}

## 5c. Alternatives

### Alternative

<!-- quick-info: {"kind":"class","name":"Alternative","module":"aivi.logic"} -->
An `Alternative` provides a choice operator — `alt` picks the first successful/non-empty value. Requires `Applicative`.
`alt` expands from `F A -> F A` to `F A -> F A -> F A`.
<!-- /quick-info -->

Use `Alternative` when you want a fallback choice between two values in the same context.

<<< ../../snippets/from_md/stdlib/core/logic/alternative.aivi{aivi}

### Plus

<!-- quick-info: {"kind":"class","name":"Plus","module":"aivi.logic"} -->
A `Plus` provides the identity for `alt`. `zero` is the failing or empty case. Requires `Alternative`.
<!-- /quick-info -->

`Plus` gives `Alternative` its neutral “empty” value.

<<< ../../snippets/from_md/stdlib/core/logic/plus.aivi{aivi}

## 6. Mapping more than one side

### Bifunctor

A `Bifunctor` maps over structures with two interesting type parameters, such as `Result E A`.

<<< ../../snippets/from_md/stdlib/core/logic/bifunctor.aivi{aivi}

### Profunctor

A `Profunctor` maps both the input and output side of a transformation.

<<< ../../snippets/from_md/stdlib/core/logic/profunctor.aivi{aivi}

## Examples

### `Functor` for `Option`

<<< ../../snippets/from_md/stdlib/core/logic/functor_for_option.aivi{aivi}

### Implementing a new `Monad`: `Id`

<<< ../../snippets/from_md/stdlib/core/logic/implementing_a_new_monad_id.aivi{aivi}

### `Monoid` for `Text`

<<< ../../snippets/from_md/stdlib/core/logic/monoid_for_text.aivi{aivi}

### `Effect` sequencing is `chain`/`bind`

`do Effect { ... }` is surface syntax for repeated sequencing. You can think of it as the readable form of chaining one effectful step after another.

<<< ../../snippets/from_md/stdlib/core/logic/effect_sequencing_is_chain_bind.aivi{aivi}

## Instance matrix

This table shows which standard types implement which classes. `use aivi.logic` brings the class methods into scope, while type-specific modules such as `aivi.option` or `aivi.collections` provide extra helpers.

| Type | Setoid | Ord | Semigroup | Monoid | Functor | Filterable | Foldable | Traversable | Apply | Applicative | Chain | Monad | Bifunctor | Alternative | Plus |
|------|:------:|:---:|:---------:|:------:|:-------:|:----------:|:--------:|:-----------:|:-----:|:-----------:|:-----:|:-----:|:---------:|:-----------:|:----:|
| **List** | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | — | ✓ | ✓ |
| **Option** | ✓ | ✓ | ✓ | — | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | — | ✓ | ✓ |
| **Result E** | ✓ | — | — | — | ✓ | — | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | — |
| **Map K** | ✓ | — | ✓ | ✓ | ✓ | ✓ | ✓ | — | — | — | — | — | — | — | — |
| **Generator** | — | — | — | — | ✓ | ✓ | ✓ | — | — | — | — | — | — | — | — |
| **Tree** | — | — | — | — | ✓ | ✓ | ✓ | — | — | — | — | — | — | — | — |
| **Stream** | — | — | — | — | ✓ | ✓ | — | — | — | — | — | — | — | — | — |

> **Set, Queue, Deque, Heap** — Foldable/Filterable instances are deferred until builtin runtime operations are added for those types. Set and Heap cannot be Functors because arbitrary mapping can break their structural invariants.
