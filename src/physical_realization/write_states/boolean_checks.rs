//! Exact SQL three-valued evaluation of canonical row-local Boolean CHECKs.

use super::*;
use crate::computed_boolean_checks::ComputedBooleanCheckPredicate as P;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Truth {
    True,
    False,
    Unknown,
}

pub(super) fn check(
    predicate: &P,
    values: &BTreeMap<String, LiteralValue>,
    schema: &RelationSchema,
) -> StateResult<()> {
    match evaluate(predicate, values, schema)? {
        Truth::True | Truth::Unknown => Ok(()), // SQL CHECK admits UNKNOWN.
        Truth::False => Err(impossible("enforced Boolean CHECK evaluated to FALSE")),
    }
}

fn evaluate(
    predicate: &P,
    values: &BTreeMap<String, LiteralValue>,
    schema: &RelationSchema,
) -> StateResult<Truth> {
    match predicate {
        P::Comparison {
            left,
            operator,
            right,
        } => {
            let lhs = super::generated::evaluate(left, values, schema)?;
            let rhs = super::generated::evaluate(right, values, schema)?;
            if lhs == LiteralValue::Null || rhs == LiteralValue::Null {
                return Ok(Truth::Unknown);
            }
            let scalar = match rhs {
                LiteralValue::Null => return Ok(Truth::Unknown),
                LiteralValue::Number(number) => ConstraintValue::Number(number),
                LiteralValue::Text(text) => ConstraintValue::String(text),
                LiteralValue::Boolean(boolean) => ConstraintValue::Boolean(boolean),
            };
            match super::check_comparisons::validate_scalar_check(&lhs, *operator, &scalar) {
                Ok(()) => Ok(Truth::True),
                Err(StateFailure::Impossible(_)) => Ok(Truth::False),
                Err(error) => Err(error),
            }
        }
        P::IsNull {
            expression,
            negated,
        } => {
            let value = super::generated::evaluate(expression, values, schema)?;
            if (value == LiteralValue::Null) != *negated {
                Ok(Truth::True)
            } else {
                Ok(Truth::False)
            }
        }
        P::And(children) => {
            let mut unknown = false;
            let mut residual = None;
            for child in children {
                match evaluate(child, values, schema) {
                    Ok(Truth::False) => return Ok(Truth::False),
                    Ok(Truth::Unknown) => unknown = true,
                    Ok(Truth::True) => {}
                    Err(StateFailure::Residual(gap, reason)) => {
                        residual = Some(StateFailure::Residual(gap, reason));
                    }
                    Err(error) => return Err(error),
                }
            }
            if let Some(error) = residual {
                Err(error)
            } else if unknown {
                Ok(Truth::Unknown)
            } else {
                Ok(Truth::True)
            }
        }
        P::Or(children) => {
            let mut unknown = false;
            let mut residual = None;
            for child in children {
                match evaluate(child, values, schema) {
                    Ok(Truth::True) => return Ok(Truth::True),
                    Ok(Truth::Unknown) => unknown = true,
                    Ok(Truth::False) => {}
                    Err(StateFailure::Residual(gap, reason)) => {
                        residual = Some(StateFailure::Residual(gap, reason));
                    }
                    Err(error) => return Err(error),
                }
            }
            if let Some(error) = residual {
                Err(error)
            } else if unknown {
                Ok(Truth::Unknown)
            } else {
                Ok(Truth::False)
            }
        }
        P::Not(child) => match evaluate(child, values, schema)? {
            Truth::True => Ok(Truth::False),
            Truth::False => Ok(Truth::True),
            Truth::Unknown => Ok(Truth::Unknown),
        },
    }
}
