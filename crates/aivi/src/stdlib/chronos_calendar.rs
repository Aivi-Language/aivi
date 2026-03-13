pub const MODULE_NAME: &str = "aivi.chronos.calendar";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.chronos.calendar
export Date, DateTime, EndOfMonth
export isLeapYear, daysInMonth, endOfMonth
export addDays, addMonths, addYears, negateDelta
export now, format
export Calendar

use aivi
use aivi.calendar (Date, DateTime, EndOfMonth)
use aivi.calendar (domain Calendar)

isLeapYear : Date -> Bool
isLeapYear = value => calendar.isLeapYear value

daysInMonth : Date -> Int
daysInMonth = value => calendar.daysInMonth value

endOfMonth : Date -> Date
endOfMonth = value => calendar.endOfMonth value

addDays : Date -> Int -> Date
addDays = value n => calendar.addDays value n

addMonths : Date -> Int -> Date
addMonths = value n => calendar.addMonths value n

addYears : Date -> Int -> Date
addYears = value n => calendar.addYears value n

negateDelta : Calendar.Delta -> Calendar.Delta
negateDelta = delta => calendar.negateDelta delta

now : Effect DateTime
now = clock.now Unit

format : Text -> Date -> Text
format = pattern value => calendar.format value pattern
"#;
