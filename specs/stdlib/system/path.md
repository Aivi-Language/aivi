# Path Domain

<!-- quick-info: {"kind":"module","name":"aivi.path"} -->
The `Path` domain provides a **structured filesystem path** type and safe, composable path operations.

Use `~path[...]` to build a `Path` literal without string-mashing separators, and `/` to join paths.
<!-- /quick-info -->
<div class="import-badge">use aivi.path<span class="domain-badge">domain</span></div>


## Module

<<< ../../snippets/from_md/stdlib/system/path/module.aivi{aivi}

## Types

<<< ../../snippets/from_md/stdlib/system/path/types.aivi{aivi}

## Domain Definition

<<< ../../snippets/from_md/stdlib/system/path/domain_definition.aivi{aivi}

## Helper Functions

| Function | Explanation |
| --- | --- |
| **parse** text<br><code>Text -> Path</code> | Parses and normalizes a textual path. |
| **toString** path<br><code>Path -> Text</code> | Renders a `Path` as text (using `/`). |
| **normalize** path<br><code>Path -> Path</code> | Removes `.` segments and resolves `..` where possible. |
| **parent** path<br><code>Path -> Option Path</code> | Returns the parent directory, if any. |
| **fileName** path<br><code>Path -> Option Text</code> | Returns the last segment, if any. |

## Usage Examples

<<< ../../snippets/from_md/stdlib/system/path/usage_examples.aivi{aivi}
