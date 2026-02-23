use std::collections::HashMap;
use std::sync::Arc;

use chrono::{offset::Offset as ChronoOffset, Datelike, TimeZone as ChronoTimeZone, Timelike, Utc};
use chrono_tz::Tz;

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

            let timestamp = get_timestamp(instant, "timezone.getOffset")?;
            let dt = Utc
                .timestamp_millis_opt(timestamp)
                .single()
                .ok_or_else(|| RuntimeError::Message("invalid timestamp".to_string()))?;

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

            let dt_fields = expect_record(dt_val.clone(), "timezone.toInstant")?;
            let year = get_int_field(&dt_fields, "year")? as i32;
            let month = get_int_field(&dt_fields, "month")? as u32;
            let day = get_int_field(&dt_fields, "day")? as u32;
            let hour = get_int_field(&dt_fields, "hour")? as u32;
            let minute = get_int_field(&dt_fields, "minute")? as u32;
            let second = get_int_field(&dt_fields, "second")? as u32;
            let millisecond = get_int_field(&dt_fields, "millisecond")? as u32;

            let naive = chrono::NaiveDate::from_ymd_opt(year, month, day)
                .and_then(|d| d.and_hms_milli_opt(hour, minute, second, millisecond))
                .ok_or_else(|| RuntimeError::Message("invalid date time".to_string()))?;

            let zdt = tz.from_local_datetime(&naive).single().ok_or_else(|| {
                RuntimeError::Message("ambiguous or invalid local time".to_string())
            })?;
            let timestamp = zdt.timestamp_millis();

            // Return Timestamp (DateTime) in UTC
            let utc_dt = Utc.timestamp_millis_opt(timestamp).single().unwrap();
            Ok(datetime_to_value(utc_dt))
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

            let dt_fields = expect_record(dt_val.clone(), "timezone.atZone")?;
            let year = get_int_field(&dt_fields, "year")? as i32;
            let month = get_int_field(&dt_fields, "month")? as u32;
            let day = get_int_field(&dt_fields, "day")? as u32;
            let hour = get_int_field(&dt_fields, "hour")? as u32;
            let minute = get_int_field(&dt_fields, "minute")? as u32;
            let second = get_int_field(&dt_fields, "second")? as u32;
            let millisecond = get_int_field(&dt_fields, "millisecond")? as u32;

            let naive = chrono::NaiveDate::from_ymd_opt(year, month, day)
                .and_then(|d| d.and_hms_milli_opt(hour, minute, second, millisecond))
                .ok_or_else(|| RuntimeError::Message("invalid date time".to_string()))?;

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
    Value::Record(Arc::new(fields))
}

fn get_zone_id(val: Value, ctx: &str) -> Result<String, RuntimeError> {
    let fields = expect_record(val, ctx)?;
    let id_val = fields
        .get("id")
        .ok_or_else(|| RuntimeError::Message(format!("{}: missing id field in TimeZone", ctx)))?;
    expect_text(id_val.clone(), ctx)
}

fn get_timestamp(val: Value, ctx: &str) -> Result<i64, RuntimeError> {
    // Timestamp is just DateTime (UTC)
    let fields = expect_record(val, ctx)?;
    let year = get_int_field(&fields, "year")? as i32;
    let month = get_int_field(&fields, "month")? as u32;
    let day = get_int_field(&fields, "day")? as u32;
    let hour = get_int_field(&fields, "hour")? as u32;
    let minute = get_int_field(&fields, "minute")? as u32;
    let second = get_int_field(&fields, "second")? as u32;
    let millisecond = get_int_field(&fields, "millisecond")? as u32;

    let dt = chrono::NaiveDate::from_ymd_opt(year, month, day)
        .and_then(|d| d.and_hms_milli_opt(hour, minute, second, millisecond))
        .ok_or_else(|| RuntimeError::Message("invalid timestamp".to_string()))?;
    Ok(dt.and_utc().timestamp_millis())
}

fn get_int_field(fields: &HashMap<String, Value>, name: &str) -> Result<i64, RuntimeError> {
    let val = fields
        .get(name)
        .ok_or_else(|| RuntimeError::Message(format!("missing field {}", name)))?;
    match val {
        Value::Int(i) => Ok(*i),
        _ => Err(RuntimeError::Message(format!(
            "field {} expected int",
            name
        ))),
    }
}

fn datetime_to_value<Tz: chrono::TimeZone>(dt: chrono::DateTime<Tz>) -> Value {
    let mut map = HashMap::new();
    map.insert("year".to_string(), Value::Int(dt.year() as i64));
    map.insert("month".to_string(), Value::Int(dt.month() as i64));
    map.insert("day".to_string(), Value::Int(dt.day() as i64));
    map.insert("hour".to_string(), Value::Int(dt.hour() as i64));
    map.insert("minute".to_string(), Value::Int(dt.minute() as i64));
    map.insert("second".to_string(), Value::Int(dt.second() as i64));
    map.insert(
        "millisecond".to_string(),
        Value::Int(dt.timestamp_subsec_millis() as i64),
    );
    Value::Record(Arc::new(map))
}
