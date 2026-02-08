# Core terms (expression kernel)

## 1.1 Variables

```text
x
```

---

## 1.2 Lambda abstraction (single-argument)

```text
λx. e
```

Multi-argument functions are **curried desugaring**.

---

## 1.3 Application

```text
e₁ e₂
```

Whitespace application is syntax only.

---

## 1.4 Let-binding

```text
let x = e₁ in e₂
```

All top-level and block bindings desugar to `let`.

---

## 1.5 Algebraic data constructors

```text
C e₁ … eₙ
```

Nullary constructors are values.

---

## 1.6 Case analysis (single eliminator)

```text
case e of
  | p₁ → e₁
  | p₂ → e₂
```

This is the **only branching construct**.

* `?`
* multi-clause functions
* predicate patterns

all desugar to `case`.
