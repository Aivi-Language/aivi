# Layout Domain

<!-- quick-info: {"kind":"module","name":"aivi.ui.layout"} -->
The `Layout` domain provides type-safe units for UI dimensions.

This prevents mixing up "10 pixels" with "10 percent" or "10 apples".

<!-- /quick-info -->
<div class="import-badge">use aivi.ui.layout<span class="domain-badge">domain</span></div>

## Overview

<<< ../../snippets/from_md/stdlib/ui/layout/overview.aivi{aivi}

The layout domain uses AIVI's [domain](../../syntax/domains.md) system to give numeric literals physical meaning. Writing `100px` is not a magic string — it constructs a typed `Length (Px 100)` value that the compiler can check.

## Domain Definition

<<< ../../snippets/from_md/stdlib/ui/layout/domain_definition.aivi{aivi}

## Types

### Length

`Length` represents an absolute or relative CSS-like length.

| Constructor | Meaning |
| --- | --- |
| `Px Int` | Pixels |
| `Em Int` | Relative to parent font size |
| `Rem Int` | Relative to root font size |
| `Vh Int` | Viewport height percentage |
| `Vw Int` | Viewport width percentage |

### Percentage

`Percentage` represents a fractional value between 0 and 100.

| Constructor | Meaning |
| --- | --- |
| `Pct Int` | Percentage (e.g. `50%` → `Pct 50`) |

### Underlying representation

<<< ../../snippets/from_md/stdlib/ui/layout/underlying_representation.aivi{aivi}

## Sigils

The domain provides the following literal sigils:

| Literal | Constructs |
| --- | --- |
| `1px` | `Px 1` |
| `1em` | `Em 1` |
| `1rem` | `Rem 1` |
| `1vh` | `Vh 1` |
| `1vw` | `Vw 1` |
| `1%` | `Pct 1` |

Numeric multipliers are applied automatically: `100px` evaluates to `Px 100`.
