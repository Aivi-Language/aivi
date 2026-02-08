# Functions and Pipes

## 2.1 Application

* Functions are **curried by default**
* Application is by whitespace

```aivi
add 5 10
```

---

## 2.2 Lambdas

`_` denotes a **single-argument lambda**.

```aivi
inc = _ + 1
```

Multi-argument lambdas must be explicit:

```aivi
add = x y => x + y
```

---

## 2.3 Pipes

Pipelines use `|>`.

```aivi
xs |> map inc |> filter (_ > 0)
```

This allows data to flow from left to right, similar to F# `|>` or Elixir `|>`.
