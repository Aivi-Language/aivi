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

## 9.2 RHS-Typed Overload Selection

When a domain body contains multiple entries for the same operator token (see [Domains: Within-Domain Operator Overloads](../syntax/domains.md#within-domain-operator-overloads-rhs-typed)), the compiler selects among them by matching the inferred `(LHS, RHS)` operand types:

| Step | Action | Example |
| :--- | :--- | :--- |
| 1. Resolve domain | from LHS carrier type (unchanged) | `identity4 × v` → domain `Matrix` (LHS is `Mat4`) |
| 2. Collect overloads | all entries for the operator token in that domain | `(×)` entries: `Mat4 -> Mat4 -> Mat4`, `Mat4 -> Vec4 -> Vec4` |
| 3. Infer RHS | type-check the RHS expression | `v : Vec4` |
| 4. Select | pick the unique entry whose `(LHS, RHS)` matches | `Mat4 -> Vec4 -> Vec4` ✓ |

### Error: No matching overload

If no overload in `D` has an `(LHS, RHS)` type compatible with the inferred operand types:

<<< ../snippets/from_md/desugaring/domains/error_no_matching_overload.aivi{aivi}

Compiler error (E4010):
```
operator not defined for (Vec3, Mat4) in domain Matrix
available: (*) : Mat4 -> Mat4 -> Mat4, (×) : Mat4 -> Mat4 -> Mat4, (×) : Mat4 -> Vec4 -> Vec4, ...
```

### Error: Ambiguous overload

If two or more overloads remain equally valid after type inference (typically when operand types are still type variables):

<<< ../snippets/from_md/desugaring/domains/error_ambiguous_overload.aivi{aivi}

Compiler error (E4011):
```
ambiguous domain operator '×' for these operand types; candidates: ... vs ...
add a type annotation to disambiguate
```

## 9.3 Ambiguity Errors (Suffix Literal Templates)

When a suffix literal template is missing:

<<< ../snippets/from_md/desugaring/domains/ambiguity_errors_suffix_literal_templates_01.aivi{aivi}

When multiple imports provide the same template name:

<<< ../snippets/from_md/desugaring/domains/ambiguity_errors_suffix_literal_templates_02.aivi{aivi}

Resolution: Avoid collisions by importing only the needed domain(s) and/or using explicit constructors/functions.

<<< ../snippets/from_md/desugaring/domains/ambiguity_errors_suffix_literal_templates_03.aivi{aivi}


## 9.4 Operator Precedence

Domain operators follow standard precedence. Domains do not redefine precedence   only semantics:

<<< ../snippets/from_md/desugaring/domains/operator_precedence.aivi{aivi}


## 9.5 Desugaring Order

1. **Type inference**   Determine carrier types
2. **Delta expansion**   Replace literals with constructors
3. **Domain resolution**   Match `(operator, carrier)` to domain `D`
4. **RHS-overload selection**   Among `D`'s entries for the operator, pick the one matching the inferred `(LHS, RHS)` pair (§9.2)
5. **Function substitution**   Replace operator with the selected implementation
