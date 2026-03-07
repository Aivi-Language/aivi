# Doc Snippets

This folder contains canonical AIVI code snippets that the documentation can embed directly.

The goal is practical: write an example once, reuse it in the docs, and keep it close enough to the real language that it stays trustworthy.

## Why snippets exist

Using shared snippets helps in two ways:

- **Readers get realistic examples.** The code shown in the docs comes from dedicated snippet files instead of being copied by hand into many markdown pages.
- **Docs stay easier to maintain.** When an example changes, you update the snippet once instead of chasing duplicates.

## How snippets are used

Markdown pages include snippets with VitePress `<<<` includes.

```md
<<< ./snippets/example.aivi
```

That keeps longer examples readable in the docs while preserving a single source of truth for the code itself.

## Checking snippets

Use the existing snippet commands when you want to verify or normalize them locally:

```bash
pnpm -C specs snippets:check
pnpm -C specs snippets:fix
```

- `snippets:check` verifies snippet formatting and consistency.
- `snippets:fix` rewrites snippets into the expected form when possible.

## Configuration

See `specs/snippets/manifest.json` for per-snippet verification settings and other snippet metadata.
