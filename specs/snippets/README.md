# Doc Snippets

This folder holds canonical `.aivi` files that the docs can include directly.

Use snippets when an example is long enough to distract from the surrounding prose, when the same example appears in more than one place, or when you want the docs to reuse code that can be verified by the existing snippet tooling.

Most generated doc snippets live under `specs/snippets/from_md/`; hand-authored shared examples can live elsewhere in this folder as long as they are listed in the manifest.

## Why snippets exist

Shared snippets make the docs easier to trust and easier to maintain:

- **Readers see real AIVI code.** Examples come from dedicated `.aivi` files instead of being copied by hand into multiple markdown pages.
- **Docs stay in sync.** You update one snippet file instead of editing every page that mentions the example.
- **Snippet tooling can verify them.** Formatting, parsing, and typechecking can be driven from the snippet manifest instead of relying on manual review.

## How snippets are included in docs

Markdown pages in `specs/` include snippets with a block-level VitePress `<<<` include and the `{aivi}` language hint.

For example, from a page such as `specs/introduction.md`:

```md
<<< ./snippets/from_md/introduction/block_01.aivi{aivi}
```

Keep the include directive on its own line with blank lines around it so the markdown stays readable and consistent with the existing include-normalization tooling.

## Authoring and verification workflow

If you are drafting a new multi-line AIVI example inside a markdown page, start with a normal fenced block:

````md
```aivi
value = 41
next = value + 1
```
````

Then use the existing commands from the repo root:

```bash
pnpm -C specs snippets:extract:dry
pnpm -C specs snippets:extract
pnpm -C specs snippets:check
pnpm -C specs snippets:fix
```

- `snippets:extract:dry` previews which fenced ` ```aivi ` blocks would be moved into `specs/snippets/from_md/` and added to the manifest.
- `snippets:extract` performs that extraction and rewrites the markdown page to use `<<< ...{aivi}` includes.
- `snippets:check` runs manifest-driven verification. Depending on each manifest entry, that can include `fmt`, `parse`, and `check`.
- `snippets:fix` applies formatter output for snippets that fail `fmt`; parse or typecheck failures still need a manual fix.

If you only changed an existing snippet file, you usually need `snippets:check` and, if formatting drifted, `snippets:fix`.

## Configuration

[`manifest.json`](./manifest.json) is the source of truth for snippet metadata and verification settings.

Each entry declares the snippet path plus the verification context it needs, such as:

- `module` for the temporary verification harness module name
- `verify` for which checks to run (`fmt`, `parse`, `check`)
- `uses` or `prelude` when the harness needs extra imports or support code
- `stdlib` when a snippet should run without the embedded standard library
