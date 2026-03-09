# Suggested changes from the specs audit

This file collects repo-wide follow-ups that came out of the `specs/**/*.md` audit.
It is intentionally focused on cross-file work, language/runtime decisions, and shared snippet/tooling cleanup rather than the file-local doc fixes that were already applied.

## Language and runtime decisions to resolve

- **Time carriers and literals**
  - Standardize the public story around `DateTime`, `Timestamp`, `~dt(...)`, and `~zdt(...)`.
  - Decide whether `~t(...)` / standalone `Time` remains a real v0.1 surface or should be removed from shared references.
  - Align timezone/calendar snippets and shared docs with the verified local-wall-clock `~zdt(...[Zone])` behavior.

- **Domain imports, suffix literals, and operator resolution**
  - Resolve the cross-doc drift around whether plain `use module` is enough for domain operators/suffixes or whether explicit `(domain D)` imports are required.
  - Reconcile calendar, units, color, layout, and general syntax docs with actual compiler behavior.

- **`aivi.tree` public model**
  - Keep the shared references aligned with the implemented rose-tree shape: `Tree A = Node A (List (Tree A))`.
  - Decide whether `Tree` should publicly support `map` / `reduce` / `filter`-style class operations; if yes, add tests and document them consistently.

- **REST / HTTP behavior**
  - Decide whether source-level `load (rest.get ...)` should decode into the expected type again or keep returning raw response records.
  - Resolve the `aivi.rest` JSON-body mismatch between spec text and runtime parsing behavior.

- **Decorator semantics**
  - make `@debug` work on ordinary `name = x => ...` definitions
  - Keep `type.jsonSchema` documentation, hover text, and examples aligned around the embedded-schema-value + `toText` workflow.
  - compile-time `env.get` should fail on missing variables

- **Effect / machine / query semantics**
  - Resolve the spec/implementation drift around `loop` / `recurse` in generic `do M` blocks.
  - Keep all machine docs aligned on init-state semantics: machines start in the init target state, and later init/boot calls should fail.
  - `join`, and `groupBy` belong in the documented `aivi.database` surface and implement if missing

- **Text / regex / operator behavior**
  - regex sigils should be parse-time validated.
  - Reconcile text runtime/spec behavior for `fromBytes`, indexing/slicing, `reverse`, `capitalize`, and `titleCase`.

- **Other API consistency items**
  - Clarify `EmailAuth` wording across shared docs so IMAP XOAUTH2 and SMTP/provider-specific token auth are described accurately.
  - Reconcile `aivi.system.env.get` docs with the source-level environment-source behavior.

## Documentation structure and placement

- **Consolidate onboarding**
  - Reduce overlap among `specs/README.md`, `specs/index.md`, `specs/introduction.md`, and `specs/language-overview.md`.
  - Keep one clear “start here” path and let the other pages specialize.

- **Centralize GTK architecture guidance**
  - Unify helper-taxonomy and routing-limit guidance across:
    - `specs/stdlib/ui/app_architecture.md`
    - `specs/stdlib/ui/gtk4.md`
    - `specs/stdlib/ui/native_gtk_apps.md`

- **Mark current behavior vs future phases more consistently**
  - Several pages mix shipped behavior, planned phases, and design intent in one voice.
  - Incremental compilation, external-source composition, and typed-codegen docs would benefit from a consistent “current / planned / implementation note” convention.

- **Normalize cross-links and quick-info examples**
  - Prefer working relative decorator links such as `./index`.
  - Escape quick-info/doc-marker examples so teaching pages do not accidentally generate real index entries.

## Shared snippet cleanup backlog

The audit surfaced many snippet directories that are stale, placeholder-only, or no longer match the implemented surface.
The highest-value cleanup is to either refresh them to executable examples or remove them when the page now uses inline examples instead.

- **Core**: `option`, `result`, `validation`, `prelude`, `collections`, `json`, `i18n`, `regex`
- **Chronos**: `instant`, `timezone`, `calendar`
- **Math**: `graph`, `geometry`, `linear_algebra`, `number`, `tree`, `vector`
- **System / IO**: `database`, `crypto`, `path`, `system`, `file`
- **UI**: `color`, `forms`
- **Syntax**: `bindings`, `decorators/test`, `opaque_types` / URL-related snippets

Suggested rule of thumb:

- if a snippet is meant to be canonical, make it formatter-clean and executable through existing snippet checks;
- if it is only explanatory and cannot stand alone, prefer a local inline block in the spec page.

## Validation and coverage gaps worth addressing

- Add explicit automated coverage for:
  - negative-weight `shortestPath` behavior in `aivi.graph`;
  - resource cleanup during cancellation plus multiple-`yield` diagnostics;
  - duplicate exact capability bindings in effect handlers;
  - machine handler ordering / lifetime and init-state behavior;
  - REST decode and `strictStatus` behavior;
  - compile-time `env.get` missing-variable behavior;
  - any publicly intended `Tree` class-style operations.

