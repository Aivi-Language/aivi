use std::collections::HashMap;
use std::sync::Arc;

use super::util::{builtin, expect_int, expect_list, expect_record, list_floats};
use crate::{RuntimeError, Value};

pub(super) fn build_linalg_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "dot".to_string(),
        builtin("linalg.dot", 2, |mut args, _| {
            let (_, left) = vec_from_value(args.pop().unwrap(), "linalg.dot")?;
            let (_, right) = vec_from_value(args.pop().unwrap(), "linalg.dot")?;
            if left.len() != right.len() {
                return Err(RuntimeError::Message(
                    "linalg.dot expects vectors of equal size".to_string(),
                ));
            }
            let sum: f64 = left.iter().zip(right.iter()).map(|(a, b)| a * b).sum();
            Ok(Value::Float(sum))
        }),
    );
    fields.insert(
        "matMul".to_string(),
        builtin("linalg.matMul", 2, |mut args, _| {
            let (rows_b, cols_b, data_b) = mat_from_value(args.pop().unwrap(), "linalg.matMul")?;
            let (rows_a, cols_a, data_a) = mat_from_value(args.pop().unwrap(), "linalg.matMul")?;
            if cols_a != rows_b {
                return Err(RuntimeError::Message(
                    "linalg.matMul expects matching dimensions".to_string(),
                ));
            }
            let mut out = vec![0.0; (rows_a * cols_b) as usize];
            let rows_a_usize = rows_a as usize;
            let cols_a_usize = cols_a as usize;
            let cols_b_usize = cols_b as usize;
            for r in 0..rows_a_usize {
                for c in 0..cols_b_usize {
                    let mut acc = 0.0;
                    for k in 0..cols_a_usize {
                        let a = data_a[r * cols_a_usize + k];
                        let b = data_b[k * cols_b_usize + c];
                        acc += a * b;
                    }
                    out[r * cols_b_usize + c] = acc;
                }
            }
            Ok(mat_to_value(rows_a, cols_b, out))
        }),
    );
    fields.insert(
        "solve2x2".to_string(),
        builtin("linalg.solve2x2", 2, |mut args, _| {
            let (_, vec) = vec_from_value(args.pop().unwrap(), "linalg.solve2x2")?;
            let (rows, cols, mat) = mat_from_value(args.pop().unwrap(), "linalg.solve2x2")?;
            if rows != 2 || cols != 2 || vec.len() != 2 {
                return Err(RuntimeError::Message(
                    "linalg.solve2x2 expects 2x2 matrix and size-2 vector".to_string(),
                ));
            }
            let a = mat[0];
            let b = mat[1];
            let c = mat[2];
            let d = mat[3];
            let det = a * d - b * c;
            if det == 0.0 {
                return Err(RuntimeError::Message(
                    "linalg.solve2x2 determinant is zero".to_string(),
                ));
            }
            let x = (d * vec[0] - b * vec[1]) / det;
            let y = (-c * vec[0] + a * vec[1]) / det;
            Ok(vec_to_value(2, vec![x, y]))
        }),
    );
    Value::Record(Arc::new(fields))
}
fn vec_from_value(value: Value, ctx: &str) -> Result<(i64, Vec<f64>), RuntimeError> {
    let record = expect_record(value, ctx)?;
    let size = match record.get("size") {
        Some(value) => expect_int(value.clone(), ctx)?,
        None => return Err(RuntimeError::Message(format!("{ctx} expects Vec.size"))),
    };
    let data_list = match record.get("data") {
        Some(value) => expect_list(value.clone(), ctx)?,
        None => return Err(RuntimeError::Message(format!("{ctx} expects Vec.data"))),
    };
    let data = list_floats(&data_list, ctx)?;
    if size < 0 || data.len() != size as usize {
        return Err(RuntimeError::Message(format!(
            "{ctx} Vec.size does not match data length"
        )));
    }
    Ok((size, data))
}
fn vec_to_value(size: i64, data: Vec<f64>) -> Value {
    let mut fields = HashMap::new();
    fields.insert("size".to_string(), Value::Int(size));
    let list = data.into_iter().map(Value::Float).collect();
    fields.insert("data".to_string(), Value::List(Arc::new(list)));
    Value::Record(Arc::new(fields))
}
fn mat_from_value(value: Value, ctx: &str) -> Result<(i64, i64, Vec<f64>), RuntimeError> {
    let record = expect_record(value, ctx)?;
    let rows = match record.get("rows") {
        Some(value) => expect_int(value.clone(), ctx)?,
        None => return Err(RuntimeError::Message(format!("{ctx} expects Mat.rows"))),
    };
    let cols = match record.get("cols") {
        Some(value) => expect_int(value.clone(), ctx)?,
        None => return Err(RuntimeError::Message(format!("{ctx} expects Mat.cols"))),
    };
    let data_list = match record.get("data") {
        Some(value) => expect_list(value.clone(), ctx)?,
        None => return Err(RuntimeError::Message(format!("{ctx} expects Mat.data"))),
    };
    let data = list_floats(&data_list, ctx)?;
    if rows < 0 || cols < 0 || data.len() != (rows * cols) as usize {
        return Err(RuntimeError::Message(format!(
            "{ctx} Mat dimensions do not match data length"
        )));
    }
    Ok((rows, cols, data))
}
fn mat_to_value(rows: i64, cols: i64, data: Vec<f64>) -> Value {
    let mut fields = HashMap::new();
    fields.insert("rows".to_string(), Value::Int(rows));
    fields.insert("cols".to_string(), Value::Int(cols));
    let list = data.into_iter().map(Value::Float).collect();
    fields.insert("data".to_string(), Value::List(Arc::new(list)));
    Value::Record(Arc::new(fields))
}
