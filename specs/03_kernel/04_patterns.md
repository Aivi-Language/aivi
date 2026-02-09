# Pattern binding (kernel)

## 4.1 Pattern forms

```text
p ::= x
    | C p₁ … pₙ
    | { l₁ = p₁, … }
```


## 4.2 Whole-value binding

```text
x @ p
```

This is **primitive**, not sugar.
