# Pattern Matching

Pattern matching lets you branch on the shape of a value instead of manually unpacking it with nested `if` checks. It is one of the most direct ways to work with `Option`, `Result`, lists, tuples, records, and custom constructors.

## 8.1 `match` branching

This is the main way to do case analysis, similar to `match` in Rust or `case` in Haskell and Elixir.

### Choosing the match subject

<!-- quick-info: {"kind":"operator","name":"match"} -->
`match` matches on the expression immediately to its left.
<!-- /quick-info -->

<<< ../snippets/from_md/syntax/pattern_matching/choosing_the_match_subject_scrutinee_01.aivi{aivi}

::: repl
```aivi
value = Some 42
value match
  | Some x => "got {x}"
  | None   => "nothing"
// => "got 42"
```
:::

This rule composes nicely with pipelines because `match` comes *after* the full expression:

<<< ../snippets/from_md/syntax/pattern_matching/multi_clause_functions_01.aivi{aivi}

In a multi-clause unary function (Section 8.2), the subject is the function's single implicit argument.

See also: [Functions and Pipes](functions.md) for `|>`, [Closed Records](types/closed_records.md) for record shapes, and [Algebraic Data Types](types/algebraic_data_types.md) for the constructors that patterns match.

Compiler checks:

- non-exhaustive matches are a compile-time error unless a catch-all arm (`_`) is present — this ensures every possible value shape is handled, preventing runtime surprises
- unreachable arms (shadowed by earlier patterns) produce a warning

## 8.2 Multi-clause functions

A multi-clause function is a compact way to define a **unary** function by listing pattern arms directly.

The leading `|` tokens in this form introduce pattern branches; they are not pipelines.

Multi-clause function definitions require an explicit type signature for the function name. With closed records, that signature provides the exact input shape used to type-check each arm.

Use this form when a function's single input is the main thing you want to branch on.

If execution reaches a call where no arm matches, evaluation fails with a non-exhaustive match runtime error.

<<< ../snippets/from_md/syntax/pattern_matching/multi_clause_functions_01.aivi{aivi}

Here is the same idea without `|>`, using rebinding plus record destructuring inside each arm:

<<< ../snippets/from_md/syntax/pattern_matching/multi_clause_functions_record_rebinding_01.aivi{aivi}

## 8.3 Record Patterns

Record patterns let you pick out only the fields you care about. The basic forms are shorthand binding (`{ name }`), binding under a new name (`{ name: n }`), and matching against a nested pattern (`{ role: Admin }`).

<<< ../snippets/from_md/syntax/pattern_matching/matching_and_renaming_instantiation.aivi{aivi}

This is often clearer than reading a field, storing it in a temporary name, and then matching on that temporary later. Use `as` when you want both the whole field value and its destructured pieces; that form is covered in Section 8.5.

## 8.4 Nested Patterns

Nested record patterns let you destructure inner fields in place, so you do not need a tower of temporary bindings. Dotted field paths are also accepted when that reads better.

<<< ../snippets/from_md/syntax/pattern_matching/as_whole_value_plus_destructuring_02.aivi{aivi}

This is the same `as` form in nested position: `profile` stays available as the full nested value while `name` and `age` are also brought into scope.

### Nested constructor patterns

Constructor patterns may themselves take pattern arguments, so you can match several layers at once:

<<< ../snippets/from_md/syntax/pattern_matching/destructuring_only_no_whole_value_binding.aivi{aivi}

### Flattened constructor-chain patterns

For readability, nested constructor patterns can be written without parentheses in pattern position:

<<< ../snippets/from_md/syntax/pattern_matching/flattened_constructor_chain_patterns.aivi{aivi}

This constructor-chain rule applies only in pattern context (after `|` and before `=>`).

## 8.5 Record Pattern Syntax: `as` and `:`

Record patterns use a few small pieces of syntax for distinct jobs.

### `:` Matching and renaming (instantiation)

`{ field: pat }` matches the field and binds the result of the nested pattern `pat`. This is the main form for both matching and renaming.

<<< ../snippets/from_md/syntax/pattern_matching/matching_and_renaming_instantiation.aivi{aivi}

### `as` Whole-value plus destructuring

`field as { pat }` binds `field` to the **entire** field value *and* destructures it. Both `field` and the contents of `pat` are in scope.

<<< ../snippets/from_md/syntax/pattern_matching/as_whole_value_plus_destructuring_01.aivi{aivi}

In nested position:

<<< ../snippets/from_md/syntax/pattern_matching/as_whole_value_plus_destructuring_02.aivi{aivi}

Here `profile` is bound to the full value of the `profile` field, and `name` / `age` are also brought into scope from within that field.

### Summary

| Syntax | `field` in scope? | `pat` contents in scope? | Use case |
| :--- | :---: | :---: | :--- |
| `{ field: pat }` | no (renamed) | yes | Match or rename a field |
| `{ field as { pat } }` | yes | yes | Keep the whole field and destructure it |
| `{ field }` | yes |   | Shorthand, binds field by name |

## 8.6 Guards

Patterns can have guards using `when`.

Use a guard when the shape matches, but you still need an extra boolean condition.

<<< ../snippets/from_md/syntax/pattern_matching/guards.aivi{aivi}

## 8.7 Usage Examples

### Option Handling

<<< ../snippets/from_md/syntax/pattern_matching/option_handling.aivi{aivi}

### Result Processing

<<< ../snippets/from_md/syntax/pattern_matching/result_processing.aivi{aivi}

### List Processing

<<< ../snippets/from_md/syntax/pattern_matching/list_processing.aivi{aivi}

### Tree Traversal

<<< ../snippets/from_md/syntax/pattern_matching/tree_traversal.aivi{aivi}

### Expression Evaluation

<<< ../snippets/from_md/syntax/pattern_matching/expression_evaluation.aivi{aivi}

## 8.8 Expressive Pattern Orchestration

Pattern matching turns complicated conditional logic into a set of named cases, which is often easier to read and easier to extend.

<<< ../snippets/from_md/syntax/pattern_matching/expressive_pattern_orchestration.aivi{aivi}

### Concise State Transitions

<<< ../snippets/from_md/syntax/pattern_matching/concise_state_transitions.aivi{aivi}

### Expressive Logic Branches

<<< ../snippets/from_md/syntax/pattern_matching/expressive_logic_branches.aivi{aivi}
