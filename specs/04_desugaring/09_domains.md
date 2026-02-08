# Domains and operator resolution

Domains are not terms; they elaborate operator syntax to named functions.

| Surface | Desugaring |
| :--- | :--- |
| `a + b` | `(+)_D ⟦a⟧ ⟦b⟧` where `D` is the resolved domain for the carrier of `a` |
| `date + 1m` | `addMonth date 1m` (or domain-specific `applyDelta`) |
| `col + 3l` | `applyLightness col 3l` |

This is a static rewrite: `(operator, carrier-type)` ↦ implementation.
