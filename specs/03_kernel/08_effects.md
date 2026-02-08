# Effects (kernel)

## 8.1 Effect type

```text
Effect ε A
```

Opaque in the kernel.

---

## 8.2 Effect bind

```text
bind : Effect ε A → (A → Effect ε B) → Effect ε B
```

---

## 8.3 Effect sequencing

Everything desugars to `bind`.

No `do`, no `effect` in kernel.
