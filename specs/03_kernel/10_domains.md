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
