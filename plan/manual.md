# AIVI Documentation (`manual/`)

## Goals

Publish a VitePress 2.x documentation site that:
- Teaches AIVI to programmers who may not know functional programming
- Renders AIVI code with the same syntax highlighting as the VSCode extension
- Validates and formats every code example with real compiler tooling
- Deploys automatically to GitHub Pages on push to `main`
- Includes a browser playground (WASM) as a later milestone

---

## Directory layout

```
manual/
├── package.json                    # standalone pnpm project
├── pnpm-lock.yaml
├── .vitepress/
│   ├── config.ts                   # Shiki language/theme + nav + sidebar
│   └── theme/
│       ├── index.ts                # registers custom theme, VPTheme extension
│       └── custom.css              # AIVI brand overrides (fonts, colours)
├── index.md                        # landing page / quick glance
├── introduction.md                 # what AIVI is, who it is for, philosophy
├── tour/
│   ├── index.md                    # tour overview + reading guide
│   ├── 01-values-types.md
│   ├── 02-functions.md
│   ├── 03-pipes.md                 # pipe algebra — centrepiece of the tour
│   ├── 04-pattern-matching.md      # ||> destructuring
│   ├── 05-signals.md               # sig, recurrence (@|>...<|@)
│   ├── 06-sources.md               # @source, lifecycle, stale suppression
│   ├── 07-markup.md                # <label>, <each>, <match>
│   └── 08-typeclasses.md
├── aivi-way/
│   ├── index.md
│   ├── async-data.md               # source + Ok/Err pipes
│   ├── forms.md                    # validated input signals with ?|>
│   ├── state.md                    # local sig vs domain-level sig
│   ├── list-rendering.md           # <each> + fan-out *|>...<|*
│   └── error-handling.md           # Ok/Err pipe chains
├── stdlib/
│   └── index.md                    # placeholder; catalog populated as stdlib grows
├── playground/
│   └── index.md                    # browser WASM playground (see Milestone 3)
├── public/
│   └── logo.svg
└── scripts/
    ├── validate-examples.ts        # runs aivi check on every ```aivi block
    └── fmt-examples.ts             # round-trips every ```aivi block through aivi fmt
```

`manual/` lives at the repo root.  It is **not** part of the `tooling/` pnpm workspace — it
references grammar/theme assets from `tooling/packages/vscode-aivi/` by relative path only.

---

## Milestone 1 — Basic site with correct highlighting

### 1.1 VitePress setup

```
manual/
└── package.json   devDependencies: vitepress@^1.6.4 (latest stable; 2.0.0 not yet released), tsx
```

Single `pnpm install` inside `manual/`, then:

```json
"scripts": {
  "docs:dev":      "vitepress dev",
  "docs:build":    "tsx scripts/validate-examples.ts && vitepress build",
  "docs:preview":  "vitepress preview",
  "docs:check":    "tsx scripts/validate-examples.ts",
  "docs:fmt":      "tsx scripts/fmt-examples.ts"
}
```

### 1.2 Syntax highlighting — single source of truth

VitePress 2 ships Shiki 1.x internally.  Shiki accepts:
- `LanguageInput` — a TextMate grammar object
- `ThemeInput`    — a VSCode-format theme object

Both already exist in the VSCode extension.  The VitePress config references them directly:

```ts
// .vitepress/config.ts
import aiviGrammar from '../tooling/packages/vscode-aivi/syntaxes/aivi.tmLanguage.json'
import aiviDark    from '../tooling/packages/vscode-aivi/themes/aivi-dark-color-theme.json'
import aiviLight   from './theme/aivi-light-color-theme.json'   // see §1.3

export default defineConfig({
  markdown: {
    languages: [aiviGrammar as LanguageInput],
    theme: { dark: aiviDark as ThemeInput, light: aiviLight as ThemeInput }
  }
})
```

`aivi-dark-color-theme.json` already has `"name": "AIVI Dark"` and `"type": "dark"` at the top
level — it is Shiki-compatible with no wrapper or transformation needed.

No grammar or theme file is duplicated.  Updating either in the VSCode extension automatically
updates the docs on the next build.

### 1.3 Light theme — AIVI Light (Ghostty-inspired)

No "Ghostty light" theme is bundled in Shiki and none exists as a published community package.
The plan is to author `manual/.vitepress/theme/aivi-light-color-theme.json` by hand, mapping
Ghostty's light terminal palette (documented at ghostty.org) to VSCode/Shiki `tokenColors`.
Ghostty's light palette uses warm whites and desaturated accents that pair naturally with the
AIVI Dark token colours when inverted.

The light theme ships inside `manual/` (not inside `vscode-aivi/`) because the VSCode extension
only offers AIVI Dark.  If a light VSCode theme is ever wanted, the file can be moved to
`vscode-aivi/themes/` and referenced from both places.

### 1.4 Theme switcher

VitePress has a built-in dark/light toggle.  The two themes are declared in `config.ts` (see
§1.2).  Custom CSS variables in `custom.css` override VitePress brand colours so the entire
site chrome (sidebar, nav, code blocks) matches the AIVI palette in both modes.

---

## Milestone 2 — Code example validation

### 2.1 `validate-examples.ts`

1. Globs `**/*.md`, extracts every ` ```aivi ` fenced block (content + source location)
2. Writes each snippet to a temp file under `/tmp/aivi-doc-check/`
3. Runs `aivi check <file>` — any compiler error is reported as:
   ```
   ERROR  tour/03-pipes.md line 42: undefined name `foobar`
   ```
4. Runs `aivi fmt --check <file>` — any formatting divergence is reported similarly
5. Exits 1 if any check or fmt-check fails

When `aivi` is not on `$PATH` the script prints a warning and exits 0, so contributors
without a compiled binary can still run `docs:dev`.

### 2.2 `fmt-examples.ts`

Same extraction logic, but pipes each snippet through `aivi fmt --stdin` and writes the
formatted result back into the markdown file in-place.  Intended for use during authoring,
not in CI.

### 2.3 Markdown table escaping rules for pipe operators

AIVI's pipe operators (`|>`, `||>`, `?|>`, `T|>`, `F|>`, `*|>`, `<|*`, `@|>`, `<|@`) all
contain `|` which is the GFM table cell delimiter.

**Rules to follow in every documentation page:**

| Situation | What to write | Renders as |
|---|---|---|
| Pipe in a prose sentence | `\|>` | \|> |
| Pipe in a table cell | `\|>` (escape each `\|`) | \|> |
| Pipe as a column heading | `\|\|>` | \|\|> |
| Pipe inside inline code in a table | `` `\|>` `` — escape is inside the backticks | `\|>` |
| Pipe in a fenced code block | no escaping needed — fences are outside Markdown table parsing | raw |

The validation script should also catch unescaped pipes in table cells during a linting
pass (post-Milestone 2 stretch goal).

A VitePress custom container can be used for "operator reference" boxes to avoid tables
entirely when showing all pipe operators at once:

```md
::: details Pipe operator quick reference
| Operator | Reads as | …
:::
```

---

## Milestone 3 — GitHub Pages deployment

### 3.1 GitHub Actions workflow

`.github/workflows/docs.yml`:

```yaml
on:
  push:
    branches: [main]
    paths: ['manual/**', 'tooling/packages/vscode-aivi/syntaxes/**',
            'tooling/packages/vscode-aivi/themes/**']

jobs:
  build-and-deploy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: pnpm/action-setup@v3
        with: { version: 9 }
      - run: pnpm install
        working-directory: manual
      - run: cargo build --release         # produces aivi binary for validate-examples
      - run: cp target/release/aivi ~/.local/bin/aivi
      - run: pnpm docs:build
        working-directory: manual
      - uses: peaceiris/actions-gh-pages@v4
        with:
          github_token: ${{ secrets.GITHUB_TOKEN }}
          publish_dir: manual/.vitepress/dist
```

The workflow is path-filtered so doc-only changes do not trigger a Rust build.  If the
compiler is already cached (GitHub Actions cache on `Cargo.lock`) the Rust step is near-free.

### 3.2 `base` URL

VitePress needs `base: '/aivi/'` (or the repo name) set in `config.ts` for GitHub Pages
subdirectory hosting.  A `CNAME` file in `manual/public/` can point to a custom domain later.

---

## Milestone 4 — Browser playground (WASM)

### 4.1 Approach

Compile `aivi-cli` to `wasm32-unknown-unknown` (or `wasm32-wasip1`) using `wasm-bindgen`.
Expose two functions from a thin `aivi-playground` crate:

```rust
#[wasm_bindgen]
pub fn check(source: &str) -> JsValue   // returns Vec<Diagnostic> as JSON
#[wasm_bindgen]
pub fn format(source: &str) -> String   // returns formatted source
```

The playground page (`playground/index.md`) embeds a custom VitePress component:
- Code editor (CodeMirror 6 with the AIVI TextMate grammar via `@lezer` or raw CM extension)
- Live error squiggles from `check()` output
- Format button
- Example selector (populated from the `demos/` directory)

### 4.2 Build integration

WASM artefact is built in CI and committed to `manual/public/wasm/` or served from a CDN.
The playground page lazy-loads the WASM module so it does not affect initial page load.

### 4.3 Dependency

The playground depends on `wasm-bindgen`, `wasm-pack`, and the `web-sys` crate.  A new
`crates/aivi-playground/` crate wraps the public surface.  The `aivi-cli` crate itself is
not changed.

---

## Content plan

| Page | Core concept | Non-FP framing |
|---|---|---|
| Quick Glance (`index.md`) | everything at 30 s | "a desktop app language that reacts to the world" |
| Introduction | signals, reactivity, GTK | "like Excel formulas, not event loops" |
| Tour 01: Values & Types | `type`, `val`, `fun` | like TypeScript unions, but exhaustive and closed |
| Tour 02: Functions | labeled params `#x`, return type prefix | no positional ambiguity |
| Tour 03: Pipes | `\|>`, `?|>`, `T|>\`/`F|>` | like Unix pipes — data flows left to right |
| Tour 04: Pattern matching | `\|\|>` | a switch that checks shapes, not just values |
| Tour 05: Signals | `sig`, `@\|>...<\|@` | like React `useState`, but declarative |
| Tour 06: Sources | `@source`, `@recur.timer` | async/await where the language handles scheduling |
| Tour 07: Markup | `<label>`, `<each>`, `<match>` | like JSX, but for GTK |
| Tour 08: Type classes | `class`, `instance` | like TypeScript interfaces with laws |
| AIVI Way: Async data | source \+ `Ok`/`Err` pipes | loading states without callback hell |
| AIVI Way: Forms | signal \+ `?|>` gate | validated fields as filtered signals |
| AIVI Way: State | local `sig` vs `domain` | local state vs app-wide store |
| AIVI Way: Lists | `<each>` \+ `*\|>...<\|*` | list rendering that scales |
| AIVI Way: Errors | `Ok`/`Err` pipe chains | errors as values, not exceptions |

---

## Open items

- Choose a concrete Ghostty light palette mapping before authoring `aivi-light-color-theme.json`
  (Ghostty terminal palette is available at ghostty.org/docs/config/reference#palette)
- Decide whether the playground WASM build is part of the main CI job or a separate workflow
- Design the stdlib catalog page structure once the stdlib surface is known (see `plan/05-stdlib-docs.md`)
