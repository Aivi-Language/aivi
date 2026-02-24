pub const MODULE_NAME: &str = "aivi.number.complex";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.number.complex
export i
export Complex

use aivi

Complex = { re: Float, im: Float }

i : Complex
i = { re: 0.0, im: 1.0 }

domain Complex over Complex = {
  (+) : Complex -> Complex -> Complex
  (+) = a b => { re: a.re + b.re, im: a.im + b.im }

  (-) : Complex -> Complex -> Complex
  (-) = a b => { re: a.re - b.re, im: a.im - b.im }

  (*) : Complex -> Complex -> Complex
  (*) = a b => {
    re: a.re * b.re - a.im * b.im
    im: a.re * b.im + a.im * b.re
  }

  (/) : Complex -> Float -> Complex
  (/) = z s => { re: z.re / s, im: z.im / s }
}"#;
