//! Conservative execution of attested row-local generated column arithmetic.
//!
//! This exact integer-only subset cannot represent sequence state, arbitrary
//! casts or expressions whose operands depend on other generated columns.

use super::*;
use crate::column_sources::{ColumnValueKind, ColumnValueSource, GeneratedValueExpression};

pub(super) fn scalar_literal(value: &ConstraintValue) -> LiteralValue {
    match value {
        ConstraintValue::Null => LiteralValue::Null,
        ConstraintValue::Boolean(value) => LiteralValue::Boolean(*value),
        ConstraintValue::Integer(value) => LiteralValue::Number(value.to_string()),
        ConstraintValue::UnsignedInteger(value) => LiteralValue::Number(value.to_string()),
        ConstraintValue::Number(value) => LiteralValue::Number(value.clone()),
        ConstraintValue::String(value) => LiteralValue::Text(value.clone()),
    }
}

pub(super) fn authoritative(source: &ColumnValueSource) -> bool {
    source.evidence().iter().any(|evidence| {
        evidence.enforcement() == ConstraintEnforcement::Enforced
            || evidence.provenance().source_kind()
                == crate::constraints::ConstraintSourceKind::SqlDdl
    })
}

pub(super) fn evaluate(
    expression: &GeneratedValueExpression,
    values: &BTreeMap<String, LiteralValue>,
) -> StateResult<LiteralValue> {
    use GeneratedValueExpression as G;
    match expression {
        G::Column(name) => values.get(name).cloned().ok_or_else(|| {
            residual_gap(
                PhysicalWriteStateGap::UnprovedConstraint,
                format!("generated expression depends on an unmaterialized column {name}"),
            )
        }),
        G::Literal(value) => Ok(scalar_literal(value)),
        G::Add(left, right) | G::Subtract(left, right) | G::Multiply(left, right) => {
            let left = evaluate(left, values)?;
            let right = evaluate(right, values)?;
            if left == LiteralValue::Null || right == LiteralValue::Null {
                return Ok(LiteralValue::Null);
            }
            let (LiteralValue::Number(left), LiteralValue::Number(right)) = (left, right) else {
                return Err(residual(
                    "generated arithmetic requires exact integer storage",
                ));
            };
            let left = left
                .parse::<i128>()
                .map_err(|_| residual("unproved generated numeric coercion"))?;
            let right = right
                .parse::<i128>()
                .map_err(|_| residual("unproved generated numeric coercion"))?;
            let result = if matches!(expression, G::Add(_, _)) {
                left.checked_add(right)
            } else if matches!(expression, G::Subtract(_, _)) {
                left.checked_sub(right)
            } else {
                left.checked_mul(right)
            }
            .ok_or_else(|| {
                residual("generated integer arithmetic exceeds exact representable range")
            })?;
            Ok(LiteralValue::Number(result.to_string()))
        }
    }
}

pub(super) fn check_declared_sources(
    sources: &[ColumnValueSource],
    row: &PhysicalWriteRow,
    controlled_identity_columns: &BTreeSet<String>,
) -> StateResult<()> {
    for source in sources {
        match source.kind() {
            ColumnValueKind::Default => {}
            ColumnValueKind::Identity => {
                if !controlled_identity_columns.contains(source.column()) {
                    return Err(residual_gap(
                        PhysicalWriteStateGap::UnprovedConstraint,
                        "identity value needs an attested sequence and allocation-state law",
                    ));
                }
            }
            ColumnValueKind::Generated => {
                if !authoritative(source) {
                    return Err(residual_gap(
                        PhysicalWriteStateGap::UnprovedConstraint,
                        "generated expression enforcement is not attested",
                    ));
                }
                let expression = source.expression().ok_or_else(|| {
                    residual("generated expression has no canonical executable semantics")
                })?;
                // SQL generated columns that depend on other generated columns,
                // or on themselves, need a dependency/evaluation-order proof.
                if expression.referenced_columns().iter().any(|name| {
                    sources.iter().any(|other| {
                        other.kind() != ColumnValueKind::Default && other.column() == *name
                    })
                }) {
                    return Err(residual(
                        "generated column depends on another generated or identity value",
                    ));
                }
                let actual = row.values.get(source.column()).ok_or_else(|| {
                    impossible("physical row is missing its declared generated column")
                })?;
                let expected = evaluate(expression, &row.values)?;
                if !matches!(
                    (&expected, actual),
                    (LiteralValue::Number(_), LiteralValue::Number(_))
                        | (LiteralValue::Null, LiteralValue::Null)
                        | (LiteralValue::Boolean(_), LiteralValue::Boolean(_))
                ) {
                    return Err(residual(
                        "generated value comparison lacks exact numeric or boolean law",
                    ));
                }
                if !super::evaluator::scalar_equality(&expected, actual)? {
                    return Err(impossible(
                        "generated column contradicts its attested expression",
                    ));
                }
            }
        }
    }
    Ok(())
}

pub(super) fn complete_generated(
    sources: &[ColumnValueSource],
    values: &mut BTreeMap<String, LiteralValue>,
) -> StateResult<()> {
    for source in sources {
        if source.kind() != ColumnValueKind::Generated {
            continue;
        }
        if values.contains_key(source.column()) {
            return Err(residual(
                "direct INSERT assignment to generated column is unproved",
            ));
        }
        if !authoritative(source) {
            return Err(residual(
                "generated expression lacks authoritative execution evidence",
            ));
        }
        let expression = source
            .expression()
            .ok_or_else(|| residual("generated column has no executable expression law"))?;
        if expression.referenced_columns().iter().any(|name| {
            sources
                .iter()
                .any(|other| other.kind() != ColumnValueKind::Default && other.column() == *name)
        }) {
            return Err(residual(
                "generated expression references a generated or identity column",
            ));
        }
        let value = evaluate(expression, values)?;
        values.insert(source.column().to_string(), value);
    }
    Ok(())
}
