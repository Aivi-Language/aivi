# Record patching (derived, not primitive)

A patch:

```text
x <= { a.b.c : f }
```

Desugars to nested `update` + `fold`:

```text
update x "a" (λa.
  update a "b" (λb.
    update b "c" f))
```

Predicates become `filter` over folds.

Removal is `update` to `None` + row shrink.
