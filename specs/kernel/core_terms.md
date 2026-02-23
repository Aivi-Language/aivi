# Core terms (expression kernel)

## 1.1 Variables

```text
x
```


## 1.2 Lambda abstraction (single-argument)

```text
λx. e
```

Multi-argument functions are **curried desugaring**.


## 1.3 Application

```text
e₁ e₂
```

Whitespace application is syntax only.


## 1.4 Blocks and binding items

```text
{ p <- e₁; e₂ }
```

The kernel models local binding through `Block` expressions with `Bind`/`Expr` items.

Code reference: `crates/aivi/src/kernel/ir.rs` — `crate::kernel::ir::KernelExpr::Block`, `crate::kernel::ir::KernelBlockItem::{Bind,Expr}`

## 1.4.1 Recursive let-binding

Recursion is represented by `Recurse` block items rather than a dedicated `let rec` expression form.

```text
{ recurse e }
```

Code reference: `crates/aivi/src/kernel/ir.rs` — `crate::kernel::ir::KernelBlockItem::Recurse`


## 1.5 Algebraic data constructors

```text
C e₁ … eₙ
```

Nullary constructors are values.


## 1.6 Case analysis (single eliminator)

```text
case e of
  | p₁ → e₁
  | p₂ → e₂
```

`case` is the primary pattern-elimination construct, and the kernel also includes an explicit `if` form.

* `match`
* multi-clause functions
* predicate patterns

all desugar to `case`.

Code reference: `crates/aivi/src/kernel/ir.rs` — `crate::kernel::ir::KernelExpr::{Match,If}`
