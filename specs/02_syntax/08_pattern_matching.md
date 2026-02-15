# Pattern Matching

## 8.1 `?` branching

<<< ../snippets/from_md/02_syntax/08_pattern_matching/block_01.aivi{aivi}

This is a concise way to do case analysis, similar to `match` in Rust or `case` in Haskell/Elixir.

### Choosing the match subject (scrutinee)

<!-- quick-info: {"kind":"operator","name":"?"} -->
`?` matches on the expression immediately to its left.
<!-- /quick-info -->

```aivi
value = parse input

value ?
  | Ok x  => x
  | Err _ => 0
```

This rule composes with pipelines because `?` comes *after* the full expression:

```aivi
input |> parse |> validate ?
  | Ok x  => x
  | Err e => handle e
```

### Deconstructor match heads (`!` subject selection)

For unary functions that destructure their argument, you can mark one or more binders in the parameter pattern with `!` and start the body with `?` (without writing `=> <scrutinee>` explicitly):

```aivi
g = { name! } ?
  | "A" => 1
  | _   => 0
```

is shorthand for:

```aivi
g = { name } => name ?
  | "A" => 1
  | _   => 0
```

If multiple binders are marked with `!`, the match scrutinee is a tuple (in left-to-right order).

In a multi-clause unary function (Section 8.2), the subject is the function's single implicit argument.

See also: [Functions and Pipes](02_functions.md) for `|>`.

Compiler checks:

- Non-exhaustive matches are a compile-time error unless a catch-all arm (`_`) is present.
- Unreachable arms (shadowed by earlier patterns) produce a warning.


## 8.2 Multi-clause functions

This is **not** a pipeline (`|>`). A leading `|` introduces an arm of a **unary** function that pattern-matches on its single (implicit) argument.

<<< ../snippets/from_md/02_syntax/08_pattern_matching/block_02.aivi{aivi}

<<< ../snippets/from_md/02_syntax/08_pattern_matching/block_03.aivi{aivi}


## 8.3 Record Patterns

<<< ../snippets/from_md/02_syntax/08_pattern_matching/block_04.aivi{aivi}


## 8.4 Nested Patterns

Record patterns support dotted keys, so nested patterns can often be written without extra braces.

<<< ../snippets/from_md/02_syntax/08_pattern_matching/block_05.aivi{aivi}

### Nested constructor patterns

Constructor patterns may themselves take pattern arguments, so you can nest them:

<<< ../snippets/from_md/02_syntax/08_pattern_matching/block_06.aivi{aivi}

### Flattened constructor-chain patterns

For readability, nested constructor patterns can be written without parentheses in pattern position:

<<< ../snippets/from_md/02_syntax/08_pattern_matching/block_07.aivi{aivi}

This "constructor chain" rule applies only in pattern context (after `|` and before `=>`).

## 8.5 Whole-value binding `@`

Patterns support whole-value binding:

```aivi
user@{ name, age }
```

This binds `user` to the entire matched value while also matching the record pattern `{ name, age }`.

To bind a nested value *and* destructure it, combine record-field paths with whole-value binding:

```aivi
{ profile@profile@{ name, age } }
```

Here the outer `profile@...` selects the record field, and the inner `profile@{ ... }` binds the full field value.

## 8.6 Guards

Patterns can have guards using `when`:

<<< ../snippets/from_md/02_syntax/08_pattern_matching/block_08.aivi{aivi}


## 8.7 Usage Examples

### Option Handling

<<< ../snippets/from_md/02_syntax/08_pattern_matching/block_09.aivi{aivi}

### Result Processing

<<< ../snippets/from_md/02_syntax/08_pattern_matching/block_10.aivi{aivi}

### List Processing

<<< ../snippets/from_md/02_syntax/08_pattern_matching/block_11.aivi{aivi}

### Tree Traversal

<<< ../snippets/from_md/02_syntax/08_pattern_matching/block_12.aivi{aivi}

### Expression Evaluation

<<< ../snippets/from_md/02_syntax/08_pattern_matching/block_13.aivi{aivi}

## 8.8 Expressive Pattern Orchestration

Pattern matching excels at simplifying complex conditional branches into readable declarations.

<<< ../snippets/from_md/02_syntax/08_pattern_matching/block_14.aivi{aivi}

### Concise State Machines

<<< ../snippets/from_md/02_syntax/08_pattern_matching/block_15.aivi{aivi}

### Expressive Logic Branches

<<< ../snippets/from_md/02_syntax/08_pattern_matching/block_16.aivi{aivi}
