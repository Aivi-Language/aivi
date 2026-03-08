# Layout Domain

<!-- quick-info: {"kind":"module","name":"aivi.ui.layout"} -->
The `Layout` domain gives common UI units a real type, so values such as `10px`, `50%`, and `2rem` are more than just strings.

That makes layout code easier to read and helps the compiler catch unit mix-ups early.
<!-- /quick-info -->
<div class="import-badge">use aivi.ui.layout<span class="domain-badge">domain</span></div>

Import `use aivi.ui.layout` for the exported types and constructors, and add `use aivi.ui.layout (domain Layout)` anywhere you want suffix literals such as `10px`, `2em`, and `50%` to be active. If you want the general rules behind domain-owned suffixes, see [Domains](/syntax/domains).

## Why this domain exists

In untyped UI systems, layout values often travel around as strings like `"12px"` or `"80%"`. That is flexible, but it is also easy to mix incompatible units or accidentally treat a count as a length.

`aivi.ui.layout` gives those values structure. Writing `100px` is not a magic string; it constructs a typed layout value that downstream code can inspect and render deliberately.

## Overview

```aivi
use aivi.ui.layout
use aivi.ui.layout (domain Layout)

width : Length
width = 100px

height : Percentage
height = 50%

gap : Length
gap = 2em
```

This is the usual pattern in UI code: keep constructors such as `Px` and `Pct` available for explicit values, but use the `Layout` domain when suffix forms make the intent easier to read.

## Domain definition

If you want to see the concrete shapes behind the sigils, start here:

```aivi
domain Layout over UnitVal = {
  Length = Px Int | Em Int | Rem Int | Vh Int | Vw Int
  Percentage = Pct Int

  1px  = Px 1
  1em  = Em 1
  1rem = Rem 1
  1vh  = Vh 1
  1vw  = Vw 1
  1%   = Pct 1
}
```

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

`Percentage` represents an integer percentage value such as `Pct 50` for `50%`. In practice, UI renderers treat it like a CSS percentage, so values usually follow the familiar `0..100` convention.

| Constructor | Meaning |
| --- | --- |
| `Pct Int` | Percentage, for example `50%` → `Pct 50` |

### Underlying representation

`Layout` is implemented over a small carrier record. Most code should treat that record as an internal detail and work with `Length`, `Percentage`, or the suffix forms instead.

```aivi
UnitVal = { val: Int }
```

## Literal sigils

When `domain Layout` is in scope, the module provides these literal forms:

| Literal | Constructs |
| --- | --- |
| `1px` | `Px 1` |
| `1em` | `Em 1` |
| `1rem` | `Rem 1` |
| `1vh` | `Vh 1` |
| `1vw` | `Vw 1` |
| `1%` | `Pct 1` |

Numeric multipliers are applied automatically, so `100px` becomes `Px 100`, `2em` becomes `Em 2`, and `50%` becomes `Pct 50`. The same rule also works for parenthesized expressions such as `(gapSize)px`; see [Operators and Context](/syntax/operators#115-units-suffix-literals-and-template-functions) for the general suffix-literal elaboration rules.

In practice, the domain is most helpful when you pass layout values into style records, widget properties, or helper functions and want the code to stay self-explanatory. For adjacent UI APIs, see [HTML](/stdlib/ui/html), [VDOM](/stdlib/ui/vdom), and [GTK4](/stdlib/ui/gtk4).
