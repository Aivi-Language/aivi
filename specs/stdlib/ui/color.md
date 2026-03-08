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

The module gives you three plain data shapes plus the `Color` domain:

<<< ../../snippets/from_md/stdlib/ui/color/overview.aivi{aivi}

- `Rgb` is the carrier type the domain operates on.
- `Hsl` is useful when you want hue, saturation, and lightness explicitly.
- `Hex` is just `Text`, typically produced by `toHex` for CSS or renderer interop.

Once the `Color` domain is in scope, suffixes such as `10l`, `30s`, `45h`, `20r`, `128g`, and `20b` become available. If you want the general rules behind domain-owned operators and suffix literals, see [Domains](/syntax/domains).

## Mental model

Reach for the HSL-style helpers when you want the code to read like a design decision: “lighter”, “more muted”, or “rotate toward another hue”.

Reach for raw channel edits when you need exact byte-like control instead. Channel literals such as `10r` and record patching such as `color <| { r: 30 }` use plain integer math, so they can go outside the usual `0..255` RGB range until you clamp or convert them.

The conversion helpers normalize values for you:

- `adjustLightness` and `adjustSaturation` move by whole-number percentage points through HSL space
- `adjustHue` rotates by degrees and wraps around the hue wheel
- `toHex` renders lowercase `#rrggbb` text and clamps out-of-range channels during rendering

## Common operations

These examples show the domain in everyday use:

```aivi
use aivi.color

primary = { r: 0, g: 123, b: 255 }

hover   = primary + 10l
pressed = primary - 8l
muted   = primary - 20s
accent  = primary + 30h
cssText = toHex accent
```

For direct channel edits, prefer record patching such as `color <| { r: 30 }` or channel literals such as `color + 10r` when you want precise low-level control. The delta helpers are most useful for perceptual adjustments like hue, saturation, and lightness.

## Optional local shorthand

If you prefer function-call style construction, define a tiny local helper in your own module:

<<< ../../snippets/from_md/stdlib/ui/color/short_constructor.aivi{aivi}

This helper is local code, not an export from `aivi.color`. The built-in representation is still the plain record form `{ r: ..., g: ..., b: ... }`.

## Under the hood

The core shapes look like this in the current implementation:

```aivi
Rgb = { r: Int, g: Int, b: Int }
Hsl = { h: Float, s: Float, l: Float }
Hex = Text

domain Color over Rgb = {
  Delta = Lightness Int | Saturation Int | Hue Int

  (+) : Rgb -> Delta -> Rgb
  (+) = col (Lightness n) => adjustLightness col n
  (+) = col (Saturation n) => adjustSaturation col n
  (+) = col (Hue n) => adjustHue col n

  (+) : Rgb -> Rgb -> Rgb
  (+) = left { r, g, b } => { r: left.r + r, g: left.g + g, b: left.b + b }

  (-) : Rgb -> Delta -> Rgb
  (-) = col delta => col + (negateDelta delta)
}
```

The literal templates live inside the domain: `1l`, `1s`, and `1h` create `Delta` values, while `1r`, `1g`, and `1b` create channel-only `Rgb` records. Importing only `Rgb` does not activate those suffixes; the `Color` domain itself must be in scope.

## Helper functions

| Function | What it helps with |
| --- | --- |
| **adjustLightness** color amount<br><code>Rgb -> Int -> Rgb</code> | Make a color lighter or darker by whole-number percentage points in HSL space. Negative amounts darken. |
| **adjustSaturation** color amount<br><code>Rgb -> Int -> Rgb</code> | Make a color more vivid or more muted without hand-editing the channels yourself. Negative amounts desaturate. |
| **adjustHue** color degrees<br><code>Rgb -> Int -> Rgb</code> | Rotate a color around the hue wheel by degrees. Useful when deriving related accents from one base color. |
| **toRgb** hsl<br><code>Hsl -> Rgb</code> | Convert an `Hsl` value into `Rgb` when another API expects RGB data. Saturation and lightness are clamped into the valid `0.0..1.0` range during conversion. |
| **toHsl** rgb<br><code>Rgb -> Hsl</code> | Convert `Rgb` into `Hsl` so you can work with hue, saturation, and lightness directly. |
| **toHex** rgb<br><code>Rgb -> Hex</code> | Render an RGB value as lowercase hex text such as `#4a90e2`. Channels outside `0..255` are clamped while rendering. |
| **negateDelta** delta<br><code>Delta -> Delta</code> | Reverse a `Lightness`, `Saturation`, or `Hue` delta so subtraction and undo operations stay readable. |

## Usage examples

The examples below show a good pattern for real projects: choose a base color once, then derive the variants you need for UI states. The final `forest` example uses channel literals for exact channel math; the earlier examples use HSL-style adjustments for more design-oriented changes.

<<< ../../snippets/from_md/stdlib/ui/color/usage_examples.aivi{aivi}
