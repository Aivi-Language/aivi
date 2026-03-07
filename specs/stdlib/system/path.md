# Path Domain

<!-- quick-info: {"kind":"module","name":"aivi.path"} -->
The `Path` domain gives you a structured way to work with filesystem paths instead of building them by hand as raw text.

Use `~path[...]` for path literals and `/` for joining path segments, so your code stays readable and avoids separator mistakes.
<!-- /quick-info -->
<div class="import-badge">use aivi.path<span class="domain-badge">domain</span></div>

## Module

<<< ../../snippets/from_md/stdlib/system/path/module.aivi{aivi}

## Types

<<< ../../snippets/from_md/stdlib/system/path/types.aivi{aivi}

## Domain Definition

<<< ../../snippets/from_md/stdlib/system/path/domain_definition.aivi{aivi}

## Why use `Path` instead of `Text`

Filesystem paths look simple, but string concatenation is easy to get wrong.
Missing separators, extra separators, or unnormalized `.` and `..` segments can lead to subtle bugs.
The `Path` domain keeps those operations explicit and composable.

## Helper functions

| Function | What it does |
| --- | --- |
| **parse** text<br><code>Text -> Path</code> | Parses and normalizes a textual path into a structured `Path` value. |
| **toString** path<br><code>Path -> Text</code> | Renders a `Path` back to text using `/` separators. |
| **normalize** path<br><code>Path -> Path</code> | Removes `.` segments and resolves `..` where that is possible. |
| **parent** path<br><code>Path -> Option Path</code> | Returns the parent directory, if one exists. |
| **fileName** path<br><code>Path -> Option Text</code> | Returns the last path segment, if there is one. |

## Common patterns

- Build paths from known pieces with literals and `/` instead of manual concatenation.
- Call `normalize` when you want a clean canonical shape before comparison or display.
- Use `parent` and `fileName` when you need to split a path into directory and leaf parts.

## Usage Examples

<<< ../../snippets/from_md/stdlib/system/path/usage_examples.aivi{aivi}
