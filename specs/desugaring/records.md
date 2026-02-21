# Records: construction and projection

| Surface | Desugaring |
| :--- | :--- |
| `{ a: e1, b: e2 }` | `{ a = ⟦e1⟧, b = ⟦e2⟧ }` |
| `r.a` | `⟦r⟧.a` |
| `r.a.b` | `(⟦r⟧.a).b` |
| `r.a.b as {x}` | `⟦r.a.b⟧ { x }` (projection + binding) |
