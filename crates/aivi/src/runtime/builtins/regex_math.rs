fn build_regex_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "compile".to_string(),
        builtin("regex.compile", 1, |mut args, _| {
            let pattern = expect_text(args.pop().unwrap(), "regex.compile")?;
            match Regex::new(&pattern) {
                Ok(regex) => Ok(make_ok(Value::Regex(Arc::new(regex)))),
                Err(err) => Ok(make_err(Value::Constructor {
                    name: "InvalidPattern".to_string(),
                    args: vec![Value::Text(err.to_string())],
                })),
            }
        }),
    );
    fields.insert(
        "test".to_string(),
        builtin("regex.test", 2, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "regex.test")?;
            let regex = expect_regex(args.pop().unwrap(), "regex.test")?;
            Ok(Value::Bool(regex.is_match(&text)))
        }),
    );
    fields.insert(
        "match".to_string(),
        builtin("regex.match", 2, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "regex.match")?;
            let regex = expect_regex(args.pop().unwrap(), "regex.match")?;
            match regex.captures(&text) {
                Some(captures) => {
                    let full = captures.get(0).map(|m| m.as_str()).unwrap_or("");
                    let mut groups = Vec::new();
                    for idx in 1..captures.len() {
                        if let Some(matched) = captures.get(idx) {
                            groups.push(make_some(Value::Text(matched.as_str().to_string())));
                        } else {
                            groups.push(make_none());
                        }
                    }
                    let mut record = HashMap::new();
                    let (start, end) = captures.get(0).map(|m| (m.start(), m.end())).unwrap_or((0, 0));
                    record.insert("full".to_string(), Value::Text(full.to_string()));
                    record.insert("groups".to_string(), list_value(groups));
                    record.insert("start".to_string(), Value::Int(start as i64));
                    record.insert("end".to_string(), Value::Int(end as i64));
                    Ok(make_some(Value::Record(Arc::new(record))))
                }
                None => Ok(make_none()),
            }
        }),
    );
    fields.insert(
        "matches".to_string(),
        builtin("regex.matches", 2, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "regex.matches")?;
            let regex = expect_regex(args.pop().unwrap(), "regex.matches")?;
            let mut matches_out = Vec::new();
            for captures in regex.captures_iter(&text) {
                let full = captures.get(0).map(|m| m.as_str()).unwrap_or("");
                let mut groups = Vec::new();
                for idx in 1..captures.len() {
                    if let Some(matched) = captures.get(idx) {
                        groups.push(make_some(Value::Text(matched.as_str().to_string())));
                    } else {
                        groups.push(make_none());
                    }
                }
                let (start, end) = captures.get(0).map(|m| (m.start(), m.end())).unwrap_or((0, 0));
                let mut record = HashMap::new();
                record.insert("full".to_string(), Value::Text(full.to_string()));
                record.insert("groups".to_string(), list_value(groups));
                record.insert("start".to_string(), Value::Int(start as i64));
                record.insert("end".to_string(), Value::Int(end as i64));
                matches_out.push(Value::Record(Arc::new(record)));
            }
            Ok(list_value(matches_out))
        }),
    );
    fields.insert(
        "find".to_string(),
        builtin("regex.find", 2, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "regex.find")?;
            let regex = expect_regex(args.pop().unwrap(), "regex.find")?;
            match regex.find(&text) {
                Some(found) => Ok(make_some(Value::Tuple(vec![
                    Value::Int(found.start() as i64),
                    Value::Int(found.end() as i64),
                ]))),
                None => Ok(make_none()),
            }
        }),
    );
    fields.insert(
        "findAll".to_string(),
        builtin("regex.findAll", 2, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "regex.findAll")?;
            let regex = expect_regex(args.pop().unwrap(), "regex.findAll")?;
            let mut out = Vec::new();
            for found in regex.find_iter(&text) {
                out.push(Value::Tuple(vec![
                    Value::Int(found.start() as i64),
                    Value::Int(found.end() as i64),
                ]));
            }
            Ok(list_value(out))
        }),
    );
    fields.insert(
        "split".to_string(),
        builtin("regex.split", 2, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "regex.split")?;
            let regex = expect_regex(args.pop().unwrap(), "regex.split")?;
            let parts = regex
                .split(&text)
                .map(|part| Value::Text(part.to_string()))
                .collect::<Vec<_>>();
            Ok(list_value(parts))
        }),
    );
    fields.insert(
        "replace".to_string(),
        builtin("regex.replace", 3, |mut args, _| {
            let replacement = expect_text(args.pop().unwrap(), "regex.replace")?;
            let text = expect_text(args.pop().unwrap(), "regex.replace")?;
            let regex = expect_regex(args.pop().unwrap(), "regex.replace")?;
            Ok(Value::Text(regex.replacen(&text, 1, replacement).to_string()))
        }),
    );
    fields.insert(
        "replaceAll".to_string(),
        builtin("regex.replaceAll", 3, |mut args, _| {
            let replacement = expect_text(args.pop().unwrap(), "regex.replaceAll")?;
            let text = expect_text(args.pop().unwrap(), "regex.replaceAll")?;
            let regex = expect_regex(args.pop().unwrap(), "regex.replaceAll")?;
            Ok(Value::Text(regex.replace_all(&text, replacement).to_string()))
        }),
    );
    Value::Record(Arc::new(fields))
}

fn angle_from_value(value: Value, ctx: &str) -> Result<f64, RuntimeError> {
    let Value::Record(fields) = value else {
        return Err(RuntimeError::Message(format!("{ctx} expects Angle")));
    };
    let radians = fields
        .get("radians")
        .cloned()
        .ok_or_else(|| RuntimeError::Message(format!("{ctx} expects Angle.radians")))?;
    expect_float(radians, ctx)
}

fn angle_value(radians: f64) -> Value {
    let mut map = HashMap::new();
    map.insert("radians".to_string(), Value::Float(radians));
    Value::Record(Arc::new(map))
}

fn gcd_i64(mut a: i64, mut b: i64) -> i64 {
    a = a.abs();
    b = b.abs();
    while b != 0 {
        let r = a % b;
        a = b;
        b = r;
    }
    a
}

fn lcm_i64(a: i64, b: i64) -> i64 {
    if a == 0 || b == 0 {
        return 0;
    }
    (a / gcd_i64(a, b)) * b
}

fn mod_pow(mut base: i64, mut exp: i64, modulus: i64) -> i64 {
    if modulus == 1 {
        return 0;
    }
    let mut result: i64 = 1 % modulus;
    base %= modulus;
    while exp > 0 {
        if exp % 2 == 1 {
            result = (result * base) % modulus;
        }
        exp /= 2;
        base = (base * base) % modulus;
    }
    result
}

fn factorial_bigint(n: i64) -> Option<BigInt> {
    if n < 0 {
        return None;
    }
    let mut acc = BigInt::from(1);
    for i in 2..=n {
        acc *= i;
    }
    Some(acc)
}

fn comb_bigint(n: i64, k: i64) -> Option<BigInt> {
    if n < 0 || k < 0 || k > n {
        return None;
    }
    let k = std::cmp::min(k, n - k);
    let mut result = BigInt::from(1);
    for i in 0..k {
        result *= n - i;
        result /= i + 1;
    }
    Some(result)
}

fn perm_bigint(n: i64, k: i64) -> Option<BigInt> {
    if n < 0 || k < 0 || k > n {
        return None;
    }
    let mut result = BigInt::from(1);
    for i in 0..k {
        result *= n - i;
    }
    Some(result)
}

fn next_after(from: f64, to: f64) -> f64 {
    if from.is_nan() || to.is_nan() {
        return f64::NAN;
    }
    if from == to {
        return to;
    }
    if from == 0.0 {
        let tiny = f64::from_bits(1);
        return if to > 0.0 { tiny } else { -tiny };
    }
    let mut bits = from.to_bits();
    if (from < to) == (from > 0.0) {
        bits = bits.wrapping_add(1);
    } else {
        bits = bits.wrapping_sub(1);
    }
    f64::from_bits(bits)
}

fn frexp_value(value: f64) -> (f64, i64) {
    if value == 0.0 || value.is_nan() || value.is_infinite() {
        return (value, 0);
    }
    let exp = value.abs().log2().floor() as i64 + 1;
    let mantissa = value / 2.0_f64.powi(exp as i32);
    (mantissa, exp)
}

fn build_math_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert("pi".to_string(), Value::Float(std::f64::consts::PI));
    fields.insert("tau".to_string(), Value::Float(std::f64::consts::TAU));
    fields.insert("e".to_string(), Value::Float(std::f64::consts::E));
    fields.insert("inf".to_string(), Value::Float(f64::INFINITY));
    fields.insert("nan".to_string(), Value::Float(f64::NAN));
    fields.insert(
        "phi".to_string(),
        Value::Float((1.0 + 5.0_f64.sqrt()) / 2.0),
    );
    fields.insert("sqrt2".to_string(), Value::Float(std::f64::consts::SQRT_2));
    fields.insert("ln2".to_string(), Value::Float(std::f64::consts::LN_2));
    fields.insert("ln10".to_string(), Value::Float(std::f64::consts::LN_10));
    fields.insert(
        "abs".to_string(),
        builtin("math.abs", 1, |mut args, _| {
            let value = args.pop().unwrap();
            match value {
                Value::Int(value) => Ok(Value::Int(value.wrapping_abs())),
                Value::Float(value) => Ok(Value::Float(value.abs())),
                _ => Err(RuntimeError::Message("math.abs expects Int or Float".to_string())),
            }
        }),
    );
    fields.insert(
        "sign".to_string(),
        builtin("math.sign", 1, |mut args, _| {
            let value = expect_float(args.pop().unwrap(), "math.sign")?;
            let out = if value > 0.0 {
                1.0
            } else if value < 0.0 {
                -1.0
            } else {
                0.0
            };
            Ok(Value::Float(out))
        }),
    );
    fields.insert(
        "copysign".to_string(),
        builtin("math.copysign", 2, |mut args, _| {
            let sign = expect_float(args.pop().unwrap(), "math.copysign")?;
            let mag = expect_float(args.pop().unwrap(), "math.copysign")?;
            Ok(Value::Float(mag.copysign(sign)))
        }),
    );
    fields.insert(
        "min".to_string(),
        builtin("math.min", 2, |mut args, _| {
            let right = expect_float(args.pop().unwrap(), "math.min")?;
            let left = expect_float(args.pop().unwrap(), "math.min")?;
            Ok(Value::Float(left.min(right)))
        }),
    );
    fields.insert(
        "max".to_string(),
        builtin("math.max", 2, |mut args, _| {
            let right = expect_float(args.pop().unwrap(), "math.max")?;
            let left = expect_float(args.pop().unwrap(), "math.max")?;
            Ok(Value::Float(left.max(right)))
        }),
    );
    fields.insert(
        "minAll".to_string(),
        builtin("math.minAll", 1, |mut args, _| {
            let list = expect_list(args.pop().unwrap(), "math.minAll")?;
            let values = list_floats(&list, "math.minAll")?;
            if values.is_empty() {
                return Ok(make_none());
            }
            let mut min = values[0];
            for value in values.iter().skip(1) {
                min = min.min(*value);
            }
            Ok(make_some(Value::Float(min)))
        }),
    );
    fields.insert(
        "maxAll".to_string(),
        builtin("math.maxAll", 1, |mut args, _| {
            let list = expect_list(args.pop().unwrap(), "math.maxAll")?;
            let values = list_floats(&list, "math.maxAll")?;
            if values.is_empty() {
                return Ok(make_none());
            }
            let mut max = values[0];
            for value in values.iter().skip(1) {
                max = max.max(*value);
            }
            Ok(make_some(Value::Float(max)))
        }),
    );
    fields.insert(
        "clamp".to_string(),
        builtin("math.clamp", 3, |mut args, _| {
            let x = expect_float(args.pop().unwrap(), "math.clamp")?;
            let high = expect_float(args.pop().unwrap(), "math.clamp")?;
            let low = expect_float(args.pop().unwrap(), "math.clamp")?;
            Ok(Value::Float(x.max(low).min(high)))
        }),
    );
    fields.insert(
        "sum".to_string(),
        builtin("math.sum", 1, |mut args, _| {
            let list = expect_list(args.pop().unwrap(), "math.sum")?;
            let values = list_floats(&list, "math.sum")?;
            Ok(Value::Float(values.into_iter().sum()))
        }),
    );
    fields.insert(
        "sumInt".to_string(),
        builtin("math.sumInt", 1, |mut args, _| {
            let list = expect_list(args.pop().unwrap(), "math.sumInt")?;
            let values = list_ints(&list, "math.sumInt")?;
            Ok(Value::Int(values.into_iter().sum()))
        }),
    );
    fields.insert(
        "floor".to_string(),
        builtin("math.floor", 1, |mut args, _| {
            let value = expect_float(args.pop().unwrap(), "math.floor")?;
            Ok(Value::Float(value.floor()))
        }),
    );
    fields.insert(
        "ceil".to_string(),
        builtin("math.ceil", 1, |mut args, _| {
            let value = expect_float(args.pop().unwrap(), "math.ceil")?;
            Ok(Value::Float(value.ceil()))
        }),
    );
    fields.insert(
        "trunc".to_string(),
        builtin("math.trunc", 1, |mut args, _| {
            let value = expect_float(args.pop().unwrap(), "math.trunc")?;
            Ok(Value::Float(value.trunc()))
        }),
    );
    fields.insert(
        "round".to_string(),
        builtin("math.round", 1, |mut args, _| {
            let value = expect_float(args.pop().unwrap(), "math.round")?;
            let trunc = value.trunc();
            let frac = value - trunc;
            let rounded = if frac.abs() == 0.5 {
                let even = (trunc as i64) % 2 == 0;
                if even {
                    trunc
                } else {
                    trunc + value.signum()
                }
            } else {
                value.round()
            };
            Ok(Value::Float(rounded))
        }),
    );
    fields.insert(
        "fract".to_string(),
        builtin("math.fract", 1, |mut args, _| {
            let value = expect_float(args.pop().unwrap(), "math.fract")?;
            Ok(Value::Float(value.fract()))
        }),
    );
    fields.insert(
        "modf".to_string(),
        builtin("math.modf", 1, |mut args, _| {
            let value = expect_float(args.pop().unwrap(), "math.modf")?;
            let int_part = value.trunc();
            let frac_part = value.fract();
            Ok(Value::Tuple(vec![Value::Float(int_part), Value::Float(frac_part)]))
        }),
    );
    fields.insert(
        "frexp".to_string(),
        builtin("math.frexp", 1, |mut args, _| {
            let value = expect_float(args.pop().unwrap(), "math.frexp")?;
            let (mantissa, exponent) = frexp_value(value);
            Ok(Value::Tuple(vec![
                Value::Float(mantissa),
                Value::Int(exponent),
            ]))
        }),
    );
    fields.insert(
        "ldexp".to_string(),
        builtin("math.ldexp", 2, |mut args, _| {
            let exponent = expect_int(args.pop().unwrap(), "math.ldexp")?;
            let mantissa = expect_float(args.pop().unwrap(), "math.ldexp")?;
            Ok(Value::Float(mantissa * 2.0_f64.powi(exponent as i32)))
        }),
    );
    fields.insert(
        "pow".to_string(),
        builtin("math.pow", 2, |mut args, _| {
            let exp = expect_float(args.pop().unwrap(), "math.pow")?;
            let base = expect_float(args.pop().unwrap(), "math.pow")?;
            Ok(Value::Float(base.powf(exp)))
        }),
    );
    fields.insert(
        "sqrt".to_string(),
        builtin("math.sqrt", 1, |mut args, _| {
            let value = expect_float(args.pop().unwrap(), "math.sqrt")?;
            Ok(Value::Float(value.sqrt()))
        }),
    );
    fields.insert(
        "cbrt".to_string(),
        builtin("math.cbrt", 1, |mut args, _| {
            let value = expect_float(args.pop().unwrap(), "math.cbrt")?;
            Ok(Value::Float(value.cbrt()))
        }),
    );
    fields.insert(
        "hypot".to_string(),
        builtin("math.hypot", 2, |mut args, _| {
            let y = expect_float(args.pop().unwrap(), "math.hypot")?;
            let x = expect_float(args.pop().unwrap(), "math.hypot")?;
            Ok(Value::Float(x.hypot(y)))
        }),
    );
    fields.insert(
        "exp".to_string(),
        builtin("math.exp", 1, |mut args, _| {
            let value = expect_float(args.pop().unwrap(), "math.exp")?;
            Ok(Value::Float(value.exp()))
        }),
    );
    fields.insert(
        "exp2".to_string(),
        builtin("math.exp2", 1, |mut args, _| {
            let value = expect_float(args.pop().unwrap(), "math.exp2")?;
            Ok(Value::Float(value.exp2()))
        }),
    );
    fields.insert(
        "expm1".to_string(),
        builtin("math.expm1", 1, |mut args, _| {
            let value = expect_float(args.pop().unwrap(), "math.expm1")?;
            Ok(Value::Float(value.exp_m1()))
        }),
    );
    fields.insert(
        "log".to_string(),
        builtin("math.log", 1, |mut args, _| {
            let value = expect_float(args.pop().unwrap(), "math.log")?;
            Ok(Value::Float(value.ln()))
        }),
    );
    fields.insert(
        "log10".to_string(),
        builtin("math.log10", 1, |mut args, _| {
            let value = expect_float(args.pop().unwrap(), "math.log10")?;
            Ok(Value::Float(value.log10()))
        }),
    );
    fields.insert(
        "log2".to_string(),
        builtin("math.log2", 1, |mut args, _| {
            let value = expect_float(args.pop().unwrap(), "math.log2")?;
            Ok(Value::Float(value.log2()))
        }),
    );
    fields.insert(
        "log1p".to_string(),
        builtin("math.log1p", 1, |mut args, _| {
            let value = expect_float(args.pop().unwrap(), "math.log1p")?;
            Ok(Value::Float(value.ln_1p()))
        }),
    );
    fields.insert(
        "sin".to_string(),
        builtin("math.sin", 1, |mut args, _| {
            let radians = angle_from_value(args.pop().unwrap(), "math.sin")?;
            Ok(Value::Float(radians.sin()))
        }),
    );
    fields.insert(
        "cos".to_string(),
        builtin("math.cos", 1, |mut args, _| {
            let radians = angle_from_value(args.pop().unwrap(), "math.cos")?;
            Ok(Value::Float(radians.cos()))
        }),
    );
    fields.insert(
        "tan".to_string(),
        builtin("math.tan", 1, |mut args, _| {
            let radians = angle_from_value(args.pop().unwrap(), "math.tan")?;
            Ok(Value::Float(radians.tan()))
        }),
    );
    fields.insert(
        "asin".to_string(),
        builtin("math.asin", 1, |mut args, _| {
            let value = expect_float(args.pop().unwrap(), "math.asin")?;
            Ok(angle_value(value.asin()))
        }),
    );
    fields.insert(
        "acos".to_string(),
        builtin("math.acos", 1, |mut args, _| {
            let value = expect_float(args.pop().unwrap(), "math.acos")?;
            Ok(angle_value(value.acos()))
        }),
    );
    fields.insert(
        "atan".to_string(),
        builtin("math.atan", 1, |mut args, _| {
            let value = expect_float(args.pop().unwrap(), "math.atan")?;
            Ok(angle_value(value.atan()))
        }),
    );
    fields.insert(
        "atan2".to_string(),
        builtin("math.atan2", 2, |mut args, _| {
            let x = expect_float(args.pop().unwrap(), "math.atan2")?;
            let y = expect_float(args.pop().unwrap(), "math.atan2")?;
            Ok(angle_value(y.atan2(x)))
        }),
    );
    fields.insert(
        "sinh".to_string(),
        builtin("math.sinh", 1, |mut args, _| {
            let value = expect_float(args.pop().unwrap(), "math.sinh")?;
            Ok(Value::Float(value.sinh()))
        }),
    );
    fields.insert(
        "cosh".to_string(),
        builtin("math.cosh", 1, |mut args, _| {
            let value = expect_float(args.pop().unwrap(), "math.cosh")?;
            Ok(Value::Float(value.cosh()))
        }),
    );
    fields.insert(
        "tanh".to_string(),
        builtin("math.tanh", 1, |mut args, _| {
            let value = expect_float(args.pop().unwrap(), "math.tanh")?;
            Ok(Value::Float(value.tanh()))
        }),
    );
    fields.insert(
        "asinh".to_string(),
        builtin("math.asinh", 1, |mut args, _| {
            let value = expect_float(args.pop().unwrap(), "math.asinh")?;
            Ok(Value::Float(value.asinh()))
        }),
    );
    fields.insert(
        "acosh".to_string(),
        builtin("math.acosh", 1, |mut args, _| {
            let value = expect_float(args.pop().unwrap(), "math.acosh")?;
            Ok(Value::Float(value.acosh()))
        }),
    );
    fields.insert(
        "atanh".to_string(),
        builtin("math.atanh", 1, |mut args, _| {
            let value = expect_float(args.pop().unwrap(), "math.atanh")?;
            Ok(Value::Float(value.atanh()))
        }),
    );
    fields.insert(
        "gcd".to_string(),
        builtin("math.gcd", 2, |mut args, _| {
            let right = expect_int(args.pop().unwrap(), "math.gcd")?;
            let left = expect_int(args.pop().unwrap(), "math.gcd")?;
            Ok(Value::Int(gcd_i64(left, right)))
        }),
    );
    fields.insert(
        "lcm".to_string(),
        builtin("math.lcm", 2, |mut args, _| {
            let right = expect_int(args.pop().unwrap(), "math.lcm")?;
            let left = expect_int(args.pop().unwrap(), "math.lcm")?;
            Ok(Value::Int(lcm_i64(left, right)))
        }),
    );
    fields.insert(
        "gcdAll".to_string(),
        builtin("math.gcdAll", 1, |mut args, _| {
            let list = expect_list(args.pop().unwrap(), "math.gcdAll")?;
            let values = list_ints(&list, "math.gcdAll")?;
            if values.is_empty() {
                return Ok(make_none());
            }
            let mut value = values[0];
            for item in values.iter().skip(1) {
                value = gcd_i64(value, *item);
            }
            Ok(make_some(Value::Int(value)))
        }),
    );
    fields.insert(
        "lcmAll".to_string(),
        builtin("math.lcmAll", 1, |mut args, _| {
            let list = expect_list(args.pop().unwrap(), "math.lcmAll")?;
            let values = list_ints(&list, "math.lcmAll")?;
            if values.is_empty() {
                return Ok(make_none());
            }
            let mut value = values[0];
            for item in values.iter().skip(1) {
                value = lcm_i64(value, *item);
            }
            Ok(make_some(Value::Int(value)))
        }),
    );
    fields.insert(
        "factorial".to_string(),
        builtin("math.factorial", 1, |mut args, _| {
            let n = expect_int(args.pop().unwrap(), "math.factorial")?;
            let value = factorial_bigint(n)
                .ok_or_else(|| RuntimeError::Message("math.factorial expects n >= 0".to_string()))?;
            Ok(Value::BigInt(Arc::new(value)))
        }),
    );
    fields.insert(
        "comb".to_string(),
        builtin("math.comb", 2, |mut args, _| {
            let k = expect_int(args.pop().unwrap(), "math.comb")?;
            let n = expect_int(args.pop().unwrap(), "math.comb")?;
            let value = comb_bigint(n, k)
                .ok_or_else(|| RuntimeError::Message("math.comb expects 0 <= k <= n".to_string()))?;
            Ok(Value::BigInt(Arc::new(value)))
        }),
    );
    fields.insert(
        "perm".to_string(),
        builtin("math.perm", 2, |mut args, _| {
            let k = expect_int(args.pop().unwrap(), "math.perm")?;
            let n = expect_int(args.pop().unwrap(), "math.perm")?;
            let value = perm_bigint(n, k)
                .ok_or_else(|| RuntimeError::Message("math.perm expects 0 <= k <= n".to_string()))?;
            Ok(Value::BigInt(Arc::new(value)))
        }),
    );
    fields.insert(
        "divmod".to_string(),
        builtin("math.divmod", 2, |mut args, _| {
            let b = expect_int(args.pop().unwrap(), "math.divmod")?;
            let a = expect_int(args.pop().unwrap(), "math.divmod")?;
            if b == 0 {
                return Err(RuntimeError::Message("math.divmod expects non-zero divisor".to_string()));
            }
            let mut q = a / b;
            let mut r = a % b;
            if r < 0 {
                let adj = if b > 0 { 1 } else { -1 };
                q -= adj;
                r += b.abs();
            }
            Ok(Value::Tuple(vec![Value::Int(q), Value::Int(r)]))
        }),
    );
    fields.insert(
        "modPow".to_string(),
        builtin("math.modPow", 3, |mut args, _| {
            let modulus = expect_int(args.pop().unwrap(), "math.modPow")?;
            let exp = expect_int(args.pop().unwrap(), "math.modPow")?;
            let base = expect_int(args.pop().unwrap(), "math.modPow")?;
            if exp < 0 || modulus == 0 {
                return Err(RuntimeError::Message("math.modPow expects exp >= 0 and modulus != 0".to_string()));
            }
            Ok(Value::Int(mod_pow(base, exp, modulus)))
        }),
    );
    fields.insert(
        "isFinite".to_string(),
        builtin("math.isFinite", 1, |mut args, _| {
            let value = expect_float(args.pop().unwrap(), "math.isFinite")?;
            Ok(Value::Bool(value.is_finite()))
        }),
    );
    fields.insert(
        "isInf".to_string(),
        builtin("math.isInf", 1, |mut args, _| {
            let value = expect_float(args.pop().unwrap(), "math.isInf")?;
            Ok(Value::Bool(value.is_infinite()))
        }),
    );
    fields.insert(
        "isNaN".to_string(),
        builtin("math.isNaN", 1, |mut args, _| {
            let value = expect_float(args.pop().unwrap(), "math.isNaN")?;
            Ok(Value::Bool(value.is_nan()))
        }),
    );
    fields.insert(
        "nextAfter".to_string(),
        builtin("math.nextAfter", 2, |mut args, _| {
            let to = expect_float(args.pop().unwrap(), "math.nextAfter")?;
            let from = expect_float(args.pop().unwrap(), "math.nextAfter")?;
            Ok(Value::Float(next_after(from, to)))
        }),
    );
    fields.insert(
        "ulp".to_string(),
        builtin("math.ulp", 1, |mut args, _| {
            let value = expect_float(args.pop().unwrap(), "math.ulp")?;
            let next = next_after(value, if value.is_sign_positive() { f64::INFINITY } else { f64::NEG_INFINITY });
            Ok(Value::Float((next - value).abs()))
        }),
    );
    fields.insert(
        "fmod".to_string(),
        builtin("math.fmod", 2, |mut args, _| {
            let b = expect_float(args.pop().unwrap(), "math.fmod")?;
            let a = expect_float(args.pop().unwrap(), "math.fmod")?;
            Ok(Value::Float(a % b))
        }),
    );
    fields.insert(
        "remainder".to_string(),
        builtin("math.remainder", 2, |mut args, _| {
            let b = expect_float(args.pop().unwrap(), "math.remainder")?;
            let a = expect_float(args.pop().unwrap(), "math.remainder")?;
            Ok(Value::Float(a - (a / b).round() * b))
        }),
    );
    Value::Record(Arc::new(fields))
}

fn date_from_value(value: Value, ctx: &str) -> Result<NaiveDate, RuntimeError> {
    let Value::Record(fields) = value else {
        return Err(RuntimeError::Message(format!("{ctx} expects Date")));
    };
    let year = fields
        .get("year")
        .cloned()
        .ok_or_else(|| RuntimeError::Message(format!("{ctx} expects Date.year")))?;
    let month = fields
        .get("month")
        .cloned()
        .ok_or_else(|| RuntimeError::Message(format!("{ctx} expects Date.month")))?;
    let day = fields
        .get("day")
        .cloned()
        .ok_or_else(|| RuntimeError::Message(format!("{ctx} expects Date.day")))?;
    let year = expect_int(year, ctx)? as i32;
    let month = expect_int(month, ctx)? as u32;
    let day = expect_int(day, ctx)? as u32;
    NaiveDate::from_ymd_opt(year, month, day)
        .ok_or_else(|| RuntimeError::Message(format!("{ctx} expects valid Date")))
}

fn date_to_value(date: NaiveDate) -> Value {
    let mut map = HashMap::new();
    map.insert("year".to_string(), Value::Int(date.year() as i64));
    map.insert("month".to_string(), Value::Int(date.month() as i64));
    map.insert("day".to_string(), Value::Int(date.day() as i64));
    Value::Record(Arc::new(map))
}

fn days_in_month(year: i32, month: u32) -> u32 {
    let (next_year, next_month) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    let first_next = NaiveDate::from_ymd_opt(next_year, next_month, 1)
        .expect("valid next month date");
    first_next.pred_opt().expect("previous day").day()
}

fn add_months(date: NaiveDate, months: i64) -> NaiveDate {
    let mut year = date.year() as i64;
    let mut month = date.month() as i64;
    let total = month - 1 + months;
    year += total.div_euclid(12);
    month = total.rem_euclid(12) + 1;
    let year_i32 = year as i32;
    let month_u32 = month as u32;
    let max_day = days_in_month(year_i32, month_u32);
    let day = date.day().min(max_day);
    NaiveDate::from_ymd_opt(year_i32, month_u32, day).expect("valid date")
}

