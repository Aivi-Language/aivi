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


## 2.3 Row types (records)

```text
{ l₁ : τ₁, … | ρ }
```

* open records
* structural typing
* patching relies on this
