use std::collections::HashMap;
use std::sync::Arc;

use chrono::{Datelike, Duration as ChronoDuration, NaiveDate};

use super::util::{builtin, expect_int};
use crate::runtime::{RuntimeError, Value};

pub(super) fn build_calendar_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "isLeapYear".to_string(),
        builtin("calendar.isLeapYear", 1, |mut args, _| {
            let date = date_from_value(args.pop().unwrap(), "calendar.isLeapYear")?;
            let year = date.year();
            let leap = (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0);
            Ok(Value::Bool(leap))
        }),
    );
    fields.insert(
        "daysInMonth".to_string(),
        builtin("calendar.daysInMonth", 1, |mut args, _| {
            let date = date_from_value(args.pop().unwrap(), "calendar.daysInMonth")?;
            Ok(Value::Int(
                days_in_month(date.year(), date.month(), "calendar.daysInMonth")? as i64,
            ))
        }),
    );
    fields.insert(
        "endOfMonth".to_string(),
        builtin("calendar.endOfMonth", 1, |mut args, _| {
            let date = date_from_value(args.pop().unwrap(), "calendar.endOfMonth")?;
            let max_day = days_in_month(date.year(), date.month(), "calendar.endOfMonth")?;
            let end = NaiveDate::from_ymd_opt(date.year(), date.month(), max_day).ok_or_else(
                || RuntimeError::InvalidArgument {
                    context: "calendar.endOfMonth".to_string(),
                    reason: "resulting date is invalid".to_string(),
                },
            )?;
            Ok(date_to_value(end))
        }),
    );
    fields.insert(
        "addDays".to_string(),
        builtin("calendar.addDays", 2, |mut args, _| {
            let days = expect_int(args.pop().unwrap(), "calendar.addDays")?;
            let date = date_from_value(args.pop().unwrap(), "calendar.addDays")?;
            let next = date
                .checked_add_signed(ChronoDuration::days(days))
                .ok_or_else(|| RuntimeError::Overflow { context: "calendar.addDays".to_string() })?;
            Ok(date_to_value(next))
        }),
    );
    fields.insert(
        "addMonths".to_string(),
        builtin("calendar.addMonths", 2, |mut args, _| {
            let months = expect_int(args.pop().unwrap(), "calendar.addMonths")?;
            let date = date_from_value(args.pop().unwrap(), "calendar.addMonths")?;
            let next = add_months(date, months, "calendar.addMonths")?;
            Ok(date_to_value(next))
        }),
    );
    fields.insert(
        "addYears".to_string(),
        builtin("calendar.addYears", 2, |mut args, _| {
            let years = expect_int(args.pop().unwrap(), "calendar.addYears")?;
            let date = date_from_value(args.pop().unwrap(), "calendar.addYears")?;
            let next = add_years(date, years, "calendar.addYears")?;
            Ok(date_to_value(next))
        }),
    );
    Value::Record(Arc::new(fields))
}
fn date_from_value(value: Value, ctx: &str) -> Result<NaiveDate, RuntimeError> {
    let Value::Record(fields) = value else {
        return Err(RuntimeError::TypeError {
            context: ctx.to_string(),
            expected: "Date".to_string(),
            got: "other".to_string(),
        });
    };
    let year = fields
        .get("year")
        .cloned()
        .ok_or_else(|| RuntimeError::InvalidArgument {
            context: ctx.to_string(),
            reason: "missing field 'year' on Date".to_string(),
        })?;
    let month = fields
        .get("month")
        .cloned()
        .ok_or_else(|| RuntimeError::InvalidArgument {
            context: ctx.to_string(),
            reason: "missing field 'month' on Date".to_string(),
        })?;
    let day = fields
        .get("day")
        .cloned()
        .ok_or_else(|| RuntimeError::InvalidArgument {
            context: ctx.to_string(),
            reason: "missing field 'day' on Date".to_string(),
        })?;
    let year = expect_int(year, ctx)? as i32;
    let month = expect_int(month, ctx)? as u32;
    let day = expect_int(day, ctx)? as u32;
    NaiveDate::from_ymd_opt(year, month, day)
        .ok_or_else(|| RuntimeError::InvalidArgument {
            context: ctx.to_string(),
            reason: "invalid Date (year/month/day combination)".to_string(),
        })
}
fn date_to_value(date: NaiveDate) -> Value {
    let mut map = HashMap::new();
    map.insert("year".to_string(), Value::Int(date.year() as i64));
    map.insert("month".to_string(), Value::Int(date.month() as i64));
    map.insert("day".to_string(), Value::Int(date.day() as i64));
    Value::Record(Arc::new(map))
}

fn add_months(date: NaiveDate, months: i64, ctx: &str) -> Result<NaiveDate, RuntimeError> {
    let base_year = i64::from(date.year());
    let base_month = i64::from(date.month());
    let total = (base_month - 1)
        .checked_add(months)
        .ok_or_else(|| RuntimeError::Overflow {
            context: ctx.to_string(),
        })?;
    let year = base_year
        .checked_add(total.div_euclid(12))
        .ok_or_else(|| RuntimeError::Overflow {
            context: ctx.to_string(),
        })?;
    let month = total.rem_euclid(12) + 1;
    let year_i32 = i32::try_from(year).map_err(|_| RuntimeError::Overflow {
        context: ctx.to_string(),
    })?;
    let month_u32 = u32::try_from(month).map_err(|_| RuntimeError::Overflow {
        context: ctx.to_string(),
    })?;
    let max_day = days_in_month(year_i32, month_u32, ctx)?;
    let day = date.day().min(max_day);
    NaiveDate::from_ymd_opt(year_i32, month_u32, day).ok_or_else(|| RuntimeError::InvalidArgument {
        context: ctx.to_string(),
        reason: "resulting date is invalid".to_string(),
    })
}

fn add_years(date: NaiveDate, years: i64, ctx: &str) -> Result<NaiveDate, RuntimeError> {
    let years_i32 = i32::try_from(years).map_err(|_| RuntimeError::Overflow {
        context: ctx.to_string(),
    })?;
    let year = date
        .year()
        .checked_add(years_i32)
        .ok_or_else(|| RuntimeError::Overflow {
            context: ctx.to_string(),
        })?;
    let max_day = days_in_month(year, date.month(), ctx)?;
    let day = date.day().min(max_day);
    NaiveDate::from_ymd_opt(year, date.month(), day).ok_or_else(|| RuntimeError::InvalidArgument {
        context: ctx.to_string(),
        reason: "resulting date is invalid".to_string(),
    })
}

fn days_in_month(year: i32, month: u32, ctx: &str) -> Result<u32, RuntimeError> {
    if !(1..=12).contains(&month) {
        return Err(RuntimeError::InvalidArgument {
            context: ctx.to_string(),
            reason: format!("month {month} out of range"),
        });
    }
    let (next_year, next_month) = if month == 12 {
        (
            year.checked_add(1).ok_or_else(|| RuntimeError::Overflow {
                context: ctx.to_string(),
            })?,
            1,
        )
    } else {
        (year, month + 1)
    };
    let first_next =
        NaiveDate::from_ymd_opt(next_year, next_month, 1).ok_or_else(|| RuntimeError::Overflow {
            context: ctx.to_string(),
        })?;
    let prev = first_next
        .pred_opt()
        .ok_or_else(|| RuntimeError::Overflow {
            context: ctx.to_string(),
        })?;
    Ok(prev.day())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_years_rejects_out_of_range_year_delta() {
        let date = NaiveDate::from_ymd_opt(2024, 1, 1).expect("valid test date");
        let years = i64::from(i32::MAX) + 1;
        let err = add_years(date, years, "calendar.addYears")
            .expect_err("expected overflow for out-of-range year delta");
        assert!(matches!(
            err,
            RuntimeError::Overflow { ref context } if context == "calendar.addYears"
        ));
    }

    #[test]
    fn add_years_rejects_wrapping_year_delta() {
        let date = NaiveDate::from_ymd_opt(2024, 1, 1).expect("valid test date");
        let err = add_years(date, 4_294_967_296, "calendar.addYears")
            .expect_err("expected overflow instead of wraparound");
        assert!(matches!(
            err,
            RuntimeError::Overflow { ref context } if context == "calendar.addYears"
        ));
    }

    #[test]
    fn add_months_rejects_out_of_range_result_year() {
        let date = NaiveDate::from_ymd_opt(2024, 1, 1).expect("valid test date");
        let err = add_months(date, 6_000_000, "calendar.addMonths")
            .expect_err("expected overflow for out-of-range resulting year");
        assert!(matches!(
            err,
            RuntimeError::Overflow { ref context } if context == "calendar.addMonths"
        ));
    }
}
