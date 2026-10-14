//! Catalog-certified integer projection domains.
//!
//! This is deliberately stricter than algebraic interval propagation: each
//! physical source requires an attested signed type and a predicate-constrained
//! non-NULL/NULL value domain. Every possible intermediate must fit its width.
//! Unproved coercion or overflow never becomes an exact projected range.

use super::*;

#[derive(Clone)]
enum IntegerValues {
    Finite(Vec<Option<i128>>),
    Interval { minimum: i128, maximum: i128 },
}

#[derive(Clone)]
struct TypedIntegers {
    bits: u16,
    values: IntegerValues,
}

fn signed_limits(bits: u16) -> Option<(i128, i128)> {
    if !matches!(bits, 16 | 32 | 64) {
        return None;
    }
    let magnitude = 1_i128 << (u32::from(bits) - 1);
    Some((-magnitude, magnitude - 1))
}

fn fits(value: i128, bits: u16) -> bool {
    signed_limits(bits).is_some_and(|(minimum, maximum)| minimum <= value && value <= maximum)
}

fn integer_value(literal: &LiteralExpression) -> Option<i128> {
    if literal.literal_type() != LiteralType::Integer {
        return None;
    }
    match literal.value() {
        LiteralValue::Number(value) => value.parse().ok(),
        _ => None,
    }
}

fn constrained_values(domain: &ValueDomain, bits: u16) -> Option<IntegerValues> {
    match domain {
        ValueDomain::Empty => Some(IntegerValues::Finite(Vec::new())),
        ValueDomain::Set(set) if set.mode() == SetMode::Include => {
            let mut values = Vec::new();
            for value in set.values() {
                let parsed = if value.literal_type() == LiteralType::Null {
                    None
                } else {
                    let number = integer_value(value)?;
                    if !fits(number, bits) {
                        return None;
                    }
                    Some(number)
                };
                values.push(parsed);
            }
            Some(IntegerValues::Finite(values))
        }
        ValueDomain::Ranges(ranges) if ranges.ranges().len() == 1 => {
            let range = &ranges.ranges()[0];
            let lower = range.lower()?;
            let upper = range.upper()?;
            let minimum =
                integer_value(lower.value())?.checked_add(i128::from(!lower.inclusive()))?;
            let maximum =
                integer_value(upper.value())?.checked_sub(i128::from(!upper.inclusive()))?;
            if !fits(minimum, bits) || !fits(maximum, bits) {
                return None;
            }
            if minimum > maximum {
                Some(IntegerValues::Finite(Vec::new()))
            } else {
                Some(IntegerValues::Interval { minimum, maximum })
            }
        }
        ValueDomain::Unbounded
        | ValueDomain::Unknown(_)
        | ValueDomain::Ranges(_)
        | ValueDomain::Set(_) => None,
    }
}

fn typed_expression(
    expression: &Expression,
    domains: &[ColumnDomain],
    sources: &[SourceRelation],
    metadata: &AnalysisMetadata<'_>,
) -> Option<TypedIntegers> {
    match expression {
        Expression::Column(column) => {
            let resolved = resolve_column(column, sources);
            let declared = metadata.column_data_type(&resolved)?;
            let declared = match declared {
                DataType::Nullable(inner) => inner.as_ref(),
                other => other,
            };
            let DataType::SignedInteger { bits: Some(bits) } = declared else {
                return None;
            };
            let values = constrained_values(
                domains
                    .iter()
                    .find(|item| item.column() == &resolved)?
                    .domain(),
                *bits,
            )?;
            Some(TypedIntegers {
                bits: *bits,
                values,
            })
        }
        Expression::Literal(literal) => {
            if literal.literal_type() == LiteralType::Null {
                return Some(TypedIntegers {
                    bits: 64,
                    values: IntegerValues::Finite(vec![None]),
                });
            }
            let value = integer_value(literal)?;
            if !fits(value, 64) {
                return None;
            }
            Some(TypedIntegers {
                bits: 64,
                values: IntegerValues::Finite(vec![Some(value)]),
            })
        }
        Expression::SignedIntegerCast(cast) => {
            let mut input = typed_expression(cast.expression(), domains, sources, metadata)?;
            if !every_value_fits(&input.values, cast.target_bits()) {
                // CAST may throw, whereas TRY/SAFE_CAST may introduce NULL.
                // Neither failure path is a total identity.
                return None;
            }
            input.bits = cast.target_bits();
            Some(input)
        }
        Expression::Unary(unary) => {
            let mut input = typed_expression(unary.operand(), domains, sources, metadata)?;
            input.values = match unary.operator() {
                UnaryOperator::Plus => input.values,
                UnaryOperator::Minus => map_unary_minus(input.values, input.bits)?,
                UnaryOperator::BitwiseNot => return None,
            };
            Some(input)
        }
        Expression::Binary(binary) => {
            let left = typed_expression(binary.left(), domains, sources, metadata)?;
            let right = typed_expression(binary.right(), domains, sources, metadata)?;
            // Conservatively demand that the result fits even the narrowest
            // admissible signed operand width, independent of dialect promotion.
            let bits = left.bits.min(right.bits);
            let values = combine_values(left.values, right.values, binary.operator(), bits)?;
            Some(TypedIntegers { bits, values })
        }
        Expression::Function(_)
        | Expression::AggregateFunction(_)
        | Expression::WindowFunction(_)
        | Expression::Case(_)
        | Expression::BooleanPredicate(_)
        | Expression::ScalarSubquery(_)
        | Expression::Unknown(_)
        | Expression::Unsupported(_) => None,
    }
}

fn every_value_fits(values: &IntegerValues, bits: u16) -> bool {
    match values {
        IntegerValues::Finite(values) => values.iter().flatten().all(|value| fits(*value, bits)),
        IntegerValues::Interval { minimum, maximum } => {
            fits(*minimum, bits) && fits(*maximum, bits)
        }
    }
}

fn map_unary_minus(values: IntegerValues, bits: u16) -> Option<IntegerValues> {
    let result = match values {
        IntegerValues::Finite(values) => IntegerValues::Finite(
            values
                .into_iter()
                .map(|value| match value {
                    Some(number) => number.checked_neg().map(Some),
                    None => Some(None),
                })
                .collect::<Option<Vec<_>>>()?,
        ),
        IntegerValues::Interval { minimum, maximum } => IntegerValues::Interval {
            minimum: maximum.checked_neg()?,
            maximum: minimum.checked_neg()?,
        },
    };
    every_value_fits(&result, bits).then_some(result)
}

fn enumerate(values: &IntegerValues) -> Option<Vec<Option<i128>>> {
    match values {
        IntegerValues::Finite(values) if values.len() <= 64 => Some(values.clone()),
        IntegerValues::Interval { minimum, maximum } if maximum.checked_sub(*minimum)? <= 63 => {
            Some((*minimum..=*maximum).map(Some).collect())
        }
        _ => None,
    }
}

fn scalar_arithmetic(left: i128, right: i128, operator: BinaryOperator, bits: u16) -> Option<i128> {
    let value = match operator {
        BinaryOperator::Add => left.checked_add(right)?,
        BinaryOperator::Subtract => left.checked_sub(right)?,
        BinaryOperator::Multiply => left.checked_mul(right)?,
        BinaryOperator::Divide => {
            if right == 0 {
                return None;
            }
            if left == 0 {
                0
            } else if matches!(right, -1 | 1) {
                let exact = 1_i128 << 53;
                let result = left.checked_div(right)?;
                if left.abs() > exact || result.abs() > exact {
                    return None;
                }
                result
            } else {
                // Fractional integer division varies by dialect.
                return None;
            }
        }
        BinaryOperator::Modulo => {
            if right == 0 {
                return None;
            }
            if left == 0 || right == 1 {
                0
            } else {
                return None;
            }
        }
        BinaryOperator::StringConcat
        | BinaryOperator::BitwiseAnd
        | BinaryOperator::BitwiseOr
        | BinaryOperator::BitwiseXor => return None,
    };
    fits(value, bits).then_some(value)
}

fn interval_arithmetic(
    left: (i128, i128),
    right: (i128, i128),
    operator: BinaryOperator,
    bits: u16,
) -> Option<IntegerValues> {
    let (left_min, left_max) = left;
    let (right_min, right_max) = right;
    let (minimum, maximum) = match operator {
        BinaryOperator::Add => (
            left_min.checked_add(right_min)?,
            left_max.checked_add(right_max)?,
        ),
        BinaryOperator::Subtract => (
            left_min.checked_sub(right_max)?,
            left_max.checked_sub(right_min)?,
        ),
        BinaryOperator::Multiply => {
            let candidates = [
                left_min.checked_mul(right_min)?,
                left_min.checked_mul(right_max)?,
                left_max.checked_mul(right_min)?,
                left_max.checked_mul(right_max)?,
            ];
            (*candidates.iter().min()?, *candidates.iter().max()?)
        }
        BinaryOperator::Divide if right_min == 1 && right_max == 1 => {
            let exact = 1_i128 << 53;
            if left_min < -exact || left_max > exact {
                return None;
            }
            (left_min, left_max)
        }
        BinaryOperator::Divide if right_min == -1 && right_max == -1 => {
            let exact = 1_i128 << 53;
            let min = left_max.checked_neg()?;
            let max = left_min.checked_neg()?;
            if left_min < -exact || left_max > exact || min < -exact || max > exact {
                return None;
            }
            (min, max)
        }
        BinaryOperator::Divide | BinaryOperator::Modulo
            if left_min == 0 && left_max == 0 && (right_min > 0 || right_max < 0) =>
        {
            (0, 0)
        }
        BinaryOperator::Modulo if right_min == 1 && right_max == 1 => (0, 0),
        BinaryOperator::Divide
        | BinaryOperator::Modulo
        | BinaryOperator::StringConcat
        | BinaryOperator::BitwiseAnd
        | BinaryOperator::BitwiseOr
        | BinaryOperator::BitwiseXor => return None,
    };
    if !fits(minimum, bits) || !fits(maximum, bits) {
        return None;
    }
    Some(IntegerValues::Interval { minimum, maximum })
}

fn combine_values(
    left: IntegerValues,
    right: IntegerValues,
    operator: BinaryOperator,
    bits: u16,
) -> Option<IntegerValues> {
    if let (Some(lhs), Some(rhs)) = (enumerate(&left), enumerate(&right)) {
        let mut values = Vec::new();
        for a in lhs {
            for b in &rhs {
                let value = match (a, *b) {
                    (Some(a), Some(b)) => Some(scalar_arithmetic(a, b, operator, bits)?),
                    _ => None,
                };
                if !values.contains(&value) {
                    values.push(value);
                }
            }
        }
        return Some(IntegerValues::Finite(values));
    }
    // A range can be propagated without finite sampling, but a union of NULL
    // plus a range is not representable as one canonical ValueDomain range.
    let range = |values: IntegerValues| match values {
        IntegerValues::Interval { minimum, maximum } => Some((minimum, maximum)),
        IntegerValues::Finite(values) if values.len() == 1 => values[0].map(|value| (value, value)),
        _ => None,
    };
    interval_arithmetic(range(left)?, range(right)?, operator, bits)
}

fn to_domain(values: IntegerValues) -> ValueDomain {
    match values {
        IntegerValues::Finite(values) => ValueDomain::set(
            SetMode::Include,
            values
                .into_iter()
                .map(|value| {
                    value.map_or_else(
                        || LiteralExpression::new(LiteralType::Null, LiteralValue::Null),
                        integer_literal,
                    )
                })
                .collect(),
        ),
        IntegerValues::Interval { minimum, maximum } => ValueDomain::ranges(vec![ValueRange::new(
            Some(Bound::new(integer_literal(minimum), true)),
            Some(Bound::new(integer_literal(maximum), true)),
        )]),
    }
}

/// Refine computed projection domains only with catalog-backed, total arithmetic.
pub(super) fn refine_catalog_arithmetic_output_domains(
    output: Output,
    domains: &[ColumnDomain],
    sources: &[SourceRelation],
    metadata: &AnalysisMetadata<'_>,
) -> Output {
    Output::new(
        output
            .columns()
            .iter()
            .cloned()
            .map(|column| {
                let Some(typed) = typed_expression(column.expression(), domains, sources, metadata)
                else {
                    return column;
                };
                let proved = to_domain(typed.values);
                let refined = if matches!(column.domain(), ValueDomain::Unknown(_)) {
                    proved
                } else {
                    intersect_domains(column.domain(), &proved)
                };
                column.with_domain(refined)
            })
            .collect(),
    )
}
