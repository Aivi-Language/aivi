# Bindings and Scope

## 1.1 Definitions

All bindings use `=`

* values
* functions
* types
* classes
* instances
* modules

<<< ../snippets/from_md/syntax/bindings/definitions.aivi{aivi}


## 1.2 Shadowing

Bindings are lexical and may be shadowed.

<<< ../snippets/from_md/syntax/bindings/shadowing.aivi{aivi}

This introduces a new binding; no mutation exists. This is common in functional languages like OCaml and Rust (re-binding) but distinct from mutation.

## 1.2.1 Recursion (module level)

Within a module body (flat or braced), top-level value bindings are **recursive**: a binding may refer to itself and to bindings that appear later in the same module body.

This supports ordinary recursive functions:

<<< ../snippets/from_md/syntax/bindings/recursion_module_level.aivi{aivi}


## 1.3 Pattern Bindings

Structural patterns may appear in bindings.

<<< ../snippets/from_md/syntax/bindings/pattern_bindings.aivi{aivi}

### Record destructuring (deconstructing records)

To deconstruct a record, use a record pattern on the left-hand side.

<<< ../snippets/from_md/syntax/bindings/record_destructuring_deconstructing_records.aivi{aivi}


You can also destructure nested records using dot-paths (Section 1.5).

* `=` may only be used where the compiler can prove the pattern is **total** (i.e., it covers all possible shapes of the data).
* Potentially failing matches (refutable patterns) must use `match` (case analysis) or appear in a context where failure can be handled.

> [!NOTE]
> Using `=` with a non-total pattern (like `[h, ...t] = []`) results in a compile-time error. For partial matches, use the `match` keyword which converts a refutable pattern into an `Option` or branch.


## 1.4 Whole-value binding with `as`

Patterns may bind the **entire value** alongside destructuring.

<<< ../snippets/from_md/syntax/bindings/whole_value_binding_with_as_01.aivi{aivi}

Semantics:

* `user` is bound to the whole value
* `{ name: n }` destructures the same value
* no duplication or copying occurs

Allowed in:

* Top-level and local bindings
* `match` pattern arms (allowing capture of the matched sub-structure)
* Function clauses 

Example:

<<< ../snippets/from_md/syntax/bindings/whole_value_binding_with_as_02.aivi{aivi}


## 1.5 Usage Examples

### Config Binding

<<< ../snippets/from_md/syntax/bindings/config_binding.aivi{aivi}

### Tuple Destructuring

<<< ../snippets/from_md/syntax/bindings/tuple_destructuring.aivi{aivi}

### Deep path destructuring

Record destructuring supports **dot-paths** to access nested fields directly. This combines path addressing with the `as` whole-value binder.

<<< ../snippets/from_md/syntax/bindings/deep_path_destructuring_01.aivi{aivi}

Semantics:
* `data.user.profile` is the path to the record being destructured.
* `as { name }` binds the fields of that specific nested record.
* Intermediate records are **not** bound unless explicitly requested.

This is exactly equivalent to the nested expansion:

<<< ../snippets/from_md/syntax/bindings/deep_path_destructuring_02.aivi{aivi}

But much more readable for deep hierarchies.

> [!NOTE]
> Deep path destructuring is a powerful tool for working with complex JSON-like data, providing both brevity and clarity.

### List Head/Tail

<<< ../snippets/from_md/syntax/bindings/list_head_tail.aivi{aivi}

### Function Definitions

<<< ../snippets/from_md/syntax/bindings/function_definitions.aivi{aivi}
