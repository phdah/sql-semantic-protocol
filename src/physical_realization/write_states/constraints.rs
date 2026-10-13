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
            validate_column_value(value, column)?;
        }
    }
    Ok(())
}

pub(super) fn validate_all_constraints(
    bundle: &AnalysisBundle,
    states: &BTreeMap<String, PhysicalWriteState>,
) -> StateResult<()> {
    validate_all_constraints_with_nested(bundle, states, &BTreeSet::new())
}

/// Non-scalar typed values have already been checked against their physical
/// schema; the scalar projection uses NULL only as an internal placeholder.
/// This exemption is deliberately limited to NOT NULL, never key equality,
/// CHECK comparison, generated expressions or referential integrity.
pub(super) fn validate_all_constraints_with_nested(
    bundle: &AnalysisBundle,
    states: &BTreeMap<String, PhysicalWriteState>,
    non_null_nested: &BTreeSet<(String, String)>,
) -> StateResult<()> {
    for set in bundle.relation_constraints() {
        let Some(state) = states.get(set.relation()) else {
            continue;
        };
        for row in &state.rows {
            super::generated::check_declared_sources(set.column_value_sources(), row)?;
        }
        if !set.diagnostics().is_empty() {
            return Err(residual_gap(
                PhysicalWriteStateGap::UnprovedConstraint,
                "unresolved canonical constraint diagnostics",
            ));
        }
        // A complete empty bag satisfies every row-level constraint regardless
        // of unknown enforcement, collation, or nullable uniqueness semantics.
        // Schema-level diagnostics above still prevent a false certification.
        if state.rows.is_empty() {
            continue;
        }
        for check in set.computed_checks() {
            if check
                .evidence()
                .iter()
                .all(|evidence| evidence.enforcement() == ConstraintEnforcement::NotEnforced)
            {
                continue;
            }
            if !check
                .evidence()
                .iter()
                .any(|evidence| evidence.enforcement() == ConstraintEnforcement::Enforced)
            {
                return Err(residual_gap(
                    PhysicalWriteStateGap::UnprovedConstraint,
                    "computed CHECK enforcement not attested",
                ));
            }
            for row in &state.rows {
                let left = super::generated::evaluate(check.left(), &row.values)?;
                let right = super::generated::evaluate(check.right(), &row.values)?;
                let value = match right {
                    LiteralValue::Null => ConstraintValue::Null,
                    LiteralValue::Boolean(value) => ConstraintValue::Boolean(value),
                    LiteralValue::Number(value) => ConstraintValue::Number(value),
                    LiteralValue::Text(value) => ConstraintValue::String(value),
                };
                super::check_comparisons::validate_scalar_check(&left, check.operator(), &value)?;
            }
        }
        for key in set.computed_keys() {
            if key
                .evidence()
                .iter()
                .all(|evidence| evidence.enforcement() == ConstraintEnforcement::NotEnforced)
            {
                continue;
            }
            if !key
                .evidence()
                .iter()
                .any(|evidence| evidence.enforcement() == ConstraintEnforcement::Enforced)
            {
                return Err(residual_gap(
                    PhysicalWriteStateGap::UnprovedConstraint,
                    "computed key enforcement not attested",
                ));
            }
            let mut seen: Vec<Vec<LiteralValue>> = Vec::new();
            for row in &state.rows {
                let values = key
                    .expressions()
                    .iter()
                    .map(|expression| super::generated::evaluate(expression, &row.values))
                    .collect::<StateResult<Vec<_>>>()?;
                if key.primary()
                    && values
                        .iter()
                        .any(|value| matches!(value, LiteralValue::Null))
                {
                    return Err(impossible("computed PRIMARY KEY cannot contain NULL"));
                }
                for prior in &seen {
                    match unique_key_collision(&values, prior)? {
                        Some(true) => {
                            return Err(impossible(
                                "enforced computed key has duplicate physical values",
                            ));
                        }
                        None => {
                            return Err(residual_gap(
                                PhysicalWriteStateGap::UnprovedConstraint,
                                "nullable functional UNIQUE key has an unknown NULL uniqueness law",
                            ));
                        }
                        Some(false) => {}
                    }
                }
                seen.push(values);
            }
        }
        for constraint in set.constraints() {
            if constraint
                .evidence()
                .iter()
                .all(|evidence| evidence.enforcement() == ConstraintEnforcement::NotEnforced)
            {
                continue;
            }
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
                        if !non_null_nested
                            .contains(&(state.relation.clone(), rule.column().to_string()))
                            && matches!(row.values.get(rule.column()), Some(LiteralValue::Null))
                        {
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
                        for prior in &state.rows[..index] {
                            let values = key
                                .columns()
                                .iter()
                                .map(|col| {
                                    row.values
                                        .get(col)
                                        .cloned()
                                        .ok_or_else(|| residual("key column missing"))
                                })
                                .collect::<StateResult<Vec<_>>>()?;
                            let existing = key
                                .columns()
                                .iter()
                                .map(|col| {
                                    prior
                                        .values
                                        .get(col)
                                        .cloned()
                                        .ok_or_else(|| residual("key column missing"))
                                })
                                .collect::<StateResult<Vec<_>>>()?;
                            match unique_key_collision(&values, &existing)? {
                                Some(true) => {
                                    return Err(impossible(
                                        "enforced key has duplicate physical values",
                                    ));
                                }
                                None => {
                                    return Err(residual_gap(
                                        PhysicalWriteStateGap::UnprovedConstraint,
                                        "nullable UNIQUE key requires dialect-specific NULL uniqueness law",
                                    ));
                                }
                                Some(false) => {}
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
                        let null_count = key
                            .columns()
                            .iter()
                            .filter(|col| matches!(row.values.get(*col), Some(LiteralValue::Null)))
                            .count();
                        if null_count == key.columns().len() {
                            continue; // All-NULL keys cannot identify a referenced tuple.
                        }
                        if null_count > 0 {
                            return Err(residual_gap(
                                PhysicalWriteStateGap::UnprovedConstraint,
                                "partially NULL composite foreign key needs attested MATCH semantics",
                            ));
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
                RelationConstraint::CheckComparison(check) => {
                    for row in &state.rows {
                        let value = row
                            .values
                            .get(check.column())
                            .ok_or_else(|| residual("CHECK column missing from physical row"))?;
                        if matches!(value, LiteralValue::Null) {
                            continue; // SQL CHECK admits UNKNOWN.
                        }
                        super::check_comparisons::validate_scalar_check(
                            value,
                            check.operator(),
                            check.value(),
                        )?;
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

/// An attested unique key is violated only by definitely identical non-NULL
/// components. One NULL-bearing row cannot collide on its own; two possibly
/// equal NULL-bearing tuples require a dialect's NULLS DISTINCT law.
fn unique_key_collision(
    left: &[LiteralValue],
    right: &[LiteralValue],
) -> StateResult<Option<bool>> {
    let mut unknown_null = false;
    for (a, b) in left.iter().zip(right) {
        match (a, b) {
            (LiteralValue::Null, LiteralValue::Null) => unknown_null = true,
            (LiteralValue::Null, _) | (_, LiteralValue::Null) => return Ok(Some(false)),
            _ if !scalar_equality(a, b)? => return Ok(Some(false)),
            _ => {}
        }
    }
    if unknown_null {
        Ok(None)
    } else {
        Ok(Some(true))
    }
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
        (LiteralValue::Number(a), ConstraintValue::Integer(b)) => {
            super::check_comparisons::exact_numeric_equality(a, &b.to_string())
                .ok_or_else(|| residual("accepted numeric equality requires finite exact decimals"))
        }
        (LiteralValue::Number(a), ConstraintValue::UnsignedInteger(b)) => {
            super::check_comparisons::exact_numeric_equality(a, &b.to_string())
                .ok_or_else(|| residual("accepted numeric equality requires finite exact decimals"))
        }
        (LiteralValue::Number(a), ConstraintValue::Number(b)) => {
            super::check_comparisons::exact_numeric_equality(a, b)
                .ok_or_else(|| residual("accepted numeric equality requires finite exact decimals"))
        }
        (LiteralValue::Null, ConstraintValue::Null) => Ok(true),
        _ => Err(residual("unproved accepted-value coercion")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_accepted_value_comparison_normalizes_decimal_scale_and_signed_zero() {
        for (actual, expected) in [
            ("1.5000", ConstraintValue::Number("1.5".into())),
            ("0005.0", ConstraintValue::Integer(5)),
            ("-0.000", ConstraintValue::UnsignedInteger(0)),
            (
                "999999999999999999999999999999.000001",
                ConstraintValue::Number("999999999999999999999999999999.0000010".into()),
            ),
        ] {
            assert!(
                constraint_scalar_equal(&LiteralValue::Number(actual.into()), &expected).unwrap(),
                "{actual}"
            );
        }
        assert!(!constraint_scalar_equal(
            &LiteralValue::Number("1.501".into()),
            &ConstraintValue::Number("1.5".into()),
        )
        .unwrap());
        assert!(matches!(
            constraint_scalar_equal(
                &LiteralValue::Number("NaN".into()),
                &ConstraintValue::Number("1.5".into()),
            ),
            Err(StateFailure::Residual(_, _))
        ));
    }
}
