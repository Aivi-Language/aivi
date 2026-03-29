use std::hash::{Hash, Hasher};

use num_bigint::{BigInt, Sign};
use rust_decimal::Decimal;

#[derive(Clone, Copy, Debug, PartialOrd)]
pub struct RuntimeFloat(f64);

impl RuntimeFloat {
    pub fn new(value: f64) -> Option<Self> {
        value.is_finite().then_some(Self(value))
    }

    pub fn parse_literal(raw: &str) -> Option<Self> {
        let value = raw.parse::<f64>().ok()?;
        Self::new(value)
    }

    pub const fn to_f64(self) -> f64 {
        self.0
    }
}

impl PartialEq for RuntimeFloat {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for RuntimeFloat {}

// Safety: `RuntimeFloat::new` rejects NaN and infinities, so every stored
// value is a finite f64 whose bit pattern is a stable, canonical identifier.
impl Hash for RuntimeFloat {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.to_bits().hash(state);
    }
}

impl std::fmt::Display for RuntimeFloat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut rendered = self.0.to_string();
        if !rendered.contains(['.', 'e', 'E']) {
            rendered.push_str(".0");
        }
        f.write_str(&rendered)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RuntimeDecimal(Decimal);

impl RuntimeDecimal {
    pub fn parse_literal(raw: &str) -> Option<Self> {
        let digits = raw.strip_suffix('d')?;
        let value = digits.parse::<Decimal>().ok()?;
        Some(Self(value))
    }

    pub(crate) fn encode_constant_bytes(&self) -> Box<[u8]> {
        let mut bytes = Vec::with_capacity(20);
        bytes.extend_from_slice(&self.0.mantissa().to_le_bytes());
        bytes.extend_from_slice(&self.0.scale().to_le_bytes());
        bytes.into_boxed_slice()
    }

    pub(crate) fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}

impl std::fmt::Display for RuntimeDecimal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}d", self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RuntimeBigInt(BigInt);

impl RuntimeBigInt {
    pub fn parse_literal(raw: &str) -> Option<Self> {
        let digits = raw.strip_suffix('n')?;
        let value = digits.parse::<BigInt>().ok()?;
        Some(Self(value))
    }

    pub(crate) fn encode_constant_bytes(&self) -> Box<[u8]> {
        let (sign, magnitude) = self.0.to_bytes_le();
        let mut bytes = Vec::with_capacity(16 + magnitude.len());
        bytes.push(match sign {
            Sign::NoSign => 0,
            Sign::Plus => 1,
            Sign::Minus => 2,
        });
        bytes.extend_from_slice(&[0; 7]);
        bytes.extend_from_slice(&(magnitude.len() as u64).to_le_bytes());
        bytes.extend_from_slice(&magnitude);
        bytes.into_boxed_slice()
    }

    pub(crate) fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}

impl std::fmt::Display for RuntimeBigInt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}n", self.0)
    }
}
