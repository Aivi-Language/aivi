# Records: construction and projection

| Surface | Desugaring |
| :--- | :--- |
| `{ a: e1, b: e2 }` | `{ a = ⟦e1⟧, b = ⟦e2⟧ }` |
| `r.a` | `⟦r⟧.a` |
| nested: `r.a.b` | `(⟦r⟧.a).b` |
