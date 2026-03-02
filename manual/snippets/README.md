# Doc Snippets

This folder holds canonical AIVI snippets that are embedded into the docs.

Goals:

- Docs never drift from the parser/formatter/compiler.
- Snippets are the source of truth; markdown embeds them rather than duplicating code.

Workflow:

- Embed snippets in markdown using VitePress `<<<` includes.
- Verify with `pnpm -C specs snippets:check` (CI) or `pnpm -C specs snippets:fix` (autofmt).

See `specs/snippets/manifest.json` for per-snippet verification configuration.

