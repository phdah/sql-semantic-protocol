//! Conservative physical literal compatibility with canonical schema types.
//!
//! This checks caller-supplied closed-world rows, not dialect-dependent SQL casts.
//! A provable contradiction is impossible; any opaque representation remains residual.

use super::*;

pub(super) fn validate_column_value(
    value: &LiteralValue,
    column: &crate::relation::SchemaColumn,
) -> StateResult<()> {
    if matches!(value, LiteralValue::Null) {
        return Ok(());
    }
    if let DataType::Timestamp { precision } = column.data_type() {
        let LiteralValue::Text(text) = value else {
            return Err(residual("timestamp requires a canonical textual instant"));
        };
        let zone = column
            .timestamp_zone()
            .ok_or_else(|| residual("timestamp zone semantics have not been declared"))?;
        let (date, time) = text
            .split_once('T')
            .or_else(|| text.split_once(' '))
            .ok_or_else(|| residual("timestamp requires ISO date and time"))?;
        validate_iso_date(date)?;
        let instant = match zone {
            crate::relation::TimestampZone::WithTimeZone => {
                time.strip_suffix('Z').ok_or_else(|| {
                    residual("timestamp-with-time-zone requires canonical UTC Z offset")
                })?
            }
            crate::relation::TimestampZone::WithoutTimeZone => {
                if time.contains('Z') || time.contains('+') {
                    return Err(residual("wall-clock timestamp cannot contain an offset"));
                }
                time
            }
        };
        return validate_iso_time(instant, *precision);
    }
    validate_value(value, column.data_type())
}

pub(super) fn validate_value(value: &LiteralValue, kind: &DataType) -> StateResult<()> {
    if matches!(value, LiteralValue::Null) {
        return Ok(());
    }
    match kind {
        DataType::Nullable(inner) => validate_value(value, inner),
        DataType::Boolean => match value {
            LiteralValue::Boolean(_) => Ok(()),
            _ => Err(impossible(
                "physical boolean value contradicts declared datatype",
            )),
        },
        DataType::SignedInteger { bits } => match value {
            LiteralValue::Number(number) => {
                let Some(value) = parse_integer(number) else {
                    return Err(residual(
                        "signed integer literal requires exact integral representation",
                    ));
                };
                let range = match bits {
                    Some(8) => i8::try_from(value).is_ok(),
                    Some(16) => i16::try_from(value).is_ok(),
                    Some(32) => i32::try_from(value).is_ok(),
                    Some(64) => i64::try_from(value).is_ok(),
                    None => return Err(residual("signed integer width is not attested")),
                    Some(128) => true,
                    _ => return Err(residual("unknown signed integer storage width")),
                };
                if range {
                    Ok(())
                } else {
                    Err(impossible(
                        "physical integer exceeds signed datatype bounds",
                    ))
                }
            }
            _ => Err(residual("unproved signed integer literal coercion")),
        },
        DataType::UnsignedInteger { bits } => match value {
            LiteralValue::Number(number) => {
                let Some(value) = parse_unsigned(number) else {
                    return Err(residual(
                        "unsigned integer literal requires exact integral representation",
                    ));
                };
                let range = match bits {
                    Some(8) => u8::try_from(value).is_ok(),
                    Some(16) => u16::try_from(value).is_ok(),
                    Some(32) => u32::try_from(value).is_ok(),
                    Some(64) => u64::try_from(value).is_ok(),
                    None => return Err(residual("unsigned integer width is not attested")),
                    Some(128) => true,
                    _ => return Err(residual("unknown unsigned integer storage width")),
                };
                if range {
                    Ok(())
                } else {
                    Err(impossible(
                        "physical integer exceeds unsigned datatype bounds",
                    ))
                }
            }
            _ => Err(residual("unproved unsigned integer literal coercion")),
        },
        DataType::Decimal { precision, scale } => match value {
            LiteralValue::Number(number) => validate_decimal(number, *precision, *scale),
            _ => Err(residual(
                "decimal source value requires exact numeric representation",
            )),
        },
        DataType::FloatingPoint { bits } => match value {
            LiteralValue::Number(number) => {
                if number.starts_with('-') && parse_integer(number) == Some(0) {
                    return Err(residual(
                        "signed floating zero requires explicit storage law",
                    ));
                }
                let Some(value) = parse_integer(number) else {
                    return Err(residual(
                        "floating point rounding/NaN/signed-zero law is unproved",
                    ));
                };
                // Integers below the significand limit are exactly represented by IEEE 754.
                // Larger values and decimal fractions need an explicit rounding law.
                let exact = match bits {
                    Some(32) => value.unsigned_abs() <= (1_u128 << 24),
                    Some(64) => value.unsigned_abs() <= (1_u128 << 53),
                    None => return Err(residual("floating point width is not attested")),
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
                let within =
                    length.is_none_or(|n| u64::try_from(text.len()).is_ok_and(|size| size <= n));
                if !within {
                    return Err(impossible(
                        "physical string exceeds declared character length",
                    ));
                }
                if *fixed && length.is_none_or(|n| u64::try_from(text.len()).ok() != Some(n)) {
                    return Err(residual("fixed-width CHAR padding is not proven"));
                }
                Ok(())
            }
            LiteralValue::Text(_) => Err(residual(
                "non-ASCII string length requires dialect collation/character semantics",
            )),
            _ => Err(residual("unproved physical string coercion")),
        },
        DataType::Date => match value {
            LiteralValue::Text(text) => validate_iso_date(text),
            _ => Err(residual("date requires canonical ISO date representation")),
        },
        DataType::Time { precision } => match value {
            LiteralValue::Text(text) => validate_iso_time(text, *precision),
            _ => Err(residual("time requires canonical clock representation")),
        },
        DataType::Uuid => match value {
            LiteralValue::Text(text)
                if text.len() == 36
                    && text.bytes().enumerate().all(|(i, c)| {
                        if matches!(i, 8 | 13 | 18 | 23) {
                            c == b'-'
                        } else {
                            c.is_ascii_digit() || (b'a'..=b'f').contains(&c)
                        }
                    }) =>
            {
                Ok(())
            }
            LiteralValue::Text(text) if text.is_ascii() && text.len() == 36 => Err(impossible(
                "canonical UUID has invalid separator or hex digit",
            )),
            _ => Err(residual(
                "UUID requires lowercase canonical hyphenated text",
            )),
        },
        DataType::Enum { values } => match value {
            LiteralValue::Text(text)
                if values
                    .iter()
                    .any(|member| member.name() == text && member.value().is_none()) =>
            {
                Ok(())
            }
            LiteralValue::Text(text)
                if values
                    .iter()
                    .all(|member| member.name() != text && member.value().is_none()) =>
            {
                Err(impossible("physical enum member not in declared domain"))
            }
            _ => Err(residual(
                "enum value requires vendor representation/comparison evidence",
            )),
        },
        _ => Err(residual(
            "physical datatype has no portable exact scalar witness law",
        )),
    }
}

fn validate_iso_date(text: &str) -> StateResult<()> {
    match crate::temporal::canonical_date_text(text) {
        Ok(_) => Ok(()),
        Err(crate::temporal::DateTextError::Syntax) => {
            Err(residual("date requires YYYY-MM-DD calendar syntax"))
        }
        Err(crate::temporal::DateTextError::Calendar) => Err(impossible(
            "canonical date violates Gregorian calendar bounds",
        )),
    }
}

fn validate_iso_time(text: &str, precision: Option<u64>) -> StateResult<()> {
    let (whole, fraction) = match text.split_once('.') {
        Some((whole, fraction)) => (whole, Some(fraction)),
        None => (text, None),
    };
    let bytes = whole.as_bytes();
    if bytes.len() != 8
        || bytes[2] != b':'
        || bytes[5] != b':'
        || bytes
            .iter()
            .enumerate()
            .any(|(i, c)| i != 2 && i != 5 && !c.is_ascii_digit())
        || fraction.is_some_and(|v| v.is_empty() || !v.bytes().all(|c| c.is_ascii_digit()))
    {
        return Err(residual(
            "time requires HH:MM:SS with an optional decimal fraction",
        ));
    }
    let hour = whole[..2].parse::<u32>().unwrap_or(99);
    let minute = whole[3..5].parse::<u32>().unwrap_or(99);
    let second = whole[6..8].parse::<u32>().unwrap_or(99);
    if hour >= 24 || minute >= 60 || second >= 60 {
        return Err(impossible("canonical physical time exceeds clock bounds"));
    }
    if let Some(fraction) = fraction {
        let significant = fraction.trim_end_matches('0').len() as u64;
        if let Some(max) = precision {
            if significant > max {
                return Err(impossible("fractional seconds exceed declared precision"));
            }
        } else if significant > 0 {
            return Err(residual("fractional seconds precision not declared"));
        }
    }
    Ok(())
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
    if precision.is_none() || scale.is_none() {
        return Err(residual("decimal storage precision/scale are not attested"));
    }
    let unsigned = text.strip_prefix('-').unwrap_or(text);
    let (whole, fractional) = unsigned.split_once('.').unwrap_or((unsigned, ""));
    if whole.is_empty()
        || !whole.bytes().all(|byte| byte.is_ascii_digit())
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
            return Err(impossible(
                "decimal fractional value exceeds declared scale",
            ));
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
            return Err(impossible(
                "decimal integer digits exceed declared precision and scale",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_decimal_precision_scale_and_overflow() {
        let decimal = DataType::Decimal {
            precision: Some(5),
            scale: Some(2),
        };
        for number in ["-123.45", "0.0000", "12.3400", "999.99"] {
            assert!(
                validate_value(&LiteralValue::Number(number.into()), &decimal).is_ok(),
                "{number}"
            );
        }
        for number in ["1000.00", "0.001", "123.456"] {
            assert!(
                matches!(
                    validate_value(&LiteralValue::Number(number.into()), &decimal),
                    Err(StateFailure::Impossible(_))
                ),
                "{number}"
            );
        }
        for number in ["1e5", "NaN", "Infinity"] {
            assert!(
                matches!(
                    validate_value(&LiteralValue::Number(number.into()), &decimal),
                    Err(StateFailure::Residual(..))
                ),
                "{number}"
            );
        }
    }

    #[test]
    fn integer_bounds_are_impossible_and_float_rounding_is_residual() {
        let small = DataType::SignedInteger { bits: Some(16) };
        assert!(validate_value(&LiteralValue::Number("-32768".into()), &small).is_ok());
        assert!(matches!(
            validate_value(&LiteralValue::Number("32768".into()), &small),
            Err(StateFailure::Impossible(_))
        ));
        let float = DataType::FloatingPoint { bits: Some(32) };
        for negative_zero in ["-0", "-00", "-000000"] {
            assert!(matches!(
                validate_value(&LiteralValue::Number(negative_zero.into()), &float),
                Err(StateFailure::Residual(..))
            ));
        }
        assert!(validate_value(&LiteralValue::Number("16777216".into()), &float).is_ok());
        assert!(matches!(
            validate_value(&LiteralValue::Number("0.1".into()), &float),
            Err(StateFailure::Residual(..))
        ));
    }

    #[test]
    fn varchar_ascii_length_and_non_ascii_remain_separate() {
        let text = DataType::String {
            length: Some(2),
            fixed: false,
        };
        assert!(validate_value(&LiteralValue::Text("ab".into()), &text).is_ok());
        assert!(matches!(
            validate_value(&LiteralValue::Text("abc".into()), &text),
            Err(StateFailure::Impossible(_))
        ));
        assert!(matches!(
            validate_value(&LiteralValue::Text("é".into()), &text),
            Err(StateFailure::Residual(..))
        ));
    }
    #[test]
    fn calendar_and_temporal_feasibility_respects_leap_days_and_precision() {
        assert!(validate_value(&LiteralValue::Text("2024-02-29".into()), &DataType::Date).is_ok());
        assert!(matches!(
            validate_value(&LiteralValue::Text("2026-02-29".into()), &DataType::Date),
            Err(StateFailure::Impossible(_))
        ));
        assert!(matches!(
            validate_value(&LiteralValue::Text("02/29/2024".into()), &DataType::Date),
            Err(StateFailure::Residual(..))
        ));
        assert!(validate_value(
            &LiteralValue::Text("23:59:59.1200".into()),
            &DataType::Time { precision: Some(2) },
        )
        .is_ok());
        assert!(matches!(
            validate_value(
                &LiteralValue::Text("23:59:59.123".into()),
                &DataType::Time { precision: Some(2) },
            ),
            Err(StateFailure::Impossible(_))
        ));
        assert!(matches!(
            validate_value(
                &LiteralValue::Text("24:00:00".into()),
                &DataType::Time { precision: Some(0) },
            ),
            Err(StateFailure::Impossible(_))
        ));
    }

    #[test]
    fn utc_timestamp_needs_zone_metadata_and_nested_json_remains_residual() {
        use crate::relation::{SchemaColumn, TimestampZone};
        let unknown = SchemaColumn::new("t", DataType::Timestamp { precision: Some(3) }).unwrap();
        let instant = LiteralValue::Text("2026-10-13T08:05:00.123Z".into());
        assert!(matches!(
            validate_column_value(&instant, &unknown),
            Err(StateFailure::Residual(..))
        ));
        let known = unknown
            .with_timestamp_zone(TimestampZone::WithTimeZone)
            .unwrap();
        assert!(validate_column_value(&instant, &known).is_ok());
        let nested = DataType::Array {
            element: Some(Box::new(DataType::SignedInteger { bits: Some(64) })),
            length: None,
        };
        for kind in [&DataType::Json, &nested] {
            assert!(matches!(
                validate_value(&LiteralValue::Text("[1,2]".into()), kind),
                Err(StateFailure::Residual(..))
            ));
        }
    }
    #[test]
    fn unspecified_numeric_storage_remains_residual() {
        for kind in [
            DataType::SignedInteger { bits: None },
            DataType::UnsignedInteger { bits: None },
            DataType::FloatingPoint { bits: None },
            DataType::Decimal {
                precision: None,
                scale: None,
            },
        ] {
            assert!(matches!(
                validate_value(&LiteralValue::Number("1".into()), &kind),
                Err(StateFailure::Residual(..))
            ));
        }
    }
}
