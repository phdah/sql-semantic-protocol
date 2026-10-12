//! Canonical scalar and SQL three-valued predicate interpreter for physical rows.

use super::*;

pub(super) struct Context<'a> {
    pub(super) target: Option<(&'a str, &'a PhysicalWriteRow)>,
    pub(super) target_alias: Option<&'a str>,
    pub(super) source: Option<(&'a str, &'a PhysicalWriteRow)>,
}

fn column<'a>(
    context: &'a Context<'_>,
    aliases: &[&str],
    relation: Option<&str>,
    name: &str,
) -> StateResult<&'a LiteralValue> {
    match relation {
        Some(qualifier) => {
            if let Some((source, row)) = context.source {
                if qualifier == source || aliases.contains(&qualifier) {
                    return row
                        .values
                        .get(name)
                        .ok_or_else(|| residual("source column missing"));
                }
            }
            if let Some((target, row)) = context.target {
                if qualifier == target || context.target_alias == Some(qualifier) {
                    return row
                        .values
                        .get(name)
                        .ok_or_else(|| residual("target column missing"));
                }
            }
            Err(residual(
                "column qualifier/target alias not canonically resolved",
            ))
        }
        None => {
            let target = context.target.and_then(|(_, row)| row.values.get(name));
            let source = context.source.and_then(|(_, row)| row.values.get(name));
            match (target, source) {
                (Some(_), Some(_)) => Err(residual("ambiguous unqualified source/target column")),
                (Some(value), None) | (None, Some(value)) => Ok(value),
                (None, None) => Err(residual("column missing from complete source snapshots")),
            }
        }
    }
}

pub(super) fn eval(
    expression: &Expression,
    context: &Context<'_>,
    aliases: &[&str],
) -> StateResult<LiteralValue> {
    match expression {
        Expression::Literal(literal) => Ok(literal.value().clone()),
        Expression::Column(col) => {
            Ok(column(context, aliases, col.relation(), col.name())?.clone())
        }
        Expression::Unary(unary) => {
            let value = eval(unary.operand(), context, aliases)?;
            match (unary.operator(), value) {
                (_, LiteralValue::Null) => Ok(LiteralValue::Null),
                (UnaryOperator::Plus, LiteralValue::Number(value)) => {
                    Ok(LiteralValue::Number(value))
                }
                (UnaryOperator::Minus, LiteralValue::Number(value)) => {
                    let number = value
                        .parse::<i128>()
                        .map_err(|_| residual("noninteger unary negation"))?;
                    Ok(LiteralValue::Number(
                        number
                            .checked_neg()
                            .ok_or_else(|| residual("numeric overflow"))?
                            .to_string(),
                    ))
                }
                _ => Err(residual("unproved unary scalar semantics")),
            }
        }
        Expression::Binary(binary) => {
            let left = eval(binary.left(), context, aliases)?;
            let right = eval(binary.right(), context, aliases)?;
            if left == LiteralValue::Null || right == LiteralValue::Null {
                return Ok(LiteralValue::Null);
            }
            let (LiteralValue::Number(left), LiteralValue::Number(right)) = (left, right) else {
                return Err(residual("binary expression requires proved numeric types"));
            };
            let a = left
                .parse::<i128>()
                .map_err(|_| residual("noninteger arithmetic"))?;
            let b = right
                .parse::<i128>()
                .map_err(|_| residual("noninteger arithmetic"))?;
            let result = match binary.operator() {
                BinaryOperator::Add => a.checked_add(b),
                BinaryOperator::Subtract => a.checked_sub(b),
                BinaryOperator::Multiply => a.checked_mul(b),
                BinaryOperator::Divide | BinaryOperator::Modulo => {
                    return Err(residual(
                        "division and modulo need dialect-specific exact scalar laws",
                    ))
                }
                _ => return Err(residual("unproved binary expression law")),
            }
            .ok_or_else(|| residual("numeric overflow"))?;
            Ok(LiteralValue::Number(result.to_string()))
        }
        Expression::BooleanPredicate(predicate) => {
            match predicate_truth(predicate, context, aliases)? {
                Truth::True => Ok(LiteralValue::Boolean(true)),
                Truth::False => Ok(LiteralValue::Boolean(false)),
                Truth::Unknown => Ok(LiteralValue::Null),
            }
        }
        _ => Err(residual(
            "unsupported scalar expression cannot be physically evaluated",
        )),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Truth {
    True,
    False,
    Unknown,
}

pub(super) fn selected(
    predicate: Option<&Predicate>,
    context: &Context<'_>,
    aliases: &[&str],
) -> StateResult<bool> {
    predicate.map_or(Ok(true), |predicate| {
        Ok(predicate_truth(predicate, context, aliases)? == Truth::True)
    })
}

fn predicate_truth(
    predicate: &Predicate,
    context: &Context<'_>,
    aliases: &[&str],
) -> StateResult<Truth> {
    match predicate {
        Predicate::Comparison(comparison) => {
            let a = eval(comparison.left(), context, aliases)?;
            let b = eval(comparison.right(), context, aliases)?;
            compare(&a, comparison.operator(), &b)
        }
        Predicate::IsNull(test) => {
            let value = eval(test.expression(), context, aliases)?;
            Ok(if (value == LiteralValue::Null) != test.negated() {
                Truth::True
            } else {
                Truth::False
            })
        }
        Predicate::And(logical) => {
            let mut truth = Truth::True;
            for predicate in logical.operands() {
                match predicate_truth(predicate, context, aliases)? {
                    Truth::False => truth = Truth::False,
                    Truth::Unknown if truth != Truth::False => truth = Truth::Unknown,
                    Truth::True | Truth::Unknown => {}
                }
            }
            Ok(truth)
        }
        Predicate::Or(logical) => {
            let mut truth = Truth::False;
            for predicate in logical.operands() {
                match predicate_truth(predicate, context, aliases)? {
                    Truth::True => truth = Truth::True,
                    Truth::Unknown if truth != Truth::True => truth = Truth::Unknown,
                    Truth::False | Truth::Unknown => {}
                }
            }
            Ok(truth)
        }
        Predicate::Not(not) => Ok(match predicate_truth(not.operand(), context, aliases)? {
            Truth::True => Truth::False,
            Truth::False => Truth::True,
            Truth::Unknown => Truth::Unknown,
        }),
        Predicate::Between(between) => {
            let value = eval(between.expression(), context, aliases)?;
            let lo = eval(between.lower(), context, aliases)?;
            let hi = eval(between.upper(), context, aliases)?;
            let lower = compare(&value, ComparisonOperator::Gte, &lo)?;
            let upper = compare(&value, ComparisonOperator::Lte, &hi)?;
            let truth = and_truth(lower, upper);
            Ok(if between.negated() {
                negate_truth(truth)
            } else {
                truth
            })
        }
        Predicate::In(test) => {
            let value = eval(test.expression(), context, aliases)?;
            let mut truth = Truth::False;
            for expression in test.values() {
                let candidate = eval(expression, context, aliases)?;
                truth = or_truth(truth, compare(&value, ComparisonOperator::Eq, &candidate)?);
            }
            Ok(if test.negated() {
                negate_truth(truth)
            } else {
                truth
            })
        }
        Predicate::BooleanExpression(expression) => match eval(expression, context, aliases)? {
            LiteralValue::Boolean(true) => Ok(Truth::True),
            LiteralValue::Boolean(false) => Ok(Truth::False),
            LiteralValue::Null => Ok(Truth::Unknown),
            _ => Err(residual("non-boolean predicate expression")),
        },
        _ => Err(residual(
            "unsupported predicate requires operator-local proof",
        )),
    }
}

fn negate_truth(truth: Truth) -> Truth {
    match truth {
        Truth::True => Truth::False,
        Truth::False => Truth::True,
        Truth::Unknown => Truth::Unknown,
    }
}
fn and_truth(a: Truth, b: Truth) -> Truth {
    if a == Truth::False || b == Truth::False {
        Truth::False
    } else if a == Truth::Unknown || b == Truth::Unknown {
        Truth::Unknown
    } else {
        Truth::True
    }
}
fn or_truth(a: Truth, b: Truth) -> Truth {
    if a == Truth::True || b == Truth::True {
        Truth::True
    } else if a == Truth::Unknown || b == Truth::Unknown {
        Truth::Unknown
    } else {
        Truth::False
    }
}

fn compare(a: &LiteralValue, operator: ComparisonOperator, b: &LiteralValue) -> StateResult<Truth> {
    if matches!(
        operator,
        ComparisonOperator::IsDistinctFrom | ComparisonOperator::IsNotDistinctFrom
    ) {
        // IS DISTINCT FROM treats NULL = NULL as not distinct.
        let equal = scalar_equality(a, b)?;
        let distinct = !equal;
        let truth = if operator == ComparisonOperator::IsDistinctFrom {
            distinct
        } else {
            !distinct
        };
        return Ok(if truth { Truth::True } else { Truth::False });
    }
    if *a == LiteralValue::Null || *b == LiteralValue::Null {
        return Ok(Truth::Unknown);
    }
    let order = match (a, b) {
        (LiteralValue::Number(left), LiteralValue::Number(right)) => {
            let left = left
                .parse::<i128>()
                .map_err(|_| residual("unproved decimal comparison"))?;
            let right = right
                .parse::<i128>()
                .map_err(|_| residual("unproved decimal comparison"))?;
            left.cmp(&right)
        }
        (LiteralValue::Boolean(left), LiteralValue::Boolean(right))
            if matches!(operator, ComparisonOperator::Eq | ComparisonOperator::Neq) =>
        {
            left.cmp(right)
        }
        (LiteralValue::Text(_), LiteralValue::Text(_)) => {
            return Err(residual(
                "string comparison requires binary collation and padding proof",
            ));
        }
        _ => return Err(residual("implicit comparison coercion is not proved")),
    };
    let ok = match operator {
        ComparisonOperator::Eq => order.is_eq(),
        ComparisonOperator::Neq => order.is_ne(),
        ComparisonOperator::Lt => order.is_lt(),
        ComparisonOperator::Lte => order.is_le(),
        ComparisonOperator::Gt => order.is_gt(),
        ComparisonOperator::Gte => order.is_ge(),
        ComparisonOperator::IsDistinctFrom | ComparisonOperator::IsNotDistinctFrom => {
            return Err(residual("unexpected distinct comparison boundary"))
        }
    };
    Ok(if ok { Truth::True } else { Truth::False })
}

pub(super) fn scalar_equality(a: &LiteralValue, b: &LiteralValue) -> StateResult<bool> {
    match (a, b) {
        (LiteralValue::Null, LiteralValue::Null) => Ok(true),
        (LiteralValue::Null, _) | (_, LiteralValue::Null) => Ok(false),
        (LiteralValue::Text(_), LiteralValue::Text(_)) => {
            Err(residual("unverified text collation"))
        }
        (LiteralValue::Number(_), LiteralValue::Number(_))
        | (LiteralValue::Boolean(_), LiteralValue::Boolean(_)) => {
            Ok(compare(a, ComparisonOperator::Eq, b)? == Truth::True)
        }
        _ => Err(residual("unproved coercion in distinct comparison")),
    }
}
