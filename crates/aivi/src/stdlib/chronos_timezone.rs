pub const MODULE_NAME: &str = "aivi.chronos.timezone";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.chronos.timezone
export TimeZone, ZonedDateTime
export domain TimeZone
export domain ZonedDateTime

use aivi
use aivi.chronos.calendar (DateTime)
use aivi.chronos.duration (Span)
use aivi.chronos.instant (Timestamp)

TimeZone = { id: Text }

ZonedDateTime = {
  dateTime: DateTime
  zone: TimeZone
  offset: Span
}

getOffset : TimeZone -> Timestamp -> Span
getOffset = zone instant => timezone.getOffset zone instant

toInstant : ZonedDateTime -> Timestamp
toInstant = zdt => timezone.toInstant zdt

atZone : ZonedDateTime -> TimeZone -> ZonedDateTime
atZone = zdt zone => timezone.atZone zdt zone

domain TimeZone over TimeZone = {
  getOffset = getOffset
}

domain ZonedDateTime over ZonedDateTime = {
  toInstant = toInstant
  atZone = atZone
}"#;
