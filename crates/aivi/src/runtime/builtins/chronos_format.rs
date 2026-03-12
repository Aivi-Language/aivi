use std::collections::{HashMap, HashSet};

use chrono::{Datelike, NaiveDate, NaiveDateTime, Timelike};

use super::util::{expect_int, expect_text, value_type_name};
use crate::runtime::{RuntimeError, Value};

const ISO_DATE_PATTERN: &str = "yyyy-MM-dd";
const ISO_ZONED_DATE_TIME_PATTERN: &str = "yyyy-MM-dd'T'HH:mm:ssXXX'['VV']'";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ZonedDateTimeParts {
    pub(super) local: NaiveDateTime,
    pub(super) zone_id: String,
    pub(super) offset_millis: i64,
}

pub(super) fn date_from_value(value: Value, ctx: &str) -> Result<NaiveDate, RuntimeError> {
    date_from_value_ref(&value, ctx)
}

pub(super) fn date_from_value_ref(value: &Value, ctx: &str) -> Result<NaiveDate, RuntimeError> {
    let Value::Record(fields) = value else {
        return Err(RuntimeError::TypeError {
            context: ctx.to_string(),
            expected: "Date".to_string(),
            got: value_type_name(value).to_string(),
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

    NaiveDate::from_ymd_opt(year, month, day).ok_or_else(|| RuntimeError::InvalidArgument {
        context: ctx.to_string(),
        reason: "invalid Date (year/month/day combination)".to_string(),
    })
}

pub(super) fn zoned_date_time_from_value(
    value: Value,
    ctx: &str,
) -> Result<ZonedDateTimeParts, RuntimeError> {
    zoned_date_time_from_value_ref(&value, ctx)
}

pub(super) fn zoned_date_time_from_value_ref(
    value: &Value,
    ctx: &str,
) -> Result<ZonedDateTimeParts, RuntimeError> {
    let Value::Record(fields) = value else {
        return Err(RuntimeError::TypeError {
            context: ctx.to_string(),
            expected: "ZonedDateTime".to_string(),
            got: value_type_name(value).to_string(),
        });
    };

    let dt_val = fields
        .get("dateTime")
        .ok_or_else(|| RuntimeError::InvalidArgument {
            context: ctx.to_string(),
            reason: "missing field 'dateTime' on ZonedDateTime".to_string(),
        })?;
    let zone_val = fields
        .get("zone")
        .ok_or_else(|| RuntimeError::InvalidArgument {
            context: ctx.to_string(),
            reason: "missing field 'zone' on ZonedDateTime".to_string(),
        })?;
    let offset_val = fields
        .get("offset")
        .ok_or_else(|| RuntimeError::InvalidArgument {
            context: ctx.to_string(),
            reason: "missing field 'offset' on ZonedDateTime".to_string(),
        })?;

    let local = parse_local_datetime_value(dt_val, ctx)?;
    let zone_id = zone_id_from_value(zone_val.clone(), ctx)?;
    let offset_millis = span_millis_from_value(offset_val.clone(), ctx)?;

    Ok(ZonedDateTimeParts {
        local,
        zone_id,
        offset_millis,
    })
}

pub(super) fn maybe_iso_text(value: &Value) -> Option<String> {
    maybe_date_from_value(value)
        .map(format_date_iso)
        .or_else(|| maybe_zoned_date_time_from_value(value).map(|zdt| format_zoned_date_time_iso(&zdt)))
}

pub(super) fn format_date_iso(date: NaiveDate) -> String {
    format_date_pattern(date, ISO_DATE_PATTERN, "calendar.format").unwrap_or_else(|_| {
        unreachable!("built-in ISO date pattern must stay valid")
    })
}

pub(super) fn format_zoned_date_time_iso(value: &ZonedDateTimeParts) -> String {
    format_zoned_date_time_pattern(value, ISO_ZONED_DATE_TIME_PATTERN, "timezone.format")
        .unwrap_or_else(|_| unreachable!("built-in ISO zoned date-time pattern must stay valid"))
}

pub(super) fn format_date_pattern(
    date: NaiveDate,
    pattern: &str,
    ctx: &str,
) -> Result<String, RuntimeError> {
    format_pattern(FormatInput::Date(date), pattern, ctx)
}

pub(super) fn format_zoned_date_time_pattern(
    value: &ZonedDateTimeParts,
    pattern: &str,
    ctx: &str,
) -> Result<String, RuntimeError> {
    format_pattern(FormatInput::ZonedDateTime(value), pattern, ctx)
}

fn maybe_date_from_value(value: &Value) -> Option<NaiveDate> {
    let Value::Record(fields) = value else {
        return None;
    };
    if !exact_field_set(fields, ["year", "month", "day"]) {
        return None;
    }
    date_from_value_ref(value, "text.toText").ok()
}

fn maybe_zoned_date_time_from_value(value: &Value) -> Option<ZonedDateTimeParts> {
    let Value::Record(fields) = value else {
        return None;
    };
    if !exact_field_set(fields, ["dateTime", "zone", "offset"]) {
        return None;
    }
    let zone = fields.get("zone")?;
    let Value::Record(zone_fields) = zone else {
        return None;
    };
    if !exact_field_set(zone_fields, ["id"]) {
        return None;
    }
    let offset = fields.get("offset")?;
    let Value::Record(offset_fields) = offset else {
        return None;
    };
    if !exact_field_set(offset_fields, ["millis"]) {
        return None;
    }
    zoned_date_time_from_value_ref(value, "text.toText").ok()
}

fn exact_field_set<const N: usize>(fields: &HashMap<String, Value>, expected: [&str; N]) -> bool {
    if fields.len() != N {
        return false;
    }
    let expected: HashSet<&str> = expected.into_iter().collect();
    fields.keys().all(|field| expected.contains(field.as_str()))
}

fn zone_id_from_value(value: Value, ctx: &str) -> Result<String, RuntimeError> {
    let Value::Record(fields) = value else {
        return Err(RuntimeError::TypeError {
            context: ctx.to_string(),
            expected: "TimeZone".to_string(),
            got: value_type_name(&value).to_string(),
        });
    };
    let id = fields
        .get("id")
        .cloned()
        .ok_or_else(|| RuntimeError::InvalidArgument {
            context: ctx.to_string(),
            reason: "missing field 'id' on TimeZone".to_string(),
        })?;
    expect_text(id, ctx)
}

fn span_millis_from_value(value: Value, ctx: &str) -> Result<i64, RuntimeError> {
    let Value::Record(fields) = value else {
        return Err(RuntimeError::TypeError {
            context: ctx.to_string(),
            expected: "Span".to_string(),
            got: value_type_name(&value).to_string(),
        });
    };
    let millis = fields
        .get("millis")
        .cloned()
        .ok_or_else(|| RuntimeError::InvalidArgument {
            context: ctx.to_string(),
            reason: "missing field 'millis' on Span".to_string(),
        })?;
    expect_int(millis, ctx)
}

pub(super) fn parse_local_datetime_value(
    value: &Value,
    ctx: &str,
) -> Result<NaiveDateTime, RuntimeError> {
    let Value::DateTime(text) = value else {
        return Err(RuntimeError::TypeError {
            context: ctx.to_string(),
            expected: "DateTime".to_string(),
            got: value_type_name(value).to_string(),
        });
    };

    if let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(text) {
        return Ok(parsed.naive_local());
    }

    let trimmed = text.trim_end_matches('Z');
    NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%dT%H:%M:%S").map_err(|err| {
        RuntimeError::InvalidArgument {
            context: ctx.to_string(),
            reason: format!("invalid DateTime string '{text}': {err}"),
        }
    })
}

enum FormatInput<'a> {
    Date(NaiveDate),
    ZonedDateTime(&'a ZonedDateTimeParts),
}

fn format_pattern(input: FormatInput<'_>, pattern: &str, ctx: &str) -> Result<String, RuntimeError> {
    let mut chars = pattern.chars().peekable();
    let mut out = String::new();

    while let Some(ch) = chars.next() {
        if ch == '\'' {
            push_quoted_literal(&mut chars, &mut out, pattern, ctx)?;
            continue;
        }

        if ch.is_ascii_alphabetic() {
            let mut count = 1usize;
            while chars.peek().copied() == Some(ch) {
                chars.next();
                count += 1;
            }
            push_token(&input, ch, count, &mut out, pattern, ctx)?;
            continue;
        }

        out.push(ch);
    }

    Ok(out)
}

fn push_quoted_literal<I>(
    chars: &mut std::iter::Peekable<I>,
    out: &mut String,
    pattern: &str,
    ctx: &str,
) -> Result<(), RuntimeError>
where
    I: Iterator<Item = char>,
{
    while let Some(ch) = chars.next() {
        if ch == '\'' {
            if chars.peek().copied() == Some('\'') {
                chars.next();
                out.push('\'');
                continue;
            }
            return Ok(());
        }
        out.push(ch);
    }

    Err(RuntimeError::InvalidArgument {
        context: ctx.to_string(),
        reason: format!("unterminated quoted literal in date/time pattern '{pattern}'"),
    })
}

fn push_token(
    input: &FormatInput<'_>,
    token: char,
    count: usize,
    out: &mut String,
    pattern: &str,
    ctx: &str,
) -> Result<(), RuntimeError> {
    match token {
        'y' => out.push_str(&format_year(value_date(input).year(), count)),
        'M' => out.push_str(&format_numeric_token(value_date(input).month(), count, token, pattern, ctx)?),
        'd' => out.push_str(&format_numeric_token(value_date(input).day(), count, token, pattern, ctx)?),
        'H' => out.push_str(&format_numeric_token(value_time(input, token, pattern, ctx)?.hour(), count, token, pattern, ctx)?),
        'm' => out.push_str(&format_numeric_token(value_time(input, token, pattern, ctx)?.minute(), count, token, pattern, ctx)?),
        's' => out.push_str(&format_numeric_token(value_time(input, token, pattern, ctx)?.second(), count, token, pattern, ctx)?),
        'X' => out.push_str(&format_offset(offset_millis(input, token, pattern, ctx)?, count, pattern, ctx)?),
        'V' => out.push_str(&format_zone(zone_id(input, token, pattern, ctx)?, count, pattern, ctx)?),
        _ => {
            return Err(RuntimeError::InvalidArgument {
                context: ctx.to_string(),
                reason: format!("unsupported format token '{}' in pattern '{pattern}'", token.to_string().repeat(count)),
            });
        }
    }
    Ok(())
}

fn value_date(input: &FormatInput<'_>) -> NaiveDate {
    match input {
        FormatInput::Date(date) => *date,
        FormatInput::ZonedDateTime(value) => value.local.date(),
    }
}

fn value_time<'a>(
    input: &'a FormatInput<'_>,
    token: char,
    pattern: &str,
    ctx: &str,
) -> Result<&'a NaiveDateTime, RuntimeError> {
    match input {
        FormatInput::ZonedDateTime(value) => Ok(&value.local),
        FormatInput::Date(_) => Err(RuntimeError::InvalidArgument {
            context: ctx.to_string(),
            reason: format!(
                "format token '{}' is only available for ZonedDateTime patterns ('{pattern}')",
                token
            ),
        }),
    }
}

fn offset_millis(
    input: &FormatInput<'_>,
    token: char,
    pattern: &str,
    ctx: &str,
) -> Result<i64, RuntimeError> {
    match input {
        FormatInput::ZonedDateTime(value) => Ok(value.offset_millis),
        FormatInput::Date(_) => Err(RuntimeError::InvalidArgument {
            context: ctx.to_string(),
            reason: format!(
                "format token '{}' is only available for ZonedDateTime patterns ('{pattern}')",
                token
            ),
        }),
    }
}

fn zone_id<'a>(
    input: &'a FormatInput<'_>,
    token: char,
    pattern: &str,
    ctx: &str,
) -> Result<&'a str, RuntimeError> {
    match input {
        FormatInput::ZonedDateTime(value) => Ok(value.zone_id.as_str()),
        FormatInput::Date(_) => Err(RuntimeError::InvalidArgument {
            context: ctx.to_string(),
            reason: format!(
                "format token '{}' is only available for ZonedDateTime patterns ('{pattern}')",
                token
            ),
        }),
    }
}

fn format_year(year: i32, count: usize) -> String {
    if count == 2 {
        return format!("{:02}", year.rem_euclid(100));
    }
    zero_pad_signed(year, count.max(1))
}

fn format_numeric_token(
    value: u32,
    count: usize,
    token: char,
    pattern: &str,
    ctx: &str,
) -> Result<String, RuntimeError> {
    match count {
        1 => Ok(value.to_string()),
        2 => Ok(format!("{value:02}")),
        _ => Err(RuntimeError::InvalidArgument {
            context: ctx.to_string(),
            reason: format!(
                "unsupported format token '{}' in pattern '{pattern}'",
                token.to_string().repeat(count)
            ),
        }),
    }
}

fn format_offset(offset_millis: i64, count: usize, pattern: &str, ctx: &str) -> Result<String, RuntimeError> {
    if offset_millis % 1000 != 0 {
        return Err(RuntimeError::InvalidArgument {
            context: ctx.to_string(),
            reason: format!(
                "offset {offset_millis}ms is not aligned to whole seconds for pattern '{pattern}'"
            ),
        });
    }

    let total_seconds = offset_millis / 1000;
    if total_seconds == 0 {
        return Ok("Z".to_string());
    }
    if total_seconds % 60 != 0 {
        return Err(RuntimeError::InvalidArgument {
            context: ctx.to_string(),
            reason: format!(
                "offset {offset_millis}ms is not aligned to whole minutes for pattern '{pattern}'"
            ),
        });
    }

    let total_minutes = total_seconds / 60;
    let sign = if total_minutes < 0 { '-' } else { '+' };
    let abs_minutes = total_minutes.abs();
    let hours = abs_minutes / 60;
    let minutes = abs_minutes % 60;

    match count {
        1 => {
            if minutes == 0 {
                Ok(format!("{sign}{hours:02}"))
            } else {
                Ok(format!("{sign}{hours:02}{minutes:02}"))
            }
        }
        2 => Ok(format!("{sign}{hours:02}{minutes:02}")),
        3 => Ok(format!("{sign}{hours:02}:{minutes:02}")),
        _ => Err(RuntimeError::InvalidArgument {
            context: ctx.to_string(),
            reason: format!("unsupported format token '{}' in pattern '{pattern}'", "X".repeat(count)),
        }),
    }
}

fn format_zone(zone_id: &str, count: usize, pattern: &str, ctx: &str) -> Result<String, RuntimeError> {
    match count {
        1 | 2 => Ok(zone_id.to_string()),
        _ => Err(RuntimeError::InvalidArgument {
            context: ctx.to_string(),
            reason: format!("unsupported format token '{}' in pattern '{pattern}'", "V".repeat(count)),
        }),
    }
}

fn zero_pad_signed(value: i32, width: usize) -> String {
    if value < 0 {
        format!("-{:0width$}", value.unsigned_abs(), width = width)
    } else {
        format!("{value:0width$}")
    }
}

#[cfg(test)]
mod tests {
    use chrono::{NaiveDate, NaiveDateTime};

    use super::*;

    #[test]
    fn formats_date_pattern() {
        let date = NaiveDate::from_ymd_opt(2024, 5, 21).expect("valid test date");
        let text = format_date_pattern(date, "dd.MM.yyyy", "test")
            .unwrap_or_else(|_| unreachable!("pattern should format"));
        assert_eq!(text, "21.05.2024");
    }

    #[test]
    fn formats_zoned_date_time_iso_text() {
        let local = NaiveDateTime::parse_from_str("2024-05-21T12:00:00", "%Y-%m-%dT%H:%M:%S")
            .expect("valid local datetime");
        let value = ZonedDateTimeParts {
            local,
            zone_id: "Europe/Paris".to_string(),
            offset_millis: 7_200_000,
        };

        assert_eq!(
            format_zoned_date_time_iso(&value),
            "2024-05-21T12:00:00+02:00[Europe/Paris]"
        );
    }

    #[test]
    fn rejects_time_tokens_for_dates() {
        let date = NaiveDate::from_ymd_opt(2024, 5, 21).expect("valid test date");
        let err = format_date_pattern(date, "yyyy-MM-dd HH:mm", "calendar.format")
            .expect_err("date pattern should reject time tokens");
        assert!(matches!(
            err,
            RuntimeError::InvalidArgument { ref context, ref reason }
                if context == "calendar.format"
                    && reason.contains("only available for ZonedDateTime")
        ));
    }

    #[test]
    fn rejects_unsupported_tokens() {
        let date = NaiveDate::from_ymd_opt(2024, 5, 21).expect("valid test date");
        let err = format_date_pattern(date, "MMMM dd, yyyy", "calendar.format")
            .expect_err("unsupported token should fail");
        assert!(matches!(
            err,
            RuntimeError::InvalidArgument { ref context, ref reason }
                if context == "calendar.format"
                    && reason.contains("unsupported format token 'MMMM'")
        ));
    }
}
