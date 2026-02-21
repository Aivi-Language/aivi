# Pattern binding with `=` (total-only)

Kernel has only `case`, so even total bindings can lower via `case`. (A compiler may optimize to projections.)

In surface syntax, these bindings appear in a `{ ... }` block:

<<< ../snippets/from_md/desugaring/patterns/pattern_binding_with_total_only_01.aivi{aivi}

desugars to `case ⟦e⟧ of \| { a = x } -> ⟦body⟧`.

<<< ../snippets/from_md/desugaring/patterns/pattern_binding_with_total_only_02.aivi{aivi}

desugars to `case ⟦e⟧ of \| (h :: t) -> ⟦body⟧`.

<<< ../snippets/from_md/desugaring/patterns/pattern_binding_with_total_only_03.aivi{aivi}

desugars to `case ⟦e⟧ of \| ⟦p⟧ -> ⟦body⟧`.

### Deep Path Destructuring
| Surface | Desugaring |
| :--- | :--- |
| `{ a.b.c as {x} }` | `⟦{ a: { b: { c: v#1@{x} } } }⟧` |

Pattern translation `⟦p⟧` uses the kernel pattern forms.


# Whole-value binding `as`

| Surface | Desugaring |
| :--- | :--- |
| `v as p` (pattern) | kernel pattern `v @ ⟦p⟧` |
| `case e of \| v as { name: n } -> b` | `case ⟦e⟧ of \| v @ { name = n } -> ⟦b⟧` |

A `{ ... }` binding with `as`:

<<< ../snippets/from_md/desugaring/patterns/whole_value_binding_as.aivi{aivi}

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

The condition must have type `Bool`. There is no `if` without `else`   every `if` is an expression that produces a value.
