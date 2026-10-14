//! Total, literal-backed signed integer casts.
//!
//! Explicit cast forms retain failure semantics in the canonical expression.
//! Only canonical ASCII integer text, integral fixed decimal or integer
//! operands are converted. Non-integral decimals, unconstrained source
//! columns and session-dependent representations remain unknown.

use super::*;

pub(super) fn exact_integral_literal(literal: &LiteralExpression) -> Option<i128> {
    match (literal.literal_type(), literal.value()) {
        (LiteralType::Integer, LiteralValue::Number(value)) => value.parse().ok(),
        (LiteralType::Decimal, LiteralValue::Number(value)) => {
            let (integer, fraction) = value.split_once('.')?;
            (!fraction.is_empty() && fraction.bytes().all(|digit| digit == b'0'))
                .then(|| integer.parse().ok())
                .flatten()
        }
        (LiteralType::String, LiteralValue::Text(value)) => {
            let digits = value.strip_prefix('-').unwrap_or(value);
            // Only a canonical base-10 spelling has cross-engine conversion
            // behavior: no whitespace, exponent, locale separators or sign '+'.
            if digits.is_empty()
                || (!digits.starts_with('0') && digits.bytes().all(|b| b.is_ascii_digit()))
                || digits == "0"
            {
                let canonical = digits == "0"
                    || (!digits.starts_with('0') && digits.bytes().all(|b| b.is_ascii_digit()));
                canonical.then(|| value.parse().ok()).flatten()
            } else {
                None
            }
        }
        _ => None,
    }
}

fn signed_fits(value: i128, bits: u16) -> bool {
    if !matches!(bits, 16 | 32 | 64) {
        return false;
    }
    let magnitude = 1_i128 << (u32::from(bits) - 1);
    -magnitude <= value && value < magnitude
}

/// Evaluate explicit, portable finite signed integer casts without inventing
/// engine-specific decimal rounding, formatting or overflow results.
pub(super) fn derive_signed_integer_cast_output_domain(
    cast: &SignedIntegerCastExpression,
) -> ValueDomain {
    let input = derive_output_domain(cast.expression());
    let ValueDomain::Set(values) = input else {
        return ValueDomain::unknown(
            "integer cast output requires finite typed input or catalog evidence",
        );
    };
    if values.mode() != SetMode::Include {
        return ValueDomain::unknown("integer cast excludes values without finite proof");
    }
    let mut result = Vec::new();
    for literal in values.values() {
        if literal.literal_type() == LiteralType::Null {
            result.push(LiteralExpression::new(
                LiteralType::Null,
                LiteralValue::Null,
            ));
            continue;
        }
        let Some(integer) = exact_integral_literal(literal) else {
            return ValueDomain::unknown(
                "integer cast source requires exact integral or canonical ASCII evidence",
            );
        };
        if signed_fits(integer, cast.target_bits()) {
            result.push(integer_literal(integer));
        } else {
            match cast.cast_kind() {
                SignedIntegerCastKind::Cast => {
                    return ValueDomain::unknown(
                        "ordinary CAST would raise on out-of-range integer",
                    );
                }
                SignedIntegerCastKind::TryCast | SignedIntegerCastKind::SafeCast => {
                    result.push(LiteralExpression::new(
                        LiteralType::Null,
                        LiteralValue::Null,
                    ));
                }
            }
        }
    }
    ValueDomain::set(SetMode::Include, result)
}
