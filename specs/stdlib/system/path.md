# Path Domain

<!-- quick-info: {"kind":"module","name":"aivi.path"} -->
The `Path` domain gives you a structured, purely lexical way to work with filesystem paths instead of building them by hand as raw text.

Use `~path[...]` for path literals and `/` for joining path values, so your code stays readable and avoids separator mistakes before you hand a path to filesystem APIs.
<!-- /quick-info -->
<div class="import-badge">use aivi.path<span class="domain-badge">domain</span></div>

## Module

<<< ../../snippets/from_md/stdlib/system/path/block_01.aivi{aivi}


## Types

<<< ../../snippets/from_md/stdlib/system/path/block_02.aivi{aivi}


`Path` is a normalized lexical value:

- `absolute` records whether the path starts at `/`.
- `segments` stores the normalized path components.
- `Path` itself does **not** check whether anything exists on disk and does not resolve symlinks.

## Domain Definition

<<< ../../snippets/from_md/stdlib/system/path/block_03.aivi{aivi}


`base / other` joins two `Path` values and then normalizes the result lexically.
If `other` is absolute, it replaces `base`; if `other` is relative, its segments are appended to `base`.

## Path literal syntax

`~path[...]` is a compiler-provided structured sigil. See also [Operators: Sigils](../../syntax/operators.md#118-sigils) and [Domains and sigils](../../syntax/domains.md#domains-and-sigils).

- `~path[/usr/local/bin]` creates an absolute path.
- `~path[src/main.aivi]` creates a relative path.
- `.` segments are removed, and `..` removes one previous segment when that is possible.
- For absolute paths, leading `..` segments are dropped at the root instead of escaping it lexically.
- `parse` and `~path[...]` are lexical helpers only: they normalize separators and segments, but they do not touch the filesystem.

## Why use `Path` instead of `Text`

Filesystem paths look simple, but string concatenation is easy to get wrong.
Missing separators, extra separators, or unnormalized `.` and `..` segments can lead to subtle bugs.
The `Path` domain keeps those operations explicit and composable.
Keep values as `Path` while you transform them, then render with `toString` only at an I/O boundary such as [`aivi.file`](./file.md).

## Helper functions

| Function | What it does |
| --- | --- |
| **parse** text<br><code>Text -> Path</code> | Parses text into a structured `Path`, trimming whitespace and normalizing separators and `.`/`..` segments lexically. |
| **toString** path<br><code>Path -> Text</code> | Renders a `Path` back to text using `/` separators. |
| **isAbsolute** path<br><code>Path -> Bool</code> | Returns `True` when the path starts at `/`. |
| **normalize** path<br><code>Path -> Path</code> | Removes `.` segments and resolves `..` where that is possible, without consulting the filesystem. |
| **parent** path<br><code>Path -> Option Path</code> | Returns the lexical parent directory, if one exists. `~path[/]` and `~path[.]` have no parent. |
| **fileName** path<br><code>Path -> Option Text</code> | Returns the last path segment, if there is one. |

## Common patterns

- Build paths from known pieces with literals and `/` instead of manual concatenation.
- Call `normalize` when you want a clean lexical shape before comparison or display.
- Use `parent` and `fileName` when you need to split a path into directory and leaf parts.
- Use `toString` only when another API really needs `Text`.

## Usage Examples

<<< ../../snippets/from_md/stdlib/system/path/block_04.aivi{aivi}


This example is easy to verify without touching the filesystem:

| Expression | Result |
| --- | --- |
| `toString root` | `"/usr/local"` |
| `toString bin` | `"/usr/local/bin"` |
| `cfgText` | `"config/app.json"` |
| `isAbsolute cfg` | `False` |
| `cfgParent` | `Some` path representing `config` |
| `cfgName` | `Some "app.json"` |
