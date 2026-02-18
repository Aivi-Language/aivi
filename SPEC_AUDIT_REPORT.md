# AIVI Specification Audit Report

*Generated: 2026-02-18*

This report documents mistakes, inconsistencies, and gaps found across the `specs/` directory. Issues are organized by severity and then by section.

---

## Executive Summary

| Category | Count |
|:---------|------:|
| **Critical** — contradictions, broken semantics, parse errors in examples | 28 |
| **High** — missing from index, wrong types, naming violations at scale | 35 |
| **Medium** — incomplete sections, formatting, unclear semantics | 52 |
| **Low** — minor prose issues, style nits | 30 |
| **Total** | **~145** |

**Top systemic problems** (ordered by impact):

1. **Parenthesized function calls `f(a, b)` pollute ~25 code examples** across stdlib — AIVI uses whitespace application `f a b`
2. **`Effect` and `Resource` used with wrong arity** — should be `Effect E A` (2 params), many files use `Effect A` (1 param)
3. **`String` used instead of `Text`** in console, crypto, and URL specs
4. **`case` referenced as syntax in 3+ files** but is not an AIVI keyword — should be `?`
5. **Operator precedence contradictions** between grammar and 11_operators.md
6. **Functor/class method argument order** incompatible with `|>` piping convention
7. **`=> =` garbled syntax** in 6+ predicate/generator examples (should be `==`)
8. **Haskell lambda syntax `\x -> ...`** in signal and linear algebra snippets (should be `x => ...`)
9. **`if ... { }` instead of `if ... then ... else ...`** in multiple chronos/geometry examples
10. **Kernel spec is severely underspecified** — missing wildcards, no type application, no formal rules

---

## 1. Index & Structural Issues

### 1.1 Missing entries in `index.md`

| Missing file | Should be listed under |
|:---|:---|
| `02_syntax/11_operators.md` | Syntax section |
| `06_runtime/02_memory_management.md` | Execution & Concurrency section |

### 1.2 Directory numbering collisions

| Issue | Location |
|:---|:---|
| `03_network/` and `03_system/` share prefix `03_` | `specs/05_stdlib/` |
| `25_system.md` and `25_url.md` share prefix `25_` | `specs/05_stdlib/03_system/` |

### 1.3 File numbering gaps

| Directory | Gap |
|:---|:---|
| `05_stdlib/00_core/` | 04–15, 17–23, 25–26 missing |
| `05_stdlib/01_math/` | 02–04, 06–08, 11–12, 16 missing |

### 1.4 Out-of-order listing in index.md

Sigils (13) listed before External Sources (12) in the Syntax TOC section.

### 1.5 "AIVI" vs "Aivi" inconsistency

All `07_tools/` files use "Aivi" for the language name. All other specs use "AIVI". Should be normalized to "AIVI" everywhere (except when referring to the CLI binary `aivi`).

---

## 2. Syntax Spec Issues (`02_syntax/`)

### 2.1 Critical: Grammar contradictions

| Issue | Files | Description |
|:---|:---|:---|
| Operator precedence | `00_grammar.md` vs `11_operators.md` | Grammar gives `<|` much higher precedence than `\|>`; operators spec lists them at the same level |
| `..` range scope | `00_grammar.md` vs `11_operators.md` | Grammar restricts `..` to list literals; operators spec says it's a general infix operator |
| `recurse` keyword | `00_grammar.md` | Listed as keyword but has no grammar production |
| Missing section | `00_grammar.md` | Section 0.8 is skipped entirely |

### 2.2 Critical: Garbled code examples

| Pattern | Affected files | Fix |
|:---|:---|:---|
| `=> =` instead of `==` | `04_predicates` blocks 05–09, `07_generators` block 08 | Replace `=> =` with `==` |
| `= :` garbled signature | `04_predicates` block 08 | Split into separate signature and binding |
| `=>` in type positions | `04_predicates` blocks 02, 04 | Use `->` for function types |
| `map.name` missing space | `02_functions` block 08 | Should be `map .name` (space before accessor) |

### 2.3 High: Functor argument order vs pipe convention

`03_types.md` defines `Functor.map : F A -> (A -> B) -> F B` (data-first), but pipe convention `xs |> map inc` requires data-last `map : (A -> B) -> F A -> F B`. All class definitions (Functor, Apply, Chain) have this same conflict.

### 2.4 High: `case` used as keyword but isn't one

`case` appears as if it were AIVI syntax in:
- `09_effects.md`: "Branching is done with ordinary expressions (`if`, `case`, `?`)"
- `11_operators.md`: "patterns, `case`/multi-clause forms"
- `00_grammar.md`: "performs a case on its input"

AIVI's keyword list does not include `case`. The construct is `?`.

### 2.5 Medium: Section numbering gaps in syntax files

- `01_bindings.md`: Section 1.2.1 (Recursion) is nested under Shadowing but unrelated
- `05_patching.md`: Section 5.6 is skipped
- `13_sigils.md`: No section numbers at all
- `15_resources.md`: Only 2 sections, no error/cancellation/composition semantics

### 2.6 Medium: Effects spec issues

- `attempt : Effect E A -> Effect E (Result E A)` — logically, the outer `Effect` should not be able to fail with `E` anymore after catching it
- `case` listed as branching expression (should be `?`)
- `or` disambiguation between effect-fallback and Result-fallback is fragile and under-documented

### 2.7 Medium: Resources spec is dangerously underspecified

`15_resources.md` does not define:
- The type of `Resource` (type parameters?)
- Error semantics if `yield` is never reached
- Cancellation interaction (effects spec mentions cleanup but resources don't mention cancellation)
- Composability/nesting of resource blocks
- Whether cleanup code can itself perform effects

### 2.8 Medium: `@` in record patterns has unspecified semantics

Grammar defines `RecordPatField := RecordPatKey [ (":" Pattern) | ("@" Pattern) ]` but no spec explains the semantic difference between `{ field: pat }` and `{ field@pat }`.

### 2.9 Low: External Sources issues

- Sections 12.5–12.9 (Database, Email, LLM, Image, S3) presented as normal specs but are unimplemented in v0.1 with no caveat
- Orphaned browser sources note after section 12.9

### 2.10 Low: Sigils spec issues

- Inconsistent snippet path (`../snippets/02_syntax/` instead of `../snippets/from_md/02_syntax/`)
- No documentation of valid delimiter types
- HTML sigil `~<html>...</html>` doesn't follow the `~tag[delimiter]` pattern

---

## 3. Kernel Spec Issues (`03_kernel/`)

### 3.1 Critical: Missing fundamental patterns

`04_patterns.md` pattern grammar omits:
- **`_` (wildcard)** — fundamental for catch-all arms, cannot be derived
- **Literal patterns** (matching `42`, `"hello"`, `True`)
- **Tuple patterns** `(p₁, ..., pₙ)`
- **List patterns** `[p₁, ..., pₖ, ...rest]`

### 3.2 Critical: Contradictions

| Issue | File | Description |
|:---|:---|:---|
| Predicate desugaring | `05_predicates.md` | General rule says `λ_. e` (wildcard) but field example correctly uses `λx. x.price > 80` — should be `λx. e` |
| Multi-arg lambda | `07_generators.md` | `yield x ≡ λk acc. k acc x` — kernel specifies single-argument lambdas only. Should be curried: `λk. λacc. k acc x` |

### 3.3 High: Missing specifications

| Missing item | Where expected |
|:---|:---|
| Type application / instantiation (elimination for `∀`) | `02_types.md` |
| `Effect E A` in type grammar | `02_types.md` |
| HKT encoding via `∀` (claimed but not shown) | `02_types.md` |
| `attempt` primitive | `08_effects.md` |
| `Resource` / `bracket` | `08_effects.md` or dedicated file |
| Record merge/spread at kernel level | `03_records.md` |
| Generator composition primitives | `07_generators.md` |
| Kind system | `02_types.md` |

### 3.4 High: Minimality table (`12_minimality.md`) gaps

Missing entries: Classes, Resources, Let binding, ADT constructors. "HKTs → ∀" is an oversimplification (needs kind polymorphism). Two H1 headers in one file.

### 3.5 Medium: Formatting and consistency

- No cross-references/internal links in any of the 12 kernel files
- Inconsistent section numbering (05, 10, 11, 12 have none)
- Inconsistent formality level (some quasi-formal, others pure prose)
- Inconsistent desugaring arrow symbols (`⇒` vs `↦` vs plain text)
- `update(e, l, f)` uses parenthesized syntax; rest uses whitespace application
- Surface syntax leaks into kernel files (01, 10, 11)

---

## 4. Desugaring Spec Issues (`04_desugaring/`)

### 4.1 Critical: Broken cross-references

`10_patching.md` line 82: References "Section 8" but should be "Section 5" (`05_predicates.md`).

### 4.2 Critical: Circular desugaring

`10_patching.md`: `patch { a: v }` desugars to `λx. x <| { a: v }` — still contains the `<|` surface operator. Should fully expand to kernel `update`.

### 4.3 High: Contradictions with kernel/syntax

| Issue | File | Description |
|:---|:---|:---|
| `x! y!` binary function | `02_functions.md` | Syntax spec says `!` is for unary destructuring functions; desugaring creates a binary function |
| `removeField` vs `update` to `None` | `10_patching.md` | Desugaring uses `removeField`; kernel says use `update` to `None` + row shrink |
| `delete` as kernel primitive | `10_patching.md` L113 | Summary lists `delete`; kernel minimality table has no `delete` |
| `attempt` missing from primitives | `07_effects.md` | `or` fallback desugaring requires `attempt` but it's not listed as a kernel primitive |
| `Some.val` vs `value` | `10_patching.md` | Prose says payload accessor is `value`; example uses `val` |

### 4.4 Medium: Structural/organizational

- Resources desugaring is wedged into `06_generators.md` instead of its own file or `07_effects.md`
- `if/then/else` desugaring (mentioned in syntax spec) is never formally written
- Multiple H1 headers in `04_patterns.md`
- Inconsistent header hierarchy and detail levels across files
- `#1` fresh binder convention used but never defined

---

## 5. Standard Library Issues (`05_stdlib/`)

### 5.1 Critical: Systemic syntax violations in code examples

| Violation | Approximate count | Affected areas |
|:---|:---|:---|
| Parenthesized function calls `f(a, b)` | ~25 instances | regex, collections, matrix, probability, signal, geometry, graph, linear algebra, calendar |
| `if ... { }` instead of `if ... then ... else ...` | ~5 instances | instant, duration, geometry, collections |
| Haskell lambda `\x -> ...` instead of `x => ...` | ~3 instances | signal, linear algebra, URL |
| `snake_case` instead of `lowerCamelCase` | ~8 instances | regex, matrix, calendar, timezone |

### 5.2 Critical: `Effect` arity (1-param instead of 2-param)

`Effect E A` requires 2 type parameters. The following files use `Effect X` (1 param):

| File | Examples |
|:---|:---|
| `20_file.md` | `Effect Unit`, `Effect Bool`, `Effect (Result Text Text)` |
| `21_console.md` | `Effect Unit`, `Effect (Result String Error)` |
| `22_crypto.md` | `Effect String`, `Effect Bytes` |

Similarly, `Resource` should take an error type but is used as `Resource Handle`, `Resource Server`, `Resource Listener` in file, HTTP server, and sockets specs.

### 5.3 Critical: `String` vs `Text`

AIVI's canonical text type is `Text`. These files use `String`:
- `21_console.md` — `log`, `println`, `print`, `error`, `readLine` signatures
- `22_crypto.md` — `sha256`, `randomUuid` signatures
- `25_url.md` — `parse`, `toString` signatures and `Url` record fields

### 5.4 High: Dead references (functions/types used in examples but not defined)

| File | Dead references |
|:---|:---|
| `09_matrix.md` | `Mat4.identity()`, `Mat4.translate(...)` |
| `13_probability.md` | `Normal(...)`, `sample()` |
| `15_geometry.md` | `Ray(...)`, `Sphere(...)`, `intersect(...)` |
| `10_number.md` | `Quat.fromEuler(...)`, `Quat.identity()`, `Quat.slerp(...)` |
| `18_linear_algebra.md` | `solve(...)`, `eigen` |
| `16_units.md` | `Length`, `Time`, `Velocity` types |

### 5.5 High: Module path inconsistencies

| Module | Path inconsistency |
|:---|:---|
| Chronos | `aivi.chronos.instant` vs `aivi.calendar` (no chronos prefix) vs `aivi.duration` (no chronos prefix) vs `aivi.chronos.timezone` |
| Linear algebra | `aivi.linalg` (quick-info) vs `aivi.linear_algebra` (snippet block_04) |
| Network | `aivi.net.http_server` (snake_case) should be `aivi.net.httpServer` |
| Log | quick-info says `"name":"aivi"` — should be `"aivi.log"` |
| Timezone | quick-info says `"name":"aivi.calendar"` — should be `"aivi.chronos.timezone"` |

### 5.6 High: `Result` parameter order

`30_concurrency.md`: `recv : Receiver A -> Effect E (Result A ChannelError)` — parameters are backwards. AIVI's `Result` is `Result E A` (error first), so should be `Result ChannelError A`.

### 5.7 Medium: Logical/semantic issues

| File | Issue |
|:---|:---|
| `02_text.md` | `toText : A -> Text` unconstrained — should require `ToText A =>` class constraint |
| `05_vector.md` | `v1 = (1.0, 2.0) v2` — circular self-reference and tuple vs record mismatch |
| `09_matrix.md` | `m * translation` uses `*` but domain defines `*` as scalar multiplication, not matrix-matrix |
| `13_probability.md` | `domain Probability over Probability` would shadow `Float` arithmetic for all `Float` values |
| `10_number.md` | `toInt : BigInt -> Int` may overflow — should return `Option Int` or `Result` per AIVI no-exceptions principle |
| `04_color.md` | `10lightness` — domain defines `1l` as the suffix, so should be `10l` |
| `29_i18n.md` | `~dt(...)` sigil undefined — should be `~zdt(...)` or bare ISO literal |

### 5.8 Medium: Incomplete specs

| File | What's missing |
|:---|:---|
| `27_testing.md` | No function/type tables, no assertion docs, no test runner behavior |
| `30_generator.md` | Only 6 functions — missing `take`, `drop`, `zip`, `flatMap`, `head`, etc. |
| `04_sockets.md` | Overview says "TCP/UDP" but no UDP functions documented |
| `05_streams.md` | `Stream A` opaque type never defined; no `map`/`filter`/`fold` |
| `15_geometry.md` | No 3D types despite examples using Ray, Sphere |

### 5.9 Low: Formatting issues

- `29_i18n.md` block_09: duplicate `| Err e => fail e` (copy-paste bug)
- `01_math.md`: broken table formatting in `divmod`/`modPow` rows (extra `|` chars)
- `01_math.md`: angles section uses 4 separate single-row tables instead of one grouped table
- `04_ui/02_vdom.md`, `03_html.md`: use 4-backtick fences inconsistently
- `03_logic.md`: snippet references out of order (block_18, block_21, block_19, block_20)
- `04_color.md`: `negateDelta` referenced but never defined (also Calendar, Duration domains)

---

## 6. Runtime & Tools Issues (`06_runtime/`, `07_tools/`)

### 6.1 Critical: Contradictions between runtime and stdlib

| Issue | runtime spec | stdlib spec |
|:---|:---|:---|
| Channel types | `Send A` / `Recv A` | `Sender A` / `Receiver A` |
| `concurrent.scope` signature | `Effect E A -> Effect E A` | `(Scope -> Effect E A) -> Effect E A` |

### 6.2 High: Pure binding for effectful operation

`01_concurrency.md` block_03: `(tx, rx) = channel.make ()` inside `effect { }` — should use `<-` not `=` since channel creation is effectful.

### 6.3 High: Undocumented functions used in runtime spec

`concurrent.spawnDetached` and `concurrent.race` are used/listed in runtime spec but absent from stdlib `30_concurrency.md`.

### 6.4 Medium: Section numbering

`01_concurrency.md` uses `20.1`, `20.2`, `20.3` — should be `1.1`, `1.2`, `1.3`.

### 6.5 Medium: Memory management spec issues

- Orphaned from index.md
- Leaks Rust implementation details (`Arc<T>`, `Env`, `Thunk`) into language spec
- Contains speculative features ("if added") in normative spec
- No code examples

### 6.6 Medium: Content duplication

`06_runtime/03_package_manager.md` substantially overlaps with `07_tools/01_cli.md` and `07_tools/04_packaging.md` with no cross-references.

### 6.7 Medium: `entry` path inconsistency

`aivi.toml` uses `entry = "main.aivi"` but `Cargo.toml`'s `[package.metadata.aivi]` uses `entry = "src/main.aivi"`. The difference is never explained.

### 6.8 ~~Medium: LiveView event format contradictions~~ RESOLVED

`05_liveview.md` has been retired and now redirects to `06_server_html.md`. The unified event format is `{"t":"event","hid":123,"kind":"click","p":{...}}`.

### 6.9 Low: Terminology

- "traits" used in LSP spec (`02_lsp_server.md`) — AIVI uses "classes"
- "Aivi" vs "AIVI" inconsistency across all `07_tools/` files

---

## 7. Recommended Priority Actions

### Immediate (blocks correctness)

1. Fix all `=> =` → `==` in predicate/generator examples
2. Fix `Effect` arity everywhere (add error type parameter)
3. Fix `String` → `Text` in console, crypto, URL specs
4. Fix `Result` parameter order in concurrency spec
5. Resolve operator precedence contradiction between grammar and operators spec
6. Add `_` wildcard to kernel pattern grammar
7. Fix channel type naming conflict (`Send`/`Recv` vs `Sender`/`Receiver`)
8. Fix `concurrent.scope` signature conflict

### Short-term (spec quality)

9. Add `11_operators.md` and `02_memory_management.md` to index.md
10. Fix parenthesized function calls in all stdlib examples
11. Fix Haskell lambda syntax and `if {} ` syntax in examples
12. Document `@` vs `:` semantics in record patterns
13. Expand resources spec (type signature, error semantics, cancellation)
14. Add cross-references throughout kernel spec files
15. Resolve Functor argument order vs pipe convention

### Medium-term (completeness)

16. Fill in incomplete stdlib specs (testing, generator, streams, sockets UDP)
17. Add missing kernel primitives (type application, attempt, Resource/bracket)
18. Add missing minimality table entries (Classes, Resources, Let, ADTs)
19. Create dedicated resources desugaring file
20. Normalize "AIVI" vs "Aivi" across all tool specs
21. Fix module path inconsistencies (chronos, linear algebra, etc.)
22. Fill dead references in math/science stdlib examples
23. Renumber colliding directories/files (03_network vs 03_system, 25_system vs 25_url)
