# Classes (kernel)

## 9.1 Class = record of functions

```text
Class C τ ≡ { methods }
```

## 9.2 Instance = value

```text
instance : Class C τ
```

Resolution is **compile-time**, not runtime.

## 9.3 Type-variable constraints (surface)

Surface `class` declarations may attach constraints to type variables used in method signatures
via `given (...)`. The kernel represents these as additional evidence requirements for the
corresponding methods.
