# Pattern Matching

## 8.1 `?` branching

```aivi
classify = _ ?
  | 0 => `zero`
  | _ => `nonzero`
```

This is a concise way to do case analysis, similar to `match` in Rust or `case` in Haskell/Elixir.

---

## 8.2 Multi-clause functions

```aivi
sum =
  | [] => 0
  | [h, ...t] => h + sum t
```
