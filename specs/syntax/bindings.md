# Bindings and Scope

Bindings are how you give names to values in AIVI. If you are coming from a language with mutable variables, the most important shift is this: `=` creates a name for a value; it does not update storage in place.

In practice, that means you describe data flow by introducing new names, shadowing old ones in a narrower scope, or destructuring values into the pieces you need.

## 1.1 Definitions

AIVI uses the same `=` syntax for many kinds of definitions:

- values
- functions
- types
- classes
- instances
- modules

Think of `=` as “define this name” rather than “assign into this variable”.

<<< ../snippets/from_md/syntax/bindings/definitions.aivi{aivi}

## 1.2 Shadowing

Bindings are lexical, so an inner scope can reuse a name from an outer scope.

<<< ../snippets/from_md/syntax/bindings/shadowing.aivi{aivi}

This creates a new binding that temporarily hides the earlier one. It is similar to Rust shadowing or `let`-binding in ML-family languages, and it is different from mutation.

Use shadowing when the new name really is “the updated version” of the old value and the narrower scope keeps that intent clear.

## 1.2.1 Recursion (module level)

Inside a module body, top-level value bindings are recursive. A top-level definition may refer to itself and to definitions that appear later in the same file.

That lets you write ordinary recursive helpers without rearranging a file to satisfy declaration order.

<<< ../snippets/from_md/syntax/bindings/recursion_module_level.aivi{aivi}

## 1.3 Pattern Bindings

A binding can destructure a value directly on the left-hand side. Use this when the shape is guaranteed and you want the interesting pieces available by name immediately.

<<< ../snippets/from_md/syntax/bindings/pattern_bindings.aivi{aivi}

### Record destructuring (deconstructing records)

To pull fields out of a record, use a record pattern on the left-hand side.

<<< ../snippets/from_md/syntax/bindings/record_destructuring_deconstructing_records.aivi{aivi}

You can also destructure nested records using dot-paths (Section 1.5).

Rules to remember:

- `=` may be used only where the compiler can prove the pattern is **total**, meaning the pattern matches every value of that shape.
- A potentially failing pattern must use `match`, or appear in a context where failure can be handled explicitly.

> [!NOTE]
> Using `=` with a non-total pattern such as `[h, ...t] = []` is a compile-time error. When a match may fail, write a `match` so the failure path is visible in the source.

## 1.4 Whole-value binding with `as`

Sometimes you want both the whole value and a few pieces inside it. `as` lets one pattern do both jobs.

<<< ../snippets/from_md/syntax/bindings/whole_value_binding_with_as_01.aivi{aivi}

Semantics:

- `user` is bound to the whole value.
- `{ name: n }` destructures the same value.
- No duplication or copying is implied by the syntax.

Allowed in:

- top-level and local bindings
- `match` arms
- function clauses

Example:

<<< ../snippets/from_md/syntax/bindings/whole_value_binding_with_as_02.aivi{aivi}

## 1.5 Usage Examples

### Config Binding

A simple binding often works best for values you want to name once and pass around.

<<< ../snippets/from_md/syntax/bindings/config_binding.aivi{aivi}

### Tuple Destructuring

Tuple patterns are useful when a function naturally returns a fixed-size group of values.

<<< ../snippets/from_md/syntax/bindings/tuple_destructuring.aivi{aivi}

### Deep path destructuring

Record destructuring supports **dot-paths**, so you can reach into nested records without spelling out every intermediate layer.

<<< ../snippets/from_md/syntax/bindings/deep_path_destructuring_01.aivi{aivi}

Semantics:

- `data.user.profile` identifies the nested record being unpacked.
- `as { name }` binds fields from that nested record.
- Intermediate records are not bound unless you ask for them explicitly.

This is equivalent to the more expanded version:

<<< ../snippets/from_md/syntax/bindings/deep_path_destructuring_02.aivi{aivi}

The shorter form is often easier to read when working with JSON-like data or configuration trees.

### List Head/Tail

Use list patterns when you want to separate the first element from the rest.

<<< ../snippets/from_md/syntax/bindings/list_head_tail.aivi{aivi}

### Function Definitions

Function definitions are bindings too, so the same scoping and shadowing rules apply.

<<< ../snippets/from_md/syntax/bindings/function_definitions.aivi{aivi}

---

## Comments

Comments are ignored by the language runtime and type system. Use them to explain intent, assumptions, or why a piece of code is shaped the way it is.

### Line comments

Line comments start with `//` and continue to the end of the line.

<<< ../snippets/from_md/syntax/comments/line_comments.aivi{aivi}

### Block comments

Block comments start with `/*` and end with `*/`. They may span multiple lines.

<<< ../snippets/from_md/syntax/comments/block_comments.aivi{aivi}

Comments may appear anywhere whitespace is allowed, and the formatter preserves them in place.

**Not supported:** doc comments (`///` / `/** */`), nested block comments, shebangs (`#!`).
