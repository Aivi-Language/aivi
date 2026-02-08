# Standard Library: Calendar Domain

## Module

```aivi
module aivi/std/calendar = {
  export domain Calendar
  export Date, Day, Month, Year, EndOfMonth
  export isLeapYear, daysInMonth, endOfMonth
}
```

## Types

```aivi
Date = { year: Int, month: Int, day: Int }

EndOfMonth = EndOfMonth
```

## Domain Definition

```aivi
domain Calendar over Date = {
  type Delta = Day Int | Month Int | Year Int | End EndOfMonth
  
  -- Add delta to date
  (+) : Date -> Delta -> Date
  (+) date (Day n)   = addDays date n
  (+) date (Month n) = addMonths date n
  (+) date (Year n)  = addYears date n
  (+) date End       = endOfMonth date
  
  -- Subtract delta from date
  (-) : Date -> Delta -> Date
  (-) date delta = date + (negateDelta delta)
  
  -- Delta literals
  1d = Day 1
  1m = Month 1
  1y = Year 1
  eom = End
}
```

## Helper Functions

```aivi
isLeapYear : Date -> Bool
isLeapYear { year } = 
  (year % 4 == 0) && ((year % 100 != 0) || (year % 400 == 0))

daysInMonth : Date -> Int
daysInMonth { year, month } = match month with
  | 2 -> if isLeapYear { year } then 29 else 28
  | 4 | 6 | 9 | 11 -> 30
  | _ -> 31

endOfMonth : Date -> Date
endOfMonth date = { date | day: daysInMonth date }

addDays : Date -> Int -> Date
addDays date n = -- normalize day overflow/underflow

addMonths : Date -> Int -> Date
addMonths date n = -- normalize month overflow, clamp days

addYears : Date -> Int -> Date
addYears { year, month, day } n = { year: year + n, month, day }

negateDelta : Delta -> Delta
negateDelta (Day n)   = Day (-n)
negateDelta (Month n) = Month (-n)
negateDelta (Year n)  = Year (-n)
negateDelta End       = End  -- idempotent
```

## Usage Examples

```aivi
use aivi/std/calendar

today = { year: 2025, month: 2, day: 8 }

tomorrow = today + 1d
nextMonth = today + 1m
lastYear = today - 1y
monthEnd = today + eom
```
