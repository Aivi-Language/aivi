use super::TypeChecker;
use crate::typecheck::types::{Scheme, Type, TypeEnv};

pub(super) fn register(checker: &mut TypeChecker, env: &mut TypeEnv) {
    let int_ty = Type::con("Int");
    let float_ty = Type::con("Float");
    let bool_ty = Type::con("Bool");
    let option_float_ty = Type::con("Option").app(vec![float_ty.clone()]);
    let angle_ty = Type::con("Angle");
    let bigint_ty = Type::con("BigInt");

    let abs_var = checker.fresh_var_id();

    let math_record = Type::Record {
        fields: vec![
            ("pi".to_string(), float_ty.clone()),
            ("tau".to_string(), float_ty.clone()),
            ("e".to_string(), float_ty.clone()),
            ("inf".to_string(), float_ty.clone()),
            ("nan".to_string(), float_ty.clone()),
            ("phi".to_string(), float_ty.clone()),
            ("sqrt2".to_string(), float_ty.clone()),
            ("ln2".to_string(), float_ty.clone()),
            ("ln10".to_string(), float_ty.clone()),
            (
                "abs".to_string(),
                Type::Func(Box::new(Type::Var(abs_var)), Box::new(Type::Var(abs_var))),
            ),
            (
                "sign".to_string(),
                Type::Func(Box::new(float_ty.clone()), Box::new(float_ty.clone())),
            ),
            (
                "copysign".to_string(),
                Type::Func(
                    Box::new(float_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(float_ty.clone()),
                        Box::new(float_ty.clone()),
                    )),
                ),
            ),
            (
                "min".to_string(),
                Type::Func(
                    Box::new(float_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(float_ty.clone()),
                        Box::new(float_ty.clone()),
                    )),
                ),
            ),
            (
                "max".to_string(),
                Type::Func(
                    Box::new(float_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(float_ty.clone()),
                        Box::new(float_ty.clone()),
                    )),
                ),
            ),
            (
                "minAll".to_string(),
                Type::Func(
                    Box::new(Type::con("List").app(vec![float_ty.clone()])),
                    Box::new(option_float_ty.clone()),
                ),
            ),
            (
                "maxAll".to_string(),
                Type::Func(
                    Box::new(Type::con("List").app(vec![float_ty.clone()])),
                    Box::new(option_float_ty.clone()),
                ),
            ),
            (
                "clamp".to_string(),
                Type::Func(
                    Box::new(float_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(float_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(float_ty.clone()),
                            Box::new(float_ty.clone()),
                        )),
                    )),
                ),
            ),
            (
                "sum".to_string(),
                Type::Func(
                    Box::new(Type::con("List").app(vec![float_ty.clone()])),
                    Box::new(float_ty.clone()),
                ),
            ),
            (
                "sumInt".to_string(),
                Type::Func(
                    Box::new(Type::con("List").app(vec![int_ty.clone()])),
                    Box::new(int_ty.clone()),
                ),
            ),
            (
                "floor".to_string(),
                Type::Func(Box::new(float_ty.clone()), Box::new(float_ty.clone())),
            ),
            (
                "ceil".to_string(),
                Type::Func(Box::new(float_ty.clone()), Box::new(float_ty.clone())),
            ),
            (
                "trunc".to_string(),
                Type::Func(Box::new(float_ty.clone()), Box::new(float_ty.clone())),
            ),
            (
                "round".to_string(),
                Type::Func(Box::new(float_ty.clone()), Box::new(float_ty.clone())),
            ),
            (
                "fract".to_string(),
                Type::Func(Box::new(float_ty.clone()), Box::new(float_ty.clone())),
            ),
            (
                "modf".to_string(),
                Type::Func(
                    Box::new(float_ty.clone()),
                    Box::new(Type::Tuple(vec![float_ty.clone(), float_ty.clone()])),
                ),
            ),
            (
                "frexp".to_string(),
                Type::Func(
                    Box::new(float_ty.clone()),
                    Box::new(Type::Tuple(vec![float_ty.clone(), int_ty.clone()])),
                ),
            ),
            (
                "ldexp".to_string(),
                Type::Func(
                    Box::new(float_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(int_ty.clone()),
                        Box::new(float_ty.clone()),
                    )),
                ),
            ),
            (
                "pow".to_string(),
                Type::Func(
                    Box::new(float_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(float_ty.clone()),
                        Box::new(float_ty.clone()),
                    )),
                ),
            ),
            (
                "sqrt".to_string(),
                Type::Func(Box::new(float_ty.clone()), Box::new(float_ty.clone())),
            ),
            (
                "cbrt".to_string(),
                Type::Func(Box::new(float_ty.clone()), Box::new(float_ty.clone())),
            ),
            (
                "hypot".to_string(),
                Type::Func(
                    Box::new(float_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(float_ty.clone()),
                        Box::new(float_ty.clone()),
                    )),
                ),
            ),
            (
                "exp".to_string(),
                Type::Func(Box::new(float_ty.clone()), Box::new(float_ty.clone())),
            ),
            (
                "exp2".to_string(),
                Type::Func(Box::new(float_ty.clone()), Box::new(float_ty.clone())),
            ),
            (
                "expm1".to_string(),
                Type::Func(Box::new(float_ty.clone()), Box::new(float_ty.clone())),
            ),
            (
                "log".to_string(),
                Type::Func(Box::new(float_ty.clone()), Box::new(float_ty.clone())),
            ),
            (
                "log10".to_string(),
                Type::Func(Box::new(float_ty.clone()), Box::new(float_ty.clone())),
            ),
            (
                "log2".to_string(),
                Type::Func(Box::new(float_ty.clone()), Box::new(float_ty.clone())),
            ),
            (
                "log1p".to_string(),
                Type::Func(Box::new(float_ty.clone()), Box::new(float_ty.clone())),
            ),
            (
                "sin".to_string(),
                Type::Func(Box::new(angle_ty.clone()), Box::new(float_ty.clone())),
            ),
            (
                "cos".to_string(),
                Type::Func(Box::new(angle_ty.clone()), Box::new(float_ty.clone())),
            ),
            (
                "tan".to_string(),
                Type::Func(Box::new(angle_ty.clone()), Box::new(float_ty.clone())),
            ),
            (
                "asin".to_string(),
                Type::Func(Box::new(float_ty.clone()), Box::new(angle_ty.clone())),
            ),
            (
                "acos".to_string(),
                Type::Func(Box::new(float_ty.clone()), Box::new(angle_ty.clone())),
            ),
            (
                "atan".to_string(),
                Type::Func(Box::new(float_ty.clone()), Box::new(angle_ty.clone())),
            ),
            (
                "atan2".to_string(),
                Type::Func(
                    Box::new(float_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(float_ty.clone()),
                        Box::new(angle_ty.clone()),
                    )),
                ),
            ),
            (
                "sinh".to_string(),
                Type::Func(Box::new(float_ty.clone()), Box::new(float_ty.clone())),
            ),
            (
                "cosh".to_string(),
                Type::Func(Box::new(float_ty.clone()), Box::new(float_ty.clone())),
            ),
            (
                "tanh".to_string(),
                Type::Func(Box::new(float_ty.clone()), Box::new(float_ty.clone())),
            ),
            (
                "asinh".to_string(),
                Type::Func(Box::new(float_ty.clone()), Box::new(float_ty.clone())),
            ),
            (
                "acosh".to_string(),
                Type::Func(Box::new(float_ty.clone()), Box::new(float_ty.clone())),
            ),
            (
                "atanh".to_string(),
                Type::Func(Box::new(float_ty.clone()), Box::new(float_ty.clone())),
            ),
            (
                "gcd".to_string(),
                Type::Func(
                    Box::new(int_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(int_ty.clone()),
                        Box::new(int_ty.clone()),
                    )),
                ),
            ),
            (
                "lcm".to_string(),
                Type::Func(
                    Box::new(int_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(int_ty.clone()),
                        Box::new(int_ty.clone()),
                    )),
                ),
            ),
            (
                "gcdAll".to_string(),
                Type::Func(
                    Box::new(Type::con("List").app(vec![int_ty.clone()])),
                    Box::new(Type::con("Option").app(vec![int_ty.clone()])),
                ),
            ),
            (
                "lcmAll".to_string(),
                Type::Func(
                    Box::new(Type::con("List").app(vec![int_ty.clone()])),
                    Box::new(Type::con("Option").app(vec![int_ty.clone()])),
                ),
            ),
            (
                "factorial".to_string(),
                Type::Func(Box::new(int_ty.clone()), Box::new(bigint_ty.clone())),
            ),
            (
                "comb".to_string(),
                Type::Func(
                    Box::new(int_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(int_ty.clone()),
                        Box::new(bigint_ty.clone()),
                    )),
                ),
            ),
            (
                "perm".to_string(),
                Type::Func(
                    Box::new(int_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(int_ty.clone()),
                        Box::new(bigint_ty.clone()),
                    )),
                ),
            ),
            (
                "divmod".to_string(),
                Type::Func(
                    Box::new(int_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(int_ty.clone()),
                        Box::new(Type::Tuple(vec![int_ty.clone(), int_ty.clone()])),
                    )),
                ),
            ),
            (
                "modPow".to_string(),
                Type::Func(
                    Box::new(int_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(int_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(int_ty.clone()),
                            Box::new(int_ty.clone()),
                        )),
                    )),
                ),
            ),
            (
                "isFinite".to_string(),
                Type::Func(Box::new(float_ty.clone()), Box::new(bool_ty.clone())),
            ),
            (
                "isInf".to_string(),
                Type::Func(Box::new(float_ty.clone()), Box::new(bool_ty.clone())),
            ),
            (
                "isNaN".to_string(),
                Type::Func(Box::new(float_ty.clone()), Box::new(bool_ty.clone())),
            ),
            (
                "nextAfter".to_string(),
                Type::Func(
                    Box::new(float_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(float_ty.clone()),
                        Box::new(float_ty.clone()),
                    )),
                ),
            ),
            (
                "ulp".to_string(),
                Type::Func(Box::new(float_ty.clone()), Box::new(float_ty.clone())),
            ),
            (
                "fmod".to_string(),
                Type::Func(
                    Box::new(float_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(float_ty.clone()),
                        Box::new(float_ty.clone()),
                    )),
                ),
            ),
            (
                "remainder".to_string(),
                Type::Func(
                    Box::new(float_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(float_ty.clone()),
                        Box::new(float_ty.clone()),
                    )),
                ),
            ),
        ]
        .into_iter()
        .collect(),
        open: true,
    };
    env.insert("math".to_string(), Scheme::mono(math_record));

    let date_ty = Type::con("Date");
    let date_time_ty = Type::con("DateTime");
    let calendar_record = Type::Record {
        fields: vec![
            (
                "isLeapYear".to_string(),
                Type::Func(Box::new(date_ty.clone()), Box::new(bool_ty.clone())),
            ),
            (
                "daysInMonth".to_string(),
                Type::Func(Box::new(date_ty.clone()), Box::new(int_ty.clone())),
            ),
            (
                "endOfMonth".to_string(),
                Type::Func(Box::new(date_ty.clone()), Box::new(date_ty.clone())),
            ),
            (
                "addDays".to_string(),
                Type::Func(
                    Box::new(date_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(int_ty.clone()),
                        Box::new(date_ty.clone()),
                    )),
                ),
            ),
            (
                "addMonths".to_string(),
                Type::Func(
                    Box::new(date_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(int_ty.clone()),
                        Box::new(date_ty.clone()),
                    )),
                ),
            ),
            (
                "addYears".to_string(),
                Type::Func(
                    Box::new(date_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(int_ty.clone()),
                        Box::new(date_ty.clone()),
                    )),
                ),
            ),
        ]
        .into_iter()
        .collect(),
        open: true,
    };
    env.insert("calendar".to_string(), Scheme::mono(calendar_record));

    let instant_record = Type::Record {
        fields: vec![
            (
                "toNanos".to_string(),
                Type::Func(Box::new(date_time_ty.clone()), Box::new(int_ty.clone())),
            ),
            (
                "fromNanos".to_string(),
                Type::Func(Box::new(int_ty.clone()), Box::new(date_time_ty.clone())),
            ),
            (
                "addMillis".to_string(),
                Type::Func(
                    Box::new(date_time_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(int_ty.clone()),
                        Box::new(date_time_ty.clone()),
                    )),
                ),
            ),
            (
                "diffMillis".to_string(),
                Type::Func(
                    Box::new(date_time_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(date_time_ty.clone()),
                        Box::new(int_ty.clone()),
                    )),
                ),
            ),
            (
                "compare".to_string(),
                Type::Func(
                    Box::new(date_time_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(date_time_ty.clone()),
                        Box::new(int_ty.clone()),
                    )),
                ),
            ),
        ]
        .into_iter()
        .collect(),
        open: true,
    };
    env.insert("instant".to_string(), Scheme::mono(instant_record));

    let rgb_ty = Type::con("Rgb");
    let hsl_ty = Type::con("Hsl");
    let hex_ty = Type::con("Hex");
    let color_record = Type::Record {
        fields: vec![
            (
                "adjustLightness".to_string(),
                Type::Func(
                    Box::new(rgb_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(int_ty.clone()),
                        Box::new(rgb_ty.clone()),
                    )),
                ),
            ),
            (
                "adjustSaturation".to_string(),
                Type::Func(
                    Box::new(rgb_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(int_ty.clone()),
                        Box::new(rgb_ty.clone()),
                    )),
                ),
            ),
            (
                "adjustHue".to_string(),
                Type::Func(
                    Box::new(rgb_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(int_ty.clone()),
                        Box::new(rgb_ty.clone()),
                    )),
                ),
            ),
            (
                "toRgb".to_string(),
                Type::Func(Box::new(hsl_ty.clone()), Box::new(rgb_ty.clone())),
            ),
            (
                "toHsl".to_string(),
                Type::Func(Box::new(rgb_ty.clone()), Box::new(hsl_ty.clone())),
            ),
            (
                "toHex".to_string(),
                Type::Func(Box::new(rgb_ty.clone()), Box::new(hex_ty.clone())),
            ),
        ]
        .into_iter()
        .collect(),
        open: true,
    };
    env.insert("color".to_string(), Scheme::mono(color_record));

    let bigint_record = Type::Record {
        fields: vec![
            (
                "fromInt".to_string(),
                Type::Func(Box::new(int_ty.clone()), Box::new(bigint_ty.clone())),
            ),
            (
                "toInt".to_string(),
                Type::Func(Box::new(bigint_ty.clone()), Box::new(int_ty.clone())),
            ),
            (
                "add".to_string(),
                Type::Func(
                    Box::new(bigint_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(bigint_ty.clone()),
                        Box::new(bigint_ty.clone()),
                    )),
                ),
            ),
            (
                "sub".to_string(),
                Type::Func(
                    Box::new(bigint_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(bigint_ty.clone()),
                        Box::new(bigint_ty.clone()),
                    )),
                ),
            ),
            (
                "mul".to_string(),
                Type::Func(
                    Box::new(bigint_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(bigint_ty.clone()),
                        Box::new(bigint_ty.clone()),
                    )),
                ),
            ),
        ]
        .into_iter()
        .collect(),
        open: true,
    };
    env.insert("bigint".to_string(), Scheme::mono(bigint_record));

    let rational_ty = Type::con("Rational");
    let rational_record = Type::Record {
        fields: vec![
            (
                "fromBigInts".to_string(),
                Type::Func(
                    Box::new(bigint_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(bigint_ty.clone()),
                        Box::new(rational_ty.clone()),
                    )),
                ),
            ),
            (
                "normalize".to_string(),
                Type::Func(Box::new(rational_ty.clone()), Box::new(rational_ty.clone())),
            ),
            (
                "numerator".to_string(),
                Type::Func(Box::new(rational_ty.clone()), Box::new(bigint_ty.clone())),
            ),
            (
                "denominator".to_string(),
                Type::Func(Box::new(rational_ty.clone()), Box::new(bigint_ty.clone())),
            ),
            (
                "add".to_string(),
                Type::Func(
                    Box::new(rational_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(rational_ty.clone()),
                        Box::new(rational_ty.clone()),
                    )),
                ),
            ),
            (
                "sub".to_string(),
                Type::Func(
                    Box::new(rational_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(rational_ty.clone()),
                        Box::new(rational_ty.clone()),
                    )),
                ),
            ),
            (
                "mul".to_string(),
                Type::Func(
                    Box::new(rational_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(rational_ty.clone()),
                        Box::new(rational_ty.clone()),
                    )),
                ),
            ),
            (
                "div".to_string(),
                Type::Func(
                    Box::new(rational_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(rational_ty.clone()),
                        Box::new(rational_ty.clone()),
                    )),
                ),
            ),
        ]
        .into_iter()
        .collect(),
        open: true,
    };
    env.insert("rational".to_string(), Scheme::mono(rational_record));
}
