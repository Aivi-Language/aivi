# Unified predicate expressions (for `filter`, path predicates, guards)

Predicate expression `pred` used where `A => Bool` expected:

| Surface | Desugaring |
| :--- | :--- |
| `filter (price > 80)` | `filter (λx#1. x#1.price > 80)` |
| `filter (_.price > 80)` | `filter (λx#1. x#1.price > 80)` |
| `filter (Some _)` | `filter (λx#1. case x#1 of \| Some _ -> True \| _ -> False)` |
| `items[price > 80]` (path segment) | traversal filter: `items[*]` + `filter` over element binding (see patch section) |

Rule (normative): inside predicate expressions, bare field `f` resolves to `_.f`.
