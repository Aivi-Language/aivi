# Effects: `effect` block

Kernel effect primitives:

* `pure : A -> Effect E A`
* `bind : Effect E A -> (A -> Effect E B) -> Effect E B`
* `fail : E -> Effect E A`

## `effect { … }`

`effect` is the same pattern but over `Effect` with `bind/pure`:

| Surface | Desugaring |
| :--- | :--- |
| `effect { x <- e; body }` | `bind ⟦e⟧ (λx. ⟦effect { body }⟧)` |
| `effect { x = e; body }` | `let x = ⟦e⟧ in ⟦effect { body }⟧` |
| `effect { e; body }` | `bind ⟦e⟧ (λ_. ⟦effect { body }⟧)` (if `e : Effect`) |
| `effect { e }` | `⟦e⟧` (the final expression must already be an `Effect`) |

If you want to return a pure value from an effect block, write `pure value` as the final expression.

If the surface allows `print` etc as effectful calls, those are already `Effect`-typed; no special desugaring beyond `bind`.
