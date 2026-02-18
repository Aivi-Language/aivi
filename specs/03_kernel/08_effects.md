# Effects (kernel)

## 8.1 Effect type

```text
Effect E A
```

Opaque in the kernel.


## 8.2 Effect bind

```text
bind : Effect E A → (A → Effect E B) → Effect E B
```

## 8.3 Effect pure / failure

```text
pure : A → Effect E A
fail : E → Effect E A
```


## 8.4 Attempt (error recovery)

```text
attempt : Effect E A → Effect F (Result E A)
```

Catches failures of type `E`, wrapping them in `Result`. The resulting effect has a *different* error type `F` because `E` is no longer unhandled.

## 8.5 Handle (effect handler)

```text
handle : Effect E A → (E → Effect F A) → Effect F A
```

Provides a handler for failures. The handler function maps the old error type `E` into a new computation with error type `F`.

## 8.6 Effect sequencing

Everything desugars to `bind`.

No `do`, no `effect` in kernel.

## 8.7 Resource bracket

```text
bracket : Effect E A → (A → Effect E Unit) → (A → Effect E B) → Effect E B
```

`bracket acquire release use` — acquires a resource, runs `use`, and guarantees `release` runs even on failure. This is the kernel primitive behind `resource { ... }` blocks (see [Section 15](../02_syntax/15_resources.md)).
