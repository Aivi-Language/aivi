# URL Domain

<!-- quick-info: {"kind":"module","name":"aivi.url"} -->
The `Url` domain treats web addresses as structured values instead of plain text.

This helps you parse, inspect, and render URLs without relying on fragile string concatenation.

<!-- /quick-info -->
<div class="import-badge">use aivi.url<span class="domain-badge">domain</span></div>

## Module

<<< ../../snippets/from_md/stdlib/system/url/module.aivi{aivi}

## Types

<<< ../../snippets/from_md/stdlib/system/url/types.aivi{aivi}

## Domain Definition

<<< ../../snippets/from_md/stdlib/system/url/domain_definition.aivi{aivi}

## Why use structured URLs

URLs contain several parts—scheme, host, path, query, and sometimes a fragment.
Building them by hand as text often leads to small mistakes such as double slashes, missing separators, or unescaped values.
Working with a structured `Url` value makes those operations clearer.

## Helper functions

| Function | What it does |
| --- | --- |
| **parse** text<br><code>Text -> Result UrlError Url</code> | Parses a URL string into a structured `Url` value. |
| **toString** url<br><code>Url -> Text</code> | Renders a `Url` value back into its string form. |

## Common patterns

- Parse incoming URLs before you inspect or modify them.
- Keep values as `Url` while your program is transforming them.
- Render with `toString` only when you need to display or send the final address.

## Usage Examples

<<< ../../snippets/from_md/stdlib/system/url/usage_examples.aivi{aivi}
