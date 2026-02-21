# Traversals (kernel)

## 6.1 Fold (only traversal primitive)

```text
fold : ∀A B. (B → A → B) → B → List A → B
```

Everything else is built from this:

* `map`
* `filter`
* patch traversals
* generators
