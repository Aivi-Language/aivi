# URL Domain

<!-- quick-info: {"kind":"module","name":"aivi.url"} -->
The `Url` domain handles **Uniform Resource Locators** without the string-mashing headaches.

A URL isn't just text; it's a structured address with protocols, hosts, and queries. Concatenating strings to build URLs leads to bugs (missing `/`, double `?`, unescaped spaces). This domain treats URLs as safe, structured records, letting you modify protocols or add query parameters without breaking the address.

<!-- /quick-info -->
<div class="import-badge">use aivi.url<span class="domain-badge">domain</span></div>

## Module

<<< ../../snippets/from_md/stdlib/system/url/module.aivi{aivi}

## Types

<<< ../../snippets/from_md/stdlib/system/url/types.aivi{aivi}

## Domain Definition

<<< ../../snippets/from_md/stdlib/system/url/domain_definition.aivi{aivi}

## Helper Functions

| Function | Explanation |
| --- | --- |
| **parse** text<br><code>Text -> Result UrlError Url</code> | Converts a URL string into a structured `Url`. |
| **toString** url<br><code>Url -> Text</code> | Renders a `Url` back into its string form. |

## Usage Examples

<<< ../../snippets/from_md/stdlib/system/url/usage_examples.aivi{aivi}
