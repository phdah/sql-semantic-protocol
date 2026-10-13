//! Enforced key, nullability and referential physical snapshot validation.

use super::evaluator::scalar_equality;
use super::*;

pub(super) fn validate_state(
    bundle: &AnalysisBundle,
    state: &PhysicalWriteState,
) -> StateResult<()> {
    let schema = bundle
        .source_schemas()
        .iter()
        .find(|schema| schema.relation() == state.relation)
        .ok_or_else(|| {
            residual_gap(
                PhysicalWriteStateGap::MissingSchema,
                format!("missing complete schema for {}", state.relation),
            )
        })?;
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
            validate_value(value, column.data_type())?;
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
            return Err(residual_gap(
                PhysicalWriteStateGap::UnprovedConstraint,
                "unresolved canonical constraint diagnostics",
            ));
        }
        for constraint in set.constraints() {
            if !constraint
                .evidence()
                .iter()
                .any(|e| e.enforcement() == ConstraintEnforcement::Enforced)
            {
                return Err(residual_gap(
                    PhysicalWriteStateGap::UnprovedConstraint,
                    "constraint enforcement not attested",
                ));
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
                            // Dialects differ on UNIQUE NULL equality (for
                            // example, SQL Server versus PostgreSQL). A generic
                            // uniqueness proof may not assume NULLS DISTINCT.
                            return Err(residual_gap(
                                PhysicalWriteStateGap::UnprovedConstraint,
                                "nullable UNIQUE key requires dialect-specific NULL uniqueness law",
                            ));
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
                    // A foreign key may reference only a dialect-legitimate
                    // unique target key. Do not infer one from coincidental
                    // sample values or from the FK declaration alone.
                    let referenced_key_attested = bundle
                        .relation_constraints()
                        .iter()
                        .filter(|set| set.relation() == key.referenced_relation())
                        .flat_map(|set| set.constraints())
                        .any(|constraint| match constraint {
                            RelationConstraint::PrimaryKey(ref_key)
                            | RelationConstraint::UniqueKey(ref_key) => {
                                ref_key.columns() == key.referenced_columns()
                                    && ref_key
                                        .evidence()
                                        .iter()
                                        .any(|e| e.enforcement() == ConstraintEnforcement::Enforced)
                            }
                            _ => false,
                        });
                    if !referenced_key_attested {
                        return Err(residual_gap(
                            PhysicalWriteStateGap::UnprovedConstraint,
                            "foreign-key referenced columns lack an attested unique key",
                        ));
                    }
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
