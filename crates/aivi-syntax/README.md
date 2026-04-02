# aivi-syntax

## Purpose

Milestone 1 surface frontend: lexer, CST, parser, and formatter for the AIVI language.
This crate translates raw source text into a concrete syntax tree (`Module`) and provides
a formatter that round-trips that tree back to canonical source. It is the only layer that
touches raw bytes; every later compiler layer operates on the typed CST it produces.

## Entry points

```rust
// Lex a source file into a token stream
lex_module(source: &str) -> LexedModule

// Parse a lexed module into a CST
parse_module(lexed: &LexedModule) -> ParsedModule

// Format a parsed module back to canonical text
Formatter::new(source: &str).format(module: &Module) -> String
```

`ParsedModule` owns the `Module` tree and carries accumulated `Diagnostic` values.
`LexedModule` exposes `tokens()` and `errors()` for inspection before parsing.

## Invariants

- Lexing is total — every byte is consumed and emitted as some `Token`; no source is silently dropped.
- Parsing is error-recovering — a `ParsedModule` is always returned; syntax errors appear in `diagnostics()`.
- The CST preserves every token including whitespace and comments via `TokenRange` on each node.
- Parse recursion depth is bounded; exceeding the limit emits `syntax::parse-depth-exceeded` instead of overflowing the stack.
- `format_module` is idempotent: formatting an already-formatted file is a no-op.

## Diagnostic codes

| Code | Description |
|---|---|
| `syntax::dangling-decorator-block` | Decorator block appears with no following declaration |
| `syntax::direct-function-parameter-annotation` | Inline type annotation on a function parameter (not supported) |
| `syntax::duplicate-standalone-type-annotation` | More than one standalone type annotation for the same name |
| `syntax::empty-result-block` | Result block with no bindings or tail expression |
| `syntax::invalid-discard-expr` | Discard (`_`) used in an expression position that does not allow it |
| `syntax::invalid-escape-sequence` | Unrecognised escape sequence inside a string literal |
| `syntax::invalid-markup-child-content` | Invalid token in a markup child position |
| `syntax::invalid-text-interpolation` | Malformed interpolation inside a text literal |
| `syntax::mismatched-markup-close` | Closing markup tag does not match the opening tag |
| `syntax::missing-class-member-type` | Class member is missing its type annotation |
| `syntax::missing-class-open-brace` | Expected `{` after class head |
| `syntax::missing-class-require-type` | `require` clause missing its type |
| `syntax::missing-class-with-type` | `with` clause missing its type |
| `syntax::missing-declaration-body` | Declaration has a name but no body |
| `syntax::missing-decorator-name` | `@` appears without a following decorator name |
| `syntax::missing-domain-carrier` | `domain` declaration is missing its carrier type |
| `syntax::missing-domain-member-body` | Domain member has no body |
| `syntax::missing-domain-member-name` | Domain member is missing its name |
| `syntax::missing-domain-member-type` | Domain member is missing its type |
| `syntax::missing-domain-open-brace` | Expected `{` after domain head |
| `syntax::missing-domain-over` | `domain` declaration is missing `over` |
| `syntax::missing-export-name` | `export` keyword is not followed by a name |
| `syntax::missing-instance-class` | `instance` declaration missing class name |
| `syntax::missing-instance-member-body` | Instance member has no body |
| `syntax::missing-instance-open-brace` | Expected `{` after instance head |
| `syntax::missing-instance-target` | `instance` declaration missing target type |
| `syntax::missing-item-name` | Declaration keyword appears without a following name |
| `syntax::missing-pipe-memo-name` | Memo stage is missing its binding name |
| `syntax::missing-provider-contract-member-value` | Provider contract member has no value |
| `syntax::missing-provider-contract-name` | Provider contract is missing its name |
| `syntax::missing-provider-contract-schema-name` | Provider contract schema field has no name |
| `syntax::missing-provider-contract-schema-type` | Provider contract schema field has no type |
| `syntax::missing-reactive-update-arm-arrow` | Reactive update arm missing `=>` |
| `syntax::missing-reactive-update-arm-body` | Reactive update arm has no body |
| `syntax::missing-reactive-update-arm-left-arrow` | Reactive update arm missing `<-` |
| `syntax::missing-reactive-update-arm-pattern` | Reactive update arm has no pattern |
| `syntax::missing-reactive-update-arm-target` | Reactive update arm has no target |
| `syntax::missing-reactive-update-arrow` | Reactive update missing `=>` |
| `syntax::missing-reactive-update-body` | Reactive update has no body |
| `syntax::missing-reactive-update-guard` | Reactive update guard clause is incomplete |
| `syntax::missing-reactive-update-left-arrow` | Reactive update missing `<-` |
| `syntax::missing-reactive-update-source` | Reactive update has no source expression |
| `syntax::missing-reactive-update-source-pattern` | Reactive update source binding has no pattern |
| `syntax::missing-reactive-update-subject` | Reactive update has no subject |
| `syntax::missing-reactive-update-target` | Reactive update has no target |
| `syntax::missing-result-binding-expr` | Result block binding has no expression |
| `syntax::missing-result-block-tail` | Result block has no tail expression |
| `syntax::missing-standalone-type-annotation` | Standalone annotation marker without a type |
| `syntax::missing-use-alias` | `as` in a use import is not followed by a name |
| `syntax::missing-use-path` | `use` keyword is not followed by a path |
| `syntax::nullary-function-declaration` | Function declared with zero parameters |
| `syntax::orphan-standalone-type-annotation` | Standalone type annotation with no following declaration |
| `syntax::parse-depth-exceeded` | Nesting depth exceeded the safety limit |
| `syntax::trailing-declaration-body-token` | Unexpected token after a complete declaration body |
| `syntax::unexpected-character` | Character not part of any valid token |
| `syntax::unexpected-token` | Token not valid in this position |
| `syntax::unexpected-top-level-token` | Token not valid at the top level of a module |
| `syntax::unsupported-class-head-constraints` | Class head contains constraint syntax not yet supported |
| `syntax::unterminated-markup-node` | Markup node opened but never closed |
| `syntax::unterminated-regex` | Regex literal opened but never closed |
| `syntax::unterminated-string` | String literal opened but never closed |
| `syntax::unterminated-text-interpolation` | `${` opened inside text but never closed |

## RFC reference

See [`../../AIVI_RFC.md`](../../AIVI_RFC.md) §4.1 (surface syntax and parser).
