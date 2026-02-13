# Standard Library: Logic (Algebraic Hierarchy)

The `aivi.logic` module defines the standard algebraic hierarchy for AIVI, based on the **Fantasy Land Specification**. These classes provide a universal language for data transformation, equality, and composition.

<<< ../../snippets/from_md/05_stdlib/00_core/03_logic/block_01.aivi{aivi}

See also:

- Syntax: classes and instances ([The Type System](../../02_syntax/03_types.md))
- Syntax: effects as monads ([Effects](../../02_syntax/09_effects.md))
- Fantasy Land upstream spec (naming + laws): https://github.com/fantasyland/fantasy-land

## 1. Equality and Ordering

### Setoid
A `Setoid` has an equivalence relation.

<<< ../../snippets/from_md/05_stdlib/00_core/03_logic/block_02.aivi{aivi}

### Ord
An `Ord` provides a [total](https://en.wikipedia.org/wiki/Total_order) ordering.

<<< ../../snippets/from_md/05_stdlib/00_core/03_logic/block_03.aivi{aivi}

## 2. Monoids and Semigroups

### Semigroup
A `Semigroup` has an associative binary operation.

<<< ../../snippets/from_md/05_stdlib/00_core/03_logic/block_04.aivi{aivi}

### Monoid
A `Monoid` provides an `empty` value.

<<< ../../snippets/from_md/05_stdlib/00_core/03_logic/block_05.aivi{aivi}

### Group
A `Group` provides an `invert` operation.

<<< ../../snippets/from_md/05_stdlib/00_core/03_logic/block_06.aivi{aivi}

## 3. Categories

### Semigroupoid

<<< ../../snippets/from_md/05_stdlib/00_core/03_logic/block_07.aivi{aivi}

### Category

<<< ../../snippets/from_md/05_stdlib/00_core/03_logic/block_08.aivi{aivi}

## 4. Functional Mappings

### Functor

<<< ../../snippets/from_md/05_stdlib/00_core/03_logic/block_09.aivi{aivi}

### Apply

<<< ../../snippets/from_md/05_stdlib/00_core/03_logic/block_10.aivi{aivi}

### Applicative

<<< ../../snippets/from_md/05_stdlib/00_core/03_logic/block_11.aivi{aivi}

### Chain

<<< ../../snippets/from_md/05_stdlib/00_core/03_logic/block_12.aivi{aivi}

### Monad

<<< ../../snippets/from_md/05_stdlib/00_core/03_logic/block_13.aivi{aivi}

## 5. Folds and Traversals

### Foldable

<<< ../../snippets/from_md/05_stdlib/00_core/03_logic/block_14.aivi{aivi}

### Traversable

<<< ../../snippets/from_md/05_stdlib/00_core/03_logic/block_15.aivi{aivi}

## 6. Higher-Order Mappings

### Bifunctor

<<< ../../snippets/from_md/05_stdlib/00_core/03_logic/block_16.aivi{aivi}

### Profunctor

<<< ../../snippets/from_md/05_stdlib/00_core/03_logic/block_17.aivi{aivi}

## Examples

### `Functor` for `Option`

<<< ../../snippets/from_md/05_stdlib/00_core/03_logic/block_18.aivi{aivi}

### Implementing a new `Monad`: `Id`

<<< ../../snippets/from_md/05_stdlib/00_core/03_logic/block_21.aivi{aivi}

### `Monoid` for `Text`

<<< ../../snippets/from_md/05_stdlib/00_core/03_logic/block_19.aivi{aivi}

### `Effect` sequencing is `chain`/`bind`

`effect { ... }` is surface syntax for repeated sequencing (see [Effects](../../02_syntax/09_effects.md)):

<<< ../../snippets/from_md/05_stdlib/00_core/03_logic/block_20.aivi{aivi}
