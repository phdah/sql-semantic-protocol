//! Enforced key, nullability and referential physical snapshot validation.

use super::*;
use super::evaluator::scalar_equality;

pub(super) fn validate_state(bundle: &AnalysisBundle, state: &PhysicalWriteState) -> StateResult<()> {
    let schema = bundle
        .source_schemas()
        .iter()
        .find(|schema| schema.relation() == state.relation)
        .ok_or_else(|| residual_gap(PhysicalWriteStateGap::MissingSchema, format!("missing complete schema for {}", state.relation)))?;
    for row in &state.rows {
        if row.values.len() != schema.columns().len() {
            return Err(impossible(
                "physical row column count contradicts declared schema",
            ));
        }
        for column in schema.columns() {
            let value = row
                .values
                .get(column.name())
                .ok_or_else(|| impossible("physical row omits a declared column"))?;
            if !value_conforms(value, column.data_type()) {
                return Err(residual(format!(
                    "unproved physical value/type compatibility: {}.{}",
                    state.relation,
                    column.name()
                )));
            }
        }
    }
    Ok(())
}

pub(super) fn validate_all_constraints(
    bundle: &AnalysisBundle,
    states: &BTreeMap<String, PhysicalWriteState>,
) -> StateResult<()> {
    for set in bundle.relation_constraints() {
        let Some(state) = states.get(set.relation()) else {
            continue;
        };
        if !set.diagnostics().is_empty() {
            return Err(residual_gap(PhysicalWriteStateGap::UnprovedConstraint, "unresolved canonical constraint diagnostics"));
        }
        for constraint in set.constraints() {
            if !constraint
                .evidence()
                .iter()
                .any(|e| e.enforcement() == ConstraintEnforcement::Enforced)
            {
                return Err(residual_gap(PhysicalWriteStateGap::UnprovedConstraint, "constraint enforcement not attested"));
            }
            match constraint {
                RelationConstraint::NotNull(rule) => {
                    for row in &state.rows {
                        if matches!(row.values.get(rule.column()), Some(LiteralValue::Null)) {
                            return Err(impossible("enforced NOT NULL violated"));
                        }
                    }
                }
                RelationConstraint::PrimaryKey(key) | RelationConstraint::UniqueKey(key) => {
                    for (index, row) in state.rows.iter().enumerate() {
                        if matches!(constraint, RelationConstraint::PrimaryKey(_))
                            && key
                                .columns()
                                .iter()
                                .any(|col| matches!(row.values.get(col), Some(LiteralValue::Null)))
                        {
                            return Err(impossible("enforced PRIMARY KEY cannot contain NULL"));
                        }
                        if key
                            .columns()
                            .iter()
                            .any(|col| matches!(row.values.get(col), Some(LiteralValue::Null)))
                        {
                            continue;
                        }
                        for prior in &state.rows[..index] {
                            if key.columns().iter().any(|col| {
                                matches!(prior.values.get(col), Some(LiteralValue::Null))
                            }) {
                                continue;
                            }
                            if equal_key_rows(row, prior, key.columns(), key.columns())? {
                                return Err(impossible(
                                    "enforced key has duplicate physical values",
                                ));
                            }
                        }
                    }
                }
                RelationConstraint::ForeignKey(key) => {
                    let referenced = states.get(key.referenced_relation()).ok_or_else(|| {
                        residual("foreign-key target population is not controlled")
                    })?;
                    for row in &state.rows {
                        if key
                            .columns()
                            .iter()
                            .any(|col| matches!(row.values.get(col), Some(LiteralValue::Null)))
                        {
                            continue;
                        }
                        let mut found = false;
                        for other in &referenced.rows {
                            if equal_key_rows(row, other, key.columns(), key.referenced_columns())?
                            {
                                found = true;
                                break;
                            }
                        }
                        if !found {
                            // Deferrability and statement-end enforcement timing
                            // have not been modeled, so do not call this impossible.
                            return Err(residual("unproved foreign-key reference or deferral"));
                        }
                    }
                }
                RelationConstraint::AcceptedValues(accepted) => {
                    for row in &state.rows {
                        let value = row
                            .values
                            .get(accepted.column())
                            .ok_or_else(|| residual("accepted-values column missing"))?;
                        if *value == LiteralValue::Null {
                            continue;
                        }
                        let mut found = false;
                        for candidate in accepted.values() {
                            if constraint_scalar_equal(value, candidate)? {
                                found = true;
                                break;
                            }
                        }
                        if !found {
                            return Err(impossible("enforced accepted-values constraint violated"));
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

fn equal_key_rows(
    left: &PhysicalWriteRow,
    right: &PhysicalWriteRow,
    left_columns: &[String],
    right_columns: &[String],
) -> StateResult<bool> {
    for (left_name, right_name) in left_columns.iter().zip(right_columns) {
        let a = left
            .values
            .get(left_name)
            .ok_or_else(|| residual("key column missing"))?;
        let b = right
            .values
            .get(right_name)
            .ok_or_else(|| residual("referenced key column missing"))?;
        if !scalar_equality(a, b)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn constraint_scalar_equal(value: &LiteralValue, candidate: &ConstraintValue) -> StateResult<bool> {
    match (value, candidate) {
        (LiteralValue::Boolean(a), ConstraintValue::Boolean(b)) => Ok(a == b),
        (LiteralValue::Text(a), ConstraintValue::String(b)) => {
            if a == b {
                Ok(true)
            } else {
                Err(residual("accepted string comparison needs collation proof"))
            }
        }
        (LiteralValue::Number(a), ConstraintValue::Integer(b)) => a
            .parse::<i128>()
            .map(|a| a == i128::from(*b))
            .map_err(|_| residual("unsupported numeric constraint comparison")),
        (LiteralValue::Number(a), ConstraintValue::UnsignedInteger(b)) => a
            .parse::<u128>()
            .map(|a| a == u128::from(*b))
            .map_err(|_| residual("unsupported unsigned constraint comparison")),
        (LiteralValue::Number(a), ConstraintValue::Number(b)) => {
            if a == b {
                Ok(true)
            } else {
                Err(residual("unproved decimal accepted-value equality"))
            }
        }
        (LiteralValue::Null, ConstraintValue::Null) => Ok(true),
        _ => Err(residual("unproved accepted-value coercion")),
    }
}

fn value_conforms(value: &LiteralValue, kind: &DataType) -> bool {
    match (value, kind) {
        (LiteralValue::Null, _) => true,
        (value, DataType::Nullable(inner)) => value_conforms(value, inner),
        (LiteralValue::Boolean(_), DataType::Boolean) => true,
        (LiteralValue::Number(number), DataType::SignedInteger { bits }) => {
            let Ok(value) = number.parse::<i128>() else {
                return false;
            };
            match bits {
                Some(16) => i16::try_from(value).is_ok(),
                Some(32) => i32::try_from(value).is_ok(),
                Some(64) | None => i64::try_from(value).is_ok(),
                _ => false,
            }
        }
        (LiteralValue::Number(number), DataType::UnsignedInteger { bits }) => {
            let Ok(value) = number.parse::<u128>() else {
                return false;
            };
            match bits {
                Some(16) => u16::try_from(value).is_ok(),
                Some(32) => u32::try_from(value).is_ok(),
                Some(64) | None => u64::try_from(value).is_ok(),
                _ => false,
            }
        }
        (LiteralValue::Text(_), DataType::String { fixed: false, .. }) => true,
        _ => false,
    }
}

