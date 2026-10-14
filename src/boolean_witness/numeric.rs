//! Width-checked affine integer predicates.
//!
//! A single physical signed integer is the only variable admitted here.
//! Each arithmetic node must be total for the *whole catalog-attested*
//! source range. A successful inversion is an exact single-column predicate;
//! otherwise the caller preserves an explicit residual instead of treating
//! finite samples as a universal absence proof.

use super::{signed_integer_literal, BooleanOperands, BooleanRowConstraint, SignedIntegerEvidence};
use crate::domain::resolve_column;
use crate::protocol::{
    BinaryOperator, ColumnExpression, ColumnRef, ComparisonOperator, Expression, SourceRelation,
    UnaryOperator,
};

#[derive(Clone, Copy)]
struct AffineInteger<'a> {
    column: &'a ColumnExpression,
    coefficient: i128,
    offset: i128,
    minimum: i128,
    maximum: i128,
    bits: u16,
}

fn signed_limits(bits: u16) -> Option<(i128, i128)> {
    if !matches!(bits, 16 | 32 | 64) {
        return None;
    }
    let magnitude = 1_i128 << (u32::from(bits) - 1);
    Some((-magnitude, magnitude - 1))
}

impl<'a> AffineInteger<'a> {
    fn transform(self, factor: i128, offset: i128) -> Option<Self> {
        let first = self.minimum.checked_mul(factor)?.checked_add(offset)?;
        let last = self.maximum.checked_mul(factor)?.checked_add(offset)?;
        let (minimum, maximum) = (first.min(last), first.max(last));
        let (lower, upper) = signed_limits(self.bits)?;
        if minimum < lower || maximum > upper {
            return None;
        }
        Some(Self {
            coefficient: self.coefficient.checked_mul(factor)?,
            offset: self.offset.checked_mul(factor)?.checked_add(offset)?,
            minimum,
            maximum,
            ..self
        })
    }

    fn with_width(self, bits: u16) -> Option<Self> {
        let (lower, upper) = signed_limits(bits)?;
        (self.minimum >= lower && self.maximum <= upper).then_some(Self { bits, ..self })
    }
}

fn bounded_affine<'a>(
    expression: &'a Expression,
    sources: &[SourceRelation],
    evidence: &impl Fn(&ColumnRef) -> Option<SignedIntegerEvidence>,
) -> Option<AffineInteger<'a>> {
    match expression {
        Expression::Column(column) => {
            let bounds = evidence(&resolve_column(column, sources))?;
            // A smaller inferred width is deliberately conservative when
            // catalog constraints narrow the source's possible values.
            let bits = [16, 32, 64].into_iter().find(|bits| {
                signed_limits(*bits)
                    .is_some_and(|(min, max)| bounds.minimum >= min && bounds.maximum <= max)
            })?;
            Some(AffineInteger {
                column,
                coefficient: 1,
                offset: 0,
                minimum: bounds.minimum,
                maximum: bounds.maximum,
                bits,
            })
        }
        Expression::SignedIntegerCast(cast) => {
            bounded_affine(cast.expression(), sources, evidence)?.with_width(cast.target_bits())
        }
        Expression::Unary(unary) => {
            let input = bounded_affine(unary.operand(), sources, evidence)?;
            match unary.operator() {
                UnaryOperator::Plus => Some(input),
                UnaryOperator::Minus => input.transform(-1, 0),
                UnaryOperator::BitwiseNot => None,
            }
        }
        Expression::Binary(binary) => {
            let left = bounded_affine(binary.left(), sources, evidence);
            let right = bounded_affine(binary.right(), sources, evidence);
            let lhs = signed_integer_literal(binary.left()).map(i128::from);
            let rhs = signed_integer_literal(binary.right()).map(i128::from);
            match binary.operator() {
                BinaryOperator::Add => {
                    if let (Some(input), Some(value)) = (left, rhs) {
                        input.transform(1, value)
                    } else if let (Some(value), Some(input)) = (lhs, right) {
                        input.transform(1, value)
                    } else {
                        None
                    }
                }
                BinaryOperator::Subtract => {
                    if let (Some(input), Some(value)) = (left, rhs) {
                        input.transform(1, -value)
                    } else if let (Some(value), Some(input)) = (lhs, right) {
                        input.transform(-1, value)
                    } else {
                        None
                    }
                }
                BinaryOperator::Multiply => {
                    if let (Some(input), Some(value)) = (left, rhs) {
                        input.transform(value, 0)
                    } else if let (Some(value), Some(input)) = (lhs, right) {
                        input.transform(value, 0)
                    } else {
                        None
                    }
                }
                // Engines such as DuckDB promote integer division to IEEE
                // double. Division by +/-1 is value-preserving only where
                // *every* attested input and result is exactly representable
                // in binary64, regardless of the engine's division type.
                BinaryOperator::Divide if rhs == Some(1) || rhs == Some(-1) => {
                    let input = left?;
                    let result = input.transform(i128::from(rhs?), 0)?;
                    let exact = 1_i128 << 53;
                    (input.minimum >= -exact
                        && input.maximum <= exact
                        && result.minimum >= -exact
                        && result.maximum <= exact)
                        .then_some(result)
                }
                // Zero divided by a catalog-certified, nonzero signed integer
                // is zero for any SQL rounding rule. NULL still propagates.
                BinaryOperator::Divide | BinaryOperator::Modulo if lhs == Some(0) => {
                    let divisor = right?;
                    (divisor.minimum > 0 || divisor.maximum < 0)
                        .then(|| divisor.transform(0, 0))
                        .flatten()
                }
                BinaryOperator::Modulo if rhs == Some(1) => left?.transform(0, 0),
                BinaryOperator::Divide
                | BinaryOperator::Modulo
                | BinaryOperator::StringConcat
                | BinaryOperator::BitwiseAnd
                | BinaryOperator::BitwiseOr
                | BinaryOperator::BitwiseXor => None,
            }
        }
        _ => None,
    }
}

/// NULL is preserved by every admitted node and no operation can overflow.
pub(super) fn total_integer_column<'a>(
    expression: &'a Expression,
    sources: &[SourceRelation],
    evidence: &impl Fn(&ColumnRef) -> Option<SignedIntegerEvidence>,
) -> Option<&'a ColumnExpression> {
    bounded_affine(expression, sources, evidence).map(|value| value.column)
}

fn constant_on_nonnull(column: ColumnRef, result: bool, null_safe: bool) -> BooleanRowConstraint {
    if null_safe {
        let children = BooleanOperands::new(vec![
            BooleanRowConstraint::NullTest {
                column: column.clone(),
                negated: false,
            },
            BooleanRowConstraint::NullTest {
                column,
                negated: true,
            },
        ])
        .expect("two complementary NULL-test operands");
        return if result {
            BooleanRowConstraint::Any(children)
        } else {
            BooleanRowConstraint::All(children)
        };
    }
    BooleanRowConstraint::IntegerColumnComparison {
        left: column.clone(),
        right: column,
        operator: if result {
            ComparisonOperator::Eq
        } else {
            ComparisonOperator::Neq
        },
    }
}

fn constant_expression_truth(
    column: ColumnRef,
    operator: ComparisonOperator,
    value: i128,
    literal: i128,
) -> BooleanRowConstraint {
    let result = compare(value, operator, literal);
    match operator {
        ComparisonOperator::IsDistinctFrom if !result => BooleanRowConstraint::NullTest {
            column,
            negated: false,
        },
        ComparisonOperator::IsNotDistinctFrom if result => BooleanRowConstraint::NullTest {
            column,
            negated: true,
        },
        ComparisonOperator::Eq
        | ComparisonOperator::Neq
        | ComparisonOperator::Lt
        | ComparisonOperator::Lte
        | ComparisonOperator::Gt
        | ComparisonOperator::Gte
        | ComparisonOperator::IsDistinctFrom
        | ComparisonOperator::IsNotDistinctFrom => constant_on_nonnull(
            column,
            result,
            matches!(
                operator,
                ComparisonOperator::IsDistinctFrom | ComparisonOperator::IsNotDistinctFrom
            ),
        ),
    }
}

fn compare(left: i128, operator: ComparisonOperator, right: i128) -> bool {
    match operator {
        ComparisonOperator::Eq | ComparisonOperator::IsNotDistinctFrom => left == right,
        ComparisonOperator::Neq | ComparisonOperator::IsDistinctFrom => left != right,
        ComparisonOperator::Lt => left < right,
        ComparisonOperator::Lte => left <= right,
        ComparisonOperator::Gt => left > right,
        ComparisonOperator::Gte => left >= right,
    }
}

fn operand_kind(expression: &Expression) -> &'static str {
    match expression {
        Expression::Column(_) => "source-column",
        Expression::SignedIntegerCast(_) => "signed-integer-cast",
        Expression::Literal(literal) => match literal.literal_type() {
            crate::protocol::LiteralType::Integer => "integer-literal",
            crate::protocol::LiteralType::Decimal => "decimal-literal",
            crate::protocol::LiteralType::String => "string-literal",
            crate::protocol::LiteralType::Boolean => "boolean-literal",
            crate::protocol::LiteralType::Null => "sql-null",
            _ => "other-literal",
        },
        Expression::Binary(_) => "computed-binary-expression",
        Expression::Unary(_) => "computed-unary-expression",
        _ => "unsupported-expression",
    }
}

/// Surface the unsupported operator/type pair in the canonical residual.
/// Do not conflate overflow, dynamic widening and dialect-sensitive casts
/// with proofs of impossible source rows.
pub(super) fn unresolved_arithmetic_reason(left: &Expression, right: &Expression) -> String {
    let computed = [left, right]
        .into_iter()
        .find(|value| matches!(value, Expression::Binary(_) | Expression::Unary(_)));
    if let Some(Expression::Binary(binary)) = computed {
        return format!(
            "arithmetic {} on {} and {} requires type/width/overflow/rounding evidence",
            binary.operator().as_str(),
            operand_kind(binary.left()),
            operand_kind(binary.right())
        );
    }
    if let Some(Expression::Unary(unary)) = computed {
        return format!(
            "arithmetic unary {} on {} requires total signed range evidence",
            unary.operator().as_str(),
            operand_kind(unary.operand())
        );
    }
    format!(
        "comparison on {} and {} has no certified typed inversion",
        operand_kind(left),
        operand_kind(right)
    )
}

/// Invert only algebraically exact, width-checked affine expressions. The
/// returned predicate retains SQL UNKNOWN when the physical column is NULL.
pub(super) fn normalize_affine_comparison(
    left: &Expression,
    operator: ComparisonOperator,
    right: &Expression,
    sources: &[SourceRelation],
    evidence: &impl Fn(&ColumnRef) -> Option<SignedIntegerEvidence>,
) -> Option<BooleanRowConstraint> {
    let (affine, operator, literal) = if let (Some(affine), Some(literal)) = (
        bounded_affine(left, sources, evidence),
        signed_integer_literal(right),
    ) {
        (affine, operator, literal)
    } else if let (Some(literal), Some(affine)) = (
        signed_integer_literal(left),
        bounded_affine(right, sources, evidence),
    ) {
        (affine, operator.reversed(), literal)
    } else {
        return None;
    };

    let column = resolve_column(affine.column, sources);
    if affine.coefficient == 0 {
        return Some(constant_expression_truth(
            column,
            operator,
            affine.offset,
            i128::from(literal),
        ));
    }
    let bounds = evidence(&column)?;
    let nullable_comparison = matches!(
        operator,
        ComparisonOperator::IsDistinctFrom | ComparisonOperator::IsNotDistinctFrom
    );
    let mut numerator = i128::from(literal).checked_sub(affine.offset)?;
    let mut coefficient = affine.coefficient;
    let mut operator = operator;
    if coefficient < 0 {
        numerator = numerator.checked_neg()?;
        coefficient = coefficient.checked_neg()?;
        operator = operator.reversed();
    }
    let floor = numerator.div_euclid(coefficient);
    let rem = numerator.rem_euclid(coefficient);
    let ceil = floor.checked_add(i128::from(rem != 0))?;
    let (operator, threshold) = match operator {
        ComparisonOperator::Eq | ComparisonOperator::IsNotDistinctFrom if rem == 0 => {
            (operator, floor)
        }
        ComparisonOperator::Neq | ComparisonOperator::IsDistinctFrom if rem == 0 => {
            (operator, floor)
        }
        ComparisonOperator::Eq | ComparisonOperator::IsNotDistinctFrom => {
            return Some(constant_on_nonnull(column, false, nullable_comparison));
        }
        ComparisonOperator::Neq | ComparisonOperator::IsDistinctFrom => {
            return Some(constant_on_nonnull(column, true, nullable_comparison));
        }
        ComparisonOperator::Lt => (operator, ceil),
        ComparisonOperator::Lte => (operator, floor),
        ComparisonOperator::Gt => (operator, floor),
        ComparisonOperator::Gte => (operator, ceil),
    };

    // Full catalog-attested source bounds certify tautologies and
    // contradictions even when the inverted threshold exceeds i64.
    let minimum_true = compare(bounds.minimum, operator, threshold);
    let maximum_true = compare(bounds.maximum, operator, threshold);
    if minimum_true == maximum_true {
        let monotone = !matches!(
            operator,
            ComparisonOperator::Eq
                | ComparisonOperator::Neq
                | ComparisonOperator::IsDistinctFrom
                | ComparisonOperator::IsNotDistinctFrom
        );
        let outside = threshold < bounds.minimum || threshold > bounds.maximum;
        if monotone || outside {
            return Some(constant_on_nonnull(
                column,
                minimum_true,
                nullable_comparison,
            ));
        }
    }

    let threshold = i64::try_from(threshold).ok()?;
    Some(BooleanRowConstraint::IntegerComparison {
        column,
        operator,
        literal: threshold,
    })
}
