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
                // A throwing CAST is not a total expression over this source.
                // The NULL-on-failure variants are total for a finite,
                // catalog-typed integer source and preserve source NULLs.
                input.values = match (cast.cast_kind(), input.values) {
                    (
                        SignedIntegerCastKind::TryCast | SignedIntegerCastKind::SafeCast,
                        IntegerValues::Finite(values),
                    ) => IntegerValues::Finite(
                        values
                            .into_iter()
                            .map(|value| value.filter(|number| fits(*number, cast.target_bits())))
                            .collect(),
                    ),
                    _ => return None,
                };
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
            // SQL engines differ on fractional integer division and some
            // promote to binary64. An integral quotient is portable only
            // when both operands and the result are binary64-exact.
            let exact = 1_u128 << 53;
            let result = left.checked_div(right)?;
            if left.unsigned_abs() > exact
                || right.unsigned_abs() > exact
                || result.unsigned_abs() > exact
                || left.checked_rem(right)? != 0
            {
                return None;
            }
            result
        }
        BinaryOperator::Modulo => {
            if right == 0 {
                return None;
            }
            // Nonnegative integer remainders with positive divisors have
            // the same value independently of signed-remainder conventions.
            if left >= 0 && right > 0 {
                left % right
            } else if left == 0 || right == 1 {
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
        BinaryOperator::Divide
            if left_min == left_max && right_min == right_max =>
        {
            let value = scalar_arithmetic(left_min, right_min, operator, bits)?;
            (value, value)
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
        BinaryOperator::Modulo
            if right_min == right_max && right_min > 0 && left_min >= 0 =>
        {
            let modulus = right_min;
            // A contiguous interval crossing a multiple of the divisor
            // contains the wraparound. Otherwise modulo is monotone.
            if left_min / modulus == left_max / modulus {
                (left_min % modulus, left_max % modulus)
            } else {
                (0, modulus - 1)
            }
        },
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
    if matches!(&left, IntegerValues::Finite(values) if values.is_empty())
        || matches!(&right, IntegerValues::Finite(values) if values.is_empty())
    {
        return Some(IntegerValues::Finite(Vec::new()));
    }
    if matches!(&left, IntegerValues::Finite(values) if values.len() == 1 && values[0].is_none())
        || matches!(&right, IntegerValues::Finite(values) if values.len() == 1 && values[0].is_none())
    {
        return Some(IntegerValues::Finite(vec![None]));
    }
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

#[derive(Clone)]
struct InjectiveAffine {
    column: ColumnRef,
    factor: i128,
    offset: i128,
}

fn injective_affine(
    expression: &Expression,
    sources: &[SourceRelation],
) -> Option<InjectiveAffine> {
    match expression {
        Expression::Column(column) => Some(InjectiveAffine {
            column: resolve_column(column, sources),
            factor: 1,
            offset: 0,
        }),
        Expression::SignedIntegerCast(cast) => injective_affine(cast.expression(), sources),
        Expression::Unary(unary) => {
            let mut result = injective_affine(unary.operand(), sources)?;
            match unary.operator() {
                UnaryOperator::Plus => {}
                UnaryOperator::Minus => {
                    result.factor = result.factor.checked_neg()?;
                    result.offset = result.offset.checked_neg()?;
                }
                UnaryOperator::BitwiseNot => return None,
            }
            Some(result)
        }
        Expression::Binary(binary) => {
            let left = injective_affine(binary.left(), sources);
            let right = injective_affine(binary.right(), sources);
            let left_literal = singleton_integer(&derive_output_domain(binary.left()));
            let right_literal = singleton_integer(&derive_output_domain(binary.right()));
            let (mut result, factor, offset) = match binary.operator() {
                BinaryOperator::Add => match (left, right, left_literal, right_literal) {
                    (Some(source), _, _, Some(constant)) => (source, 1, constant),
                    (_, Some(source), Some(constant), _) => (source, 1, constant),
                    _ => return None,
                },
                BinaryOperator::Subtract => match (left, right, left_literal, right_literal) {
                    (Some(source), _, _, Some(constant)) => (source, 1, constant.checked_neg()?),
                    (_, Some(source), Some(constant), _) => (source, -1, constant),
                    _ => return None,
                },
                BinaryOperator::Multiply => match (left, right, left_literal, right_literal) {
                    (Some(source), _, _, Some(constant)) => (source, constant, 0),
                    (_, Some(source), Some(constant), _) => (source, constant, 0),
                    _ => return None,
                },
                // Identity and sign inversion are the only portable
                // injective division families. A full-source totality
                // check below still guards binary64 and signed overflow.
                BinaryOperator::Divide => match (left, right_literal) {
                    (Some(source), Some(1)) => (source, 1, 0),
                    (Some(source), Some(-1)) => (source, -1, 0),
                    _ => return None,
                },
                BinaryOperator::Modulo
                | BinaryOperator::StringConcat
                | BinaryOperator::BitwiseAnd
                | BinaryOperator::BitwiseOr
                | BinaryOperator::BitwiseXor => return None,
            };
            if factor == 0 {
                return None;
            }
            result.factor = result.factor.checked_mul(factor)?;
            result.offset = result.offset.checked_mul(factor)?.checked_add(offset)?;
            Some(result)
        }
        _ => None,
    }
}

fn affine_excluded_domain(
    expression: &Expression,
    domains: &[ColumnDomain],
    sources: &[SourceRelation],
    metadata: &AnalysisMetadata<'_>,
) -> Option<ValueDomain> {
    let affine = injective_affine(expression, sources)?;
    if affine.factor == 0 {
        return None;
    }
    let column_domain = domains
        .iter()
        .find(|item| item.column() == &affine.column)?;
    let ValueDomain::Set(excluded) = column_domain.domain() else {
        return None;
    };
    if excluded.mode() != SetMode::Exclude {
        return None;
    }
    let declared = metadata.column_data_type(&affine.column)?;
    let declared = match declared {
        DataType::Nullable(inner) => inner.as_ref(),
        other => other,
    };
    let DataType::SignedInteger { bits: Some(bits) } = declared else {
        return None;
    };
    let (min, max) = signed_limits(*bits)?;
    // Check *all* admissible values across all intermediate operations.
    // A finite sample cannot prove that inversion avoids overflow.
    let full_domain = [ColumnDomain::new(
        affine.column.clone(),
        ValueDomain::ranges(vec![ValueRange::new(
            Some(Bound::new(integer_literal(min), true)),
            Some(Bound::new(integer_literal(max), true)),
        )]),
    )];
    typed_expression(expression, &full_domain, sources, metadata)?;
    let mut values = Vec::new();
    for literal in excluded.values() {
        if literal.literal_type() == LiteralType::Null {
            values.push(literal.clone());
        } else {
            let scalar = integer_value(literal)?;
            let value = affine
                .factor
                .checked_mul(scalar)?
                .checked_add(affine.offset)?;
            values.push(integer_literal(value));
        }
    }
    Some(ValueDomain::set(SetMode::Exclude, values))
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
                // Preserve exact open and closed bounds for plain projections.
                if matches!(column.expression(), Expression::Column(_)) {
                    return column;
                }
                if let Some(excluded) =
                    affine_excluded_domain(column.expression(), domains, sources, metadata)
                {
                    return column.with_domain(excluded);
                }
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
