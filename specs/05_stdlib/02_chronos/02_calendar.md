# Calendar Domain

The `Calendar` domain gives you robust tools for handling **Dates** and **Human Time**.

Handling time is deceptively hard. Ideally, a day is 24 hours. In reality, months have 28-31 days, years have 365 or 366 days, and timezones shift clocks back and forth.

The `Calendar` domain hides this chaos. Writing `timestamp + 86400` works until a leap second deletes your data. This domain ensures that when you say "Next Month," it handles the math correctly—whether it's February or July—making your scheduling logic reliable and legible.

## Overview

```aivi
use aivi.calendar (Date, DateTime)

now = DateTime.now()

birthday = ~d(1990-12-31)
timestamp = ~dt(2025-02-08T12:34:56Z)

// "Human" math: Add 7 days, regardless of seconds
next_week = now + 7days
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

| Function | Explanation |
| --- | --- |
| **isLeapYear** date<br><pre><code>`Date -> Bool`</code></pre> | Returns whether `date.year` is a leap year. |
| **daysInMonth** date<br><pre><code>`Date -> Int`</code></pre> | Returns the number of days in `date.month`. |
| **endOfMonth** date<br><pre><code>`Date -> Date`</code></pre> | Returns the last day of the month for `date`. |
| **addDays** date n<br><pre><code>`Date -> Int -> Date`</code></pre> | Applies a day delta with calendar normalization. |
| **addMonths** date n<br><pre><code>`Date -> Int -> Date`</code></pre> | Applies a month delta with normalization and day clamping. |
| **addYears** date n<br><pre><code>`Date -> Int -> Date`</code></pre> | Applies a year delta. |
| **negateDelta** delta<br><pre><code>`Delta -> Delta`</code></pre> | Returns the inverse delta (except `End`, which is idempotent). |

## Usage Examples

```aivi
use aivi.calendar

today = { year: 2025, month: 2, day: 8 }

tomorrow = today + 1d
nextMonth = today + 1m
lastYear = today - 1y
monthEnd = today + eom
```
