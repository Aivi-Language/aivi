# Standard Library: Logic (Algebraic Hierarchy)

<!-- quick-info: {"kind":"module","name":"aivi.logic"} -->
The `aivi.logic` module defines the standard algebraic hierarchy for AIVI, based on the **Fantasy Land Specification**. These classes provide a universal language for data transformation, equality, and composition.
<!-- /quick-info -->
<div class="import-badge">use aivi.logic</div>

<<< ../../snippets/from_md/stdlib/core/logic/standard_library_logic_algebraic_hierarchy.aivi{aivi}

See also:

- Syntax: classes and instances ([The Type System](../../syntax/types.md))
- Syntax: effects as monads ([Effects](../../syntax/effects.md))
- Fantasy Land upstream spec (naming + laws): https://github.com/fantasyland/fantasy-land

## 1. Equality and Ordering

### Setoid
A `Setoid` has an equivalence relation.

<<< ../../snippets/from_md/stdlib/core/logic/setoid.aivi{aivi}

### Ord
An `Ord` provides a [total](https://en.wikipedia.org/wiki/Total_order) ordering.

<<< ../../snippets/from_md/stdlib/core/logic/ord.aivi{aivi}

## 2. Monoids and Semigroups

### Semigroup
A `Semigroup` has an associative binary operation.

<<< ../../snippets/from_md/stdlib/core/logic/semigroup.aivi{aivi}

### Monoid
A `Monoid` provides an `empty` value.

<<< ../../snippets/from_md/stdlib/core/logic/monoid.aivi{aivi}

### Group
A `Group` provides an `invert` operation.

<<< ../../snippets/from_md/stdlib/core/logic/group.aivi{aivi}

## 3. Categories

### Semigroupoid

<<< ../../snippets/from_md/stdlib/core/logic/semigroupoid.aivi{aivi}

### Category

<<< ../../snippets/from_md/stdlib/core/logic/category.aivi{aivi}

## 4. Functional Mappings

### Functor

<!-- quick-info: {"kind":"class","name":"Functor","module":"aivi.logic"} -->
<<< ../../snippets/from_md/stdlib/core/logic/functor.aivi{aivi}
<!-- /quick-info -->

### Apply

<<< ../../snippets/from_md/stdlib/core/logic/apply.aivi{aivi}

### Applicative

<<< ../../snippets/from_md/stdlib/core/logic/applicative.aivi{aivi}

### Chain

<<< ../../snippets/from_md/stdlib/core/logic/chain.aivi{aivi}

### Monad

<<< ../../snippets/from_md/stdlib/core/logic/monad.aivi{aivi}

## 5. Folds and Traversals

### Foldable

<<< ../../snippets/from_md/stdlib/core/logic/foldable.aivi{aivi}

### Traversable

<<< ../../snippets/from_md/stdlib/core/logic/traversable.aivi{aivi}

## 6. Higher-Order Mappings

### Bifunctor

<<< ../../snippets/from_md/stdlib/core/logic/bifunctor.aivi{aivi}

### Profunctor

<<< ../../snippets/from_md/stdlib/core/logic/profunctor.aivi{aivi}

## Examples

### `Functor` for `Option`

<<< ../../snippets/from_md/stdlib/core/logic/functor_for_option.aivi{aivi}

### Implementing a new `Monad`: `Id`

<<< ../../snippets/from_md/stdlib/core/logic/implementing_a_new_monad_id.aivi{aivi}

### `Monoid` for `Text`

<<< ../../snippets/from_md/stdlib/core/logic/monoid_for_text.aivi{aivi}

### `Effect` sequencing is `chain`/`bind`

`do Effect { ... }` is surface syntax for repeated sequencing (see [Effects](../../syntax/effects.md)):

<<< ../../snippets/from_md/stdlib/core/logic/effect_sequencing_is_chain_bind.aivi{aivi}
