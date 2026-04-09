# Recent surface syntax audit

This page summarizes the user-facing syntax work that landed on `main` over the last two days and
where that surface is now documented.

## Covered syntax changes

### `from` grouped derived-signal sugar

Top-level `from source = { ... }` groups several derived bindings behind one shared upstream signal.
The implementation lowers each entry to an ordinary derived binding fed by that shared source.

Primary user-facing docs:

- `manual/guide/signals.md` — grouped derivations with `from`
- `manual/guide/surface-feature-matrix.md` — feature status
- `AIVI_RFC.md` — top-level forms and signal rules
- `wiki/signal-model.md` — lowering summary and examples

### Closed-sum companion bodies

Brace-bodied closed sums colocate constructors with ordinary companion helpers. The current surface
accepts explicit receiver forms such as `name = self => ...`, receiver-only unary-subject sugar such
as `name = . ...`, and selected-subject continuations such as `name = self! |> ...`.

Primary user-facing docs:

- `manual/guide/types.md` — companion helper examples and selected-subject note
- `AIVI_RFC.md` — companion-body rules
- `wiki/type-system.md` — closed-sum companion summary

### Selected-subject function headers

`param!` and `param { path! }` let a `func` or companion member start immediately from one chosen
argument without writing an explicit `=>` head expression first.

Primary user-facing docs:

- `manual/guide/values-and-functions.md` — selected-subject header examples
- `manual/guide/record-patterns.md` — projection-rooted `{ path! }` form
- `manual/guide/types.md` — companion-member selected-subject note
- `AIVI_RFC.md` — function and companion header rules
- `wiki/pipe-algebra.md` — lowering/semantics summary

### Pipe memos `#name`

`#name` binds a stage input or stage result inside one ordinary pipe spine, including grouped branch
results.

Primary user-facing docs:

- `manual/guide/pipes.md` — memo forms and examples
- `manual/guide/values-and-functions.md` and `manual/guide/thinking-in-aivi.md` — practical usage
- `AIVI_RFC.md` — pipe memo rules
- `wiki/pipe-algebra.md` — memo semantics and grouped-branch behavior

### Temporal replay heads `|> delay` / `|> burst`

Temporal signal replays now use reserved `|>` stage heads with explicit duration/count syntax:
`|> delay <duration>` and `|> burst <duration> <count>`.

Primary user-facing docs:

- `manual/guide/pipes.md` and `manual/guide/signals.md` — temporal replay behavior
- `manual/guide/surface-feature-matrix.md` — implementation status
- `syntax.md` and `AIVI_RFC.md` — normative surface spelling
- `wiki/pipe-algebra.md` and `wiki/temporal-design.md` — implementation notes

## Notes

- The `fix(pipes): correct syntax for truthy/falsy pipe carriers in documentation` commit adjusted
  documentation only; it did not add a new surface form.
- The Reversi-focused cleanup commits exercised `#name` memos and selected-subject headers more
  broadly in demos, but they relied on the same surface already described above rather than adding
  new syntax.

## Sources

- `git log main --since='2 days ago'`
- `AIVI_RFC.md`
- `manual/guide/signals.md`
- `manual/guide/pipes.md`
- `manual/guide/types.md`
- `manual/guide/values-and-functions.md`
- `manual/guide/record-patterns.md`
- `manual/guide/surface-feature-matrix.md`
- `wiki/signal-model.md`
- `wiki/pipe-algebra.md`
- `wiki/type-system.md`
