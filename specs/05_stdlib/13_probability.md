# Standard Library: Probability & Distribution Domain

## Module

```aivi
module aivi.std.probability = {
  export domain Probability
  export Probability, Distribution
  export clamp, bernoulli, uniform, expectation
}
```

## Types

```aivi
Probability = Float
Distribution a = { pdf: a -> Probability }
```

## Domain Definition

```aivi
domain Probability over Probability = {
  (+) : Probability -> Probability -> Probability
  (-) : Probability -> Probability -> Probability
  (*) : Probability -> Probability -> Probability
}
```

## Helper Functions

```aivi
clamp : Probability -> Probability
clamp p = max 0.0 (min 1.0 p)

bernoulli : Probability -> Distribution Bool
bernoulli p = { pdf: \x -> if x then clamp p else clamp (1.0 - p) }

uniform : Float -> Float -> Distribution Float
uniform lo hi = { pdf: \x -> if x < lo || x > hi then 0.0 else 1.0 / (hi - lo) }

expectation : Distribution Float -> Float -> Float
expectation d x = x * d.pdf x
```

## Usage Examples

```aivi
use aivi.std.probability

p = clamp 0.7
coin = bernoulli p
probHeads = coin.pdf true
```