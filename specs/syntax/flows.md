# Flow Syntax

<!-- quick-info: {"kind":"topic","name":"Flow Syntax"} -->
AIVI v0.2 flow syntax is the flat, spine-aligned workflow surface for sequential flow, guards, recovery, applicative siblings, fan-out, cleanup, retry, and restart anchors.
<!-- /quick-info -->
## 1. Status and intent

This document defines AIVI v2 Flow Syntax as a flat, spine-aligned surface syntax for left-to-right flows across sequential, applicative, generator-shaped, resource-scoped, and event-shaped code.

It is intended to subsume the common user-authored shapes currently expressed with `do Effect { ... }`, `do M { ... }`, `do Applicative { ... }`, `generate { ... }`, `resource { ... }`, and most handler bodies currently written with `do Event { ... }`.

If a carrier such as `Query` still needs specialized internal lowering or optimization, that is an implementation concern rather than a reason to preserve a separate user-surface `do` form.

## 2. Design goals

AIVI v2 Flow Syntax has six goals:

1. Preserve a visually stable execution spine.
2. Keep ordinary sequential flow flat.
3. Support common branching, recovery, fan-out, concurrency, retry, cleanup, and validation without nested callback structure.
4. Keep data shaping in ordinary functions and pipes rather than embedding a second collection DSL.
5. Remain compatible in spirit with existing AIVI pipe, effect, guard, attempt, duration, resource, generator, test, mock, and event semantics.
6. Eliminate user-authored `do` blocks from the common language surface.

## 3. Non-goals

AIVI v2 Flow Syntax does not attempt to:

- introduce a general-purpose collection or reduction DSL
- redefine existing declaration decorator semantics
- force all carriers to have identical runtime scheduling semantics
- model foreign runtime event pumps such as GTK’s outer main loop as language-level recursion

---

## 4. Core model

A flow carries a current value called the **spine subject**.

Each line consumes the current spine subject and may:

- transform it
- bind it
- guard it
- attempt a fallible effect
- recover from a captured failure
- branch on it
- fan out over items
- run applicative siblings over it
- run side effects without changing it
- register cleanup for scope exit
- mark a recursion anchor

This remains consistent with AIVI’s expression orientation, explicit effects, immutable bindings, pipe semantics, `attempt`/`given` style, resource cleanup guarantees, and event handler style.

---

## 5. Surface grammar

### 5.1 Canonical line form

Each flow line has the form:

```text
[operator] [expression] [@modifier ...] [#binding]
```

Where:

- `operator` is a three-character sigil
- `expression` is the line body
- `@modifier` is an optional flow-line modifier
- `#binding` is an optional binding target

A modifier may refer to the successful line result positionally.
For `@cleanup`, the successful line result is passed as the final argument if the modifier expression is a callable value.

### 5.2 Spine alignment rule

All flow operators are visually aligned in a single left spine.

The ordinary flow operator is written with **one leading space** so that its arrow aligns with the other three-character operators.

Canonical operator spellings:

```text
@|>
>|>
!|>
*|>
*-|
?|>
||>
~|>
 |>   // note the leading space
&|>
```

The leading space on ordinary flow is required for presentation and should be preserved in all spec examples.

---

## 6. Bindings

### 6.1 Ordinary binding

`#name` binds the successful unwrapped result of the line to `name`.

```aivi
authCode
   |> github.oauth.exchangeToken #token
```

The binding is immutable and visible to later lines in the same flow.

### 6.2 Explicit continuation after sibling blocks

`&|>` keeps the original incoming spine subject after the block completes. If a later line should continue from one of the sibling bindings, make that choice explicitly with an ordinary `|>` line.

```aivi
token
   &|> github.api.getUser token.accessToken #profile
   &|> github.api.getEmails token.accessToken #emails
    |> emails
    |> find (.primary)
```

Rules:

- `#name` binds values from sibling lines as usual
- the next line after a contiguous `&|>` block starts from the original incoming subject
- use `|> boundName` (or any other ordinary expression) when you want to continue from one of the bound sibling results
- `#name!` was removed in v0.2; continuation subject choice is always explicit

---

## 7. Operators

### 7.1 `|>` — Flow

Ordinary sequential flow.

```aivi
x
   |> f
   |> g a
```

Semantics:

- if the expression is pure, it maps over the current spine subject
- if the expression returns an effect or carrier, it binds and unwraps the successful result
- the result becomes the next spine subject

This follows the current AIVI rule that pipes pass the left value as the last argument.

### 7.2 `~|>` — Tap

Effectful observation that preserves the incoming spine subject.

```aivi
user
   ~|> log "user {user.id}"
    |> persistUser
```

Semantics:

- evaluates the expression for its side effects
- discards the expression result
- passes the original incoming spine subject to the next line unchanged

### 7.3 `>|>` — Guard

Predicate gate over the current spine subject.

```aivi
token
   >|> token.isValid or fail InvalidGrant
    |> continueAuth
```

Semantics:

- if the predicate is true, flow continues unchanged
- if false and `or fail err` is present, the flow fails with `err`
- if false and no failure branch is present, the line is only valid where dropping is already well-defined by the enclosing carrier

Generator rule:

- inside `*|>`, `>|>` without `or fail ...` means “skip this item if false”

In ordinary effect flow, the intended model matches current `given cond or failExpr`, so silent guards should not be the default outside generator-style fan-out.

### 7.4 `?|>` — Attempt

Captures a fallible step instead of immediately failing the whole flow.

```aivi
request
   ?|> stripe.paymentIntents.create { amount: _, customer: customer.id }
   !|> CardDeclined e => handleDeclinedCard e
```

Semantics:

- executes a fallible effect
- captures failure
- makes the captured result available to following `!|>` recovery lines

This is the flow-syntax analogue of `attempt`.

### 7.5 `!|>` — Recover

Recovery arm for the nearest preceding `?|>`.

```aivi
request
   ?|> riskyPayment
   !|> CardDeclined e   => handleDeclinedCard e
   !|> RateLimitError _ => handleRateLimit request
    |> finishCheckout
```

Semantics:

- only valid immediately after an attempt region
- if the attempt succeeded, all recovery arms are skipped
- if the attempt failed, the first matching arm runs
- if nothing matches, the failure propagates

Rules:

- `!|>` lines form a contiguous block
- the first matching arm wins
- if the attempted line also has `@retry`, recovery sees only the terminal failure after retry exhaustion

### 7.6 `||>` — Branch

Value branch on the current spine subject.

```aivi
eventType
   ||> "invoice.paid"           => handleInvoicePaid payload
   ||> "invoice.payment_failed" => handleInvoiceFailed payload
   ||> _                        => markUnhandledEvent payload
```

Semantics:

- matches the current spine subject against patterns or literals
- the first matching arm runs
- branch arms are contiguous

This should remain exhaustive in the same spirit as ordinary `match`.

### 7.7 `&|>` — Sibling blocks

Independent sibling steps that all receive the same incoming spine subject.

```aivi
token
   &|> github.api.getUser token.accessToken #profile
   &|> github.api.getEmails token.accessToken #emails
    |> emails
    |> find (.primary)
```

Semantics:

- all contiguous `&|>` lines form a sibling block
- each sibling line receives the same incoming spine subject
- sibling lines are independent and are not sequenced by binding dependencies
- if the carrier/runtime supports it, the block may be scheduled concurrently; `@concurrent` only tunes that scheduling
- sibling bindings become available together after the block completes
- the next line continues from the original incoming spine subject; if later lines should continue from one sibling result, choose it explicitly with an ordinary `|>` line

`&|>` is not sequential bind. It is for independent sibling work from shared input, and `@concurrent` only changes scheduling.

Scheduling rule:

- `@concurrent n` on the first line of the block limits runtime parallelism for that block

### 7.8 `*|>` — Fan-out / generator

Per-item spine block over an iterable, producing a normal list result.

```aivi
users
   *|> _
    |> normalizeUser
    |> saveUser
   *-|
    |> map .id
    |> toSet
```

Semantics:

- the right-hand expression must evaluate to an iterable
- each item becomes the spine subject of the fan-out body between `*|>` and the matching `*-|`
- the fan-out body is evaluated once per item
- the block yields a normal list of per-item outputs
- any later shaping such as `map`, `filter`, `partition`, `groupBy`, `toSet`, or `fold` is done with ordinary pipes and functions after `*-|` rejoins the outer spine

Generator rule:

- inside a `*|>` fan-out body, a `>|>` line without `or fail ...` means “skip this item if false”
- this is the direct flat-flow replacement for generator guards such as `x -> pred`

Concurrency rule:

- `@concurrent n` on the `*|>` line limits the number of item fan-out bodies that may run at once

This is intentionally a control-flow construct, not a second collection DSL.

### 7.9 Carrier-specific sibling behavior

Independent sibling steps over the same incoming spine subject.

```aivi
input
   &|> validateName #name
   &|> validateEmail #email
   &|> validatePassword #password
    |> createUser { name, email, password }
```

Semantics:

- all contiguous `&|>` lines form one sibling block
- every line receives the same incoming spine subject
- every line must yield the same applicative carrier shape `F A`
- lines must be independent; no line in the block may depend on a binding introduced by another line in the same block
- successful bindings become available together after the block completes
- the next line continues from the original incoming spine subject; use an ordinary `|> boundName` line when later steps should continue from a bound applicative result

Carrier-specific behavior:

- for `Validation E A`, failures accumulate according to the validation carrier
- for `Result E A`, `Option A`, and similar carriers, failure behavior follows the carrier’s applicative instance
- for `Effect E A`, the block is still applicative rather than monadic; runtime concurrency is controlled separately by `@concurrent`

Default continuation rule:

- after a successful applicative block, the original incoming spine subject continues
- use an ordinary `|> name` line when you want to continue from one of the applicative bindings

This is the flat replacement for `do Applicative`, including but not limited to validation.

### 7.10 `@|>` — Anchor

Named restart marker for recursion.

```aivi
request
  @|> retryAnchor
   |> pause 500ms
   |> retryRequest
```

Semantics:

- marks a named location in the spine
- does not transform the subject
- may be targeted by `recurse`

This aligns with current local `loop`/`recurse` support.

---

## 8. Modifiers

v2.0 defines five flow-line modifiers:

- `@timeout <duration>`
- `@delay <duration>`
- `@concurrent <int>`
- `@retry <count>x <interval> [exp]`
- `@cleanup <expression>`

This reuses existing duration literal conventions such as `30s` and `10min`.

### 8.1 `@timeout <duration>`

Limits the execution time of the annotated line.

```aivi
url
   |> http.get @timeout 5s
```

Semantics:

- starts when the line begins execution
- applies only to that line’s own execution
- fails if execution exceeds the duration

### 8.2 `@delay <duration>`

Waits before starting the annotated line.

```aivi
request
   ?|> retryPayment @delay 250ms
```

Semantics:

- inserts a delay before first execution of the line
- does not change the incoming spine subject
- does not add delay between retries; retry delays are controlled by `@retry`

### 8.3 `@concurrent <int>`

Limits runtime parallelism for a multi-child flow construct.

```aivi
users
   *|> _ @concurrent 8
    |> normalizeAndSaveUser
   *-|
```

```aivi
request
   &|> loadProfile @concurrent 8 #profile
   &|> loadPermissions #perms
   &|> loadTeams #teams
```

Semantics:

- `n` must be a positive integer
- on `*|>`, it limits the number of per-item fan-out bodies running at once
- on the first line of a contiguous `&|>` block whose carrier/runtime supports parallel evaluation, it limits parallel evaluation for that block
- specifying `@concurrent` on a later line of an already-started contiguous sibling block is a static error
- `@concurrent 1` means explicitly sequential scheduling

### 8.4 `@retry <count>x <interval> [exp]`

Retries the annotated fallible line before propagating failure to following `!|>` recovery arms.

Canonical form:

```aivi
?|> http.get url @retry 5x 1s
```

With exponential backoff:

```aivi
?|> http.get url @retry 5x 1s exp
```

Parameters:

- `<count>` — total number of attempts, including the first attempt
- `<interval>` — base delay between attempts
- `[exp]` — optional keyword controlling backoff behavior:
  - omitted means constant delay
  - `exp` means exponential backoff from the base interval

Examples:

```aivi
?|> fetchProfile userId @retry 3x 250ms
```

```aivi
?|> chargeCard request @retry 5x 1s exp
```

Semantics:

- the line executes once and may be retried until the total number of attempts reaches `<count>`
- retries occur only when the line fails
- with constant retry, each retry waits the same `<interval>`
- with exponential retry, delays grow from the base interval on each retry
- if all attempts fail, the terminal failure is passed to the following contiguous `!|>` recovery block
- `@retry` does not perform failure matching or recovery itself
- failure selection remains the responsibility of `!|>` arms, using the same matching rules as any other `?|>` failure

Delay schedule:

- `@retry 5x 1s` means attempts are separated by `1s`, `1s`, `1s`, `1s`
- `@retry 5x 1s exp` means attempts are separated by `1s`, `2s`, `4s`, `8s`

Interaction rules:

- `@retry` is only valid on fallible lines
- if `@timeout` is also present, the timeout applies to each individual attempt
- if `@delay` is also present, `@delay` runs once before the first attempt; retry delays apply only between later attempts
- if the first attempt succeeds, no retry delay is incurred
- `!|>` arms run only after retry exhaustion, and they see only the final failure

### 8.5 `@cleanup <expression>`

Registers cleanup for the successful result of the annotated line.

```aivi
path
   |> file.open @cleanup file.close #handle
   |> file.readAll handle
```

Semantics:

- cleanup is registered only if the line succeeds
- cleanup runs on scope exit, failure, or cancellation
- multiple cleanups run in LIFO order
- the cleanup expression receives the successful line result as its final argument
- cleanup registration does not change the spine subject

Examples:

```aivi
path
   |> file.open @cleanup file.close #handle
```

```aivi
socketSpec
   |> tcp.connect @cleanup closeConnection #conn
```

```aivi
listenerSpec
   |> sockets.listen @cleanup (_.shutdown Graceful) #listener
```

### 8.6 Combined behavior

If a line uses `@delay`, `@retry`, and `@timeout`:

```aivi
request
   ?|> riskyCall
       @delay 250ms
       @retry 5x 500ms exp
       @timeout 3s
```

the meaning is:

- wait `250ms` before the first attempt
- run the line with a `3s` timeout
- on failure, wait according to the retry schedule
- retry with the same per-attempt timeout
- after final failure, expose the terminal failure to following `!|>` arms

The timeout budget is per attempt, not whole-line wall-clock time.

### 8.7 Modifier position and parser rule

Flow-line modifiers are distinct from declaration decorators because they appear in flow-line modifier position, not declaration position.

---

## 9. Subflows

Subflows on the right-hand side of `||>` and `!|>` may be written either inline or as parent-scope helpers.

### 9.1 Inline subflow

Use inline form when the handler remains a single readable expression.

```aivi
payload
   ||> "invoice.paid" => _.data.object |> fulfillSubscription
```

### 9.2 Parent-scope helper

Use parent-scope delegation when the handler needs several steps, local bindings, or reuse.

```aivi
payload
   ||> "invoice.payment_failed" => handleInvoiceFailed payload
```

Normative rule:

> Nested branch and catch blocks are not required in v2. A subflow may stay on one line if it is readable as a single expression; otherwise it should move to parent scope.

---

## 10. Scoping

Bindings follow ordinary immutable lexical scope.

Rules:

- `#name` binds the successful unwrapped result of a line
- bindings are visible to later lines in the same flow
- delegated branch and recovery helpers do not receive flow bindings implicitly; values must be passed or captured normally
- sibling bindings from `&|>` become available together after their contiguous block completes
- per-item bindings inside `*|>` are local to the fan-out body; the overall block yields a list result
- cleanup registrations live for the enclosing flow scope and unwind when that scope exits

---

## 11. Restart and recursion

A flow may restart from an anchor using `recurse`.

Recommended forms:

```aivi
recurse retryAnchor
recurse retryAnchor nextValue
```

Meaning:

- without an explicit value, restart from the anchor with the current spine subject
- with an explicit value, restart with the supplied subject

This is the flat-flow equivalent of current local recursion support in effect and generator blocks.

---

## 12. Applicative cases

`&|>` is the generic replacement for `do Applicative`, not only a validation-only form.

Examples:

### 12.1 Validation accumulation

```aivi
input
   &|> validateName #name
   &|> validateEmail #email
   &|> validatePassword #password
    |> createUser { name, email, password }
```

### 12.2 Independent parsing in `Result`

```aivi
query
   &|> query.get "page" |> parseInt #page
   &|> query.get "size" |> parseInt #size
   &|> query.get "sort" |> parseSort #sort
    |> { page, size, sort }
```

### 12.3 Independent effectful fetches

```aivi
request
   &|> loadProfile request.userId #profile
   &|> loadPermissions request.userId #permissions
   &|> loadTeams request.userId #teams
    |> assembleDashboard { profile, permissions, teams }
```

The block remains applicative even when the runtime can evaluate lines concurrently. Dependency edges inside the block are not allowed.

---

## 13. Replacing generators and resources

### 13.1 Generator replacement

`*|>` plus ordinary pipes replaces `generate { ... }`.

Current generator shape:

```aivi
generate {
  x <- [1 .. 10]
  x -> x % 2 == 0
  yield x * 2
}
```

Flat replacement:

```aivi
[1 .. 10]
   *|> _
   >|> _ % 2 == 0
    |> _ * 2
   *-|
```

### 13.2 Resource replacement

A line with `@cleanup` replaces `resource { ... }` acquisition and release.

Current resource shape:

```aivi
resource {
  handle <- file.open path
  yield handle
  file.close handle
}
```

Flat replacement:

```aivi
path
   |> file.open @cleanup file.close #handle
```

Multiple cleanup registrations unwind in reverse acquisition order.

---

## 14. Events and hosted runtimes

A hosted runtime event pump such as GTK’s outer main loop is not modeled as a flow operator.

The outer application loop remains a runtime boundary, for example:

```aivi
main =
  buildApp
    |> gtk.run
```

Flow syntax replaces handler internals and event bodies, not the foreign runtime’s scheduling loop.

`do Event { ... }` is replaced by a standard library lift from a flow-shaped handler into an event handle, for example `event.from`.

Illustrative form:

```aivi
saveClicked =
  event.from (payload =>
    payload
       |> draftFrom
      &|> validateTitle #title
      &|> validateEmail #email
       |> api.saveUser { title, email }
  )
```

GTK callback positions such as `onClick`, `onActivate`, `onInput`, and related payload-oriented handlers may reference plain functions whose bodies are written in flat flow syntax.

GTK main app loop rule:

- the GTK main loop is owned by GTK and is not expressed with `@|>` or `recurse`
- flow syntax is for handler bodies and helper functions invoked from GTK callbacks

---

## 15. Tests and mocks

`@test` remains a declaration decorator rather than a flow-line modifier.

A test body may be any ordinary expression, including a flat flow expression.

`mock qualified.path = replacement in expr` remains the scoped mocking form.
The `in` target is any expression and commonly a flow root.

Examples:

```aivi
@test "fetchUsers returns mocked data"
fetchUsersTest =
  mock rest.get = _ => pure [{ id: 1, name: "Ada" }]
  in
    "/users"
       |> fetchUsersFrom
      >|> length == 1 or fail (AssertEq 1 (length _))
```

```aivi
@test "snapshot"
snapshotTest =
  mock snapshot "./__snapshots__"
  in
    input
       |> renderView
       ~|> assertSnapshot "rendered-view"
```

Rules:

- `@test` stays a declaration-level decorator
- `mock` remains scoped to the expression after `in`
- nested mocks shadow outer mocks
- deep calls inside the mocked expression see the mock
- flow syntax does not change existing mock resolution rules; it only changes the typical body shape

---

## 16. Static restrictions

The following restrictions apply in v2.0:

- `#name!` is removed; continuation subject selection must be expressed with ordinary flow lines
- every `*|>` body must end with a matching `*-|` at the same spine alignment
- `!|>` must immediately follow a `?|>` region
- `||>` and `!|>` blocks are contiguous
- `&|>` lines must all yield the same applicative carrier shape
- `&|>` lines must be independent of one another
- `*|>` yields a normal list value and does not define its own collection DSL
- inside `*|>`, `>|>` without `or fail ...` means item-skip rather than whole-flow failure
- `@|>` is marker-only and cannot bind
- `@cleanup` registers only on successful line completion
- cleanup registrations unwind in reverse registration order
- `@retry` is only valid on lines whose result carrier can represent failure
- `@concurrent` is only valid on `*|>`, or on the first line of a contiguous `&|>` block
- `@delay` applies before the first execution of a line, not between retries
- ordinary `|>` examples and emitted formatting should preserve the required leading space for spine alignment

---

## 17. Desugaring model

The spec does not require a particular compiler implementation, but the intended reading is:

- `|>` desugars to ordinary pipe-based map/bind behavior
- `>|>` desugars to guard/precondition logic
- `?|>` / `!|>` desugar to `attempt` plus pattern-based recovery
- `*|>` desugars to per-item mapping of a fan-out body that yields a list
- `&|>` desugars to applicative composition over shared input
- `@cleanup` desugars to scope-registered finalization in the same spirit as resource blocks
- `@retry` desugars to a retry loop with policy-controlled delay
- `@|>` plus `recurse` desugar to local loop/restart structure
- event helpers such as `event.from` desugar to runtime-specific event handle construction

This keeps the surface syntax new while keeping the underlying semantics close to existing AIVI constructs.

---

## 18. Boundary between control flow and data shaping

AIVI v2 Flow Syntax draws a hard line:

- operators define control flow
- ordinary functions define data shaping

So:

- `*|>`, `?|>`, `!|>`, `||>`, `>|>`, `~|>`, `&|>`, and `@|>` are execution structure
- `@timeout`, `@delay`, `@concurrent`, `@retry`, and `@cleanup` are line modifiers on that structure
- `map`, `filter`, `find`, `partition`, `groupBy`, `toSet`, `fold`, and similar helpers remain ordinary functions
- event lifting such as `event.from` is a library boundary, not a new control-flow operator

This avoids embedding a second list-processing or event-processing language inside flow syntax.

---

## 19. Examples

### 19.1 OAuth with concurrent siblings

```aivi
handleGithubAuth = authCode =>
  authCode
     |> github.oauth.exchangeToken @timeout 5s #token
    >|> token.isValid or fail InvalidGrant
    &|> github.api.getUser token.accessToken @timeout 2s #profile
    &|> github.api.getEmails token.accessToken @timeout 2s #emails
     |> emails
     |> find (.primary)
    >|> .verified or fail UnverifiedEmail
    ~|> log "User {profile.login} authenticated"
     |> { ...profile, email: .email, token: token.accessToken }
```

### 19.2 Checkout with retry and recovery

```aivi
processCheckout = request =>
  request
     |> stripe.customers.create { email: request.email } @timeout 5s #customer
     |> request.cart.total
    ?|> stripe.paymentIntents.create { amount: _, customer: customer.id }
        @delay 250ms
        @retry 5x 500ms exp
        @timeout 8s
    !|> CardDeclined e   => handleDeclinedCard e
    !|> RateLimitError _ => handleRateLimit request
    ~|> metrics.increment "checkout_success"
     |> sendOrderConfirmation

handleDeclinedCard = error =>
  error
     |> formatDeclineMessage
     |> sendFailureEmail
     |> fail PaymentDeclined

handleRateLimit = request =>
  request
    @|> retryAnchor
     |> pause 500ms
     |> processCheckout
```

### 19.3 Branching with inline and delegated subflows

```aivi
handleStripeWebhook = signature payload =>
  payload
     |> stripe.webhooks.verify signature
    ||> "invoice.paid"           => _.data.object |> fulfillSubscription
    ||> "invoice.payment_failed" => handleInvoiceFailed payload
    ||> _                        => markUnhandledEvent payload

handleInvoiceFailed = payload =>
  payload.data.object
     |> suspendService
     |> sendDunningEmail
```

### 19.4 Fan-out with ordinary list shaping after rejoin

```aivi
cancelStaleSubscriptions = customerId =>
  customerId
     |> stripe.subscriptions.list { status: "past_due" }
    *|> .data @concurrent 8
    ?|> stripe.subscriptions.cancel (.id)
    !|> _ => pure None
    *-|
     |> filter isSome
     |> map unwrap
     |> map .id
     |> toSet
```

### 19.5 Flat validation

```aivi
registerUser = input =>
  input
    &|> validateName #name
    &|> validateEmail #email
    &|> validatePassword #password
     |> createUser { name, email, password }
```

### 19.6 Resource cleanup

```aivi
readConfig = path =>
  path
     |> file.open @cleanup file.close #handle
     |> file.readAll handle
     |> decodeConfig
```

### 19.7 Generator replacement

```aivi
evenSquares = max =>
  [1 .. max]
    *|> _ @concurrent 8
    >|> _ % 2 == 0
     |> _ * _
    *-|
```

### 19.8 Event handler body

```aivi
saveClicked =
  event.from (payload =>
    payload
       |> draftFrom
      &|> validateTitle #title
      &|> validateEmail #email
       |> api.saveUser { title, email }
  )
```

---

## 20. Normative summary

AIVI v2 Flow Syntax is a flat, spine-aligned syntax for sequential and lightly branching flows. Ordinary flow lines have the form `[operator] [expression] [@modifier ...] [#binding]`, and `*-|` closes the nearest open `*|>` fan-out body. The core operators are `@|>`, `>|>`, `!|>`, `*|>`, `*-|`, `?|>`, `||>`, `~|>`, `|>`, and `&|>`. The ordinary flow operator must be rendered with one leading space so that its arrow aligns with the rest of the spine.

`*|>` expresses a per-item fan-out body delimited by `*-|` and yields a normal list.
`&|>` expresses independent sibling work over shared input; carrier type determines how sibling results combine.
`?|>` and `!|>` express attempt and recovery.
`||>` expresses value branching.
`>|>` expresses guarding.
`~|>` expresses side-effect observation without subject change.
`@|>` expresses restart anchors.

v2.0 defines five flow-line modifiers: `@timeout <duration>`, `@delay <duration>`, `@concurrent <int>`, `@retry <count>x <interval> [exp]`, and `@cleanup <expression>`.

Subflows may remain inline on one line or be extracted to parent scope. Data shaping remains in ordinary functions and pipes. Tests keep `@test` and `mock ... in ...`. Hosted runtimes such as GTK keep their own outer app loop; flow syntax replaces handler bodies rather than the runtime pump itself.

The intended language-surface result is that ordinary user code no longer needs `do Effect`, `do M`, `do Applicative`, `generate`, `resource`, or `do Event` blocks.
