pub const MODULE_NAME: &str = "aivi.number.bigint";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.number.bigint = {
  export BigInt, fromInt, toInt, absInt
  export domain BigInt

  use aivi

  absInt : Int -> Int
  absInt n = if n < 0 then -n else n

  fromInt : Int -> BigInt
  fromInt value = bigint.fromInt value

  toInt : BigInt -> Int
  toInt value = bigint.toInt value

  domain BigInt over BigInt = {
    (+) : BigInt -> BigInt -> BigInt
    (+) a b = bigint.add a b

    (-) : BigInt -> BigInt -> BigInt
    (-) a b = bigint.sub a b

    (*) : BigInt -> BigInt -> BigInt
    (*) a b = bigint.mul a b

    1n = fromInt 1
  }
}
"#;
