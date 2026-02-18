# Pattern Matching

## 8.1 `match` branching

<<< ../snippets/from_md/02_syntax/08_pattern_matching/block_01.aivi{aivi}

This is a concise way to do case analysis, similar to `match` in Rust or `case` in Haskell/Elixir.

### Choosing the match subject (scrutinee)

<!-- quick-info: {"kind":"operator","name":"match"} -->
`match` matches on the expression immediately to its left.
<!-- /quick-info -->

```aivi
value = parse input

value match
  | Ok x  => x
  | Err _ => 0
```

This rule composes with pipelines because `match` comes *after* the full expression:

```aivi
input |> parse |> validate match
  | Ok x  => x
  | Err e => handle e
```

### Deconstructor match heads (`!` subject selection)

For unary functions that destructure their argument, you can mark one or more binders in the parameter pattern with `!` and start the body with `match` (without writing `=> <scrutinee>` explicitly):

```aivi
g = { name! } match
  | "A" => 1
  | _   => 0
```

is shorthand for:

```aivi
g = { name } => name match
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

## 8.5 Record Pattern Syntax: `@`, `.{ }`, and `:`

Record patterns use three distinct operators for different purposes:

### `:` — Matching and renaming (instantiation)

`{ field: pat }` matches the field and binds the result of the nested pattern `pat`. This is the primary form for both matching and renaming:

```aivi
{ name: n }           // binds field 'name' to variable 'n'
{ role: Admin }       // matches field 'role' against constructor 'Admin'
{ name }              // shorthand for { name: name }
```

### `@` — Whole-value plus destructuring

`field@{ pat }` binds `field` to the **entire** field value *and* destructures it. Both `field` and the contents of `pat` are in scope:

```aivi
user@{ name, age }    // 'user' holds the whole record; 'name' and 'age' are also bound
```

In nested position:

```aivi
{ profile@{ name, age } }
```

Here `profile` is bound to the full value of the `profile` field, and `name`/`age` are also brought into scope from within that field.

### `.{ }` — Destructuring only (no whole-value binding)

`field.{ pat }` destructures the field but does **not** bind the field itself. Only the contents of the nested pattern are in scope:

```aivi
{ profile.{ name, age } }
```

Here `name` and `age` are in scope, but `profile` is **not** — it is only used as a path to reach the nested fields.

### Summary

| Syntax | `field` in scope? | `pat` contents in scope? | Use case |
| :--- | :---: | :---: | :--- |
| `{ field: pat }` | no (renamed) | yes | Match/rename a field |
| `{ field@{ pat } }` | yes | yes | Keep whole field + destructure |
| `{ field.{ pat } }` | no | yes | Destructure only, discard field |
| `{ field }` | yes | — | Shorthand, binds field by name |

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
