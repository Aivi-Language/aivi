use std::collections::HashMap;
use std::sync::Arc;

use chrono::{SecondsFormat, TimeZone, Utc};

use super::util::{builtin, expect_int};
use crate::{RuntimeError, Value};

pub(super) fn build_instant_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "toNanos".to_string(),
        builtin("instant.toNanos", 1, |mut args, _| {
            let text = expect_datetime(args.pop().unwrap(), "instant.toNanos")?;
            let nanos = parse_datetime_text(&text, "instant.toNanos")?;
            let nanos = i128_to_i64(nanos, "instant.toNanos")?;
            Ok(Value::Int(nanos))
        }),
    );
    fields.insert(
        "fromNanos".to_string(),
        builtin("instant.fromNanos", 1, |mut args, _| {
            let nanos = expect_int(args.pop().unwrap(), "instant.fromNanos")? as i128;
            let text = nanos_to_rfc3339(nanos, "instant.fromNanos")?;
            Ok(Value::DateTime(text))
        }),
    );
    fields.insert(
        "addMillis".to_string(),
        builtin("instant.addMillis", 2, |mut args, _| {
            let millis = expect_int(args.pop().unwrap(), "instant.addMillis")? as i128;
            let text = expect_datetime(args.pop().unwrap(), "instant.addMillis")?;
            let base = parse_datetime_text(&text, "instant.addMillis")?;
            let delta = millis
                .checked_mul(1_000_000)
                .ok_or_else(|| RuntimeError::Message("instant.addMillis overflow".to_string()))?;
            let nanos = base
                .checked_add(delta)
                .ok_or_else(|| RuntimeError::Message("instant.addMillis overflow".to_string()))?;
            let text = nanos_to_rfc3339(nanos, "instant.addMillis")?;
            Ok(Value::DateTime(text))
        }),
    );
    fields.insert(
        "diffMillis".to_string(),
        builtin("instant.diffMillis", 2, |mut args, _| {
            let right = expect_datetime(args.pop().unwrap(), "instant.diffMillis")?;
            let left = expect_datetime(args.pop().unwrap(), "instant.diffMillis")?;
            let left = parse_datetime_text(&left, "instant.diffMillis")?;
            let right = parse_datetime_text(&right, "instant.diffMillis")?;
            let delta = left
                .checked_sub(right)
                .ok_or_else(|| RuntimeError::Message("instant.diffMillis overflow".to_string()))?;
            let millis = delta / 1_000_000;
            let millis = i128_to_i64(millis, "instant.diffMillis")?;
            Ok(Value::Int(millis))
        }),
    );
    fields.insert(
        "compare".to_string(),
        builtin("instant.compare", 2, |mut args, _| {
            let right = expect_datetime(args.pop().unwrap(), "instant.compare")?;
            let left = expect_datetime(args.pop().unwrap(), "instant.compare")?;
            let left = parse_datetime_text(&left, "instant.compare")?;
            let right = parse_datetime_text(&right, "instant.compare")?;
            let cmp = if left < right {
                -1
            } else if left > right {
                1
            } else {
                0
            };
            Ok(Value::Int(cmp))
        }),
    );
    Value::Record(Arc::new(fields))
}

fn expect_datetime(value: Value, ctx: &str) -> Result<String, RuntimeError> {
    match value {
        Value::DateTime(text) => Ok(text),
        other => Err(RuntimeError::Message(format!(
            "{ctx} expects DateTime, got {}",
            crate::format_value(&other)
        ))),
    }
}

fn parse_datetime_text(text: &str, ctx: &str) -> Result<i128, RuntimeError> {
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(text) {
        let seconds = dt.timestamp() as i128;
        let nanos = dt.timestamp_subsec_nanos() as i128;
        return seconds
            .checked_mul(1_000_000_000)
            .and_then(|base| base.checked_add(nanos))
            .ok_or_else(|| RuntimeError::Message(format!("{ctx} overflow")));
    }
    if let Some(nanos) = parse_epoch_seconds(text) {
        return Ok(nanos);
    }
    Err(RuntimeError::Message(format!(
        "{ctx} expects RFC3339 or epoch DateTime"
    )))
}

fn parse_epoch_seconds(text: &str) -> Option<i128> {
    let trimmed = text.trim();
    let body = trimmed.strip_suffix('Z')?;
    let (sign, body) = if let Some(rest) = body.strip_prefix('-') {
        (-1i128, rest)
    } else {
        (1i128, body)
    };
    let (secs_text, frac_text) = match body.split_once('.') {
        Some((secs, frac)) => (secs, Some(frac)),
        None => (body, None),
    };
    if secs_text.is_empty() {
        return None;
    }
    let seconds: i128 = secs_text.parse().ok()?;
    let mut nanos: i128 = 0;
    if let Some(frac) = frac_text {
        if frac.is_empty() {
            nanos = 0;
        } else if frac.len() > 9 || !frac.chars().all(|ch| ch.is_ascii_digit()) {
            return None;
        } else {
            let frac_val: i128 = frac.parse().ok()?;
            let scale = 10i128.pow((9 - frac.len()) as u32);
            nanos = frac_val * scale;
        }
    }
    let total = seconds.checked_mul(1_000_000_000)?.checked_add(nanos)?;
    total.checked_mul(sign)
}

fn nanos_to_rfc3339(nanos: i128, ctx: &str) -> Result<String, RuntimeError> {
    let nanos = i128_to_i64(nanos, ctx)?;
    let seconds = nanos.div_euclid(1_000_000_000);
    let subsec = nanos.rem_euclid(1_000_000_000) as u32;
    let dt = Utc
        .timestamp_opt(seconds, subsec)
        .single()
        .ok_or_else(|| RuntimeError::Message(format!("{ctx} out of range")))?;
    Ok(dt.to_rfc3339_opts(SecondsFormat::Nanos, true))
}

fn i128_to_i64(value: i128, ctx: &str) -> Result<i64, RuntimeError> {
    if value > i64::MAX as i128 || value < i64::MIN as i128 {
        return Err(RuntimeError::Message(format!("{ctx} out of range")));
    }
    Ok(value as i64)
}
