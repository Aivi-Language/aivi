use std::collections::HashMap;
use std::sync::Arc;

use chrono::{offset::Offset as ChronoOffset, SecondsFormat, TimeZone as ChronoTimeZone};
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
            get_offset_value(zone_val, instant)
        }),
    );
    fields.insert(
        "toInstant".to_string(),
        builtin("timezone.toInstant", 1, |mut args, _| {
            let zdt_val = args.pop().unwrap();
            to_instant_value(zdt_val)
        }),
    );
    fields.insert(
        "atZone".to_string(),
        builtin("timezone.atZone", 2, |mut args, _| {
            let target_zone_val = args.pop().unwrap();
            let zdt_val = args.pop().unwrap();
            at_zone_value(zdt_val, target_zone_val)
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

fn get_offset_value(zone_val: Value, instant: Value) -> Result<Value, RuntimeError> {
    let zone_id = get_zone_id(zone_val, "timezone.getOffset")?;
    let tz: Tz = zone_id
        .parse()
        .map_err(|_| RuntimeError::Message(format!("invalid timezone id: {}", zone_id)))?;

    let naive = parse_datetime_value(&instant, "timezone.getOffset")?;
    let dt = naive.and_utc();

    let offset = tz.offset_from_utc_datetime(&dt.naive_utc());
    let millis = i64::from(offset.fix().local_minus_utc()) * 1000;

    Ok(span_value(millis))
}

fn to_instant_value(zdt_val: Value) -> Result<Value, RuntimeError> {
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

    let zdt = tz
        .from_local_datetime(&naive)
        .single()
        .ok_or_else(|| RuntimeError::Message("ambiguous or invalid local time".to_string()))?;

    Ok(Value::DateTime(
        zdt.with_timezone(&chrono::Utc)
            .to_rfc3339_opts(SecondsFormat::AutoSi, true),
    ))
}

fn at_zone_value(zdt_val: Value, target_zone_val: Value) -> Result<Value, RuntimeError> {
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
        .ok_or_else(|| RuntimeError::Message("ambiguous or invalid local time".to_string()))?;

    let target_zone_id = get_zone_id(target_zone_val.clone(), "timezone.atZone")?;
    let target_tz: Tz = target_zone_id.parse().map_err(|_| {
        RuntimeError::Message(format!("invalid target timezone id: {}", target_zone_id))
    })?;

    let target_zdt = source_zdt.with_timezone(&target_tz);
    let offset_millis = i64::from(target_zdt.offset().fix().local_minus_utc()) * 1000;

    let mut result = HashMap::new();
    result.insert(
        "dateTime".to_string(),
        datetime_to_value(target_zdt.naive_local()),
    );
    result.insert("zone".to_string(), target_zone_val);
    result.insert("offset".to_string(), span_value(offset_millis));

    Ok(Value::Record(Arc::new(result)))
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

fn span_value(millis: i64) -> Value {
    let mut span_map = HashMap::new();
    span_map.insert("millis".to_string(), Value::Int(millis));
    Value::Record(Arc::new(span_map))
}

fn datetime_to_value(local_naive: chrono::NaiveDateTime) -> Value {
    Value::DateTime(
        local_naive
            .and_utc()
            .to_rfc3339_opts(SecondsFormat::AutoSi, true),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unwrap_ok<T>(result: Result<T, RuntimeError>, context: &str) -> T {
        match result {
            Ok(value) => value,
            Err(err) => panic!("{context}: {err}"),
        }
    }

    fn zone_value(id: &str) -> Value {
        let mut zone_map = HashMap::new();
        zone_map.insert("id".to_string(), Value::Text(id.to_string()));
        Value::Record(Arc::new(zone_map))
    }

    fn zoned_date_time_value(local: &str, zone_id: &str, offset_millis: i64) -> Value {
        let mut offset_map = HashMap::new();
        offset_map.insert("millis".to_string(), Value::Int(offset_millis));

        let mut zdt_map = HashMap::new();
        zdt_map.insert("dateTime".to_string(), Value::DateTime(format!("{local}Z")));
        zdt_map.insert("zone".to_string(), zone_value(zone_id));
        zdt_map.insert("offset".to_string(), Value::Record(Arc::new(offset_map)));

        Value::Record(Arc::new(zdt_map))
    }

    fn expect_datetime(value: Value, expected: &str) {
        match value {
            Value::DateTime(text) => assert_eq!(text, expected),
            other => panic!(
                "expected DateTime {expected}, got {}",
                crate::runtime::format_value(&other)
            ),
        }
    }

    fn record_field<'a>(fields: &'a Arc<HashMap<String, Value>>, name: &str) -> &'a Value {
        fields
            .get(name)
            .unwrap_or_else(|| panic!("missing field {name}"))
    }

    fn expect_int_field(fields: &Arc<HashMap<String, Value>>, name: &str, expected: i64) {
        match record_field(fields, name) {
            Value::Int(value) => assert_eq!(*value, expected),
            other => panic!(
                "expected Int field {name}, got {}",
                crate::runtime::format_value(other)
            ),
        }
    }

    #[test]
    fn to_instant_preserves_fractional_seconds() {
        let zdt = zoned_date_time_value("2024-05-21T12:00:00.123456789", "Europe/Paris", 7_200_000);

        let instant = unwrap_ok(to_instant_value(zdt), "toInstant should succeed");

        expect_datetime(instant, "2024-05-21T10:00:00.123456789Z");
    }

    #[test]
    fn at_zone_preserves_fractional_seconds() {
        let zdt = zoned_date_time_value("2024-05-21T12:00:00.123456789", "Europe/Paris", 7_200_000);

        let tokyo = unwrap_ok(
            at_zone_value(zdt, zone_value("Asia/Tokyo")),
            "atZone should succeed",
        );
        let tokyo_fields = unwrap_ok(expect_record(tokyo.clone(), "timezone test"), "record");

        expect_datetime(
            record_field(&tokyo_fields, "dateTime").clone(),
            "2024-05-21T19:00:00.123456789Z",
        );
        let offset_fields = unwrap_ok(
            expect_record(
                record_field(&tokyo_fields, "offset").clone(),
                "timezone test",
            ),
            "offset record",
        );
        expect_int_field(&offset_fields, "millis", 32_400_000);
        let instant = unwrap_ok(to_instant_value(tokyo), "toInstant should preserve instant");
        expect_datetime(instant, "2024-05-21T10:00:00.123456789Z");
    }

    #[test]
    fn to_instant_rejects_invalid_local_time() {
        let zdt = zoned_date_time_value("2024-03-31T02:30:00", "Europe/Paris", 3_600_000);

        let err = to_instant_value(zdt).expect_err("invalid local time should fail");

        assert!(matches!(
            err,
            RuntimeError::Message(ref message) if message == "ambiguous or invalid local time"
        ));
    }
}
