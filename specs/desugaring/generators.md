# Generators

## Generator core encoding

Generator type:

* `Generator A ≡ ∀R. (R -> A -> R) -> R -> R`

Primitive “constructors” as definable macros:

* `genEmpty = ΛR. λk. λz. z`
* `genYield a = ΛR. λk. λz. k z a`
* `genAppend g1 g2 = ΛR. λk. λz. g2 k (g1 k z)`
* `genMap f g = ΛR. λk. λz. g (λacc a. k acc (f a)) z`
* `genFilter p g = ΛR. λk. λz. g (λacc a. case p a of | True -> k acc a | False -> acc) z`

## `generate { … }`

- `generate { yield e }` desugars to `genYield ⟦e⟧`.

- Sequencing:

<<< ../snippets/from_md/04_desugaring/06_generators/block_01.aivi{aivi}

  desugars to `genAppend ⟦gen s1⟧ ⟦gen s2⟧`.

- Binding:

<<< ../snippets/from_md/04_desugaring/06_generators/block_02.aivi{aivi}

  desugars to `genBind ⟦g⟧ (λx. ⟦generate { body }⟧)` where `genBind g f = ΛR. λk. λz. g (λacc a. (f a) k acc) z`.

- Filtering:

<<< ../snippets/from_md/04_desugaring/06_generators/block_03.aivi{aivi}

  desugars to `genFilter (λx. ⟦pred⟧[_ := x]) ⟦generate { body }⟧`.

- Loops:

  `generate { loop pat = init => body }` desugars by defining local `recurse` and starting it: `let rec recurse pat = ⟦generate { body }⟧ in recurse ⟦init⟧`.
