# Pattern binding with `=` (total-only)

Kernel has only `case`, so even total bindings can lower via `case`. (A compiler may optimize to projections.)

In surface syntax, these bindings appear in a `{ ... }` block:

<<< ../snippets/from_md/04_desugaring/04_patterns/block_01.aivi{aivi}

desugars to `case ⟦e⟧ of \| { a = x } -> ⟦body⟧`.

<<< ../snippets/from_md/04_desugaring/04_patterns/block_02.aivi{aivi}

desugars to `case ⟦e⟧ of \| (h :: t) -> ⟦body⟧`.

<<< ../snippets/from_md/04_desugaring/04_patterns/block_03.aivi{aivi}

desugars to `case ⟦e⟧ of \| ⟦p⟧ -> ⟦body⟧`.

### Deep Path Destructuring
| Surface | Desugaring |
| :--- | :--- |
| `{ a.b.c@{x} }` | `⟦{ a: { b: { c: v#1@{x} } } }⟧` |

Pattern translation `⟦p⟧` uses the kernel pattern forms.


# Whole-value binding `@`

| Surface | Desugaring |
| :--- | :--- |
| `v@p` (pattern) | kernel pattern `v @ ⟦p⟧` |
| `case e of \| v@{ name: n } -> b` | `case ⟦e⟧ of \| v @ { name = n } -> ⟦b⟧` |

A `{ ... }` binding with `@`:

<<< ../snippets/from_md/04_desugaring/04_patterns/block_04.aivi{aivi}

desugars to `case ⟦e⟧ of \| v @ ⟦p⟧ -> ⟦body⟧`.


# Pattern matching `match`

Surface `match` is syntactic sugar for `case` with ordered arms.

| Surface | Desugaring |
| :--- | :--- |
| `e match \| p1 => b1 \| p2 => b2` | `case ⟦e⟧ of \| ⟦p1⟧ -> ⟦b1⟧ \| ⟦p2⟧ -> ⟦b2⟧` |
| guard: `\| p when g => b` | `\| ⟦p⟧ -> case ⟦g⟧ of \| True -> ⟦b⟧ \| False -> nextArm` (compiled as nested cases) |

Multi-clause functions:

| Surface | Desugaring |
| :--- | :--- |
| `f = \| p1 => b1 \| p2 => b2` | `f = λx#1. case x#1 of \| ⟦p1⟧ -> ⟦b1⟧ \| ⟦p2⟧ -> ⟦b2⟧` |


# Conditional `if / then / else`

`if … then … else …` is syntactic sugar for a `case` on `True`/`False`:

| Surface | Desugaring |
| :--- | :--- |
| `if c then a else b` | `case ⟦c⟧ of \| True -> ⟦a⟧ \| False -> ⟦b⟧` |

The condition must have type `Bool`. There is no `if` without `else` — every `if` is an expression that produces a value.
