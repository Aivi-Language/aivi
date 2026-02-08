# Effects: `effect` block and `do`

Kernel effect primitives:

* `pure : A -> Effect ε A`
* `bind : Effect ε A -> (A -> Effect ε B) -> Effect ε B`

## `do` (Monad comprehension)

| Surface | Desugaring |
| :--- | :--- |
| `do { x <- mx; body }` | `flatMap ⟦mx⟧ (λx. ⟦do { body }⟧)` |
| `do { x = e; body }` | `let x = ⟦e⟧ in ⟦do { body }⟧` |
| `do { e }` | `⟦e⟧` |

## `effect { … }`

`effect` is the same pattern but over `Effect` with `bind/pure`:

| Surface | Desugaring |
| :--- | :--- |
| `effect { x <- e; body }` | `bind ⟦e⟧ (λx. ⟦effect { body }⟧)` |
| `effect { x = e; body }` | `let x = ⟦e⟧ in ⟦effect { body }⟧` |
| `effect { e }` | `⟦e⟧` |

If the surface allows `print` etc as effectful calls, those are already `Effect`-typed; no special desugaring beyond `bind`.
