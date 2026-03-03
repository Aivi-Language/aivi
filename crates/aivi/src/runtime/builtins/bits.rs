use std::collections::HashMap;
use std::sync::Arc;

use super::util::{builtin, expect_bytes, expect_int};
use crate::runtime::{RuntimeError, Value};

pub(super) fn build_bits_record() -> Value {
    let mut fields = HashMap::new();

    // bits.fromInt : Int -> Bytes
    fields.insert(
        "fromInt".to_string(),
        builtin("bits.fromInt", 1, |mut args, _| {
            let n = expect_int(args.pop().unwrap(), "bits.fromInt")?;
            Ok(Value::Bytes(Arc::new(n.to_be_bytes().to_vec())))
        }),
    );

    // bits.toInt : Bytes -> Int
    fields.insert(
        "toInt".to_string(),
        builtin("bits.toInt", 1, |mut args, _| {
            let bytes = expect_bytes(args.pop().unwrap(), "bits.toInt")?;
            if bytes.len() > 8 {
                return Err(RuntimeError::Message(
                    "bits.toInt: Bits value exceeds 8 bytes (64 bits)".to_string(),
                ));
            }
            let mut buf = [0u8; 8];
            let offset = 8 - bytes.len();
            buf[offset..].copy_from_slice(&bytes);
            Ok(Value::Int(i64::from_be_bytes(buf)))
        }),
    );

    // bits.fromBytes : Bytes -> Bytes (identity — semantic alias)
    fields.insert(
        "fromBytes".to_string(),
        builtin("bits.fromBytes", 1, |mut args, _| {
            let bytes = expect_bytes(args.pop().unwrap(), "bits.fromBytes")?;
            Ok(Value::Bytes(bytes))
        }),
    );

    // bits.toBytes : Bytes -> Bytes (identity — semantic alias)
    fields.insert(
        "toBytes".to_string(),
        builtin("bits.toBytes", 1, |mut args, _| {
            let bytes = expect_bytes(args.pop().unwrap(), "bits.toBytes")?;
            Ok(Value::Bytes(bytes))
        }),
    );

    // bits.zero : Int -> Bytes
    fields.insert(
        "zero".to_string(),
        builtin("bits.zero", 1, |mut args, _| {
            let n = expect_int(args.pop().unwrap(), "bits.zero")?;
            if n < 0 {
                return Err(RuntimeError::Message(
                    "bits.zero: byte count must be non-negative".to_string(),
                ));
            }
            let n = n as usize;
            Ok(Value::Bytes(Arc::new(vec![0u8; n])))
        }),
    );

    // bits.ones : Int -> Bytes
    fields.insert(
        "ones".to_string(),
        builtin("bits.ones", 1, |mut args, _| {
            let n = expect_int(args.pop().unwrap(), "bits.ones")?;
            if n < 0 {
                return Err(RuntimeError::Message(
                    "bits.ones: byte count must be non-negative".to_string(),
                ));
            }
            let n = n as usize;
            Ok(Value::Bytes(Arc::new(vec![0xFFu8; n])))
        }),
    );

    // bits.and : Bytes -> Bytes -> Bytes
    fields.insert(
        "and".to_string(),
        builtin("bits.and", 2, |mut args, _| {
            let a = expect_bytes(args.pop().unwrap(), "bits.and")?;
            let b = expect_bytes(args.pop().unwrap(), "bits.and")?;
            let len = a.len().max(b.len());
            let mut result = vec![0u8; len];
            for i in 0..len {
                let va = if i < a.len() { a[i] } else { 0 };
                let vb = if i < b.len() { b[i] } else { 0 };
                result[i] = va & vb;
            }
            Ok(Value::Bytes(Arc::new(result)))
        }),
    );

    // bits.or : Bytes -> Bytes -> Bytes
    fields.insert(
        "or".to_string(),
        builtin("bits.or", 2, |mut args, _| {
            let a = expect_bytes(args.pop().unwrap(), "bits.or")?;
            let b = expect_bytes(args.pop().unwrap(), "bits.or")?;
            let len = a.len().max(b.len());
            let mut result = vec![0u8; len];
            for i in 0..len {
                let va = if i < a.len() { a[i] } else { 0 };
                let vb = if i < b.len() { b[i] } else { 0 };
                result[i] = va | vb;
            }
            Ok(Value::Bytes(Arc::new(result)))
        }),
    );

    // bits.xor : Bytes -> Bytes -> Bytes
    fields.insert(
        "xor".to_string(),
        builtin("bits.xor", 2, |mut args, _| {
            let a = expect_bytes(args.pop().unwrap(), "bits.xor")?;
            let b = expect_bytes(args.pop().unwrap(), "bits.xor")?;
            let len = a.len().max(b.len());
            let mut result = vec![0u8; len];
            for i in 0..len {
                let va = if i < a.len() { a[i] } else { 0 };
                let vb = if i < b.len() { b[i] } else { 0 };
                result[i] = va ^ vb;
            }
            Ok(Value::Bytes(Arc::new(result)))
        }),
    );

    // bits.complement : Bytes -> Bytes
    fields.insert(
        "complement".to_string(),
        builtin("bits.complement", 1, |mut args, _| {
            let a = expect_bytes(args.pop().unwrap(), "bits.complement")?;
            let result: Vec<u8> = a.iter().map(|b| !b).collect();
            Ok(Value::Bytes(Arc::new(result)))
        }),
    );

    // bits.shiftLeft : Int -> Bytes -> Bytes
    fields.insert(
        "shiftLeft".to_string(),
        builtin("bits.shiftLeft", 2, |mut args, _| {
            let data = expect_bytes(args.pop().unwrap(), "bits.shiftLeft")?;
            let n = expect_int(args.pop().unwrap(), "bits.shiftLeft")?;
            if n < 0 {
                return Err(RuntimeError::Message(
                    "bits.shiftLeft: shift amount must be non-negative".to_string(),
                ));
            }
            let n = n as usize;
            Ok(Value::Bytes(Arc::new(shift_left(&data, n))))
        }),
    );

    // bits.shiftRight : Int -> Bytes -> Bytes
    fields.insert(
        "shiftRight".to_string(),
        builtin("bits.shiftRight", 2, |mut args, _| {
            let data = expect_bytes(args.pop().unwrap(), "bits.shiftRight")?;
            let n = expect_int(args.pop().unwrap(), "bits.shiftRight")?;
            if n < 0 {
                return Err(RuntimeError::Message(
                    "bits.shiftRight: shift amount must be non-negative".to_string(),
                ));
            }
            let n = n as usize;
            Ok(Value::Bytes(Arc::new(shift_right(&data, n))))
        }),
    );

    // bits.get : Int -> Bytes -> Bool
    fields.insert(
        "get".to_string(),
        builtin("bits.get", 2, |mut args, _| {
            let data = expect_bytes(args.pop().unwrap(), "bits.get")?;
            let idx = expect_int(args.pop().unwrap(), "bits.get")?;
            if idx < 0 {
                return Err(RuntimeError::Message(
                    "bits.get: index must be non-negative".to_string(),
                ));
            }
            let idx = idx as usize;
            let total_bits = data.len() * 8;
            if idx >= total_bits {
                return Ok(Value::Bool(false));
            }
            let byte_idx = idx / 8;
            let bit_idx = 7 - (idx % 8); // MSB-first
            Ok(Value::Bool((data[byte_idx] >> bit_idx) & 1 == 1))
        }),
    );

    // bits.set : Int -> Bytes -> Bytes
    fields.insert(
        "set".to_string(),
        builtin("bits.set", 2, |mut args, _| {
            let data = expect_bytes(args.pop().unwrap(), "bits.set")?;
            let idx = expect_int(args.pop().unwrap(), "bits.set")?;
            if idx < 0 {
                return Err(RuntimeError::Message(
                    "bits.set: index must be non-negative".to_string(),
                ));
            }
            let idx = idx as usize;
            let total_bits = data.len() * 8;
            if idx >= total_bits {
                return Err(RuntimeError::Message(format!(
                    "bits.set: index {idx} out of range (0..{total_bits})"
                )));
            }
            let mut result = data.as_ref().clone();
            let byte_idx = idx / 8;
            let bit_idx = 7 - (idx % 8);
            result[byte_idx] |= 1 << bit_idx;
            Ok(Value::Bytes(Arc::new(result)))
        }),
    );

    // bits.clear : Int -> Bytes -> Bytes
    fields.insert(
        "clear".to_string(),
        builtin("bits.clear", 2, |mut args, _| {
            let data = expect_bytes(args.pop().unwrap(), "bits.clear")?;
            let idx = expect_int(args.pop().unwrap(), "bits.clear")?;
            if idx < 0 {
                return Err(RuntimeError::Message(
                    "bits.clear: index must be non-negative".to_string(),
                ));
            }
            let idx = idx as usize;
            let total_bits = data.len() * 8;
            if idx >= total_bits {
                return Err(RuntimeError::Message(format!(
                    "bits.clear: index {idx} out of range (0..{total_bits})"
                )));
            }
            let mut result = data.as_ref().clone();
            let byte_idx = idx / 8;
            let bit_idx = 7 - (idx % 8);
            result[byte_idx] &= !(1 << bit_idx);
            Ok(Value::Bytes(Arc::new(result)))
        }),
    );

    // bits.toggle : Int -> Bytes -> Bytes
    fields.insert(
        "toggle".to_string(),
        builtin("bits.toggle", 2, |mut args, _| {
            let data = expect_bytes(args.pop().unwrap(), "bits.toggle")?;
            let idx = expect_int(args.pop().unwrap(), "bits.toggle")?;
            if idx < 0 {
                return Err(RuntimeError::Message(
                    "bits.toggle: index must be non-negative".to_string(),
                ));
            }
            let idx = idx as usize;
            let total_bits = data.len() * 8;
            if idx >= total_bits {
                return Err(RuntimeError::Message(format!(
                    "bits.toggle: index {idx} out of range (0..{total_bits})"
                )));
            }
            let mut result = data.as_ref().clone();
            let byte_idx = idx / 8;
            let bit_idx = 7 - (idx % 8);
            result[byte_idx] ^= 1 << bit_idx;
            Ok(Value::Bytes(Arc::new(result)))
        }),
    );

    // bits.length : Bytes -> Int (bit count = bytes * 8)
    fields.insert(
        "length".to_string(),
        builtin("bits.length", 1, |mut args, _| {
            let data = expect_bytes(args.pop().unwrap(), "bits.length")?;
            Ok(Value::Int((data.len() * 8) as i64))
        }),
    );

    // bits.slice : Int -> Int -> Bytes -> Bytes
    // Extract bytes from startByte to endByte (exclusive)
    fields.insert(
        "slice".to_string(),
        builtin("bits.slice", 3, |mut args, _| {
            let data = expect_bytes(args.pop().unwrap(), "bits.slice")?;
            let start = expect_int(args.pop().unwrap(), "bits.slice")? as usize;
            let end = expect_int(args.pop().unwrap(), "bits.slice")? as usize;
            if start > data.len() || end > data.len() || start > end {
                return Err(RuntimeError::Message(format!(
                    "bits.slice: range {start}..{end} out of bounds for {} bytes",
                    data.len()
                )));
            }
            Ok(Value::Bytes(Arc::new(data[start..end].to_vec())))
        }),
    );

    // bits.popCount : Bytes -> Int (number of set bits)
    fields.insert(
        "popCount".to_string(),
        builtin("bits.popCount", 1, |mut args, _| {
            let data = expect_bytes(args.pop().unwrap(), "bits.popCount")?;
            let count: u32 = data.iter().map(|b| b.count_ones()).sum();
            Ok(Value::Int(count as i64))
        }),
    );

    Value::Record(Arc::new(fields))
}

fn shift_left(data: &[u8], n: usize) -> Vec<u8> {
    if data.is_empty() || n == 0 {
        return data.to_vec();
    }
    let total_bits = data.len() * 8;
    if n >= total_bits {
        return vec![0u8; data.len()];
    }
    let byte_shift = n / 8;
    let bit_shift = n % 8;
    let len = data.len();
    let mut result = vec![0u8; len];
    for (i, byte) in result.iter_mut().enumerate() {
        let src = i + byte_shift;
        if src < len {
            *byte = data[src] << bit_shift;
            if bit_shift > 0 && src + 1 < len {
                *byte |= data[src + 1] >> (8 - bit_shift);
            }
        }
    }
    result
}

fn shift_right(data: &[u8], n: usize) -> Vec<u8> {
    if data.is_empty() || n == 0 {
        return data.to_vec();
    }
    let total_bits = data.len() * 8;
    if n >= total_bits {
        return vec![0u8; data.len()];
    }
    let byte_shift = n / 8;
    let bit_shift = n % 8;
    let len = data.len();
    let mut result = vec![0u8; len];
    for i in (0..len).rev() {
        if i >= byte_shift {
            let src = i - byte_shift;
            result[i] = data[src] >> bit_shift;
            if bit_shift > 0 && src > 0 {
                result[i] |= data[src - 1] << (8 - bit_shift);
            }
        }
    }
    result
}
