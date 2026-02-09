# Calendar Domain

Robust tools for handling **Dates** and **Human Time**.

## What is this?

Handling time is deceptively hard. Ideally, a day is 24 hours. In reality:
*   Months have 28, 29, 30, or 31 days.
*   Years have 365 or 366 days.
*   Timezones shift clocks back and forth.

The `Calendar` domain hides this chaos. It ensures that when you say "Next Month," it handles the math correctly, whether it's February or July.

## Why this exists

Writing `timestamp + 86400` to add a day works... until there's a leap second or daylight savings switch. Using a dedicated domain ensures your scheduling logic (e.g., "Meeting every Monday") is reliable and legible.

## Overview

```aivi
import aivi.std.chronos.calendar use { Date, DateTime }

let now = DateTime.now()

let birthday = ~d(1990-12-31)
let timestamp = ~dt(2025-02-08T12:34:56Z)

// "Human" math: Add 7 days, regardless of seconds
let next_week = now + 7`days`
```

## Features

```aivi
Date = { year: Int, month: Int, day: Int }

EndOfMonth = EndOfMonth
```

## Domain Definition

```aivi
domain Calendar over Date = {
  type Delta = Day Int | Month Int | Year Int | End EndOfMonth
  
  // Add delta to date
  (+) : Date -> Delta -> Date
  (+) date (Day n)   = addDays date n
  (+) date (Month n) = addMonths date n
  (+) date (Year n)  = addYears date n
  (+) date End       = endOfMonth date
  
  // Subtract delta from date
  (-) : Date -> Delta -> Date
  (-) date delta = date + (negateDelta delta)
  
  // Delta literals
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
daysInMonth { year, month } =
  month ?
  | 2  => if isLeapYear { year } then 29 else 28
  | 4  => 30
  | 6  => 30
  | 9  => 30
  | 11 => 30
  | _  => 31

endOfMonth : Date -> Date
endOfMonth date = date <| { day: daysInMonth date }

addDays : Date -> Int -> Date
addDays date n = // normalize day overflow/underflow

addMonths : Date -> Int -> Date
addMonths date n = // normalize month overflow, clamp days

addYears : Date -> Int -> Date
addYears { year, month, day } n = { year: year + n, month, day }

negateDelta : Delta -> Delta
negateDelta (Day n)   = Day (-n)
negateDelta (Month n) = Month (-n)
negateDelta (Year n)  = Year (-n)
negateDelta End       = End  // idempotent
```

## Usage Examples

```aivi
use aivi.std.calendar

today = { year: 2025, month: 2, day: 8 }

tomorrow = today + 1d
nextMonth = today + 1m
lastYear = today - 1y
monthEnd = today + eom
```
