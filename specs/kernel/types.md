# Types (kernel)

## 2.1 Types

```text
τ ::= α | τ → τ | T τ₁ … τₙ
```


## 2.2 Universal quantification

```text
∀α. τ
```

This corresponds to `*` in surface syntax.


## 2.3 Record types (closed)

```text
{ l₁ : τ₁, … }
```

* closed records
* structural typing over declared fields
* patching relies on closed-shape updates

## 2.4 Row transforms (derived)

`Pick`, `Omit`, `Optional`, `Required`, `Rename`, and `Defaulted` are surface-level type
operators that transform record rows. They are derived and elaborate to plain record
types in the kernel.
