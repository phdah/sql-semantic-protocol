//! Portable, exact scalar decimal arithmetic on representable SQL literals.
//!
//! Without attested engine precision/rounding laws, arbitrary DECIMAL arithmetic
//! cannot be evaluated by the protocol. The admitted subset uses bounded
//! fixed-point integers and requires *every* operand and result to be exactly
//! representable in binary64 as well as in fixed decimal. This prevents a
//! dialect using floating arithmetic from diverging from a decimal engine.

use super::*;

#[derive(Clone, Copy)]
struct ExactDecimal {
    units: i128,
    scale: u32,
}

impl ExactDecimal {
    fn from_literal(literal: &LiteralExpression) -> Option<Self> {
        if !matches!(literal.literal_type(), LiteralType::Integer | LiteralType::Decimal) {
            return None;
        }
        let LiteralValue::Number(text) = literal.value() else {
            return None;
        };
        let (negative, number) = match text.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, text.as_str()),
        };
        let number = number.strip_prefix('+').unwrap_or(number);
        let (integer, fraction) = number.split_once('.').unwrap_or((number, ""));
        if integer.is_empty() || !integer.bytes().all(|byte| byte.is_ascii_digit())
            || !fraction.bytes().all(|byte| byte.is_ascii_digit())
        {
            return None;
        }
        let scale = u32::try_from(fraction.len()).ok()?;
        if scale > 15 {
            return None;
        }
        let magnitude = format!("{integer}{fraction}").parse::<i128>().ok()?;
        let units = if negative {
            magnitude.checked_neg()?
        } else {
            magnitude
        };
        let parsed = Self { units, scale };
        parsed.is_portably_exact().then_some(parsed)
    }

    // For units/10^scale to be an exact binary fraction, 5^scale must
    // divide units. A <=53-bit numerator and scale<=15 fit binary64 exactly.
    fn is_portably_exact(self) -> bool {
        let mut numerator = self.units;
        for _ in 0..self.scale {
            if numerator % 5 != 0 {
                return false;
            }
            numerator /= 5;
        }
        numerator.unsigned_abs() <= (1_u128 << 53)
    }

    fn rescale(self, target: u32) -> Option<i128> {
        self.units.checked_mul(10_i128.checked_pow(target.checked_sub(self.scale)?)?)
    }

    fn value(self) -> LiteralExpression {
        let magnitude = self.units.unsigned_abs().to_string();
        let scale = usize::try_from(self.scale).unwrap_or(0);
        let text = if scale == 0 {
            format!("{magnitude}.0")
        } else {
            let padded = format!("{magnitude:0>width$}", width = scale + 1);
            let cut = padded.len() - scale;
            let integer = &padded[..cut];
            let fractional = padded[cut..].trim_end_matches('0');
            if fractional.is_empty() {
                format!("{integer}.0")
            } else {
                format!("{integer}.{fractional}")
            }
        };
        let text = if self.units < 0 { format!("-{text}") } else { text };
        LiteralExpression::new(LiteralType::Decimal, LiteralValue::Number(text))
    }

    fn unary_minus(self) -> Option<Self> {
        let result = Self {
            units: self.units.checked_neg()?,
            ..self
        };
        result.is_portably_exact().then_some(result)
    }

    fn binary(self, rhs: Self, operator: BinaryOperator) -> Option<Self> {
        let result = match operator {
            BinaryOperator::Add | BinaryOperator::Subtract => {
                let scale = self.scale.max(rhs.scale);
                let left = self.rescale(scale)?;
                let right = rhs.rescale(scale)?;
                let units = if operator == BinaryOperator::Add {
                    left.checked_add(right)?
                } else {
                    left.checked_sub(right)?
                };
                Self { units, scale }
            }
            BinaryOperator::Multiply => Self {
                units: self.units.checked_mul(rhs.units)?,
                scale: self.scale.checked_add(rhs.scale)?,
            },
            BinaryOperator::Divide => {
                if rhs.units == 0 {
                    return None;
                }
                let denominator = rhs.units.checked_mul(10_i128.checked_pow(self.scale)?)?;
                let base_numerator = self.units.checked_mul(10_i128.checked_pow(rhs.scale)?)?;
                let mut found = None;
                for scale in 0..=15 {
                    let Some(numerator) = base_numerator.checked_mul(10_i128.checked_pow(scale)?) else {
                        break;
                    };
                    if numerator % denominator == 0 {
                        found = Some(Self {
                            units: numerator / denominator,
                            scale,
                        });
                        break;
                    }
                }
                found?
            }
            BinaryOperator::Modulo if self.units >= 0 && rhs.units > 0 => {
                let scale = self.scale.max(rhs.scale);
                Self {
                    units: self.rescale(scale)? % rhs.rescale(scale)?,
                    scale,
                }
            }
            BinaryOperator::Modulo
            | BinaryOperator::StringConcat
            | BinaryOperator::BitwiseAnd
            | BinaryOperator::BitwiseOr
            | BinaryOperator::BitwiseXor => return None,
        };
        (result.scale <= 15 && result.is_portably_exact()).then_some(result)
    }
}

fn decimal_from_domain(domain: &ValueDomain) -> Option<(ExactDecimal, bool)> {
    let ValueDomain::Set(values) = domain else {
        return None;
    };
    if values.mode() != SetMode::Include {
        return None;
    }
    let [literal] = values.values() else {
        return None;
    };
    Some((
        ExactDecimal::from_literal(literal)?,
        literal.literal_type() == LiteralType::Decimal,
    ))
}

/// A fixed decimal singleton only when no supported SQL numeric engine can
/// disagree about its exact numeric value.
pub(super) fn exact_decimal_binary_domain(binary: &BinaryExpression) -> Option<ValueDomain> {
    let left = derive_output_domain(binary.left());
    let right = derive_output_domain(binary.right());
    let (lhs, left_is_decimal) = decimal_from_domain(&left)?;
    let (rhs, right_is_decimal) = decimal_from_domain(&right)?;
    if !left_is_decimal && !right_is_decimal {
        return None;
    }
    let result = lhs.binary(rhs, binary.operator())?;
    Some(ValueDomain::set(SetMode::Include, vec![result.value()]))
}

pub(super) fn exact_decimal_unary_domain(unary: &UnaryExpression) -> Option<ValueDomain> {
    let operand = derive_output_domain(unary.operand());
    let (value, is_decimal) = decimal_from_domain(&operand)?;
    if !is_decimal {
        return None;
    }
    let result = match unary.operator() {
        UnaryOperator::Plus => value,
        UnaryOperator::Minus => value.unary_minus()?,
        UnaryOperator::BitwiseNot => return None,
    };
    Some(ValueDomain::set(SetMode::Include, vec![result.value()]))
}
