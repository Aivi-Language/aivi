# AIVI Zed Extension

Syntax highlighting, document outline, and a Dark Modern theme for the [AIVI language](https://github.com/Aivi-Language/aivi) in [Zed](https://zed.dev).

## Features

- **Syntax highlighting** — keywords, types, functions, strings, sigils, operators, decorators, units, and colour literals via tree-sitter.
- **Document outline** — module declarations, type definitions, type annotations, and value/function bindings surface in the outline panel.
- **Bracket matching** — `()`, `[]`, `{}`.
- **Smart indentation** — auto-indent inside `do`, `effect`, `generate`, and `resource` blocks.
- **AIVI Dark Modern theme** — a dark theme derived from the official AIVI VSCode theme.

## Structure

```
zed/
  extension.toml              — Zed extension manifest
  grammars/
    aivi/
      grammar.js              — Tree-sitter grammar source
      src/
        parser.c              — Generated tree-sitter parser (do not edit)
  languages/
    aivi/
      config.toml             — Language config (file extensions, comments, brackets)
      highlights.scm          — Syntax highlight queries
      brackets.scm            — Bracket pair queries
      outline.scm             — Symbol outline queries
      indents.scm             — Indentation queries
  themes/
    aivi-dark.json            — AIVI Dark Modern colour theme
```

## Installing from source

1. Clone the AIVI repository.
2. Open **Zed → Extensions → Install Dev Extension**.
3. Point it at the `zed/` directory in this repository.

## Updating the grammar

If the grammar source (`grammar.js`) is modified, regenerate the parser before committing:

```sh
cd zed/grammars/aivi
npm install
npx tree-sitter generate
```

The `extension.toml` `[grammars.aivi]` commit hash should then be updated to the new HEAD.

## License

MIT — see [LICENCE.md](../LICENCE.md).
