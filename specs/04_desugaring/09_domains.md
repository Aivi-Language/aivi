# Domains and operator resolution

Domains are not terms; they elaborate operator syntax to named functions.

| Surface | Desugaring |
| :--- | :--- |
| `a + b` | `(+)_D ⟦a⟧ ⟦b⟧` where `D` is the resolved domain for the carrier of `a` |
| `date + 1m` | `addMonth date 1m` (or domain-specific `applyDelta`) |
| `col + 3l` | `applyLightness col 3l` |
| `(x)px` | `1px ⟦x⟧` (apply the in-scope literal template) |

This is a static rewrite: `(operator, carrier-type)` ↦ implementation.


## 9.1 Delta Literal Resolution

Delta literals are **domain-scoped**. In v0.1, resolution is lexical:

| Step | Action | Example |
| :--- | :--- | :--- |
| 1. Lexical lookup | Find the literal template `1{suffix}` in scope | `10ms` → requires `1ms` to be in scope |

### Resolution Chain

```text
date + 1m
  ↓ (step 1: find template)
  1m resolves to the in-scope template `1m`
  ↓ (step 2: expand delta)
  date + (Month 1)
  ↓ (step 3: resolve operator)
  Calendar.(+) date (Month 1)
  ↓ (step 4: desugar to implementation)
  addMonth date (Month 1)
```

If `1m` is not in scope, this is a compile-time error. If multiple imports provide a `1m` template, the current compiler does not perform carrier-based disambiguation; prefer avoiding such collisions via selective imports/hiding or explicit constructors.

## 9.2 Ambiguity Errors

When a suffix literal template is missing:

<<< ../snippets/from_md/04_desugaring/09_domains/block_01.aivi{aivi}

When multiple imports provide the same template name:

<<< ../snippets/from_md/04_desugaring/09_domains/block_02.aivi{aivi}

Resolution: Avoid collisions by importing only the needed domain(s) and/or using explicit constructors/functions.

<<< ../snippets/from_md/04_desugaring/09_domains/block_03.aivi{aivi}


## 9.3 Operator Precedence

Domain operators follow standard precedence. Domains do not redefine precedence   only semantics:

<<< ../snippets/from_md/04_desugaring/09_domains/block_04.aivi{aivi}


## 9.4 Desugaring Order

1. **Type inference**   Determine carrier types
2. **Delta expansion**   Replace literals with constructors
3. **Domain resolution**   Match (operator, carrier) to domain
4. **Function substitution**   Replace operator with implementation
