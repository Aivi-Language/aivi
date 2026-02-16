pub const MODULE_NAME: &str = "aivi.duration";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.duration
export Span, negateDelta
export domain Duration

use aivi

Span = { millis: Int }

negateDelta : Delta -> Delta
negateDelta = delta => delta ?
  | Millisecond n => Millisecond (-n)
  | Second n => Second (-n)
  | Minute n => Minute (-n)
  | Hour n => Hour (-n)

domain Duration over Span = {
  Delta = Millisecond Int | Second Int | Minute Int | Hour Int

  (+) : Span -> Delta -> Span
  (+) = span (Millisecond n) => { millis: span.millis + n }
  (+) = span (Second n) => { millis: span.millis + n * 1000 }
  (+) = span (Minute n) => { millis: span.millis + n * 60000 }
  (+) = span (Hour n) => { millis: span.millis + n * 3600000 }

  (-) : Span -> Delta -> Span
  (-) = span delta => span + (negateDelta delta)

  (+) : Span -> Span -> Span
  (+) = s1 s2 => { millis: s1.millis + s2.millis }

  1ms : Int -> Delta
  1s : Int -> Delta
  1min : Int -> Delta
  1h : Int -> Delta

  1ms = Millisecond 1
  1s = Second 1
  1min = Minute 1
  1h = Hour 1
}"#;
