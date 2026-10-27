//! Typed SQL three-valued WHERE evaluation for complete physical window populations.
//!
//! Compile every predicate branch before evaluating any source row. This prevents
//! an unsupported branch from becoming an unsound Exact claim when short-circuited.

use super::*;
use crate::protocol::{ComparisonOperator, LiteralType, LiteralValue};
use crate::relation::RelationSchema;

enum Scalar {
    Column(String),
    Integer(Option<i128>),
}

impl Scalar {
    fn from_expression(
        expression: &Expression,
        source: &crate::protocol::SourceRelation,
        schema: &RelationSchema,
    ) -> Result<Self, &'static str> {
        match expression {
            Expression::Column(column) => {
                if column
                    .relation()
                    .is_some_and(|name| name != source.name() && Some(name) != source.alias())
                {
                    return Err("WHERE column has an unresolved source qualifier");
                }
                let declared = schema
                    .columns()
                    .iter()
                    .find(|item| item.name() == column.name())
                    .ok_or("WHERE column lacks authoritative schema evidence")?;
                if !super::aggregate_bags::signed_integer(declared.data_type()) {
                    return Err("WHERE comparison lacks portable signed integer laws");
                }
                Ok(Self::Column(column.name().to_string()))
            }
            Expression::Literal(value) if value.literal_type() == LiteralType::Null => {
                Ok(Self::Integer(None))
            }
            Expression::Literal(value) if value.literal_type() == LiteralType::Integer => {
                let LiteralValue::Number(value) = value.value() else {
                    return Err("WHERE integer literal has unexpected value type");
                };
                let number = value
                    .parse::<i128>()
                    .map_err(|_| "WHERE numeric literal exceeds certified integer width")?;
                Ok(Self::Integer(Some(number)))
            }
            _ => Err("WHERE scalar expression has no certified integer law"),
        }
    }

    fn evaluate(&self, row: &PhysicalWriteRow) -> Result<Option<i128>, &'static str> {
        match self {
            Self::Integer(value) => Ok(*value),
            Self::Column(name) => super::rank_bags::key_value(row, name),
        }
    }
}

/// A fully type-checked predicate. None represents SQL UNKNOWN, never FALSE.
pub(super) enum Filter {
    Comparison {
        left: Scalar,
        operator: ComparisonOperator,
        right: Scalar,
    },
    And(Vec<Self>),
    Or(Vec<Self>),
    Not(Box<Self>),
    IsNull {
        operand: Scalar,
        negated: bool,
    },
}

impl Filter {
    /// Compile the entire canonical WHERE tree against one authoritative source schema.
    pub(super) fn compile(
        predicate: &Predicate,
        source: &crate::protocol::SourceRelation,
        schema: &RelationSchema,
    ) -> Result<Self, &'static str> {
        match predicate {
            Predicate::Comparison(comparison) => {
                if matches!(
                    comparison.operator(),
                    ComparisonOperator::IsDistinctFrom | ComparisonOperator::IsNotDistinctFrom
                ) {
                    return Err("WHERE NULL-safe comparison lacks a portable execution law");
                }
                Ok(Self::Comparison {
                    left: Scalar::from_expression(comparison.left(), source, schema)?,
                    operator: comparison.operator(),
                    right: Scalar::from_expression(comparison.right(), source, schema)?,
                })
            }
            Predicate::And(and) => Ok(Self::And(
                and.operands()
                    .iter()
                    .map(|item| Self::compile(item, source, schema))
                    .collect::<Result<Vec<_>, _>>()?,
            )),
            Predicate::Or(or) => Ok(Self::Or(
                or.operands()
                    .iter()
                    .map(|item| Self::compile(item, source, schema))
                    .collect::<Result<Vec<_>, _>>()?,
            )),
            Predicate::Not(not) => Ok(Self::Not(Box::new(Self::compile(
                not.operand(),
                source,
                schema,
            )?))),
            Predicate::IsNull(is_null) => Ok(Self::IsNull {
                operand: Scalar::from_expression(is_null.expression(), source, schema)?,
                negated: is_null.negated(),
            }),
            _ => Err("WHERE predicate has no certified physical filter law"),
        }
    }

    /// Return SQL TRUE/FALSE/UNKNOWN for one independently attested physical row.
    pub(super) fn evaluate(&self, row: &PhysicalWriteRow) -> Result<Option<bool>, &'static str> {
        match self {
            Self::Comparison {
                left,
                operator,
                right,
            } => {
                let (Some(left), Some(right)) = (left.evaluate(row)?, right.evaluate(row)?) else {
                    return Ok(None);
                };
                Ok(Some(match operator {
                    ComparisonOperator::Eq => left == right,
                    ComparisonOperator::Neq => left != right,
                    ComparisonOperator::Lt => left < right,
                    ComparisonOperator::Lte => left <= right,
                    ComparisonOperator::Gt => left > right,
                    ComparisonOperator::Gte => left >= right,
                    ComparisonOperator::IsDistinctFrom | ComparisonOperator::IsNotDistinctFrom => {
                        return Err("unattested NULL-safe comparison")
                    }
                }))
            }
            Self::IsNull { operand, negated } => {
                Ok(Some(operand.evaluate(row)?.is_none() != *negated))
            }
            Self::Not(predicate) => Ok(predicate.evaluate(row)?.map(|value| !value)),
            Self::And(operands) => {
                let mut unknown = false;
                for operand in operands {
                    match operand.evaluate(row)? {
                        Some(false) => return Ok(Some(false)),
                        Some(true) => {}
                        None => unknown = true,
                    }
                }
                Ok(if unknown { None } else { Some(true) })
            }
            Self::Or(operands) => {
                let mut unknown = false;
                for operand in operands {
                    match operand.evaluate(row)? {
                        Some(true) => return Ok(Some(true)),
                        Some(false) => {}
                        None => unknown = true,
                    }
                }
                Ok(if unknown { None } else { Some(false) })
            }
        }
    }
}
