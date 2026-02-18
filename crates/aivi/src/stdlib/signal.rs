pub const MODULE_NAME: &str = "aivi.signal";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.signal
export Spectrum
export fft, ifft, windowHann, normalize
export domain Signal

use aivi
use aivi.number.complex (Complex)

Signal = { samples: List Float, rate: Float }
Spectrum = { bins: List Complex, rate: Float }

map : (A -> B) -> List A -> List B
map = f items => items match
  | [] => []
  | [x, ...xs] => [f x, ...map f xs]

zipWith : (A -> B -> C) -> List A -> List B -> List C
zipWith = f left right => (left, right) match
  | ([], _) => []
  | (_, []) => []
  | ([x, ...xs], [y, ...ys]) => [f x y, ...zipWith f xs ys]

add : Float -> Float -> Float
add = a b => a + b

domain Signal over Signal = {
  (+) : Signal -> Signal -> Signal
  (+) = a b => { samples: zipWith add a.samples b.samples, rate: a.rate }

  (*) : Signal -> Float -> Signal
  (*) = s k => { samples: map (_ * k) s.samples, rate: s.rate }
}

fft : Signal -> Spectrum
fft = sig => signal.fft sig

ifft : Spectrum -> Signal
ifft = spec => signal.ifft spec

windowHann : Signal -> Signal
windowHann = sig => signal.windowHann sig

normalize : Signal -> Signal
normalize = sig => signal.normalize sig
"#;
