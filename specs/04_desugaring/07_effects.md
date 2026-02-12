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
| `effect { e; body }` | `bind ⟦e⟧ (λ_. ⟦effect { body }⟧)` (if `e : Effect E Unit`) |
| `effect { e }` | `⟦e⟧` (the final expression must already be an `Effect`) |
| `effect { }` | `pure Unit` |
| `effect { s1; ...; sn }` (no final expression) | `⟦effect { s1; ...; sn; pure Unit }⟧` |

If you want to return a pure value from an effect block, write `pure value` as the final expression.

If the surface allows `print` etc as effectful calls, those are already `Effect`-typed; no special desugaring beyond `bind`.

## `or` fallback (surface sugar)

`or` is not a general matcher. It is fallback-only sugar.

- Result fallback:

  `res or rhs` desugars to a match on `res` with an implicit `Ok` passthrough arm.

- Effect fallback (only after `<-` inside `effect {}`):

  `x <- eff or rhs` desugars by inserting `attempt` and matching on `Result`:

  `attempt eff` produces `Effect E (Result E A)`, then `Ok a` becomes `pure a` and `Err e` becomes `pure rhs` (or falls through to `fail e` if no fallback arm matches).
