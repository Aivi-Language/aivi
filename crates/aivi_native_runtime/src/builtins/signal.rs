use std::collections::HashMap;
use std::sync::Arc;

use rustfft::{num_complex::Complex as FftComplex, FftPlanner};

use super::util::{builtin, expect_float, expect_list, expect_record, list_floats};
use crate::{RuntimeError, Value};

pub(super) fn build_signal_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "fft".to_string(),
        builtin("signal.fft", 1, |mut args, _| {
            let (samples, rate) = signal_from_value(args.pop().unwrap(), "signal.fft")?;
            if samples.is_empty() {
                return Ok(spectrum_to_value(Vec::new(), rate));
            }
            let mut planner = FftPlanner::new();
            let fft = planner.plan_fft_forward(samples.len());
            let mut buffer: Vec<FftComplex<f64>> = samples
                .into_iter()
                .map(|value| FftComplex::new(value, 0.0))
                .collect();
            fft.process(&mut buffer);
            Ok(spectrum_to_value(buffer, rate))
        }),
    );
    fields.insert(
        "ifft".to_string(),
        builtin("signal.ifft", 1, |mut args, _| {
            let (mut bins, rate) = spectrum_from_value(args.pop().unwrap(), "signal.ifft")?;
            if bins.is_empty() {
                return Ok(signal_to_value(Vec::new(), rate));
            }
            let mut planner = FftPlanner::new();
            let fft = planner.plan_fft_inverse(bins.len());
            fft.process(&mut bins);
            let scale = bins.len() as f64;
            let samples = bins.into_iter().map(|value| value.re / scale).collect();
            Ok(signal_to_value(samples, rate))
        }),
    );
    fields.insert(
        "windowHann".to_string(),
        builtin("signal.windowHann", 1, |mut args, _| {
            let (samples, rate) = signal_from_value(args.pop().unwrap(), "signal.windowHann")?;
            let len = samples.len();
            if len == 0 {
                return Ok(signal_to_value(samples, rate));
            }
            let denom = (len - 1) as f64;
            let mut out = Vec::with_capacity(len);
            for (i, value) in samples.into_iter().enumerate() {
                let phase = 2.0 * std::f64::consts::PI * (i as f64) / denom;
                let w = 0.5 * (1.0 - phase.cos());
                out.push(value * w);
            }
            Ok(signal_to_value(out, rate))
        }),
    );
    fields.insert(
        "normalize".to_string(),
        builtin("signal.normalize", 1, |mut args, _| {
            let (samples, rate) = signal_from_value(args.pop().unwrap(), "signal.normalize")?;
            let mut max = 0.0;
            for value in &samples {
                let abs = value.abs();
                if abs > max {
                    max = abs;
                }
            }
            if max == 0.0 {
                return Ok(signal_to_value(samples, rate));
            }
            let out = samples.into_iter().map(|value| value / max).collect();
            Ok(signal_to_value(out, rate))
        }),
    );
    Value::Record(Arc::new(fields))
}
fn signal_from_value(value: Value, ctx: &str) -> Result<(Vec<f64>, f64), RuntimeError> {
    let record = expect_record(value, ctx)?;
    let rate = match record.get("rate") {
        Some(value) => expect_float(value.clone(), ctx)?,
        None => return Err(RuntimeError::Message(format!("{ctx} expects Signal.rate"))),
    };
    let samples_list = match record.get("samples") {
        Some(value) => expect_list(value.clone(), ctx)?,
        None => {
            return Err(RuntimeError::Message(format!(
                "{ctx} expects Signal.samples"
            )))
        }
    };
    let samples = list_floats(&samples_list, ctx)?;
    Ok((samples, rate))
}
fn signal_to_value(samples: Vec<f64>, rate: f64) -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "samples".to_string(),
        Value::List(Arc::new(samples.into_iter().map(Value::Float).collect())),
    );
    fields.insert("rate".to_string(), Value::Float(rate));
    Value::Record(Arc::new(fields))
}
fn spectrum_from_value(
    value: Value,
    ctx: &str,
) -> Result<(Vec<FftComplex<f64>>, f64), RuntimeError> {
    let record = expect_record(value, ctx)?;
    let rate = match record.get("rate") {
        Some(value) => expect_float(value.clone(), ctx)?,
        None => {
            return Err(RuntimeError::Message(format!(
                "{ctx} expects Spectrum.rate"
            )))
        }
    };
    let bins_list = match record.get("bins") {
        Some(value) => expect_list(value.clone(), ctx)?,
        None => {
            return Err(RuntimeError::Message(format!(
                "{ctx} expects Spectrum.bins"
            )))
        }
    };
    let mut bins = Vec::with_capacity(bins_list.len());
    for item in bins_list.iter() {
        let record = expect_record(item.clone(), ctx)?;
        let re = match record.get("re") {
            Some(value) => expect_float(value.clone(), ctx)?,
            None => return Err(RuntimeError::Message(format!("{ctx} expects Complex.re"))),
        };
        let im = match record.get("im") {
            Some(value) => expect_float(value.clone(), ctx)?,
            None => return Err(RuntimeError::Message(format!("{ctx} expects Complex.im"))),
        };
        bins.push(FftComplex::new(re, im));
    }
    Ok((bins, rate))
}
fn spectrum_to_value(bins: Vec<FftComplex<f64>>, rate: f64) -> Value {
    let mut fields = HashMap::new();
    let list = bins
        .into_iter()
        .map(|value| {
            let mut complex = HashMap::new();
            complex.insert("re".to_string(), Value::Float(value.re));
            complex.insert("im".to_string(), Value::Float(value.im));
            Value::Record(Arc::new(complex))
        })
        .collect();
    fields.insert("bins".to_string(), Value::List(Arc::new(list)));
    fields.insert("rate".to_string(), Value::Float(rate));
    Value::Record(Arc::new(fields))
}
