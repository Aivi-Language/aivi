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
            Ok(Value::Int(days_in_month(date.year(), date.month()) as i64))
        }),
    );
    fields.insert(
        "endOfMonth".to_string(),
        builtin("calendar.endOfMonth", 1, |mut args, _| {
            let date = date_from_value(args.pop().unwrap(), "calendar.endOfMonth")?;
            let max_day = days_in_month(date.year(), date.month());
            let end = NaiveDate::from_ymd_opt(date.year(), date.month(), max_day)
                .expect("valid end-of-month date");
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
                .ok_or_else(|| RuntimeError::Message("calendar.addDays overflow".to_string()))?;
            Ok(date_to_value(next))
        }),
    );
    fields.insert(
        "addMonths".to_string(),
        builtin("calendar.addMonths", 2, |mut args, _| {
            let months = expect_int(args.pop().unwrap(), "calendar.addMonths")?;
            let date = date_from_value(args.pop().unwrap(), "calendar.addMonths")?;
            Ok(date_to_value(add_months(date, months)))
        }),
    );
    fields.insert(
        "addYears".to_string(),
        builtin("calendar.addYears", 2, |mut args, _| {
            let years = expect_int(args.pop().unwrap(), "calendar.addYears")?;
            let date = date_from_value(args.pop().unwrap(), "calendar.addYears")?;
            let year = date.year() + years as i32;
            let max_day = days_in_month(year, date.month());
            let day = date.day().min(max_day);
            let next = NaiveDate::from_ymd_opt(year, date.month(), day)
                .ok_or_else(|| RuntimeError::Message("calendar.addYears invalid date".to_string()))?;
            Ok(date_to_value(next))
        }),
    );
    Value::Record(Arc::new(fields))
}
fn date_from_value(value: Value, ctx: &str) -> Result<NaiveDate, RuntimeError> {
    let Value::Record(fields) = value else {
        return Err(RuntimeError::Message(format!("{ctx} expects Date")));
    };
    let year = fields
        .get("year")
        .cloned()
        .ok_or_else(|| RuntimeError::Message(format!("{ctx} expects Date.year")))?;
    let month = fields
        .get("month")
        .cloned()
        .ok_or_else(|| RuntimeError::Message(format!("{ctx} expects Date.month")))?;
    let day = fields
        .get("day")
        .cloned()
        .ok_or_else(|| RuntimeError::Message(format!("{ctx} expects Date.day")))?;
    let year = expect_int(year, ctx)? as i32;
    let month = expect_int(month, ctx)? as u32;
    let day = expect_int(day, ctx)? as u32;
    NaiveDate::from_ymd_opt(year, month, day)
        .ok_or_else(|| RuntimeError::Message(format!("{ctx} expects valid Date")))
}
fn date_to_value(date: NaiveDate) -> Value {
    let mut map = HashMap::new();
    map.insert("year".to_string(), Value::Int(date.year() as i64));
    map.insert("month".to_string(), Value::Int(date.month() as i64));
    map.insert("day".to_string(), Value::Int(date.day() as i64));
    Value::Record(Arc::new(map))
}
fn add_months(date: NaiveDate, months: i64) -> NaiveDate {
    let mut year = date.year() as i64;
    let mut month = date.month() as i64;
    let total = month - 1 + months;
    year += total.div_euclid(12);
    month = total.rem_euclid(12) + 1;
    let year_i32 = year as i32;
    let month_u32 = month as u32;
    let max_day = days_in_month(year_i32, month_u32);
    let day = date.day().min(max_day);
    NaiveDate::from_ymd_opt(year_i32, month_u32, day).expect("valid date")
}
fn days_in_month(year: i32, month: u32) -> u32 {
    let (next_year, next_month) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    let first_next = NaiveDate::from_ymd_opt(next_year, next_month, 1)
        .expect("valid next month date");
    first_next.pred_opt().expect("previous day").day()
}
