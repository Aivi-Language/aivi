# Functions and Pipes

Functions are the everyday building blocks of AIVI. This page shows how to call them, write anonymous helpers, and use pipelines so a transformation reads from left to right.

## 2.1 Application

Function application is written with whitespace, not call parentheses.

- `f x` means “call `f` with `x`”.
- `f x y` means “call `f` with `x`, then call the result with `y`”.
- Because functions are curried by default, supplying fewer arguments produces a new function.

<<< ../snippets/from_md/syntax/functions/block_01.aivi{aivi}

::: repl
```aivi
add = x y => x + y
add 2 3
// => 5
add2 = add 2
add2 10
// => 12
```
:::

A multi-argument definition is still curried, so `add 2` returns a new function that remembers the first argument.

## 2.2 Lambdas

Use `=>` to write anonymous functions directly where you need them.

<<< ../snippets/from_md/syntax/functions/block_02.aivi{aivi}


Use `_` as shorthand for a **single-argument lambda** when the surrounding context already expects a unary function.

<<< ../snippets/from_md/syntax/functions/block_03.aivi{aivi}


Outside a unary-function position, write the parameter explicitly instead of relying on `_`.

When you need more than one argument, or when a callback deserves a clear name, write the parameters explicitly.

<<< ../snippets/from_md/syntax/functions/block_04.aivi{aivi}


---

## 2.3 Pipes

<!-- quick-info: {"kind":"operator","name":"|>"} -->
Pipelines use `|>`.
<!-- /quick-info -->

Pipes are for readable data flow. Start with a value on the left, then keep transforming it step by step on the right.

<<< ../snippets/from_md/syntax/functions/block_05.aivi{aivi}

::: repl
```aivi
[1, 2, 3, 4, 5]
  |> filter (_ > 2)
  |> map (_ * 10)
// => [30, 40, 50]
```
:::

### Choosing the pipe subject (and argument position)

`|>` applies the expression on the right to the value on the left.

<<< ../snippets/from_md/syntax/functions/block_06.aivi{aivi}


is equivalent to:

<<< ../snippets/from_md/syntax/functions/block_07.aivi{aivi}


If the right-hand side is already an application, the piped value becomes the **final** argument.

That makes pipelines a good fit for helpers such as `map`, `filter`, and `fold`, where the collection being processed usually comes last.

<<< ../snippets/from_md/syntax/functions/block_08.aivi{aivi}


is equivalent to:

<<< ../snippets/from_md/syntax/functions/block_09.aivi{aivi}


<<< ../snippets/from_md/syntax/functions/block_10.aivi{aivi}


In the example above, `filter active` uses predicate lifting and `map.name` uses accessor sugar. See [Predicates](predicates.md) and [Operators and Context](operators.md) for those shorthand forms.

A good rule of thumb: put the value you are “working on” on the left, and keep helper functions on the right.

Pipelines also pair well with `match`, which lets you keep a left-to-right reading order even when branching.

<<< ../snippets/from_md/syntax/functions/block_11.aivi{aivi}


See also: [Pattern Matching](pattern_matching.md) for the `match` operator.

---

## 2.4 Usage Examples

These examples show common styles you will see in real AIVI code: ordinary function calls, higher-order helpers, partial application, pipelines, and concise shorthand forms.

### Basic Functions

These examples show ordinary function definitions and calls.

<<< ../snippets/from_md/syntax/functions/block_12.aivi{aivi}


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

<<< ../snippets/from_md/syntax/functions/block_13.aivi{aivi}


Here `keepEmailLikes` is defined without naming the list argument explicitly.

### Lambda Shorthand

This shorthand keeps small callbacks lightweight. Accessor sugar such as `.name` and predicate lifting such as `filter active` are described in more detail in [Operators and Context](operators.md) and [Predicates](predicates.md).

<<< ../snippets/from_md/syntax/functions/lambda_shorthand.aivi{aivi}
