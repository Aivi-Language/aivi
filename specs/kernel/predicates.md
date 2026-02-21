# Predicates (kernel)

There is **no predicate syntax** in the kernel.

A predicate is just:

```text
A → Bool
```

### Predicate sugar desugars to:

```text
λx. e
```

where `x` is a fresh variable bound as the implicit argument; field accesses and comparisons in `e` resolve against `x`.

Field shortcuts:

```text
price > 80
⇒ λx. x.price > 80
```

Pattern predicates:

```text
Some _
⇒ λx. case x of Some _ → True | _ → False
```
