pub const MODULE_NAME: &str = "aivi.probability";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.probability
export Distribution
export clamp, bernoulli, uniform, expectation
export domain Probability

use aivi

Probability = Float
Distribution A = { pdf: A -> Probability }

domain Probability over Probability = {
  (+) : Probability -> Probability -> Probability
  (+) = a b => a + b

  (-) : Probability -> Probability -> Probability
  (-) = a b => a - b

  (*) : Probability -> Probability -> Probability
  (*) = a b => a * b
}

clamp : Probability -> Probability
clamp = p => if p < 0.0 then 0.0 else if p > 1.0 then 1.0 else p

bernoulli : Probability -> Distribution Bool
bernoulli = p => { pdf: b => if b then p else 1.0 - p }

uniform : Float -> Float -> Distribution Float
uniform = lo hi => {
  pdf: x => if x < lo then 0.0 else if x > hi then 0.0 else if lo == hi then 0.0 else 1.0 / (hi - lo)
}

expectation : Distribution Float -> Float -> Float
expectation = dist x => (dist.pdf x) * x
"#;
