# Functions and Pipes

Functions are the everyday building blocks of AIVI. This page shows how to call them, write small inline lambdas, and use pipelines so a transformation reads from left to right.

## 2.1 Application

Function application is written with whitespace, not call parentheses.

- `f x` means “call `f` with `x`”.
- `f x y` means “call `f` with `x`, then call the result with `y`”.
- Because functions are curried by default, supplying fewer arguments produces a new function.

<<< ../snippets/from_md/syntax/functions/choosing_the_pipe_subject_and_argument_position_01.aivi{aivi}

A curried definition is convenient when you want to build reusable helpers:

```aivi
add = x => y => x + y

add 2 3 // call with both arguments
add 2   // returns a new function that adds 2
```

## 2.2 Lambdas

Use `_` as shorthand for a **single-argument lambda**. This is handy for short transforms such as `map (_ + 1)`.

<<< ../snippets/from_md/syntax/functions/choosing_the_pipe_subject_and_argument_position_02.aivi{aivi}

When you need more than one argument, or when a callback deserves a clear name, write the parameters explicitly.

<<< ../snippets/from_md/syntax/functions/choosing_the_pipe_subject_and_argument_position_03.aivi{aivi}

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

<<< ../snippets/from_md/syntax/functions/choosing_the_pipe_subject_and_argument_position_04.aivi{aivi}

### Choosing the pipe subject (and argument position)

`|>` applies the expression on the right to the value on the left.

<<< ../snippets/from_md/syntax/functions/choosing_the_pipe_subject_and_argument_position_01.aivi{aivi}

is equivalent to:

<<< ../snippets/from_md/syntax/functions/choosing_the_pipe_subject_and_argument_position_02.aivi{aivi}

If the right-hand side is already an application, the piped value becomes the **final** argument.

That makes pipelines a good fit for helpers such as `map`, `filter`, and `fold`, where the collection being processed usually comes last.

<<< ../snippets/from_md/syntax/functions/choosing_the_pipe_subject_and_argument_position_03.aivi{aivi}

is equivalent to:

<<< ../snippets/from_md/syntax/functions/choosing_the_pipe_subject_and_argument_position_04.aivi{aivi}

A good rule of thumb: put the value you are “working on” on the left, and keep helper functions on the right.

Pipelines also pair well with `match`, which lets you keep a left-to-right reading order even when branching.

<<< ../snippets/from_md/syntax/functions/basic_functions.aivi{aivi}

See also: [Pattern Matching](pattern_matching.md) for the `match` operator.

---

## 2.4 Usage Examples

### Basic Functions

These examples show ordinary function definitions and calls.

<<< ../snippets/from_md/syntax/functions/basic_functions.aivi{aivi}

### Higher-Order Functions

A higher-order function takes another function as input or returns one as output.

<<< ../snippets/from_md/syntax/functions/higher_order_functions.aivi{aivi}

### Partial Application

Because AIVI functions are curried, you can supply some arguments now and the rest later.

<<< ../snippets/from_md/syntax/functions/partial_application.aivi{aivi}

### Block Pipelines

Pipelines are especially useful when a transformation would otherwise become deeply nested.

<<< ../snippets/from_md/syntax/functions/block_pipelines.aivi{aivi}

### Expressive Logic: Point-Free Style

Point-free style builds a new function by composing existing ones. It can be concise, but use it when the result stays readable to someone seeing the code for the first time.

<<< ../snippets/from_md/syntax/functions/expressive_logic_point_free_style.aivi{aivi}

### Lambda Shorthand

This shorthand keeps small callbacks lightweight.

<<< ../snippets/from_md/syntax/functions/lambda_shorthand.aivi{aivi}
