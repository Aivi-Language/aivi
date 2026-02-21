# Pattern Matching

## 8.1 `match` branching

<<< ../snippets/from_md/syntax/pattern_matching/match_branching.aivi{aivi}

This is a concise way to do case analysis, similar to `match` in Rust or `case` in Haskell/Elixir.

### Choosing the match subject (scrutinee)

<!-- quick-info: {"kind":"operator","name":"match"} -->
`match` matches on the expression immediately to its left.
<!-- /quick-info -->

<<< ../snippets/from_md/syntax/pattern_matching/choosing_the_match_subject_scrutinee_01.aivi{aivi}


This rule composes with pipelines because `match` comes *after* the full expression:

<<< ../snippets/from_md/syntax/pattern_matching/choosing_the_match_subject_scrutinee_02.aivi{aivi}


In a multi-clause unary function (Section 8.2), the subject is the function's single implicit argument.

See also: [Functions and Pipes](functions.md) for `|>`.

Compiler checks:

- Non-exhaustive matches are a compile-time error unless a catch-all arm (`_`) is present.
- Unreachable arms (shadowed by earlier patterns) produce a warning.


## 8.2 Multi-clause functions

This is **not** a pipeline (`|>`). A leading `|` introduces an arm of a **unary** function that pattern-matches on its single (implicit) argument.

<<< ../snippets/from_md/syntax/pattern_matching/multi_clause_functions_01.aivi{aivi}

<<< ../snippets/from_md/syntax/pattern_matching/multi_clause_functions_02.aivi{aivi}


## 8.3 Record Patterns

<<< ../snippets/from_md/syntax/pattern_matching/record_patterns.aivi{aivi}


## 8.4 Nested Patterns

Record patterns support dotted keys, so nested patterns can often be written without extra braces.

<<< ../snippets/from_md/syntax/pattern_matching/nested_patterns.aivi{aivi}

### Nested constructor patterns

Constructor patterns may themselves take pattern arguments, so you can nest them:

<<< ../snippets/from_md/syntax/pattern_matching/nested_constructor_patterns.aivi{aivi}

### Flattened constructor-chain patterns

For readability, nested constructor patterns can be written without parentheses in pattern position:

<<< ../snippets/from_md/syntax/pattern_matching/flattened_constructor_chain_patterns.aivi{aivi}

This "constructor chain" rule applies only in pattern context (after `|` and before `=>`).

## 8.5 Record Pattern Syntax: `as`, `.{ }`, and `:`

Record patterns use three distinct operators for different purposes:

### `:`   Matching and renaming (instantiation)

`{ field: pat }` matches the field and binds the result of the nested pattern `pat`. This is the primary form for both matching and renaming:

<<< ../snippets/from_md/syntax/pattern_matching/matching_and_renaming_instantiation.aivi{aivi}


### `as`   Whole-value plus destructuring

`field as { pat }` binds `field` to the **entire** field value *and* destructures it. Both `field` and the contents of `pat` are in scope:

<<< ../snippets/from_md/syntax/pattern_matching/as_whole_value_plus_destructuring_01.aivi{aivi}


In nested position:

<<< ../snippets/from_md/syntax/pattern_matching/as_whole_value_plus_destructuring_02.aivi{aivi}


Here `profile` is bound to the full value of the `profile` field, and `name`/`age` are also brought into scope from within that field.

### `.{ }`   Destructuring only (no whole-value binding)

`field.{ pat }` destructures the field but does **not** bind the field itself. Only the contents of the nested pattern are in scope:

<<< ../snippets/from_md/syntax/pattern_matching/destructuring_only_no_whole_value_binding.aivi{aivi}


Here `name` and `age` are in scope, but `profile` is **not**   it is only used as a path to reach the nested fields.

### Summary

| Syntax | `field` in scope? | `pat` contents in scope? | Use case |
| :--- | :---: | :---: | :--- |
| `{ field: pat }` | no (renamed) | yes | Match/rename a field |
| `{ field as { pat } }` | yes | yes | Keep whole field + destructure |
| `{ field.{ pat } }` | no | yes | Destructure only, discard field |
| `{ field }` | yes |   | Shorthand, binds field by name |

## 8.6 Guards

Patterns can have guards using `when`:

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

Pattern matching excels at simplifying complex conditional branches into readable declarations.

<<< ../snippets/from_md/syntax/pattern_matching/expressive_pattern_orchestration.aivi{aivi}

### Concise State Machines

<<< ../snippets/from_md/syntax/pattern_matching/concise_state_machines.aivi{aivi}

### Expressive Logic Branches

<<< ../snippets/from_md/syntax/pattern_matching/expressive_logic_branches.aivi{aivi}
