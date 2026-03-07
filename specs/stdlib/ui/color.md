# Color Domain

<!-- quick-info: {"kind":"module","name":"aivi.color"} -->
The `Color` domain helps you work with colors in ways that match how people usually think about them. Instead of manually tweaking raw RGB channels, you can adjust hue, saturation, and lightness while keeping the code readable.

That makes common UI tasks—hover states, disabled variants, theme palettes, and gentle emphasis changes—much easier to express.
<!-- /quick-info -->
<div class="import-badge">use aivi.color<span class="domain-badge">domain</span></div>

## What this domain is for

Use `aivi.color` when you want color operations to mean something at the design level:

- “make this button a little brighter on hover”,
- “shift the accent color toward blue”,
- “reuse one base color across several UI states”,
- “convert a color to hex before handing it to a renderer”.

Start with the overview snippet to see the basic shapes:

<<< ../../snippets/from_md/stdlib/ui/color/overview.aivi{aivi}

## Common operations

These examples show the domain in everyday use:

<<< ../../snippets/from_md/stdlib/ui/color/features.aivi{aivi}

For direct channel edits, prefer record patching such as `color <| { r: 30 }` when you want precise low-level control. The delta helpers are most useful for perceptual adjustments like hue, saturation, and lightness.

## Short constructor

If you already know the RGB channel values, `rgb` is the most direct way to build a color:

| Constructor | Type | Equivalent |
| --- | --- | --- |
| `rgb r g b` | `Int -> Int -> Int -> Rgb` | `{ r: r, g: g, b: b }` |

<<< ../../snippets/from_md/stdlib/ui/color/short_constructor.aivi{aivi}

## Under the hood

The domain definition shows the concrete data shapes behind the friendly helpers:

<<< ../../snippets/from_md/stdlib/ui/color/domain_definition.aivi{aivi}

## Helper functions

| Function | What it helps with |
| --- | --- |
| **adjustLightness** color amount<br><code>Rgb -> Int -> Rgb</code> | Make a color lighter or darker by a percentage amount. Handy for hover, pressed, and disabled states. |
| **adjustSaturation** color amount<br><code>Rgb -> Int -> Rgb</code> | Make a color more vivid or more muted without manually editing channels. |
| **adjustHue** color degrees<br><code>Rgb -> Int -> Rgb</code> | Rotate a color around the hue wheel. Useful when deriving related accents from one base color. |
| **toRgb** hsl<br><code>Hsl -> Rgb</code> | Convert an HSL value into RGB when another API expects RGB data. |
| **toHsl** rgb<br><code>Rgb -> Hsl</code> | Convert RGB into HSL so you can work with hue, saturation, and lightness directly. |
| **toHex** rgb<br><code>Rgb -> Hex</code> | Render an RGB value as a hex color string such as `#4a90e2`. |

## Usage examples

The examples below show a good pattern for real projects: choose a base color once, then derive the variants you need for UI states.

<<< ../../snippets/from_md/stdlib/ui/color/usage_examples.aivi{aivi}
