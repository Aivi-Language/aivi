pub const MODULE_NAME: &str = "aivi.chronos.calendar";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.chronos.calendar
export Date, DateTime, EndOfMonth
export isLeapYear, daysInMonth, endOfMonth
export addDays, addMonths, addYears, negateDelta
export now
export domain Calendar

use aivi.calendar (Date, DateTime, EndOfMonth, isLeapYear, daysInMonth, endOfMonth, addDays, addMonths, addYears, negateDelta, now)
use aivi.calendar (domain Calendar)
"#;
