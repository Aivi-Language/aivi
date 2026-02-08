# Domains, Units, and Deltas

Domains define **semantics**, not values.

```aivi
domain Calendar
domain Duration
domain Color
domain Vector
```

---

## 6.1 Delta literals

Deltas represent **change**, not quantities.

```aivi
1d
1m
1y
3l
90deg
```

Properties:

* deltas are not numbers
* deltas have no arithmetic
* deltas are interpreted by domains

---

## 6.2 Domain-directed operators

Operators have **no intrinsic meaning**.

```aivi
date + 1m
color + 3l
vector1 + vector2
```

Valid only if the domain defines the operation.

### From Haskell
Similar to type classes defining operators, but strictly for domain-specific semantics rather than generic arithmetic.

### From TypeScript
No direct equivalent; usually handled by libraries like `date-fns` or specialized objects. AIVI makes this a language feature.
