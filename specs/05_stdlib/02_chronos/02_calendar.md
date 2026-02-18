# Calendar Domain

<!-- quick-info: {"kind":"module","name":"aivi.chronos.calendar"} -->
The `Calendar` domain gives you robust tools for handling **Dates** and **Human Time**.

Handling time is deceptively hard. Ideally, a day is 24 hours. In reality, months have 28-31 days, years have 365 or 366 days, and timezones shift clocks back and forth.

The `Calendar` domain hides this chaos. Writing `timestamp + 86400` works until a leap second deletes your data. This domain ensures that when you say "Next Month," it handles the math correctly—whether it's February or July—making your scheduling logic reliable and legible.

<!-- /quick-info -->
## Overview

<<< ../../snippets/from_md/05_stdlib/02_chronos/02_calendar/block_01.aivi{aivi}

## Features

<<< ../../snippets/from_md/05_stdlib/02_chronos/02_calendar/block_02.aivi{aivi}

## Domain Definition

<<< ../../snippets/from_md/05_stdlib/02_chronos/02_calendar/block_03.aivi{aivi}

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

<<< ../../snippets/from_md/05_stdlib/02_chronos/02_calendar/block_04.aivi{aivi}
