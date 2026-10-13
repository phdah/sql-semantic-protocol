//! Exact, cast-free comparisons for enforced scalar CHECK predicates.
//!
//! Base-ten decimals are compared digitwise rather than through binary floats,
//! preserving arbitrarily large precision without rounding or overflow.

use super::*;
use std::cmp::Ordering;
use crate::constraints::CheckCompareOperator;

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExactDecimal {
    negative: bool,
    integer: String,
    fraction: String,
}

impl ExactDecimal {
    fn parse(source: &str) -> Option<Self> {
        let (negative, body) = if let Some(rest) = source.strip_prefix('-') {
            (true, rest)
        } else {
            (false, source.strip_prefix('+').unwrap_or(source))
        };
        let (whole, fraction) = body.split_once('.').unwrap_or((body, ""));
        if whole.is_empty() && fraction.is_empty() {
            return None;
        }
        if !whole.bytes().all(|c| c.is_ascii_digit())
            || !fraction.bytes().all(|c| c.is_ascii_digit())
        {
            return None;
        }
        let integer = whole.trim_start_matches('0');
        let integer = if integer.is_empty() { "0" } else { integer };
        let fraction = fraction.trim_end_matches('0');
        Some(Self {
            negative: negative && (integer != "0" || !fraction.is_empty()),
            integer: integer.to_string(),
            fraction: fraction.to_string(),
        })
    }

    fn compare(&self, other: &Self) -> Ordering {
        if self.negative != other.negative {
            return if self.negative { Ordering::Less } else { Ordering::Greater };
        }
        let magnitude = self.integer.len().cmp(&other.integer.len())
            .then_with(|| self.integer.cmp(&other.integer))
            .then_with(|| {
                let n = self.fraction.len().max(other.fraction.len());
                (0..n).map(|index| {
                    (self.fraction.as_bytes().get(index).copied().unwrap_or(b'0'))
                        .cmp(&other.fraction.as_bytes().get(index).copied().unwrap_or(b'0'))
                }).find(|order| *order != Ordering::Equal).unwrap_or(Ordering::Equal)
            });
        if self.negative { magnitude.reverse() } else { magnitude }
    }
}

fn matches_comparison(order: Ordering, operator: CheckCompareOperator) -> bool {
    match operator {
        CheckCompareOperator::Eq => order == Ordering::Equal,
        CheckCompareOperator::NotEq => order != Ordering::Equal,
        CheckCompareOperator::Gt => order == Ordering::Greater,
        CheckCompareOperator::Gte => order != Ordering::Less,
        CheckCompareOperator::Lt => order == Ordering::Less,
        CheckCompareOperator::Lte => order != Ordering::Greater,
    }
}

/// Validate a CHECK comparator only when its exact physical operand law is known.
pub(super) fn validate_scalar_check(
    literal: &LiteralValue,
    operator: CheckCompareOperator,
    constant: &ConstraintValue,
) -> StateResult<()> {
    if matches!(literal, LiteralValue::Null) || matches!(constant, ConstraintValue::Null) {
        return Ok(()); // Three-valued SQL CHECK admits UNKNOWN.
    }
    let comparison = match (literal, constant) {
        (LiteralValue::Number(left), ConstraintValue::Integer(right)) => {
            ExactDecimal::parse(left).zip(ExactDecimal::parse(&right.to_string()))
                .map(|(left, right)| left.compare(&right))
        }
        (LiteralValue::Number(left), ConstraintValue::UnsignedInteger(right)) => {
            ExactDecimal::parse(left).zip(ExactDecimal::parse(&right.to_string()))
                .map(|(left, right)| left.compare(&right))
        }
        (LiteralValue::Number(left), ConstraintValue::Number(right)) => {
            ExactDecimal::parse(left).zip(ExactDecimal::parse(right))
                .map(|(left, right)| left.compare(&right))
        }
        (LiteralValue::Boolean(left), ConstraintValue::Boolean(right))
            if matches!(operator, CheckCompareOperator::Eq | CheckCompareOperator::NotEq) =>
        {
            Some(left.cmp(right))
        }
        (LiteralValue::Text(left), ConstraintValue::String(right))
            if left == right && matches!(operator, CheckCompareOperator::Eq | CheckCompareOperator::NotEq) =>
        {
            // Equality of byte-identical strings is reflexive regardless
            // of the physical collation; distinct bytes need collation law.
            Some(Ordering::Equal)
        }
        _ => None,
    };
    let order = comparison.ok_or_else(|| residual(
        "CHECK comparison needs exact decimal representation or attested collation/coercion law",
    ))?;
    if matches_comparison(order, operator) {
        Ok(())
    } else {
        Err(impossible("enforced scalar CHECK comparison violated"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decimals_compare_without_binary_float_rounding() {
        let a = ExactDecimal::parse("999999999999999999999999999999.01").unwrap();
        let b = ExactDecimal::parse("999999999999999999999999999999.010").unwrap();
        assert_eq!(a.compare(&b), Ordering::Equal);
        assert_eq!(
            ExactDecimal::parse("-0.200").unwrap().compare(&ExactDecimal::parse("-0.19").unwrap()),
            Ordering::Less
        );
        assert_eq!(ExactDecimal::parse("0").unwrap(), ExactDecimal::parse("-0.0").unwrap());
        assert!(ExactDecimal::parse("NaN").is_none());
        assert!(ExactDecimal::parse("3e4").is_none());
    }

    #[test]
    fn check_null_boolean_decimal_and_unproved_text() {
        assert!(validate_scalar_check(
            &LiteralValue::Null, CheckCompareOperator::Eq, &ConstraintValue::Integer(4)
        ).is_ok());
        assert!(validate_scalar_check(
            &LiteralValue::Number("1.50".into()), CheckCompareOperator::Gte,
            &ConstraintValue::Number("1.5".into())
        ).is_ok());
        assert!(matches!(validate_scalar_check(
            &LiteralValue::Number("1.49".into()), CheckCompareOperator::Gte,
            &ConstraintValue::Number("1.5".into())
        ), Err(StateFailure::Impossible(_))));
        assert!(validate_scalar_check(
            &LiteralValue::Boolean(false), CheckCompareOperator::NotEq,
            &ConstraintValue::Boolean(true)
        ).is_ok());
        assert!(matches!(validate_scalar_check(
            &LiteralValue::Text("a".into()), CheckCompareOperator::Eq,
            &ConstraintValue::String("A".into())
        ), Err(StateFailure::Residual(_, _))));
    }
}
