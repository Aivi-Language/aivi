# Bindings and Scope

## 1.1 Definitions

All bindings use `=`:

* values
* functions
* types
* classes
* instances
* modules

```aivi
pi = 3.14159
add = x y => x + y
```

---

## 1.2 Shadowing

Bindings are lexical and may be shadowed.

```aivi
x = 1
x = x + 1
```

This introduces a new binding; no mutation exists. This is common in functional languages like OCaml and Rust (re-binding) but distinct from mutation.

---

## 1.3 Pattern Bindings

Structural patterns may appear in bindings.

```aivi
{ name: n } = user
[h, ...t] = xs
```

Rule:

* `=` may only be used where the compiler can prove the pattern is **total**
* potentially failing matches must use `?` (case analysis)

---

## 1.4 Whole-value binding with `@`

Patterns may bind the **entire value** alongside destructuring.

```aivi
user@{ name: n } = input
```

Semantics:

* `user` is bound to the whole value
* `{ name: n }` destructures the same value
* no duplication or copying occurs

Allowed in:

* bindings
* `?` pattern arms
* function clauses

Example:

```aivi
describe = u@{ id, name } => `{id}: {name}`
```

---

## 1.5 Usage Examples

### Config Binding

```aivi
config = {
  host: `localhost`
  port: 8080
  debug: True
}

{ host, port } = config
serverUrl = `http://{host}:{port}`
```

### Tuple Destructuring

```aivi
point = (10, 20)
(x, y) = point

distance = sqrt (x * x + y * y)
```

### Nested Destructuring

```aivi
response = {
  data: {
    user: { id: 1, name: `Alice` }
    token: `abc123`
  }
  status: 200
}

{ data: { user: { name } } } = response
greeting = `Hello, {name}!`
```

### List Head/Tail

```aivi
numbers = [1, 2, 3, 4, 5]
[first, second, ...rest] = numbers

-- first = 1, second = 2, rest = [3, 4, 5]
```

### Function Definitions

```aivi
-- Named function
greet = name => `Hello, {name}!`

-- Multi-argument
add = x y => x + y

-- With type annotation
multiply : Int -> Int -> Int
multiply = a b => a * b
```
