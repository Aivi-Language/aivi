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


## 8.3 Effect sequencing

Everything desugars to `bind`.

No `do`, no `effect` in kernel.
