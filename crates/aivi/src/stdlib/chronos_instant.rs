pub const MODULE_NAME: &str = "aivi.chronos.instant";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.chronos.instant
export Timestamp
export Instant

use aivi
use aivi.chronos.duration (Span)

Timestamp = DateTime

addMillis : Timestamp -> Int -> Timestamp
addMillis = value millis => instant.addMillis value millis

diffMillis : Timestamp -> Timestamp -> Int
diffMillis = left right => instant.diffMillis left right

domain Instant over Timestamp = {
  (<) : Timestamp -> Timestamp -> Bool
  (<) = left right => instant.compare left right < 0

  (<=) : Timestamp -> Timestamp -> Bool
  (<=) = left right => instant.compare left right <= 0

  (>) : Timestamp -> Timestamp -> Bool
  (>) = left right => instant.compare left right > 0

  (>=) : Timestamp -> Timestamp -> Bool
  (>=) = left right => instant.compare left right >= 0

  (+) : Timestamp -> Span -> Timestamp
  (+) = value span => addMillis value span.millis

  (-) : Timestamp -> Span -> Timestamp
  (-) = value span => addMillis value (-span.millis)

  (-) : Timestamp -> Timestamp -> Span
  (-) = left right => { millis: diffMillis left right }
}"#;
