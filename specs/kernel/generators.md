# Generators (kernel)

## 7.1 Generator as a Church-encoded fold

```text
Generator A ≡ ∀R. (R → A → R) → R → R
```

This means:

* generators are **just folds**
* no runtime suspension
* no special execution model

## 7.2 `yield`

```text
yield x ≡ λk. λacc. k acc x
```
