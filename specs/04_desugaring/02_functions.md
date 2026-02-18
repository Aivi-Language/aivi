# Functions and application (comma-free)

| Surface | Desugaring |
| :--- | :--- |
| `x y => e` | `λx. λy. ⟦e⟧` |
| `x => e` | `λx. ⟦e⟧` |
| `f a b` | `⟦f⟧ ⟦a⟧ ⟦b⟧` (left-assoc) |
| `(e)` | `⟦e⟧` |

## Pipes `|>`

`|>` is just application with the operands swapped.

| Surface | Desugaring |
| :--- | :--- |
| `x |> f` | `⟦f⟧ ⟦x⟧` |
| `x |> f a` | `⟦f⟧ ⟦a⟧ ⟦x⟧` |
| `x |> f a b` | `⟦f⟧ ⟦a⟧ ⟦b⟧ ⟦x⟧` |

## Deconstructor heads (`!` subject selection)

When a unary function's parameter pattern contains one or more binders marked with `!`, the function body may start with `|>` or `match` and omit the explicit scrutinee/pipe subject.

`!` marks the **subject binders**; if there are multiple, the subject is a tuple in left-to-right order.

| Surface | Desugaring |
| :--- | :--- |
| `f = { name! } |> step` | `f = { name } => name |> step` |
| `f = x! y! |> step` | `f = x y => (x, y) |> step` |
| `f = { name! } match \| p => b` | `f = { name } => name match \| p => b` |


# Placeholder lambda `_`

`_` is only valid where a unary function is expected (syntactically or by typing). It desugars to a fresh binder.

| Surface | Desugaring |
| :--- | :--- |
| `_ + 1` (in lambda position) | `λx#1. x#1 + 1` |
| `toUpper` (value) | `toUpper` (no change) |
