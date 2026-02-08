# Pattern binding with `=` (total-only)

Kernel has only `case`, so even total bindings can lower via `case`. (A compiler may optimize to projections.)

| Surface | Desugaring |
| :--- | :--- |
| `{ a: x } = e; body` | `case ⟦e⟧ of \| { a = x } -> ⟦body⟧` |
| `[h, ...t] = e; body` | `case ⟦e⟧ of \| (h :: t) -> ⟦body⟧` |
| `p = e; body` | `case ⟦e⟧ of \| ⟦p⟧ -> ⟦body⟧` |

Pattern translation `⟦p⟧` uses the kernel pattern forms.

---

# Whole-value binding `@`

| Surface | Desugaring |
| :--- | :--- |
| `v@p` (pattern) | kernel pattern `v @ ⟦p⟧` |
| `case e of \| v@{ name: n } -> b` | `case ⟦e⟧ of \| v @ { name = n } -> ⟦b⟧` |
| binding: `v@p = e; body` | `case ⟦e⟧ of \| v @ ⟦p⟧ -> ⟦body⟧` |

---

# Pattern matching `?`

Surface `?` is syntactic sugar for `case` with ordered arms.

| Surface | Desugaring |
| :--- | :--- |
| `e ? \| p1 => b1 \| p2 => b2` | `case ⟦e⟧ of \| ⟦p1⟧ -> ⟦b1⟧ \| ⟦p2⟧ -> ⟦b2⟧` |
| guard: `\| p when g => b` | `\| ⟦p⟧ -> case ⟦g⟧ of \| True -> ⟦b⟧ \| False -> nextArm` (compiled as nested cases) |

Multi-clause functions:

| Surface | Desugaring |
| :--- | :--- |
| `f = \| p1 => b1 \| p2 => b2` | `f = λx#1. case x#1 of \| ⟦p1⟧ -> ⟦b1⟧ \| ⟦p2⟧ -> ⟦b2⟧` |
