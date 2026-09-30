use std::hash::{DefaultHasher, Hash, Hasher};

use aivi_backend::{RuntimeFloat, RuntimeMap, RuntimeMapEntry, RuntimeValue};

fn hash(value: &impl Hash) -> u64 {
    let mut state = DefaultHasher::new();
    value.hash(&mut state);
    state.finish()
}

fn float(value: f64) -> RuntimeValue {
    RuntimeValue::Float(RuntimeFloat::new(value).unwrap())
}

fn map(entries: Vec<(RuntimeValue, RuntimeValue)>) -> RuntimeMap {
    RuntimeMap::from_entries(
        entries
            .into_iter()
            .map(|(key, value)| RuntimeMapEntry { key, value })
            .collect(),
    )
}

#[test]
fn signed_zeros_hash_equally_without_losing_their_sign() {
    let positive = RuntimeFloat::new(0.0).unwrap();
    let negative = RuntimeFloat::new(-0.0).unwrap();
    assert_eq!(positive, negative);
    assert_eq!(
        positive.partial_cmp(&negative),
        Some(std::cmp::Ordering::Equal)
    );
    assert_eq!(hash(&positive), hash(&negative));
    assert_eq!(positive.to_f64().to_bits(), 0.0_f64.to_bits());
    assert_eq!(negative.to_f64().to_bits(), (-0.0_f64).to_bits());
    assert_eq!(positive.to_string(), "0.0");
    assert_eq!(negative.to_string(), "-0.0");
}

fn assert_equal_keys_lookup_and_replace(first: RuntimeValue, equal: RuntimeValue) {
    assert_eq!(first, equal);
    // IndexMap skips hashing for singleton lookups, so include another key.
    let entries = vec![
        (first.clone(), RuntimeValue::Int(1)),
        (RuntimeValue::Unit, RuntimeValue::Int(99)),
    ];
    assert_eq!(
        map(entries.clone()).get(&equal),
        Some(&RuntimeValue::Int(1))
    );
    let mut replaced = entries;
    replaced.push((equal.clone(), RuntimeValue::Int(2)));
    let replaced = map(replaced);
    assert_eq!(replaced.len(), 2);
    assert_eq!(replaced.get(&first), Some(&RuntimeValue::Int(2)));
    assert_eq!(replaced.get(&equal), Some(&RuntimeValue::Int(2)));
    assert_eq!(
        replaced.get(&RuntimeValue::Unit),
        Some(&RuntimeValue::Int(99))
    );
    assert_eq!(
        format!("{:?}", replaced.iter().next().unwrap().0),
        format!("{first:?}"),
    );
}

#[test]
fn signed_zero_scalar_map_keys_support_lookup_and_replacement() {
    assert_equal_keys_lookup_and_replace(float(0.0), float(-0.0));
    assert_equal_keys_lookup_and_replace(float(-0.0), float(0.0));
}

#[test]
fn signed_zero_in_structural_map_keys_supports_lookup_and_replacement() {
    for (positive, negative) in [
        (
            RuntimeValue::List(vec![float(0.0)]),
            RuntimeValue::List(vec![float(-0.0)]),
        ),
        (
            RuntimeValue::Map(map(vec![(float(0.0), RuntimeValue::Int(1))])),
            RuntimeValue::Map(map(vec![(float(-0.0), RuntimeValue::Int(1))])),
        ),
        (
            RuntimeValue::Map(map(vec![(RuntimeValue::Int(1), float(0.0))])),
            RuntimeValue::Map(map(vec![(RuntimeValue::Int(1), float(-0.0))])),
        ),
    ] {
        assert_eq!(hash(&positive), hash(&negative));
        assert_equal_keys_lookup_and_replace(positive, negative);
    }
}

#[test]
fn finite_float_postcard_roundtrips_preserve_all_bits_and_wire_format() {
    // Sweep signs/exponents with representative fraction bits, including both
    // zeros, subnormals, smallest normals and largest finite magnitudes.
    for sign in [0, 1_u64 << 63] {
        for exponent in 0..0x7ff_u64 {
            for fraction in [0, 1, 0x0008_0000_0000_0000, 0x000f_ffff_ffff_ffff] {
                let bits = sign | (exponent << 52) | fraction;
                let raw = f64::from_bits(bits);
                let value = RuntimeFloat::new(raw).unwrap();
                let bytes = postcard::to_stdvec(&value).unwrap();
                assert_eq!(bytes, postcard::to_stdvec(&raw).unwrap());
                let decoded: RuntimeFloat = postcard::from_bytes(&bytes).unwrap();
                assert_eq!(decoded.to_f64().to_bits(), bits);
                assert_eq!(decoded, value);
                assert_eq!(hash(&decoded), hash(&value));
            }
        }
    }
}

#[test]
fn float_deserialization_rejects_nonfinite_values_even_inside_runtime_values() {
    for bits in [
        0x7ff0_0000_0000_0000, // infinity
        0xfff0_0000_0000_0000, // negative infinity
        0x7ff8_0000_0000_0000, // quiet NaN
        0xfff8_0000_0000_0001, // negative NaN with payload
        0x7ff0_0000_0000_0001, // signaling NaN
    ] {
        let raw = f64::from_bits(bits);
        assert!(RuntimeFloat::new(raw).is_none());
        let bytes = postcard::to_stdvec(&raw).unwrap();
        assert!(
            postcard::from_bytes::<RuntimeFloat>(&bytes).is_err(),
            "{bits:016x}"
        );

        let mut nested = postcard::to_stdvec(&RuntimeValue::List(vec![float(0.0)])).unwrap();
        let payload_start = nested.len() - bytes.len();
        nested[payload_start..].copy_from_slice(&bytes);
        assert!(
            postcard::from_bytes::<RuntimeValue>(&nested).is_err(),
            "{bits:016x}"
        );
    }
}

#[test]
fn float_json_decoding_preserves_finite_values_and_rejects_invalid_input() {
    for raw in [0.0, -0.0, 1.5, -2.25] {
        let value = RuntimeFloat::new(raw).unwrap();
        let json = serde_json::to_string(&value).unwrap();
        assert_eq!(json, serde_json::to_string(&raw).unwrap());
        let decoded: RuntimeFloat = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.to_f64().to_bits(), raw.to_bits());
    }
    for json in [
        "null", "\"NaN\"", "NaN", "Infinity", "1e999", "-1e999", "[]", "{}",
    ] {
        assert!(
            serde_json::from_str::<RuntimeFloat>(json).is_err(),
            "{json}"
        );
    }
}

#[test]
fn float_deserializer_preserves_newtype_protocol_and_reports_finite_invariant() {
    use serde::{Deserialize, Deserializer, de::IntoDeserializer};

    struct NamedFloat(f64);

    impl<'de> Deserializer<'de> for NamedFloat {
        type Error = serde::de::value::Error;

        fn deserialize_newtype_struct<V: serde::de::Visitor<'de>>(
            self,
            name: &'static str,
            visitor: V,
        ) -> Result<V::Value, Self::Error> {
            assert_eq!(name, "RuntimeFloat");
            visitor.visit_newtype_struct(self.0.into_deserializer())
        }

        fn deserialize_any<V: serde::de::Visitor<'de>>(
            self,
            _visitor: V,
        ) -> Result<V::Value, Self::Error> {
            panic!("RuntimeFloat must request its named newtype representation")
        }

        serde::forward_to_deserialize_any! {
            bool i8 i16 i32 i64 u8 u16 u32 u64 f32 f64 char str string bytes
            byte_buf option unit unit_struct seq tuple tuple_struct map struct
            enum identifier ignored_any
        }
    }

    let value = RuntimeFloat::deserialize(NamedFloat(-0.0)).unwrap();
    assert_eq!(value.to_f64().to_bits(), (-0.0_f64).to_bits());
    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let error = RuntimeFloat::deserialize(NamedFloat(invalid)).unwrap_err();
        assert_eq!(error.to_string(), "expected a finite Float");
    }
}
