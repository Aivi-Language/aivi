# Proposal: Unified Workflow Pipelines

> **Proposal status:** this page is a forward-looking design proposal. It does **not** replace the current v0.1 language reference yet.
>
> Today, workflow sequencing is still defined by [Effects](../syntax/effects.md), [do Notation](../syntax/do_notation.md), [Resources](../syntax/resources.md), and [Generators](../syntax/generators.md). If this proposal lands, those pages become migration references during the refactor and are then removed or rewritten at cutover.

## Why this proposal exists

AIVI currently splits workflow-shaped code across several surfaces:

- plain `|>` pipelines for ordinary transforms,
- `do Effect { ... }` for effects and resource acquisition,
- `do M { ... }` for generic chaining,
- `do Applicative { ... }` for independent validation,
- `generate { ... }` for zero-many workflows,
- `resource { ... }` for explicit acquisition and cleanup.

Each of those forms is coherent on its own, but together they force programmers, formatters, and tooling to switch between several syntactic models for what is often the same left-to-right idea: take a current value, run the next step, and decide what to do with success, absence, multiplicity, failure, cleanup, and control flow.

This proposal replaces that split with one pipeline calculus so ordinary sequencing, optional binding, failure propagation, applicative fan-out, generator expansion, resource cleanup, retries, timeouts, concurrency, and looping all read in the same surface language.

The intended end state is deliberate: **one workflow style, not several equivalent ones**.

## Design goals

1. **One workflow surface.** Effects, `Option`, `Result`, applicatives, generators, and resources should look like variations of one core idea rather than separate notations.
2. **Left-to-right reading.** The common case should read as a single pipeline without nested `do Effect { ... }` blocks.
3. **Explicit control.** Failure handling, retries, timeouts, cleanup, concurrency, and branching should remain visible in the syntax.
4. **Formatter-friendly structure.** Indentation and subflow boundaries should come from the parsed tree, not from heuristics.
5. **Mechanical migration.** The compiler should be able to lower the new syntax to the same internal workflow machinery during the refactor.
6. **No permanent aliases.** If adopted, legacy workflow block forms should be removed instead of kept as long-term duplicates.

## Non-goals

- This proposal does **not** redesign patching (`<|`) or signal writes (`<<-`).
- This proposal does **not** make JSON decoding implicit by expected type; explicit decode stages stay clearer and give better diagnostics.
- This proposal does **not** keep legacy workflow blocks as permanent sugar. Temporary migration support is acceptable during the refactor, but the target language should end with one way to express workflow sequencing.

## Core model

Every workflow pipeline conceptually carries three things:

- the **current** subject value,
- a lexical map of remembered names introduced by `:|> #name`,
- a stack of **control policies** that affect the next stage or the next nested subflow.

Data-flow stages transform or unwrap the current subject.
Control stages do not directly replace the current subject; instead they remember names, attach policies, open grouped subflows, or branch between subflows.

In other words:

- `|>`, `!|>`, `?|>`, `&|>`, and `*|>` describe **what data flow happens next**,
- `:|>` describes **how the next part of the workflow behaves**.

## Stage family

| Surface | Kind | Meaning |
| --- | --- | --- |
| `|>` | pure transform | Apply the right-hand side to the current subject. |
| `!|>` | fail-fast bind | Run and unwrap the next `Effect E A` or `Result E A`; short-circuit on failure unless a control policy says otherwise. |
| `?|>` | optional bind | Unwrap the next `Option A`; short-circuit on `None` unless a control policy says otherwise. |
| `&|>` | applicative branch | Start an independent branch from the same incoming subject, typically inside an `all` group. |
| `*|>` | sequence / generator bind | Expand zero-many outputs from the current subject and flatten them into the surrounding pipeline. |
| `:|>` | control / meta stage | Remember names, attach policies, open grouped subflows, or branch. |

The current `|>` rule still applies: the value on the left flows into the final argument position on the right.

## Control clauses

`:` remains outside the data-flow family on purpose. A `:|>` stage never means "transform the current subject". Instead, it configures the next stage or opens a scoped subflow.

Comma-separated clauses inside one `:|>` stage are applied left-to-right.

```aivi
fetch url
  :|> retry 3x, timeout 3s, #response
  !|> .body
  !|> decode User
  :|> #user
   |> { user, response }
```

### Remembered names

`#name` remembers the current subject without changing it.

```aivi
fetch url
  :|> #response
  !|> .body
  !|> decode User
  :|> #user
   |> { user, response }
```

Remembered names are visible to later stages in the same pipeline and to nested subflows opened from that point onward.

### Failure, fallback, and recovery

This proposal replaces the old `or` split with explicit control clauses:

| Clause | Meaning |
| --- | --- |
| `attempt` | Capture the failure of the next fail-fast stage or grouped subflow as `Result`. |
| `recover \| ... => ...` | Handle the failure of the current fail-aware result and resume with a replacement value or failure. |
| `default expr` | Provide a default for short-circuiting `Option`, `Result`, or effect failure when the fallback does not depend on the failure payload. |
| `expect err` | Upgrade an in-scope `None` short-circuit into an explicit failure value. |
| `guard pred else err` | Fail immediately unless the predicate holds for the current subject. |

Examples:

```aivi
url
  :|> attempt
      !|> fetch
      :|> timeout 3s
      !|> decode User
```

```aivi
lookup settings "port"
  :|> expect MissingPort
  !|> parsePort
```

```aivi
config
  :|> guard .ok else BadConfig
  !|> startSystem
```

`!|>` is fail-fast by default. `attempt`, `recover`, `default`, and `expect` are the explicit ways to turn that failure into data or a fallback.

### Guards and conditional subflows

`when` and `unless` become control clauses that gate the next stage or the next grouped subflow.

If the gated stage is skipped, the current subject is preserved.

```aivi
user
  :|> when .needsNormalization
      !|> normalizeUser
  :|> unless .isActive
      |> markInactive
```

Branching uses a dedicated control form so grouped branch pipelines remain formatter-friendly:

```aivi
request
  :|> if useCache
      then cache.read key
      else fetch key
        :|> timeout 3s
        !|> decode User
  :|> #user
   |> "{user.firstName} {user.lastName}"
```

## Resources and cleanup

This proposal removes the dedicated `resource { ... }` workflow surface. Resource lifetime becomes a pipeline concern through explicit cleanup registration.

| Clause | Meaning |
| --- | --- |
| `cleanup expr` | Register a finalizer to run when the enclosing subflow exits by success, failure, or cancellation. |
| `cancelOnError` | Cancel in-flight sibling work when one branch fails. |

Example:

```aivi
path
  !|> file.open
  :|> #handle, cleanup file.close handle
  !|> file.readAll
```

The concrete runtime may still lower resource-aware stages through the existing `Resource` machinery. The source language no longer needs a separate block form for that lifetime rule.

## Applicatives and validation

Applicative composition becomes an explicitly grouped pipeline shape rather than a separate `do Applicative { ... }` block.

| Clause | Meaning |
| --- | --- |
| `all` | Open an applicative group whose `&|>` branches all start from the same incoming subject. |
| `collectErrors` | Accumulate validation failures across the next `all` group instead of failing fast. |

Example:

```aivi
form
  :|> collectErrors
  :|> all
      &|> .name  |> validateName  :|> #name
      &|> .email |> validateEmail :|> #email
      &|> .age   |> validateAge   :|> #age
   |> User { name, email, age }
```

Inside an `all` group:

- each `&|>` branch receives the same pre-group subject,
- remembered names from successful branches become available after the group,
- `collectErrors` switches the group to accumulation semantics similar to `Validation`,
- without `collectErrors`, the default policy is fail-fast.

## Generators, zero-many workflows, and concurrency

`*|>` replaces the separate `generate { ... }` surface. A `*|>` stage expands zero-many outputs and flattens them into the surrounding workflow.

| Clause | Meaning |
| --- | --- |
| `concurrent n` | Permit up to `n` in-flight branches for the next fan-out stage or grouped subflow. |
| `ordered` | Preserve source order when a concurrent fan-out stage would otherwise emit in completion order. |

Example:

```aivi
seed
  :|> loop #cursor = seed
  !|> fetchPage cursor
  :|> #page
  *|> .items
  :|> recurse #cursor = page.nextCursor while page.hasMore
```

Concurrent fan-out:

```aivi
urls
  :|> concurrent 8, cancelOnError
  *|> fetch
  :|> timeout 3s
  !|> decode Item
```

The compiler should reject `concurrent` on stages that cannot fan out, because silent no-op concurrency settings would be misleading.

## Grouping and subflows

The proposal needs one generic grouping rule rather than many unrelated block syntaxes.

Any `:|>` clause that owns an indented body opens a **subflow**. Dedenting closes that subflow.

Examples:

```aivi
request
  :|> flow
      !|> fetch key
      :|> timeout 3s
      !|> decode User
  :|> #user
   |> "{user.firstName} {user.lastName}"
```

```aivi
url
  :|> attempt
      !|> fetch
      !|> decode User
```

This keeps grouping uniform:

- `if` owns `then` and `else` subflows,
- `when` and `unless` can own subflows,
- `attempt` can own a subflow,
- `all` owns applicative branches,
- `flow` is the generic "just group these stages" form.

## Loop and recurse

This proposal keeps structured feedback, but it moves it into the control-stage family instead of a separate block statement syntax.

| Clause | Meaning |
| --- | --- |
| `loop #name = expr, ...` | Introduce named loop-carried state for the current subflow. |
| `recurse #name = expr, ... while pred` | Jump back to the nearest loop head with updated carried state while the predicate holds. |

Example:

```aivi
initialCursor
  :|> loop #cursor = initialCursor, #all = []
  !|> fetchPage cursor
  :|> #page
  :|> recurse #cursor = page.nextCursor, #all = all ++ page.items while page.hasMore
   |> all ++ page.items
```

Resource cleanup registered during one iteration runs before the next iteration begins unless the cleanup is intentionally registered outside the loop scope.

## Formatting and indentation

The formatter should own this style. Authors should not have to hand-align complex subflows.

### Alignment rule

In a multi-line workflow pipeline, the formatter aligns stage operators on the `|` column.

That means the bare pipe is rendered with one leading space when it appears alongside sigiled stages:

```aivi
request
  :|> if useCache
      then cache.read key
      else fetch key
        :|> timeout 3s
        !|> decode User
  :|> #user
   |> "{user.firstName} {user.lastName}"
```

Formatting rules:

- sigiled stages (`:|>`, `!|>`, `?|>`, `&|>`, `*|>`) keep their prefix immediately before `|`,
- the plain stage is rendered as ` |>`,
- `then` and `else` align with each other,
- branch-local stages indent under their owning branch,
- dedenting closes the current subflow and returns to the outer pipeline.

### Why this matters

If the `|` column always lines up, long workflow pipelines stay visually scannable even when they mix pure transforms, failure-aware steps, metadata stages, and grouped branches.

## Migration and refactor plan

Adopting this proposal would require a deliberate end-to-end refactor.

### Phase 1: grammar and CST

- add the new pipe-family tokens and grouped `:|>` clause forms,
- parse subflows explicitly so the formatter and LSP can recover structure,
- keep legacy workflow forms temporarily only if needed for migration,
- emit diagnostics that point users toward the new surface as soon as it exists.

### Phase 2: lowering and typechecking

- lower unified workflow pipelines to the existing effect / chain / applicative / generator / resource core,
- preserve source spans so diagnostics still mention the correct stage,
- add policy-aware lowering for `attempt`, `recover`, `default`, `expect`, `cleanup`, `concurrent`, and `loop` / `recurse`,
- make `&|>` groups check independence the same way `do Applicative { ... }` does today.

### Phase 3: runtime and standard library integration

- route retries, timeouts, cancellation, and cleanup through the runtime effect layer,
- define how concurrent `*|>` and `&|>` interact with cancellation and failure propagation,
- keep resource cleanup guaranteed on success, failure, and cancellation,
- expose any supporting helpers needed by the lowered forms without reintroducing user-visible duplicate syntax.

### Phase 4: formatter, LSP, and editor tooling

- teach the formatter the `|` alignment rule,
- format nested subflows deterministically,
- add semantic tokens, hover help, and completion support for the new clauses,
- regenerate VSCode syntax assets after the grammar settles.

### Phase 5: tests and documentation

- add parser, formatter, and compile-fail coverage for every new stage family and control clause,
- migrate integration examples from `do Effect`, `do M`, `do Applicative`, `generate`, and `resource`,
- update `AIVI_LANGUAGE.md` only after the implementation exists,
- update language-overview and reference pages at the same time as the cutover.

### Phase 6: cutover and deletion

When the new surface is complete and validated:

- delete the legacy workflow grammar and lowering paths,
- delete obsolete examples and integration tests that cover only the removed forms,
- rewrite the affected syntax reference pages so they describe only the unified pipeline surface,
- remove long-term duplicate documentation and "old vs new" split guidance.

The success condition is strict: after cutover, AIVI should no longer have two different surface families for the same workflow concepts.

## Open questions for the refactor

This proposal is intentionally concrete enough to guide a refactor, but a few implementation details still need hard decisions before the parser work starts:

- should `default` apply uniformly to `Option`, `Result`, and effect failure, or should some cases require `recover` for clarity?
- should `ordered` be the default for concurrent fan-out, with an opt-in clause for completion-order emission?
- how much of the old `do Event { ... }` surface should lower directly through the unified pipeline core versus staying as library-level helpers on top?
- should grouped `recover` and `attempt` remain keyword clauses inside `:|>`, or should one of them gain a shorter dedicated spelling after real-world formatter experiments?

Those decisions belong in the implementation RFC and parser prototype, not in the current v0.1 reference pages.
