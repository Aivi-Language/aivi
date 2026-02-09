# AIVI (draft language spec)

> [!IMPORTANT]
> AIVI is a fictional language spec-in-progress that may or may not turn into a real thing.
> Feedback is welcome — especially from language experts who spot problems with the current definition.

AIVI is a high-integrity functional language aimed at WebAssembly. This repo primarily contains the **language specification** and standard-library sketches.

- Read the spec entrypoint: `specs/README.md`
- Browse the docs index: `specs/index.md`
- Build the docs site: `pnpm docs:dev` / `pnpm docs:build`

## Syntax sketch (very early)

The snippets below are written in AIVI syntax; GitHub highlighting is approximate.

```haskell
module demo.counter = {
  export Model, Msg, init, update, view
}

Model = { count: Int, step: Int }
Msg = Inc | Dec | SetStep Int

init : Model
init = { count: 0, step: 1 }

update : Msg -> Model -> Model
update msg model =
  msg ?
  | Inc        => model <= { count: _ + model.step }
  | Dec        => model <= { count: _ - model.step }
  | SetStep s  => model <= { step: _ <- s }

// Pipes and a few “ligature-friendly” operators: -> => |> ?? <= >= != && ||
renderCount = model =>
  model.count
    |> toText
    |> (t => "Count: " + t)

// Domain-directed deltas (examples from the spec’s stdlib ideas)
deadline = now + 2w + 3d
shade    = { r: 255, g: 85, b: 0 } + 10l - 30s
width    = 100%   // typed Style percentage delta
height   = 100svh // typed Style viewport delta
```

JSX literals are sugar for the `Html` [domain](https://mendrik.github.io/aivi/02_syntax/06_domains) and [JSX literals](https://mendrik.github.io/aivi/02_syntax/13_jsx_literals):

```tsx
use aivi.std.html

Header = title => <div class="header">
  <h1>{title}</h1>
</div>

Nav = links => <ul class="nav">
  {links |> map (l => <li><a href={l.url}>{l.label}</a></li>)}
</ul>
```

## Feedback

If you see type-soundness issues, unclear semantics, bad ergonomics, or “this desugaring can’t work” problems:

- Open an issue / PR with a minimal counterexample.
- Point to a specific page in `specs/` (or propose a rewrite).
