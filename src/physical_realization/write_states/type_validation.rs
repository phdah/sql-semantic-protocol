//! Conservative physical literal compatibility with canonical schema types.
//!
//! This checks caller-supplied closed-world rows, not dialect-dependent SQL casts.
//! A provable contradiction is impossible; any opaque representation remains residual.

use super::*;

pub(super) fn validate_value(value: &LiteralValue, kind: &DataType) -> StateResult<()> {
    if matches!(value, LiteralValue::Null) {
        return Ok(());
    }
    match kind {
        DataType::Nullable(inner) => validate_value(value, inner),
        DataType::Boolean => match value {
            LiteralValue::Boolean(_) => Ok(()),
            _ => Err(impossible("physical boolean value contradicts declared datatype")),
        },
        DataType::SignedInteger { bits } => match value {
            LiteralValue::Number(number) => {
                let Some(value) = parse_integer(number) else {
                    return Err(residual("signed integer literal requires exact integral representation"));
                };
                let range = match bits {
                    Some(8) => i8::try_from(value).is_ok(),
                    Some(16) => i16::try_from(value).is_ok(),
                    Some(32) => i32::try_from(value).is_ok(),
                    Some(64) | None => i64::try_from(value).is_ok(),
                    Some(128) => true,
                    _ => return Err(residual("unknown signed integer storage width")),
                };
                if range {
                    Ok(())
                } else {
                    Err(impossible("physical integer exceeds signed datatype bounds"))
                }
            }
            _ => Err(residual("unproved signed integer literal coercion")),
        },
        DataType::UnsignedInteger { bits } => match value {
            LiteralValue::Number(number) => {
                let Some(value) = parse_unsigned(number) else {
                    return Err(residual("unsigned integer literal requires exact integral representation"));
                };
                let range = match bits {
                    Some(8) => u8::try_from(value).is_ok(),
                    Some(16) => u16::try_from(value).is_ok(),
                    Some(32) => u32::try_from(value).is_ok(),
                    Some(64) | None => u64::try_from(value).is_ok(),
                    Some(128) => true,
                    _ => return Err(residual("unknown unsigned integer storage width")),
                };
                if range {
                    Ok(())
                } else {
                    Err(impossible("physical integer exceeds unsigned datatype bounds"))
                }
            }
            _ => Err(residual("unproved unsigned integer literal coercion")),
        },
        DataType::Decimal { precision, scale } => match value {
            LiteralValue::Number(number) => validate_decimal(number, *precision, *scale),
            _ => Err(residual("decimal source value requires exact numeric representation")),
        },
        DataType::FloatingPoint { bits } => match value {
            LiteralValue::Number(number) => {
                let Some(value) = parse_integer(number) else {
                    return Err(residual("floating point rounding/NaN/signed-zero law is unproved"));
                };
                // Integers below the significand limit are exactly represented by IEEE 754.
                // Larger values and decimal fractions need an explicit rounding law.
                let exact = match bits {
                    Some(32) => value.unsigned_abs() <= (1_u128 << 24),
                    Some(64) | None => value.unsigned_abs() <= (1_u128 << 53),
                    _ => return Err(residual("unknown floating point format")),
                };
                if exact {
                    Ok(())
                } else {
                    Err(residual("floating point literal may round during storage"))
                }
            }
            _ => Err(residual("unproved floating point literal coercion")),
        },
        DataType::String { length, fixed } => match value {
            LiteralValue::Text(text) if text.is_ascii() => {
                let within = length.is_none_or(|n| u64::try_from(text.len()).is_ok_and(|size| size <= n));
                if !within {
                    return Err(impossible("physical string exceeds declared character length"));
                }
                if *fixed && length.is_none_or(|n| u64::try_from(text.len()).ok() != Some(n)) {
                    return Err(residual("fixed-width CHAR padding is not proven"));
                }
                Ok(())
            }
            LiteralValue::Text(_) => Err(residual("non-ASCII string length requires dialect collation/character semantics")),
            _ => Err(residual("unproved physical string coercion")),
        },
        DataType::Enum { values } => match value {
            LiteralValue::Text(text) if values.iter().any(|member| member.name() == text && member.value().is_none()) => Ok(()),
            LiteralValue::Text(text) if values.iter().all(|member| member.name() != text && member.value().is_none()) => Err(impossible("physical enum member not in declared domain")),
            _ => Err(residual("enum value requires vendor representation/comparison evidence")),
        },
        _ => Err(residual("physical datatype has no portable exact scalar witness law")),
    }
}

fn parse_integer(text: &str) -> Option<i128> {
    if text.starts_with('+') || text.trim() != text || text.is_empty() {
        return None;
    }
    text.parse::<i128>().ok()
}

fn parse_unsigned(text: &str) -> Option<u128> {
    if text.starts_with('+') || text.starts_with('-') || text.trim() != text || text.is_empty() {
        return None;
    }
    text.parse::<u128>().ok()
}

fn validate_decimal(text: &str, precision: Option<u64>, scale: Option<u64>) -> StateResult<()> {
    let unsigned = text.strip_prefix('-').unwrap_or(text);
    let (whole, fractional) = unsigned.split_once('.').unwrap_or((unsigned, ""));
    if whole.is_empty() || !whole.bytes().all(|byte| byte.is_ascii_digit())
        || !fractional.bytes().all(|byte| byte.is_ascii_digit())
        || unsigned.matches('.').count() > 1
    {
        return Err(residual("decimal requires a finite base-10 exact literal"));
    }
    // Trailing fractional zeroes do not change the represented value.
    let fractional_digits = fractional.trim_end_matches('0').len() as u64;
    let integer_digits = whole.trim_start_matches('0').len() as u64;
    if let Some(max_scale) = scale {
        if fractional_digits > max_scale {
            return Err(impossible("decimal fractional value exceeds declared scale"));
        }
    } else if fractional_digits > 0 && precision.is_some() {
        return Err(residual("decimal scale is unspecified"));
    }
    if let Some(max_precision) = precision {
        let Some(max_scale) = scale else {
            return Err(residual("decimal precision has no certified scale"));
        };
        let Some(max_integer_digits) = max_precision.checked_sub(max_scale) else {
            return Err(residual("invalid declared decimal precision and scale"));
        };
        if integer_digits > max_integer_digits {
            return Err(impossible("decimal integer digits exceed declared precision and scale"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_decimal_precision_scale_and_overflow() {
        let decimal = DataType::Decimal { precision: Some(5), scale: Some(2) };
        for number in ["-123.45", "0.0000", "12.3400", "999.99"] {
            assert!(validate_value(&LiteralValue::Number(number.into()), &decimal).is_ok(), "{number}");
        }
        for number in ["1000.00", "0.001", "123.456"] {
            assert!(matches!(validate_value(&LiteralValue::Number(number.into()), &decimal), Err(StateFailure::Impossible(_))), "{number}");
        }
        for number in ["1e5", "NaN", "Infinity"] {
            assert!(matches!(validate_value(&LiteralValue::Number(number.into()), &decimal), Err(StateFailure::Residual(..))), "{number}");
        }
    }

    #[test]
    fn integer_bounds_are_impossible_and_float_rounding_is_residual() {
        let small = DataType::SignedInteger { bits: Some(16) };
        assert!(validate_value(&LiteralValue::Number("-32768".into()), &small).is_ok());
        assert!(matches!(validate_value(&LiteralValue::Number("32768".into()), &small), Err(StateFailure::Impossible(_))));
        let float = DataType::FloatingPoint { bits: Some(32) };
        assert!(validate_value(&LiteralValue::Number("16777216".into()), &float).is_ok());
        assert!(matches!(validate_value(&LiteralValue::Number("0.1".into()), &float), Err(StateFailure::Residual(..))));
    }

    #[test]
    fn varchar_ascii_length_and_non_ascii_remain_separate() {
        let text = DataType::String { length: Some(2), fixed: false };
        assert!(validate_value(&LiteralValue::Text("ab".into()), &text).is_ok());
        assert!(matches!(validate_value(&LiteralValue::Text("abc".into()), &text), Err(StateFailure::Impossible(_))));
        assert!(matches!(validate_value(&LiteralValue::Text("é".into()), &text), Err(StateFailure::Residual(..))));
    }
}
