pub const MODULE_NAME: &str = "aivi.calendar";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.calendar
export Date, DateTime, EndOfMonth
export isLeapYear, daysInMonth, endOfMonth
export addDays, addMonths, addYears, negateDelta
export now
export Calendar

use aivi

Date = { year: Int, month: Int, day: Int }
EndOfMonth = EndOfMonth

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

negateDelta : Delta -> Delta
negateDelta = delta => delta match
  | Day n => Day (-n)
  | Month n => Month (-n)
  | Year n => Year (-n)
  | End => End

now : Effect DateTime
now = clock.now Unit

domain Calendar over Date = {
  Delta = Day Int | Month Int | Year Int | End EndOfMonth

  (+) : Date -> Delta -> Date
  (+) = date (Day n) => addDays date n
  (+) = date (Month n) => addMonths date n
  (+) = date (Year n) => addYears date n
  (+) = date End => endOfMonth date

  (-) : Date -> Delta -> Date
  (-) = date delta => date + (negateDelta delta)

  1d = Day 1
  1m = Month 1
  1y = Year 1
  eom = End
}"#;
