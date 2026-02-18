# Notation conventions

The following conventions are used throughout the desugaring rules:

| Notation | Meaning |
| :--- | :--- |
| `⟦e⟧` | Recursive desugaring of surface expression `e` |
| `λx. e` | Kernel lambda abstraction |
| `x#1`, `v#1` | **Fresh binders** — compiler-generated names guaranteed not to clash with user-written identifiers. The `#n` suffix is a disambiguation index (not valid surface syntax). |

# Bindings, blocks, and shadowing

| Surface | Desugaring |
| :--- | :--- |
| `x = e` (top-level) | kernel `let rec x = ⟦e⟧ in …` (module elaboration; module-level bindings are recursive by default) |
| block: `f = a => b1 b2 b3` | `f = a => let _ = ⟦b1⟧ in let _ = ⟦b2⟧ in ⟦b3⟧` if `b1,b2` are effectless statements; if they are bindings, see next rows |
| block binding: `x = e` inside block | `let x = ⟦e⟧ in …` |
| shadowing: `x = 1; x = x + 1` | `let x = 1 in let x = x + 1 in …` |
