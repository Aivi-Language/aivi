# Writing a stockroom application in AIVI

This audit used the working application in
[`demos/stockroom`](https://github.com/Aivi-Language/aivi/tree/main/demos/stockroom#readme).
It was conducted on 12 September 2026, starting from revision
`e2cec71446d129491f9ef39c36183de52777b7f0`. The fixes described below are in the accompanying change.
The original experiment and recommendations are retained below. The [implementation follow-up](#implementation-follow-up) records the subsequent consistency improvements. Remaining syntax and schema proposals are not implemented semantics.

## The experiment

The application reads a local JSON inventory, validates it, computes a restock list, and shows
that list in a native GTK window. It supports case-insensitive search, deterministic ordering,
manual reload, empty results, malformed data, missing files, and recovery after correcting the
file. Reload preserves the current search. It never writes inventory data.

This is a deliberately small operational tool: more demanding than a counter, but narrow enough
that failures can be isolated. It exercises modules, closed records, ADTs, higher-order functions,
pipes, typed external decoding, signal accumulation, source lifecycle, markup patterns, keyed
children, executable tests, and packaging. It does not measure large-data performance or prove
that a full inventory-management product is ready to ship.

The architecture follows AIVI's existing model:

- `inventory.aivi` owns pure validation and transformations.
- `main.aivi` owns the filesystem source, UI events, derived signals, and markup.
- `tests.aivi` checks the domain without a window or real filesystem.
- `smoke.py` drives the real application through MCP, using actual GTK events and temporary files.

The pure pipeline is the clearest part of the program. With typed predicates and a comparator,
restocking reads as a sequence of domain operations:

```text
catalog.items
  |> filter needsRestock
  |> filter (matchesSearch query)
  |> map restock
  |> sortBy priority
```

Shortages sort by descending quantity and then ascending SKU. Nonnegative quantities are
validated before subtraction. SKU uniqueness uses a scan per item, which is quadratic; this
implementation is intended for a small inventory. Search and rendering likewise process the
whole list. These are explicit application costs, not performance claims about AIVI.

## How writing it felt

**The pure core is pleasant once its types are explicit.** Small named functions compose well,
closed records make the data model easy to inspect, and ADT matching gives loading and failure
states a concrete representation. The tests are ordinary AIVI values. Domain behavior does not
require GTK fixtures or mutable test doubles.

**Derived signals remove real bookkeeping.** `visiblePlan query screen` updates the displayed
list when either input changes. There is no separate instruction to refresh the list after each
search or successful reload. The source declaration also keeps filesystem work outside the pure
model. A typed `Result FsError Catalog` gives the external boundary a useful contract.

**The boundary was easier to declare than to trust.** Before the fixes, a missing file could leave
the screen loading indefinitely, and a valid signal pattern could pass `check` but fail runnable
lowering. The strongest DX problem was therefore confidence: a green check did not establish
that this small program could run. That matters more than saving a few characters of syntax.

**Higher-order inference has surprising cliffs.** An inline predicate inside
`length (filter (...) items)` exposed recursive re-entry into module inference and crashed the
compiler. After the crash fix, that underconstrained example produces a diagnostic; it has not
become a supported inference case. A named, typed `sameSku` function and an explicit list import
made the intended program work. During development, an ambient `length` expression also passed
checking but failed typed-core lowering; explicitly importing `aivi.list (length)` resolved it.
A direct polymorphic `getOrElse` call on a signal also failed native lowering; the delivered
program gives the pure operation a typed `filePath` helper before lifting it. Developers should
not need to discover these distinctions through execution.

**Pipe restrictions can push local logic into module-level helpers.** Nested pipe bodies were
rejected while expressing validation and state conversion. The final program uses
`validateUnique` and `validatedScreen`. These names are reasonable, but creating a helper solely
to satisfy a control-flow restriction interrupts local reading. The existing reuse and typed
block forms should be evaluated before adding another binding mechanism.

**The syntax has a learning cost that documentation must earn back.** `|>`, `||>`, `T|>`, `F|>`,
and `+|>` express different operations compactly. Their distinctions become manageable in this
small module, but ordinary words are easier to search for. Standalone `type A -> B` signatures
also require looking at the adjacent declaration to discover which function they describe.
These are readability tradeoffs, not evidence that the operators should all be replaced.

## Defects found and repaired

| Failed invariant | Repair | Regression evidence |
| --- | --- | --- |
| A contextual type probe must not restart the inference pass that invoked it. | Preserve the current inference seeds, in-progress markers, and inference policy in isolated probes. Keep speculative call evidence local. | The reduced collection-comparison program now exits with a diagnostic instead of stack overflow. |
| A markup pattern over a signal binds fields of its payload type. | Build markup case environments from the signal payload. Point runnable-lowering errors at the failing expression. | Lowering tests cover sum, option, and record signal patterns; the application's `Failed reason` branch runs. |
| A filesystem read must use the application's configured working directory and publish representable failures. | Resolve `fs.read` paths against provider context; map missing, permission, other I/O, and decoding failures to the standard `FsError` constructors. Reads remain on workers. | Runtime tests exercise valid data, missing files, invalid data, and a directory read with the actual stdlib error type. The GTK test verifies recovery across repeated failures. |
| A synthetic MCP click must dispatch each requested button event before settling. | Emit the button's click directly; preserve the separate activation operation. Use the same click dispatch for double-click requests. | A GTK regression asserts three consecutive clicks arrive synchronously; the application smoke test performs repeated reloads without timing sleeps. |
| Reactive fragments must retain the workspace identities and implementations used by the full program. | Thread workspace context through reactive fragment lowering, including imported ADTs and function bodies. | Package and launch the application using `Result FsError Catalog`. |
| A source must wait for configuration dependencies before activation. | Defer activation until configuration signals have committed; do not require an initial event-trigger value. | A source → derived endpoint → dependent source startup test, plus an environment-selected inventory path. |
| Native execution must implement the text operations used by the application. | Add native Unicode trimming, lowercasing, substring search, and lexicographic ordering using arena-owned helper calls. | Native artifact round-trip tests cover Unicode, empty strings, embedded NUL, and ordering boundaries; packaged launch exercises the whole application. |
| An imported error accepted by a destination layout must match that layout's constructors. | Canonicalize a validated sum's module-local ID to the destination layout before inline-pipe matching. Preserve type-name, variant, and payload checks. | An interpreter regression accepts the same nominal type with a source-module ID and rejects a different type; malformed-file recovery exercises `FsError`. |
| Native list quantifiers must support bound predicates and short-circuit correctly. | Share the existing native predicate loop between `any` and `all`, with explicit quantifier semantics. | Native artifact tests cover empty lists, bound threshold arguments, and early exits before division by zero. |
| Documented widget slots must agree with the implemented schema. | Correct `ToolbarView.top` to `ToolbarView.topBar` in the markup guide. | The catalog was checked against the guide; manual snippets are validated separately. |

Native compilation also rejected direct interpolation of an arbitrary `FsError` value. The
application now matches its constructors and formats their text fields explicitly, producing
better user-facing messages. General ADT interpolation remains a native-backend limitation;
this audit did not add a universal formatting implementation.

The inference crash came from a missing boundary between speculative checking and the active
inference pass. The filesystem failures exposed incomplete adaptation between provider errors
and the standard error ADT. The markup failure exposed inconsistent signal unwrapping between
checking and runnable lowering. These were compiler/runtime defects, not reasons to make the
application accept weaker types.

Permission failure mapping is implemented, but the end-to-end filesystem test does not depend
on Unix permissions: that would be unreliable when run with elevated privileges. The smoke
exercise verifies actual reads and recovery; it is not a scheduler stress or leak test.

## Comparison with other languages

This is a comparison of programming models against their documentation. Equivalent applications
were not implemented in the other languages, so it is not a measured productivity ranking.

| Concern | AIVI in this experiment | Comparison |
| --- | --- | --- |
| Updating a view | Explicit input signals and automatic pure derivations. | Elm organizes changes through Model, Update, and View. That makes transitions explicit; AIVI saves update bookkeeping for this derived list. [Elm architecture](https://guide.elm-lang.org/architecture/) |
| Decoding an ordinary record | The source's `Catalog` annotation supplies the shape. | Elm's JSON guide constructs decoders explicitly. That is more work for this shape, while making boundary adaptation visible in code. [Elm JSON](https://guide.elm-lang.org/effects/json) |
| Evolving external schemas | Exact records are useful for catching accidental fields, but stricter than many external APIs. | Serde provides field rename, alias, and default controls, plus explicit unknown-field rejection and enum representation controls. AIVI needs an equally discoverable boundary vocabulary while preserving closed internal types. [Serde fields](https://serde.rs/field-attrs.html), [Serde containers](https://serde.rs/container-attrs.html) |
| Modeling UI states | Closed `Screen` constructors and pattern bindings. | TypeScript also supports discriminated unions and exhaustiveness through `never`; the syntactic shape differs, but explicit state modeling is shared. [TypeScript narrowing](https://www.typescriptlang.org/docs/handbook/2/narrowing.html) |
| Native UI wiring | Markup events publish into the signal system. | GTK's direct API connects widget signals to handlers. AIVI reduces manual wiring for this reactive view, while depending on its widget catalog being correct. [GTK getting started](https://docs.gtk.org/gtk4/getting_started.html) |

For this particular tool, AIVI's strongest advantage is the combination of pure transformations,
typed sources, and derived native UI. The weakness is uneven implementation coverage between
those layers. More syntax will not compensate for that gap.

## Recommended changes, in priority order

### 1. Make checking predict execution

Share runnable-expression and widget-schema validation with `check` for a selected app/view,
or add an explicit `check --runnable` mode that editors can invoke. `build` already validates the
runnable surface; expose that confidence earlier without requiring packaging. Report unsupported
lowering at the exact expression, with the missing type or capability. Add the stockroom example
to the set of applications that are checked, lowered, and exercised together.

Acceptance example: `Failed reason` inside a signal match either checks and runs, or fails during
checking at `reason`. An ambiguous ambient collection operation must not get its first useful
error from typed-core lowering.

### 2. Improve callback context before expanding overloads

Propagate known collection element types into callback parameters consistently, including inside
comparisons and nested applications. Keep overload candidate probing isolated and deterministic.
When imports remain ambiguous, name the candidates and suggest an explicit import or annotation.
Do not select an arbitrary overload or loosen record closure to get an answer.

Acceptance example: the checker can explain the element type of `other` in a predicate passed to
`filter`, or identifies exactly which annotation is missing. Deep or cyclic inference always
terminates with a diagnostic.

### 3. Allow local composition without losing pipe clarity

Evaluate a scoped nested-pipe expression whose input and implicit projections are lexical to
that expression. This could avoid extracting a one-use helper when matching a validation result
inside a larger branch. The design must specify branch result types, signal lifting, and which
scope an implicit projection refers to; nesting should never change an outer pipe's meaning.

As a smaller independent option, consider a co-located function signature. This is a proposal,
not valid syntax in the delivered example:

```text
func sameSku : Stock -> Stock -> Bool = item other => other.sku == item.sku
```

Compare it with today's two-line spelling in a reading exercise before adopting it. The current
standalone signature remains compact and could benefit from editor association rather than a
language change. Both forms should not be added without a clear formatting and migration policy.

### 4. Separate wire schemas from domain types

Keep `Catalog` closed. Add or consolidate a declarative codec/schema facility for external names,
aliases, explicit missing-field defaults, and tagged-union representations. It should produce a
typed decode plan and retain source paths in failures. Defaults must distinguish a missing field
from explicit null, a wrong type, and a malformed value. Unknown-field tolerance should belong
to the boundary codec, never silently open an internal record.

A useful design test is an API that calls `onHand` `on_hand`, adds a harmless metadata field, and
omits `target` in older payloads. The developer should express that adaptation once, explicitly,
without hand-walking untyped JSON. Renaming and defaults belong in the codec; “quantity must be
nonnegative” belongs in domain validation. Reuse existing validation abstractions for collecting
multiple domain errors rather than introducing a second validation system.

### 5. Make source state composition easier to discover

The current source companions already expose success, error, loading, and trigger state. Build
documentation and a reusable standard helper around those capabilities before adding new syntax.
A common resource state should distinguish initial loading from refreshing old data and make the
stale-data policy explicit. This demo intentionally clears the visible plan on failure; another
application may need the last good plan plus an error banner.

Acceptance example: a developer can implement either policy with a small pure state transition,
and retry works after transport, decode, and domain errors. No worker mutates UI-owned state.

### 6. Treat the GTK catalog as a shared schema

Generate or validate slot/property examples and completion metadata against the catalog.
`top` versus `topBar` is a small spelling mismatch with a large interruption cost. A diagnostic
should offer the valid slot name before launching. This is a tooling/schema consistency change;
it does not require exposing GTK implementation details in application code.

## Implementation follow-up

The follow-up preserves AIVI's signal graph, pure transformations, and pipe model. It adds no
Model/Update/View architecture, new operators, or alternative function-signature syntax.

| Audit concern | Delivered behavior | Regression boundary |
| --- | --- | --- |
| Checking stopped before runnable validation. | `check --runnable [--view name]` uses the same eager preparation and native frozen bundle encoder as `build`, entirely in memory. Ordinary `check` remains useful for libraries. | CLI tests check view selection, invalid options, invalid widget slots, and a HIR-valid program with unsupported native formatting, without a display. |
| Inline callbacks lost a later collection argument's element type. | A resolved generic call first gathers type bindings from data arguments, then checks its callback with that context. Successful concrete callback signature evidence reaches module inference, including captured arguments. Unconstrained callback probes do not poison the inference cache. | The explicit `aivi.list` example `length (filter (other => other.sku == item.sku) items) == 1` checks; missing fields and incompatible callback operators remain errors. |
| Lifted polymorphic functions specialized their type variable to the entire signal. | Elaboration specializes the pure callee using the payload result and keeps signal lifting on the application. `getOrElse "inventory.json" configuredPath` works directly. Explicit `Signal` parameters retain their declared type; backend dependency environments consistently carry committed values. | Native artifact serialization, relinking, and execution test both `None` and `Some`, a signal-provided fallback, the same dependency used through explicit and implicitly lifted parameters, and explicit `&|>` fan-out. |
| Native interpolation rejected even supported scalar signals. | Formatting follows committed signal payload layouts through the existing scalar ABI. Fresh and replayed JIT call signatures use that same payload ABI instead of declaring scalar inputs as pointers. Carrier traversal uses a bounded loop. | Development evaluation, fresh JIT execution, and a serialized native artifact produce the same text for Text, Int, Bool, Float, and Unit, in derived signals and functions with explicit signal parameters. |
| Packaging failures identified internal kernels without source positions. | Native compilation errors retain the owning declaration and failing expression's file, line, and column. `build` and runnable checking share this rendering. | The unsupported ADT interpolation regression asserts the interpolation's source line. |
| Widget slot errors omitted the valid vocabulary. | Unknown groups list names from the existing GTK widget schema, including `topBar`; markup and bridge validation use catalog data. | A misspelled `ToolbarView.top` fails runnable checking and names `topBar`. |

Stockroom now uses the captured inline predicate and direct signal `getOrElse` call, so its
ordinary domain tests, GTK exercise, and native package cover the improved spelling. Its pure
file-error formatter remains deliberate application behavior: it gives users actionable messages.

Runnable checking validates compilation and serialization for the current target. It does not
activate sources or execute a task entry point, and cannot prove that external services or files
will be available at deployment. It is explicitly more expensive than checking syntax and types.

Validation of the follow-up passed 695 compiler/HIR/core/backend/GTK tests (one ignored
documentation test), all 55 CLI checking tests, and the focused native signal and callback
regressions. All five executable packaging tests pass, including Snake, Reversi, and Stockroom.
Stockroom passes runnable checking and the GTK smoke. The manual scanner still
reports 678 blocks with no todos and complete stdlib API coverage.

The full CLI unit run passed 126 tests, failed the Reversi computer-final and Snake restart
fixtures, and ignored one test. Both failures passed in isolated reruns. This leaves a
suite-level timing/isolation caveat; it is not a clean full CLI unit run.

The remaining work is:

- Generalize contextual inference to unresolved class-member/ambient operations and callbacks
  whose result type also needs inference. Explicit imports remain necessary for the reduced
  ambient `length`/`filter` example; the previous crash regression still requires a diagnostic.
- Extend native formatting to arbitrary ADTs and aggregates with one specified representation
  shared with development execution. Runnable checking now catches this gap before packaging.
- Resolve direct signal-operator consistency across HIR checking and backend lowering; typed
  pure helper functions remain the working spelling for the audited direct text comparison.
- Design the proposed wire-codec field aliases, missing-field defaults, and unknown-field policy
  against existing decoding machinery; keep internal records closed.
- Evaluate scoped local pipes and co-located signatures separately before adopting syntax;
  document reusable source loading/refreshing/stale-data transitions using existing companions.
- Stabilize the GTK game fixtures that failed in the full CLI run but passed in isolation.
- Validate or generate editor completion metadata and manual widget examples from the catalog,
  and evaluate a smaller runtime-only packaged launcher.

## Reproducing the audit

Follow the [example README](https://github.com/Aivi-Language/aivi/tree/main/demos/stockroom#readme)
to check, test, run, package, and
exercise the app. The smoke test verifies initial ordering, normalized search, retained search on
reload, empty results, wrong field types, unexpected and missing fields, domain rejection, a
missing file, recovery, and a 360-pixel-wide launch. Screenshots are written under `out/`.

The audit does not cover screen readers, localization, a full light/dark/high-contrast theme
matrix, huge inventories, write transactions, or cross-platform packaging. No language proposal
above was silently implemented. The delivered application uses existing AIVI syntax with the
specific correctness fixes described here.

### Original audit validation results

The final runtime/backend/FFI test run passed all 390 tests. All five CLI packaging tests passed,
including the existing Snake and Reversi bundles and the new stockroom executable. The earlier
HIR/CLI run passed 613 tests; after the runtime/native changes, the CLI unit run passed 127,
ignored one, and timed out in the timer-dependent Reversi pass-chain test while other heavy
checks were running. That test passed on a focused rerun. This is a validation caveat, not a
claim that the full final CLI run was clean.

The six domain examples, GTK smoke, native object compilation, executable packaging, Clippy
with warnings denied, and AIVI formatting checks passed. The final workspace-wide Rust formatting
check reported differences in concurrently edited benchmark, GC, and query files outside this
audit; it reported no differences in the audit files. Those concurrent edits were preserved. The manual check scanned 678 code blocks
with no remaining todos; its API audit reported 381/381 functions covered and 978/978 public
exports documented. A separate packaged launch probe confirmed that the executable opened an
absolute external inventory path and remained running on malformed input.

The measured packaged launcher was 75.1 MiB after stripping a 385.9 MiB development binary,
before adding the app bundle. This is a development-build packaging observation, not a release
size comparison. A smaller runtime-only launcher would be a useful deployment improvement.
