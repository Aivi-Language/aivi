# Layout Domain

<!-- quick-info: {"kind":"module","name":"aivi.ui.layout"} -->
The `Layout` domain gives common UI units a real type, so values such as `10px`, `50%`, and `2rem` are more than just strings.

That makes layout code easier to read and helps the compiler catch unit mix-ups early.
<!-- /quick-info -->
<div class="import-badge">use aivi.ui.layout<span class="domain-badge">domain</span></div>

## Why this domain exists

In untyped UI systems, layout values often travel around as strings like `"12px"` or `"80%"`. That is flexible, but it is also easy to mix incompatible units or accidentally treat a count as a length.

`aivi.ui.layout` gives those values structure. Writing `100px` is not a magic string; it constructs a typed layout value that downstream code can inspect and render deliberately.

## Overview

<<< ../../snippets/from_md/stdlib/ui/layout/overview.aivi{aivi}

## Domain definition

If you want to see the concrete shapes behind the sigils, start here:

<<< ../../snippets/from_md/stdlib/ui/layout/domain_definition.aivi{aivi}

## Types

### Length

`Length` represents an absolute or relative CSS-like length.

| Constructor | Meaning |
| --- | --- |
| `Px Int` | Pixels |
| `Em Int` | Relative to the parent font size |
| `Rem Int` | Relative to the root font size |
| `Vh Int` | Percentage of viewport height |
| `Vw Int` | Percentage of viewport width |

### Percentage

`Percentage` represents a fractional value between 0 and 100.

| Constructor | Meaning |
| --- | --- |
| `Pct Int` | Percentage, for example `50%` → `Pct 50` |

### Underlying representation

<<< ../../snippets/from_md/stdlib/ui/layout/underlying_representation.aivi{aivi}

## Literal sigils

The domain provides these literal forms:

| Literal | Constructs |
| --- | --- |
| `1px` | `Px 1` |
| `1em` | `Em 1` |
| `1rem` | `Rem 1` |
| `1vh` | `Vh 1` |
| `1vw` | `Vw 1` |
| `1%` | `Pct 1` |

Numeric multipliers are applied automatically, so `100px` becomes `Px 100`.

In practice, the domain is most helpful when you pass layout values into style records, widget properties, or helper functions and want the code to stay self-explanatory.
