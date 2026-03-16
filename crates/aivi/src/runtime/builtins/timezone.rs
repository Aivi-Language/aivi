use std::collections::HashMap;
use std::sync::Arc;

use chrono::{offset::Offset as ChronoOffset, TimeZone as ChronoTimeZone};
use chrono_tz::Tz;

use super::chronos_format::{
    format_zoned_date_time_pattern, parse_local_datetime_value, zoned_date_time_from_value,
};
use super::util::{builtin, expect_record, expect_text};
use crate::runtime::{RuntimeError, Value};

pub(super) fn build_timezone_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "getOffset".to_string(),
        builtin("timezone.getOffset", 2, |mut args, _| {
            let instant = args.pop().unwrap();
            let zone_val = args.pop().unwrap();

            let zone_id = get_zone_id(zone_val, "timezone.getOffset")?;
            let tz: Tz = zone_id
                .parse()
                .map_err(|_| RuntimeError::Message(format!("invalid timezone id: {}", zone_id)))?;

            let naive = parse_datetime_value(&instant, "timezone.getOffset")?;
            let dt = naive.and_utc();

            let offset = tz.offset_from_utc_datetime(&dt.naive_utc());
            let millis = i64::from(offset.fix().local_minus_utc()) * 1000;

            let mut span_map = HashMap::new();
            span_map.insert("millis".to_string(), Value::Int(millis));
            Ok(Value::Record(Arc::new(span_map)))
        }),
    );
    fields.insert(
        "toInstant".to_string(),
        builtin("timezone.toInstant", 1, |mut args, _| {
            let zdt_val = args.pop().unwrap();
            let zdt_fields = expect_record(zdt_val, "timezone.toInstant")?;

            let dt_val = zdt_fields
                .get("dateTime")
                .ok_or_else(|| RuntimeError::Message("missing dateTime".to_string()))?;
            let zone_val = zdt_fields
                .get("zone")
                .ok_or_else(|| RuntimeError::Message("missing zone".to_string()))?;

            let zone_id = get_zone_id(zone_val.clone(), "timezone.toInstant")?;
            let tz: Tz = zone_id
                .parse()
                .map_err(|_| RuntimeError::Message(format!("invalid timezone id: {}", zone_id)))?;

            let naive = parse_datetime_value(dt_val, "timezone.toInstant")?;

            let zdt = tz.from_local_datetime(&naive).single().ok_or_else(|| {
                RuntimeError::Message("ambiguous or invalid local time".to_string())
            })?;

            // Return Timestamp (DateTime) in UTC
            let utc_naive = zdt.naive_utc();
            let text = format!("{}Z", utc_naive.format("%Y-%m-%dT%H:%M:%S"));
            Ok(Value::DateTime(text))
        }),
    );
    fields.insert(
        "atZone".to_string(),
        builtin("timezone.atZone", 2, |mut args, _| {
            let target_zone_val = args.pop().unwrap();
            let zdt_val = args.pop().unwrap();

            // First convert ZonedDateTime to instant (UTC timestamp)
            let zdt_fields = expect_record(zdt_val, "timezone.atZone")?;
            let dt_val = zdt_fields
                .get("dateTime")
                .ok_or_else(|| RuntimeError::Message("missing dateTime".to_string()))?;
            let source_zone_val = zdt_fields
                .get("zone")
                .ok_or_else(|| RuntimeError::Message("missing zone".to_string()))?;

            let source_zone_id = get_zone_id(source_zone_val.clone(), "timezone.atZone")?;
            let source_tz: Tz = source_zone_id.parse().map_err(|_| {
                RuntimeError::Message(format!("invalid source timezone id: {}", source_zone_id))
            })?;

            let naive = parse_datetime_value(dt_val, "timezone.atZone")?;

            let source_zdt = source_tz
                .from_local_datetime(&naive)
                .single()
                .ok_or_else(|| {
                    RuntimeError::Message("ambiguous or invalid local time".to_string())
                })?;

            // Now convert to target zone
            let target_zone_id = get_zone_id(target_zone_val.clone(), "timezone.atZone")?;
            let target_tz: Tz = target_zone_id.parse().map_err(|_| {
                RuntimeError::Message(format!("invalid target timezone id: {}", target_zone_id))
            })?;

            let target_zdt = source_zdt.with_timezone(&target_tz);
            let offset_millis = i64::from(target_zdt.offset().fix().local_minus_utc()) * 1000;

            let mut result = HashMap::new();
            result.insert("dateTime".to_string(), datetime_to_value(target_zdt));
            result.insert("zone".to_string(), target_zone_val);

            let mut offset_map = HashMap::new();
            offset_map.insert("millis".to_string(), Value::Int(offset_millis));
            result.insert("offset".to_string(), Value::Record(Arc::new(offset_map)));

            Ok(Value::Record(Arc::new(result)))
        }),
    );
    fields.insert(
        "format".to_string(),
        builtin("timezone.format", 2, |mut args, _| {
            let pattern = expect_text(args.pop().unwrap(), "timezone.format")?;
            let zdt = zoned_date_time_from_value(args.pop().unwrap(), "timezone.format")?;
            let text = format_zoned_date_time_pattern(&zdt, &pattern, "timezone.format")?;
            Ok(Value::Text(text))
        }),
    );
    Value::Record(Arc::new(fields))
}

fn get_zone_id(val: Value, ctx: &str) -> Result<String, RuntimeError> {
    let fields = expect_record(val, ctx)?;
    let id_val = fields
        .get("id")
        .ok_or_else(|| RuntimeError::Message(format!("{}: missing id field in TimeZone", ctx)))?;
    expect_text(id_val.clone(), ctx)
}

fn parse_datetime_value(val: &Value, ctx: &str) -> Result<chrono::NaiveDateTime, RuntimeError> {
    parse_local_datetime_value(val, ctx)
}

fn datetime_to_value<Tz: chrono::TimeZone>(dt: chrono::DateTime<Tz>) -> Value
where
    Tz::Offset: std::fmt::Display,
{
    let local_naive = dt.naive_local();
    Value::DateTime(format!("{}Z", local_naive.format("%Y-%m-%dT%H:%M:%S")))
}
