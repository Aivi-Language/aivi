use std::collections::HashMap;
use std::sync::Arc;

use palette::{FromColor, Hsl, RgbHue, Srgb};

use super::util::{builtin, expect_float, expect_int};
use crate::runtime::{RuntimeError, Value};

pub(super) fn build_color_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "adjustLightness".to_string(),
        builtin("color.adjustLightness", 2, |mut args, _| {
            let amount = expect_int(args.pop().unwrap(), "color.adjustLightness")?;
            let (r, g, b) = rgb_from_value(args.pop().unwrap(), "color.adjustLightness")?;
            let hsl: Hsl = Hsl::from_color(Srgb::new(r, g, b));
            let delta = amount as f32 / 100.0;
            let next = Hsl::new(
                hsl.hue,
                hsl.saturation,
                (hsl.lightness + delta).clamp(0.0, 1.0),
            );
            Ok(rgb_to_value(Srgb::from_color(next)))
        }),
    );
    fields.insert(
        "adjustSaturation".to_string(),
        builtin("color.adjustSaturation", 2, |mut args, _| {
            let amount = expect_int(args.pop().unwrap(), "color.adjustSaturation")?;
            let (r, g, b) = rgb_from_value(args.pop().unwrap(), "color.adjustSaturation")?;
            let hsl: Hsl = Hsl::from_color(Srgb::new(r, g, b));
            let delta = amount as f32 / 100.0;
            let next = Hsl::new(
                hsl.hue,
                (hsl.saturation + delta).clamp(0.0, 1.0),
                hsl.lightness,
            );
            Ok(rgb_to_value(Srgb::from_color(next)))
        }),
    );
    fields.insert(
        "adjustHue".to_string(),
        builtin("color.adjustHue", 2, |mut args, _| {
            let degrees = expect_int(args.pop().unwrap(), "color.adjustHue")?;
            let (r, g, b) = rgb_from_value(args.pop().unwrap(), "color.adjustHue")?;
            let hsl: Hsl = Hsl::from_color(Srgb::new(r, g, b));
            let hue = (hsl.hue.into_degrees() + degrees as f32).rem_euclid(360.0);
            let next = Hsl::new(RgbHue::from_degrees(hue), hsl.saturation, hsl.lightness);
            Ok(rgb_to_value(Srgb::from_color(next)))
        }),
    );
    fields.insert(
        "toRgb".to_string(),
        builtin("color.toRgb", 1, |mut args, _| {
            let hsl = hsl_from_value(args.pop().unwrap(), "color.toRgb")?;
            Ok(rgb_to_value(Srgb::from_color(hsl)))
        }),
    );
    fields.insert(
        "toHsl".to_string(),
        builtin("color.toHsl", 1, |mut args, _| {
            let (r, g, b) = rgb_from_value(args.pop().unwrap(), "color.toHsl")?;
            let hsl: Hsl = Hsl::from_color(Srgb::new(r, g, b));
            Ok(hsl_to_value(hsl))
        }),
    );
    fields.insert(
        "toHex".to_string(),
        builtin("color.toHex", 1, |mut args, _| {
            let (r, g, b) = rgb_from_value(args.pop().unwrap(), "color.toHex")?;
            let r = (r * 255.0).round().clamp(0.0, 255.0) as u8;
            let g = (g * 255.0).round().clamp(0.0, 255.0) as u8;
            let b = (b * 255.0).round().clamp(0.0, 255.0) as u8;
            Ok(Value::Text(format!("#{r:02x}{g:02x}{b:02x}")))
        }),
    );
    Value::Record(Arc::new(fields))
}

fn rgb_from_value(value: Value, ctx: &str) -> Result<(f32, f32, f32), RuntimeError> {
    let Value::Record(fields) = value else {
        return Err(RuntimeError::Message(format!("{ctx} expects Rgb")));
    };
    let r = expect_int(
        fields
            .get("r")
            .cloned()
            .ok_or_else(|| RuntimeError::Message(format!("{ctx} expects Rgb.r")))?,
        ctx,
    )?;
    let g = expect_int(
        fields
            .get("g")
            .cloned()
            .ok_or_else(|| RuntimeError::Message(format!("{ctx} expects Rgb.g")))?,
        ctx,
    )?;
    let b = expect_int(
        fields
            .get("b")
            .cloned()
            .ok_or_else(|| RuntimeError::Message(format!("{ctx} expects Rgb.b")))?,
        ctx,
    )?;
    let clamp = |v: i64| v.clamp(0, 255) as f32 / 255.0;
    Ok((clamp(r), clamp(g), clamp(b)))
}

fn rgb_to_value(rgb: Srgb<f32>) -> Value {
    let r = (rgb.red * 255.0).round().clamp(0.0, 255.0) as i64;
    let g = (rgb.green * 255.0).round().clamp(0.0, 255.0) as i64;
    let b = (rgb.blue * 255.0).round().clamp(0.0, 255.0) as i64;
    let mut map = HashMap::new();
    map.insert("r".to_string(), Value::Int(r));
    map.insert("g".to_string(), Value::Int(g));
    map.insert("b".to_string(), Value::Int(b));
    Value::Record(Arc::new(map))
}

fn hsl_from_value(value: Value, ctx: &str) -> Result<Hsl, RuntimeError> {
    let Value::Record(fields) = value else {
        return Err(RuntimeError::Message(format!("{ctx} expects Hsl")));
    };
    let h = expect_float(
        fields
            .get("h")
            .cloned()
            .ok_or_else(|| RuntimeError::Message(format!("{ctx} expects Hsl.h")))?,
        ctx,
    )?;
    let s = expect_float(
        fields
            .get("s")
            .cloned()
            .ok_or_else(|| RuntimeError::Message(format!("{ctx} expects Hsl.s")))?,
        ctx,
    )?;
    let l = expect_float(
        fields
            .get("l")
            .cloned()
            .ok_or_else(|| RuntimeError::Message(format!("{ctx} expects Hsl.l")))?,
        ctx,
    )?;
    let hue = RgbHue::from_degrees(h as f32);
    let s = s.clamp(0.0, 1.0) as f32;
    let l = l.clamp(0.0, 1.0) as f32;
    Ok(Hsl::new(hue, s, l))
}

fn hsl_to_value(hsl: Hsl) -> Value {
    let mut map = HashMap::new();
    map.insert("h".to_string(), Value::Float(hsl.hue.into_degrees() as f64));
    map.insert("s".to_string(), Value::Float(hsl.saturation as f64));
    map.insert("l".to_string(), Value::Float(hsl.lightness as f64));
    Value::Record(Arc::new(map))
}
