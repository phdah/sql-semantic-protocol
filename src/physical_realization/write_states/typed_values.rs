//! Typed physical values for Arrow-style nested witness feasibility.
//!
//! Values here are caller-controlled materializations, not SQL parser literals.
//! The scalar validator remains canonical; nested shapes add only structural
//! obligations and never invent implicit casts or warehouse comparison laws.

use super::type_validation::validate_value;
use super::*;

/// One physical scalar or nested value, preserving ordered Arrow-style fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhysicalTypedValue {
    /// Existing canonical SQL literal, including NULL.
    Scalar(LiteralValue),
    /// Exact bytes, with no implicit character encoding.
    Bytes(Vec<u8>),
    /// Fully parsed JSON document, distinct from SQL NULL and textual casts.
    Json(serde_json::Value),
    /// Ordered list of independently typed values.
    Array(Vec<Self>),
    /// Ordered key/value pairs. SQL map key equality needs a proven law.
    Map(Vec<(Self, Self)>),
    /// Struct fields in canonical declared order.
    Struct(Vec<Self>),
}

/// A complete physical value proof, a contradiction, or a precise gap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhysicalTypeProof {
    /// The canonical type admits this exact typed value.
    Feasible,
    /// Exact byte/shape/type evidence disproves this physical value.
    Impossible { reason: String },
    /// SQL or warehouse behavior is insufficiently attested.
    Residual { reason: String },
}

fn typed_error(result: StateResult<()>) -> PhysicalTypeProof {
    match result {
        Ok(()) => PhysicalTypeProof::Feasible,
        Err(StateFailure::Impossible(reason)) => PhysicalTypeProof::Impossible { reason },
        Err(StateFailure::Residual(_, reason)) => PhysicalTypeProof::Residual { reason },
    }
}

/// Check a complete Arrow-like physical value against the canonical datatype.
///
/// This proves exact scalar, binary and nested shape laws. The API does not
/// assume string key collation, JSON storage identity, float NaN encodings,
/// nullable struct child defaults or automatic numeric widening.
pub fn physical_typed_value_plan(
    value: &PhysicalTypedValue,
    data_type: &DataType,
) -> PhysicalTypeProof {
    match data_type {
        DataType::Nullable(inner) => {
            if matches!(value, PhysicalTypedValue::Scalar(LiteralValue::Null)) {
                PhysicalTypeProof::Feasible
            } else {
                physical_typed_value_plan(value, inner)
            }
        }
        DataType::Array {
            element: Some(element),
            length,
        } => {
            let PhysicalTypedValue::Array(values) = value else {
                return PhysicalTypeProof::Residual {
                    reason: "array physical encoding is not attested".to_string(),
                };
            };
            if length.is_some_and(|max| u64::try_from(values.len()).ok() != Some(max)) {
                return PhysicalTypeProof::Impossible {
                    reason: "physical array length contradicts declared fixed length".to_string(),
                };
            }
            for element_value in values {
                let check = physical_typed_value_plan(element_value, element);
                if check != PhysicalTypeProof::Feasible {
                    return check;
                }
            }
            PhysicalTypeProof::Feasible
        }
        DataType::Array { element: None, .. } => PhysicalTypeProof::Residual {
            reason: "array element datatype is unknown".to_string(),
        },
        DataType::Struct { fields } => {
            let PhysicalTypedValue::Struct(values) = value else {
                return PhysicalTypeProof::Residual {
                    reason: "struct field encoding is not attested".to_string(),
                };
            };
            if fields.len() != values.len() {
                return PhysicalTypeProof::Impossible {
                    reason: "struct physical field arity differs from declared arity".to_string(),
                };
            }
            for (item, field) in values.iter().zip(fields) {
                let check = physical_typed_value_plan(item, field.data_type());
                if check != PhysicalTypeProof::Feasible {
                    return check;
                }
            }
            PhysicalTypeProof::Feasible
        }
        DataType::Map {
            key,
            value: map_value,
        } => {
            let PhysicalTypedValue::Map(entries) = value else {
                return PhysicalTypeProof::Residual {
                    reason: "map physical representation is not attested".to_string(),
                };
            };
            let mut numeric_keys = std::collections::BTreeSet::<String>::new();
            for (entry_key, entry_value) in entries {
                let PhysicalTypedValue::Scalar(LiteralValue::Number(number)) = entry_key else {
                    return PhysicalTypeProof::Residual {
                        reason: "map-key equality or null semantics are not attested".to_string(),
                    };
                };
                match physical_typed_value_plan(entry_key, key) {
                    PhysicalTypeProof::Feasible => {}
                    gap => return gap,
                }
                // Canonicalize exact integer and decimal keys before checking
                // equality. 05, 5.0 and 5.000 are the same decimal key.
                let identity = match key.as_ref() {
                    DataType::SignedInteger { .. } => {
                        number.parse::<i128>().ok().map(|n| format!("signed:{n}"))
                    }
                    DataType::UnsignedInteger { .. } => {
                        number.parse::<u128>().ok().map(|n| format!("unsigned:{n}"))
                    }
                    DataType::Decimal { .. } => {
                        super::check_comparisons::canonical_decimal_identity(number)
                            .map(|n| format!("decimal:{n}"))
                    }
                    _ => None,
                };
                let Some(identity) = identity else {
                    return PhysicalTypeProof::Residual {
                        reason: "map key equality needs an exact numeric storage law".to_string(),
                    };
                };
                if !numeric_keys.insert(identity) {
                    return PhysicalTypeProof::Impossible {
                        reason: "duplicate exact numeric map key".to_string(),
                    };
                }
                let check = physical_typed_value_plan(entry_value, map_value);
                if check != PhysicalTypeProof::Feasible {
                    return check;
                }
            }
            PhysicalTypeProof::Feasible
        }
        DataType::Json => match value {
            PhysicalTypedValue::Json(document) => json_document_proof(document),
            _ => PhysicalTypeProof::Residual {
                reason: "JSON requires a typed document rather than an implicit SQL cast"
                    .to_string(),
            },
        },
        DataType::Binary { length, fixed } => {
            let PhysicalTypedValue::Bytes(bytes) = value else {
                return PhysicalTypeProof::Residual {
                    reason: "binary value requires exact bytes".to_string(),
                };
            };
            let size = bytes.len() as u64;
            if length.is_some_and(|max| size > max) {
                PhysicalTypeProof::Impossible {
                    reason: "binary value exceeds declared length".to_string(),
                }
            } else if *fixed && length.is_some_and(|size_required| size != size_required) {
                PhysicalTypeProof::Residual {
                    reason: "binary fixed-width padding needs an engine law".to_string(),
                }
            } else {
                PhysicalTypeProof::Feasible
            }
        }
        _ => match value {
            PhysicalTypedValue::Scalar(literal) => typed_error(validate_value(literal, data_type)),
            _ => PhysicalTypeProof::Residual {
                reason: "nested value has no matching canonical scalar type".to_string(),
            },
        },
    }
}

/// Only engine-independent JSON scalars and containers are constructive.
/// Fractional or oversized numbers require explicit decimal/floating storage
/// laws, rather than inferring vendor JSON numeric representation.
fn json_document_proof(document: &serde_json::Value) -> PhysicalTypeProof {
    match document {
        serde_json::Value::Null | serde_json::Value::Bool(_) => PhysicalTypeProof::Feasible,
        serde_json::Value::Number(number) => {
            let exactly_representable = number
                .as_i64()
                .map(|value| value.unsigned_abs() <= (1_u64 << 53))
                .or_else(|| number.as_u64().map(|value| value <= (1_u64 << 53)))
                .unwrap_or(false);
            if exactly_representable {
                PhysicalTypeProof::Feasible
            } else {
                PhysicalTypeProof::Residual {
                    reason: "JSON floating or large-number storage law is not attested"
                        .to_string(),
                }
            }
        }
        serde_json::Value::String(text) => {
            if text.is_ascii() {
                PhysicalTypeProof::Feasible
            } else {
                PhysicalTypeProof::Residual {
                    reason: "JSON non-ASCII normalization is not attested".to_string(),
                }
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                let proof = json_document_proof(item);
                if proof != PhysicalTypeProof::Feasible {
                    return proof;
                }
            }
            PhysicalTypeProof::Feasible
        }
        serde_json::Value::Object(fields) => {
            for (name, item) in fields {
                if !name.is_ascii() {
                    return PhysicalTypeProof::Residual {
                        reason: "JSON non-ASCII object-key normalization is not attested"
                            .to_string(),
                    };
                }
                let proof = json_document_proof(item);
                if proof != PhysicalTypeProof::Feasible {
                    return proof;
                }
            }
            PhysicalTypeProof::Feasible
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_type::DataTypeField;

    #[test]
    fn nested_struct_arrays_are_feasible_with_exact_scalar_bounds() {
        let kind = DataType::Struct {
            fields: vec![
                DataTypeField::new(
                    Some("id".into()),
                    DataType::SignedInteger { bits: Some(32) },
                ),
                DataTypeField::new(
                    Some("tags".into()),
                    DataType::Array {
                        element: Some(Box::new(DataType::String {
                            length: Some(5),
                            fixed: false,
                        })),
                        length: Some(2),
                    },
                ),
            ],
        };
        let good = PhysicalTypedValue::Struct(vec![
            PhysicalTypedValue::Scalar(LiteralValue::Number("7".into())),
            PhysicalTypedValue::Array(vec![
                PhysicalTypedValue::Scalar(LiteralValue::Text("foo".into())),
                PhysicalTypedValue::Scalar(LiteralValue::Text("bar".into())),
            ]),
        ]);
        assert_eq!(
            physical_typed_value_plan(&good, &kind),
            PhysicalTypeProof::Feasible
        );
        let bad = PhysicalTypedValue::Struct(vec![PhysicalTypedValue::Scalar(
            LiteralValue::Number("7".into()),
        )]);
        assert!(matches!(
            physical_typed_value_plan(&bad, &kind),
            PhysicalTypeProof::Impossible { .. }
        ));
    }

    #[test]
    fn map_duplicate_exact_keys_are_impossible_unverified_text_keys_residual() {
        let kind = DataType::Map {
            key: Box::new(DataType::SignedInteger { bits: Some(64) }),
            value: Box::new(DataType::Boolean),
        };
        let one = (
            PhysicalTypedValue::Scalar(LiteralValue::Number("5".into())),
            PhysicalTypedValue::Scalar(LiteralValue::Boolean(true)),
        );
        assert_eq!(
            physical_typed_value_plan(&PhysicalTypedValue::Map(vec![one.clone()]), &kind),
            PhysicalTypeProof::Feasible
        );
        assert!(matches!(
            physical_typed_value_plan(&PhysicalTypedValue::Map(vec![one.clone(), one]), &kind),
            PhysicalTypeProof::Impossible { .. }
        ));
        let text_key = PhysicalTypedValue::Map(vec![(
            PhysicalTypedValue::Scalar(LiteralValue::Text("A".into())),
            PhysicalTypedValue::Scalar(LiteralValue::Boolean(true)),
        )]);
        assert!(matches!(
            physical_typed_value_plan(&text_key, &kind),
            PhysicalTypeProof::Residual { .. }
        ));
    }

    #[test]
    fn exact_binary_is_possible_but_json_opaque_and_float_nan_residual() {
        let kind = DataType::Binary {
            length: Some(4),
            fixed: false,
        };
        assert_eq!(
            physical_typed_value_plan(&PhysicalTypedValue::Bytes(vec![0, 255]), &kind),
            PhysicalTypeProof::Feasible
        );
        assert!(matches!(
            physical_typed_value_plan(&PhysicalTypedValue::Bytes(vec![0; 5]), &kind),
            PhysicalTypeProof::Impossible { .. }
        ));
        for (value, kind) in [
            (LiteralValue::Text("{\"a\":1}".into()), DataType::Json),
            (
                LiteralValue::Number("NaN".into()),
                DataType::FloatingPoint { bits: Some(64) },
            ),
        ] {
            assert!(matches!(
                physical_typed_value_plan(&PhysicalTypedValue::Scalar(value), &kind),
                PhysicalTypeProof::Residual { .. }
            ));
        }
    }
    #[test]
    fn high_precision_decimal_map_keys_use_numeric_not_lexical_identity() {
        let kind = DataType::Map {
            key: Box::new(DataType::Decimal {
                precision: Some(38),
                scale: Some(6),
            }),
            value: Box::new(DataType::Boolean),
        };
        let entry = |number: &str| {
            (
                PhysicalTypedValue::Scalar(LiteralValue::Number(number.to_string())),
                PhysicalTypedValue::Scalar(LiteralValue::Boolean(true)),
            )
        };
        let large = "999999999999999999999999999999.000001";
        assert_eq!(
            physical_typed_value_plan(
                &PhysicalTypedValue::Map(vec![entry(large), entry("1.5")]),
                &kind
            ),
            PhysicalTypeProof::Feasible
        );
        assert!(matches!(
            physical_typed_value_plan(
                &PhysicalTypedValue::Map(vec![entry("01.5000"), entry("1.500000")]),
                &kind
            ),
            PhysicalTypeProof::Impossible { .. }
        ));
        assert!(matches!(
            physical_typed_value_plan(
                &PhysicalTypedValue::Map(vec![entry("0.000000"), entry("-0.0")]),
                &kind
            ),
            PhysicalTypeProof::Impossible { .. }
        ));
    }
}
