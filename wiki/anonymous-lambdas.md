# Anonymous lambdas

AIVI now supports anonymous lambda expressions in ordinary expression positions.

## Surface forms

- Explicit lambdas use `=>` with one or more named parameters: `coord => coord == cell`, `left right => left == right`.
- Unary shorthand lambdas reuse dot-rooted subject syntax for composed expressions: `. == cell`, `.score >= limit`.

The shorthand is intentionally narrow and always unary. Bare `.` and `.field` keep their old
ambient-subject meaning so existing pipes, patch selectors, patch bodies, unary-subject `func`
sugar, and `from` selectors do not silently change meaning.

## Lowering model

The frontend adds lambda nodes to CST and HIR, resolves parameter bindings normally, then hoists
resolved lambdas into synthetic hidden function items before later lowering/typechecking passes.

Captures are modeled as leading synthetic parameters plus partial application. That keeps later
compiler layers on existing callable-item machinery instead of introducing new closure/runtime
semantics.

## Parser boundary

Implicit shorthand parsing is disabled inside ambient-subject surfaces that already use `.`:

- unary-subject `func` bodies such as `func scoreLineFor = "Score: {.}"`
- selected-subject continuations
- `from` selector bodies such as `.score >= threshold`
- patch selector and patch instruction expressions
- pipe-stage ambient bodies

That keeps old subject-driven surfaces stable while still allowing standalone shorthand lambdas like
`any (. == cell) items`.
