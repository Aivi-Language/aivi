# Domains (kernel)

Domains are **not values**.

They are **static rewrite rules**:

```text
(operator, carrier-type) ↦ implementation
```

Example:

```text
(+, Date × MonthDelta) ↦ addMonth
```

This is **outside the term language**, like typing rules.

In v0.1, domains participate in two related rewrites:

1. **Operator resolution**: surface operators like `+` / `<` elaborate to calls to in-scope operator functions (e.g. `(+)`) when the operand types are not plain `Int`.
2. **Literal templates**: suffixed number literals like `10ms` elaborate to applying an in-scope template function `1ms` to `10`.

These are compile-time elaborations. Domain names do not exist as term-level values.
