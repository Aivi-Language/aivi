# Functions and application (comma-free)

| Surface | Desugaring |
| :--- | :--- |
| `x y => e` | `λx. λy. ⟦e⟧` |
| `x => e` | `λx. ⟦e⟧` |
| `f a b` | `⟦f⟧ ⟦a⟧ ⟦b⟧` (left-assoc) |
| `(e)` | `⟦e⟧` |


# Placeholder lambda `_`

`_` is only valid where a unary function is expected (syntactically or by typing). It desugars to a fresh binder.

| Surface | Desugaring |
| :--- | :--- |
| `_ + 1` (in lambda position) | `λx#1. x#1 + 1` |
| `toUpper` (value) | `toUpper` (no change) |
