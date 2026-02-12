use std::collections::HashMap;
use std::sync::Arc;

use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{ToPrimitive, Zero};
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::Decimal;

use super::util::{
    builtin, expect_bigint, expect_decimal, expect_float, expect_int, expect_rational,
};
use crate::runtime::{RuntimeError, Value};

pub(super) fn build_bigint_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "fromInt".to_string(),
        builtin("bigint.fromInt", 1, |mut args, _| {
            let value = expect_int(args.pop().unwrap(), "bigint.fromInt")?;
            Ok(Value::BigInt(Arc::new(BigInt::from(value))))
        }),
    );
    fields.insert(
        "toInt".to_string(),
        builtin("bigint.toInt", 1, |mut args, _| {
            let value = expect_bigint(args.pop().unwrap(), "bigint.toInt")?;
            let out = value
                .to_i64()
                .ok_or_else(|| RuntimeError::Message("bigint.toInt overflow".to_string()))?;
            Ok(Value::Int(out))
        }),
    );
    fields.insert(
        "add".to_string(),
        builtin("bigint.add", 2, |mut args, _| {
            let right = expect_bigint(args.pop().unwrap(), "bigint.add")?;
            let left = expect_bigint(args.pop().unwrap(), "bigint.add")?;
            Ok(Value::BigInt(Arc::new(&*left + &*right)))
        }),
    );
    fields.insert(
        "sub".to_string(),
        builtin("bigint.sub", 2, |mut args, _| {
            let right = expect_bigint(args.pop().unwrap(), "bigint.sub")?;
            let left = expect_bigint(args.pop().unwrap(), "bigint.sub")?;
            Ok(Value::BigInt(Arc::new(&*left - &*right)))
        }),
    );
    fields.insert(
        "mul".to_string(),
        builtin("bigint.mul", 2, |mut args, _| {
            let right = expect_bigint(args.pop().unwrap(), "bigint.mul")?;
            let left = expect_bigint(args.pop().unwrap(), "bigint.mul")?;
            Ok(Value::BigInt(Arc::new(&*left * &*right)))
        }),
    );
    Value::Record(Arc::new(fields))
}
pub(super) fn build_rational_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "fromBigInts".to_string(),
        builtin("rational.fromBigInts", 2, |mut args, _| {
            let denom = expect_bigint(args.pop().unwrap(), "rational.fromBigInts")?;
            let numer = expect_bigint(args.pop().unwrap(), "rational.fromBigInts")?;
            if denom.is_zero() {
                return Err(RuntimeError::Message(
                    "rational.fromBigInts expects non-zero denominator".to_string(),
                ));
            }
            Ok(Value::Rational(Arc::new(BigRational::new(
                (*numer).clone(),
                (*denom).clone(),
            ))))
        }),
    );
    fields.insert(
        "normalize".to_string(),
        builtin("rational.normalize", 1, |mut args, _| {
            let value = expect_rational(args.pop().unwrap(), "rational.normalize")?;
            Ok(Value::Rational(Arc::new((*value).clone())))
        }),
    );
    fields.insert(
        "numerator".to_string(),
        builtin("rational.numerator", 1, |mut args, _| {
            let value = expect_rational(args.pop().unwrap(), "rational.numerator")?;
            Ok(Value::BigInt(Arc::new(value.numer().clone())))
        }),
    );
    fields.insert(
        "denominator".to_string(),
        builtin("rational.denominator", 1, |mut args, _| {
            let value = expect_rational(args.pop().unwrap(), "rational.denominator")?;
            Ok(Value::BigInt(Arc::new(value.denom().clone())))
        }),
    );
    fields.insert(
        "add".to_string(),
        builtin("rational.add", 2, |mut args, _| {
            let right = expect_rational(args.pop().unwrap(), "rational.add")?;
            let left = expect_rational(args.pop().unwrap(), "rational.add")?;
            Ok(Value::Rational(Arc::new(&*left + &*right)))
        }),
    );
    fields.insert(
        "sub".to_string(),
        builtin("rational.sub", 2, |mut args, _| {
            let right = expect_rational(args.pop().unwrap(), "rational.sub")?;
            let left = expect_rational(args.pop().unwrap(), "rational.sub")?;
            Ok(Value::Rational(Arc::new(&*left - &*right)))
        }),
    );
    fields.insert(
        "mul".to_string(),
        builtin("rational.mul", 2, |mut args, _| {
            let right = expect_rational(args.pop().unwrap(), "rational.mul")?;
            let left = expect_rational(args.pop().unwrap(), "rational.mul")?;
            Ok(Value::Rational(Arc::new(&*left * &*right)))
        }),
    );
    fields.insert(
        "div".to_string(),
        builtin("rational.div", 2, |mut args, _| {
            let right = expect_rational(args.pop().unwrap(), "rational.div")?;
            let left = expect_rational(args.pop().unwrap(), "rational.div")?;
            Ok(Value::Rational(Arc::new(&*left / &*right)))
        }),
    );
    Value::Record(Arc::new(fields))
}
pub(super) fn build_decimal_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "fromFloat".to_string(),
        builtin("decimal.fromFloat", 1, |mut args, _| {
            let value = expect_float(args.pop().unwrap(), "decimal.fromFloat")?;
            let decimal = Decimal::from_f64(value).ok_or_else(|| {
                RuntimeError::Message("decimal.fromFloat expects finite Float".to_string())
            })?;
            Ok(Value::Decimal(decimal))
        }),
    );
    fields.insert(
        "toFloat".to_string(),
        builtin("decimal.toFloat", 1, |mut args, _| {
            let value = expect_decimal(args.pop().unwrap(), "decimal.toFloat")?;
            let out = value
                .to_f64()
                .ok_or_else(|| RuntimeError::Message("decimal.toFloat overflow".to_string()))?;
            Ok(Value::Float(out))
        }),
    );
    fields.insert(
        "round".to_string(),
        builtin("decimal.round", 2, |mut args, _| {
            let places = expect_int(args.pop().unwrap(), "decimal.round")?;
            let value = expect_decimal(args.pop().unwrap(), "decimal.round")?;
            let places = places.max(0) as u32;
            Ok(Value::Decimal(value.round_dp(places)))
        }),
    );
    fields.insert(
        "add".to_string(),
        builtin("decimal.add", 2, |mut args, _| {
            let right = expect_decimal(args.pop().unwrap(), "decimal.add")?;
            let left = expect_decimal(args.pop().unwrap(), "decimal.add")?;
            Ok(Value::Decimal(left + right))
        }),
    );
    fields.insert(
        "sub".to_string(),
        builtin("decimal.sub", 2, |mut args, _| {
            let right = expect_decimal(args.pop().unwrap(), "decimal.sub")?;
            let left = expect_decimal(args.pop().unwrap(), "decimal.sub")?;
            Ok(Value::Decimal(left - right))
        }),
    );
    fields.insert(
        "mul".to_string(),
        builtin("decimal.mul", 2, |mut args, _| {
            let right = expect_decimal(args.pop().unwrap(), "decimal.mul")?;
            let left = expect_decimal(args.pop().unwrap(), "decimal.mul")?;
            Ok(Value::Decimal(left * right))
        }),
    );
    fields.insert(
        "div".to_string(),
        builtin("decimal.div", 2, |mut args, _| {
            let right = expect_decimal(args.pop().unwrap(), "decimal.div")?;
            let left = expect_decimal(args.pop().unwrap(), "decimal.div")?;
            Ok(Value::Decimal(left / right))
        }),
    );
    Value::Record(Arc::new(fields))
}
