# Domains and operator resolution

Domains are not terms; they elaborate operator syntax to named functions.

| Surface | Desugaring |
| :--- | :--- |
| `a + b` | `(+)_D ⟦a⟧ ⟦b⟧` where `D` is the resolved domain for the carrier of `a` |
| `date + 1m` | `addMonth date 1m` (or domain-specific `applyDelta`) |
| `col + 3l` | `applyLightness col 3l` |

This is a static rewrite: `(operator, carrier-type)` ↦ implementation.

---

## 9.1 Delta Literal Resolution

Delta literals are **domain-scoped**. Resolution follows a two-step process:

| Step | Action | Example |
| :--- | :--- | :--- |
| 1. Lexical lookup | Find delta binding in imported domains | `1m` → defined in Calendar, Physics |
| 2. Carrier disambiguation | Select domain matching operand type | `date + 1m` → Calendar (date : Date) |

### Resolution Chain

```text
date + 1m
  ↓ (step 1: find delta)
  1m is defined in: Calendar.Delta.Month, Physics.Delta.Meter
  ↓ (step 2: carrier type)
  date : Date → Calendar domain
  ↓ (step 3: expand delta)
  date + (Month 1)
  ↓ (step 4: resolve operator)
  Calendar.(+) date (Month 1)
  ↓ (step 5: desugar to implementation)
  addMonth date (Month 1)
```

---

## 9.2 Ambiguity Errors

When carrier type cannot disambiguate:

```aivi
x + 1m  -- Error: x : Int, neither Calendar nor Physics apply
```

When multiple domains match:

```aivi
-- If both Calendar and Physics define (+) over the same carrier
ambiguous + 1m  -- Error: Ambiguous domain for (+)
```

Resolution: Use qualified literals or operators.

```aivi
date + Calendar.1m
position + Physics.1m
```

---

## 9.3 Operator Precedence

Domain operators follow standard precedence. Domains do not redefine precedence — only semantics:

```aivi
date + 1m * 2  -- Parsed as: date + (1m * 2)
               -- Error if (*) not defined for Delta
```

---

## 9.4 Desugaring Order

1. **Type inference** — Determine carrier types
2. **Delta expansion** — Replace literals with constructors
3. **Domain resolution** — Match (operator, carrier) to domain
4. **Function substitution** — Replace operator with implementation

