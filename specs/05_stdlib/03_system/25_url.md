# URL Domain

<!-- quick-info: {"kind":"module","name":"aivi.url"} -->
The `Url` domain handles **Uniform Resource Locators** without the string-mashing headaches.

A URL isn't just text; it's a structured address with protocols, hosts, and queries. Concatenating strings to build URLs leads to bugs (missing `/`, double `?`, unescaped spaces). This domain treats URLs as safe, structured records, letting you modify protocols or add query parameters without breaking the address.

<!-- /quick-info -->
<div class="import-badge">use aivi.url<span class="domain-badge">domain</span></div>

## Module

<<< ../../snippets/from_md/05_stdlib/03_system/25_url/block_01.aivi{aivi}

## Types

<<< ../../snippets/from_md/05_stdlib/03_system/25_url/block_02.aivi{aivi}

## Domain Definition

<<< ../../snippets/from_md/05_stdlib/03_system/25_url/block_03.aivi{aivi}

## Helper Functions

| Function | Explanation |
| --- | --- |
| **parse** text<br><pre><code>`Text -> Result UrlError Url`</code></pre> | Converts a URL string into a structured `Url`. |
| **toString** url<br><pre><code>`Url -> Text`</code></pre> | Renders a `Url` back into its string form. |

## Usage Examples

<<< ../../snippets/from_md/05_stdlib/03_system/25_url/block_04.aivi{aivi}
