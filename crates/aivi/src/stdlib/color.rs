pub const MODULE_NAME: &str = "aivi.color";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.color
export Rgb, Hsl, Hex
export adjustLightness, adjustSaturation, adjustHue
export toRgb, toHsl, toHex
export negateDelta
export domain Color

use aivi

Rgb = { r: Int, g: Int, b: Int }
Hsl = { h: Float, s: Float, l: Float }
Hex = Text

adjustLightness : Rgb -> Int -> Rgb
adjustLightness = value amount => color.adjustLightness value amount

adjustSaturation : Rgb -> Int -> Rgb
adjustSaturation = value amount => color.adjustSaturation value amount

adjustHue : Rgb -> Int -> Rgb
adjustHue = value amount => color.adjustHue value amount

toRgb : Hsl -> Rgb
toRgb = value => color.toRgb value

toHsl : Rgb -> Hsl
toHsl = value => color.toHsl value

toHex : Rgb -> Hex
toHex = value => color.toHex value

negateDelta : Delta -> Delta
negateDelta = delta => delta ?
  | Lightness n => Lightness (-n)
  | Saturation n => Saturation (-n)
  | Hue n => Hue (-n)

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

  1l = Lightness 1
  1s = Saturation 1
  1h = Hue 1
  1r = { r: 1, g: 0, b: 0 }
  1g = { r: 0, g: 1, b: 0 }
  1b = { r: 0, g: 0, b: 1 }
}"#;
