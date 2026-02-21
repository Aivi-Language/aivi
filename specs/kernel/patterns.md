# Pattern binding (kernel)

## 4.1 Pattern forms

```text
p ::= x
    | _
    | n                       (literal: Int, Float, Text, Char)
    | C p₁ … pₙ
    | (p₁, …, pₙ)
    | [p₁, …, pₖ, ...rest]
    | { l₁ = p₁, … }
```


## 4.2 Whole-value binding

```text
x @ p
```

This is **primitive**, not sugar.
