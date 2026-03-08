# Functions and Pipes

Functions are the everyday building blocks of AIVI. This page shows how to call them, write anonymous helpers, and use pipelines so a transformation reads from left to right.

## 2.1 Application

Function application is written with whitespace, not call parentheses.

- `f x` means “call `f` with `x`”.
- `f x y` means “call `f` with `x`, then call the result with `y`”.
- Because functions are curried by default, supplying fewer arguments produces a new function.

```aivi
add : Int -> Int -> Int
add = x y => x + y

sum   = add 2 3
add2  = add 2
total = add2 10
```

A multi-argument definition is still curried, so `add 2` returns a new function that remembers the first argument.

## 2.2 Lambdas

Use `=>` to write anonymous functions directly where you need them.

```aivi
increment = x => x + 1
pair      = x y => (x, y)
```

Use `_` as shorthand for a **single-argument lambda** when the surrounding context already expects a unary function.

```aivi
numbers |> map (_ + 1)
scores  |> filter (_ > 0)
```

Outside a unary-function position, write the parameter explicitly instead of relying on `_`.

When you need more than one argument, or when a callback deserves a clear name, write the parameters explicitly.

```aivi
applyTwice = f x => f (f x)
labelUser  = user => "{user.name} ({user.age})"
```

---

## 2.3 Pipes

<!-- quick-info: {"kind":"operator","name":"|>"} -->
Pipelines use `|>`.
<!-- /quick-info -->

Pipes are for readable data flow. Start with a value on the left, then keep transforming it step by step on the right.

```aivi
order
  |> validate // first check the value you are working on
  |> save     // then pass the validated value to the next step
```

### Choosing the pipe subject (and argument position)

`|>` applies the expression on the right to the value on the left.

```aivi
x |> f
```

is equivalent to:

```aivi
f x
```

If the right-hand side is already an application, the piped value becomes the **final** argument.

That makes pipelines a good fit for helpers such as `map`, `filter`, and `fold`, where the collection being processed usually comes last.

```aivi
x |> f a b
```

is equivalent to:

```aivi
f a b x
```

```aivi
users
  |> filter active
  |> map.name
```

In the example above, `filter active` uses predicate lifting and `map.name` uses accessor sugar. See [Predicates](predicates.md) and [Operators and Context](operators.md) for those shorthand forms.

A good rule of thumb: put the value you are “working on” on the left, and keep helper functions on the right.

Pipelines also pair well with `match`, which lets you keep a left-to-right reading order even when branching.

```aivi
input |> parse match
  | Ok x  => x
  | Err _ => 0
```

See also: [Pattern Matching](pattern_matching.md) for the `match` operator.

---

## 2.4 Usage Examples

These examples show common styles you will see in real AIVI code: ordinary function calls, higher-order helpers, partial application, pipelines, and concise shorthand forms.

### Basic Functions

These examples show ordinary function definitions and calls.

```aivi
double : Int -> Int
double = x => x * 2

add : Int -> Int -> Int
add = x y => x + y

answer = add 10 (double 5)
```

### Higher-Order Functions

A higher-order function takes another function as input or returns one as output.

<<< ../snippets/from_md/syntax/functions/higher_order_functions.aivi{aivi}

### Partial Application

Because AIVI functions are curried, you can supply some arguments now and the rest later.

<<< ../snippets/from_md/syntax/functions/partial_application.aivi{aivi}

### Block Pipelines

Pipelines are especially useful when a transformation would otherwise become deeply nested.

<<< ../snippets/from_md/syntax/functions/block_pipelines.aivi{aivi}

This example relies on the same shorthands from Section 2.3: `filter active` treats `active` as a predicate on each element, and `map.name` builds an accessor from `.name`.

### Expressive Logic: Point-Free Style

Point-free style builds a new function by partially applying or composing existing helpers. It can be concise, but use it when the result stays readable to someone seeing the code for the first time.

```aivi
use aivi.text (contains)

isEmailLike : Text -> Bool
isEmailLike = contains "@"

keepEmailLikes : List Text -> List Text
keepEmailLikes = filter isEmailLike
```

Here `keepEmailLikes` is defined without naming the list argument explicitly.

### Lambda Shorthand

This shorthand keeps small callbacks lightweight. Accessor sugar such as `.name` and predicate lifting such as `filter active` are described in more detail in [Operators and Context](operators.md) and [Predicates](predicates.md).

<<< ../snippets/from_md/syntax/functions/lambda_shorthand.aivi{aivi}
