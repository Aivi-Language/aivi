# AIVI Language Summary (LLM Context)

## 1) Core principles
- Statically typed, purely functional, expression-oriented.
- Immutable bindings; no mutation.
- No null or exceptions: use Option / Result.
- No loops: use recursion, folds, generators.
- Pattern matches are total by default; refutable matches require `?`.
- Records are open and structural (row polymorphism).
- Effects are explicit: `Effect E A`.
- Domains define operator and literal semantics.

## 2) Lexical basics
- Line comments: `//` or `--`; block comments: `/* ... */`.
- Identifiers: lowerIdent for values/functions/fields, UpperIdent for types/constructors/modules/domains/classes.
- Text literals use `"..."` with `{ expr }` interpolation.
- Literals: Int, Float, Text, Char, ISO instant, suffixed numbers (e.g. `10px`).

## 3) Bindings and scope
```aivi
x = 42
add = a b => a + b
{ name, age } = user
user@{ name } = user
```
- All bindings use `=` and are lexical; shadowing is allowed.
- Top-level bindings in a module are recursive.
- Use `@` to bind the whole value alongside destructuring.
- Deep path destructuring in record patterns uses dot paths.

## 4) Functions and application
- Functions are curried; application is whitespace.
- Lambdas: `x => ...` or `_` for unary placeholder.
- Pipes: `x |> f` == `f x`; `x |> f a b` == `f a b x`.
- Deconstructor pipe heads: mark binders with `!` then start body with `|>`.

```aivi
f = { name! } |> toUpper
```

## 5) Pattern matching and `?`
- `?` matches the expression immediately to its left.
- Multi-clause unary functions use leading `|` arms.
- Guards with `when`.
- Whole-value binders `@` work in patterns.

```aivi
value ?
  | Ok x  => x
  | Err _ => 0
```

## 6) Predicates
- Predicate expressions are Bool expressions using literals, field access, patterns, `_`.
- Implicit binding: `_` is the current element; bare field `active` means `_.active`.
- Predicate lifting: in predicate positions, `pred` is desugared to `_ => pred`.
- No auto-lifting over Option/Result in predicates.

## 7) Types
- ADTs: `Type = Con A | Con2 B C`.
- Records are open structural types: `{ name: Text }` means at least that field.
- Type operators: `->`, `with` (record/type composition), `|>` in types.
- Row transforms: `Pick`, `Omit`, `Optional`, `Required`, `Rename`, `Defaulted`.
- Classes and instances support ad-hoc polymorphism and HKTs (`*`).
- Expected-type coercions only, via in-scope instances (e.g., `ToText`).

## 8) Records, lists, tuples
```aivi
p = { x: 1, y: 2 }
xs = [1, 2, 3]
t = (1, "a")
```
- Record spread: `{ ...base, x: 3 }` (later fields win).
- List spread: `[head, ...tail]`.
- Range item: `a .. b` (inclusive), usable in list literals.

## 9) Patching (structural updates)
- `<|` applies a patch: `target <| { path: value }`.
- Patch literals are declarative, type-checked updates.
- Paths support dot fields, traversals, predicates, and map key selectors.
- Instructions: replace, transform, `:=` for function-as-data, `-` to remove.

```aivi
user2 = user <| { profile.name: "Sam" }
```

## 10) Domains, units, and operators
- Domains define semantics for operators and literal templates.
- Suffix literals (e.g. `10ms`, `(x)kg`) resolve to template functions like `1ms`.
- Domain-resolved operators (when non-Int carrier): `+ - * × / % < <= > >=`.
- `==`, `!=`, `&&`, `||`, `|>`, `<|`, `..` are built-in.
- Domains are imported explicitly: `use aivi.calendar (domain Calendar)`.

## 11) Generators
- Pure, pull-based sequences: `Generator A`.
- `generate { ... }` with `yield`, `x <- xs`, `x = e`, and guards `x -> pred`.
- `loop state = init => { ... recurse next }` for local tail recursion.

## 12) Effects
- `Effect E A` models typed effects with explicit error domain.
- `effect { ... }` sequences effects using `<-` for binding.
- `x = e` inside `effect` is pure-only; use `<-` to run effects.
- Expression statements must be `Effect E Unit` unless bound.
- `or` is fallback-only sugar for effects or `Result`.

## 13) Resources
- `resource { ... }` with `yield` for acquire/use/release.
- Acquired in `effect` blocks with `<-`; released LIFO on scope exit.

## 14) Modules and imports
- One module per file: `module path.name` must be first non-empty item.
- `use` imports values/types; `use ... (domain D)` imports domain semantics.
- `export` controls public symbols; `export domain D` exports domain members.
- Implicit prelude: `use aivi.prelude`, disabled via `@no_prelude`.
- Circular dependencies are forbidden.

## 15) External sources
- `Source K A` represents typed external data.
- `load : Source K A -> Effect (SourceError K) A`.
- Provided sources: file, http/https, env, db, email, llm, image, s3.
- `@static` can embed compile-time sources.

## 16) Sigils
- Custom literals: `~tag(...)`, `~tag[...]`, `~tag{...}`, `~tag/.../`.
- Structured sigils include `~map{...}` and `~set[...]`.
- Some compiler-provided sigils: `~u(...)`, `~url(...)`, `~path[...]`.

## 17) Decorators (v0.1)
- `@static`, `@inline`, `@deprecated`, `@debug`, `@test`, `@no_prelude`.
- Unknown decorators are compile errors; user-defined decorators are not supported.

## 18) Operator reference (selected)
- Binding: `=`
- Effect/generator/resource bind: `<-`
- Guard: `->` (generators only)
- Pipe: `|>`
- Patch: `<|`
- Match/refutable: `?`
- Arms: `|`
- Lambda/arm: `=>`
- Spread/rest: `...`
- Range: `..`
- Accessor: `.` and `.field` accessor function

## 19) Block forms
- `{ ... }` is either a record literal or a block; disambiguated by first entry.
- `effect { ... }`, `generate { ... }`, `resource { ... }` are dedicated blocks.
