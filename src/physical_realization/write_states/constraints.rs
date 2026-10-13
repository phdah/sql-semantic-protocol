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
    validate_all_constraints_with_identity(bundle, states, non_null_nested, &BTreeSet::new())
}

/// Certify row-level database identity values only when a caller has supplied
/// the complete next-allocation state for the corresponding canonical source.
pub(super) fn validate_all_constraints_with_identity(
    bundle: &AnalysisBundle,
    states: &BTreeMap<String, PhysicalWriteState>,
    non_null_nested: &BTreeSet<(String, String)>,
    identity_columns: &BTreeSet<(String, String)>,
) -> StateResult<()> {
    // A closed-world proof cannot silently ignore a constrained relation or
    // an FK target merely because the caller omitted its physical population.
    // Unrelated unconstrained catalog entries do not require materialization.
    let mut required_relations = BTreeSet::new();
    for set in bundle.relation_constraints() {
        if !set.constraints().is_empty()
            || !set.computed_keys().is_empty()
            || !set.computed_checks().is_empty()
            || !set.computed_boolean_checks().is_empty()
            || !set.column_value_sources().is_empty()
            || !set.diagnostics().is_empty()
        {
            required_relations.insert(set.relation().to_string());
        }
        for constraint in set.constraints() {
            if let RelationConstraint::ForeignKey(key) = constraint {
                required_relations.insert(key.referenced_relation().to_string());
            }
        }
    }
    // Only relations actually participating in the analyzed workload are
    // required. An unrelated catalog entry does not make a proof incomplete,
    // but an SQL-read source or an existing mutation target does.
    for layer in bundle.layers() {
        for source in layer.consumes() {
            required_relations.insert(source.to_string());
        }
        if layer
            .write_kind()
            .is_some_and(|kind| kind != WriteKind::Definition)
        {
            for output in layer.produces() {
                if let Some(relation) = output.relation_name() {
                    required_relations.insert(relation.to_string());
                }
            }
        }
    }
    for relation in &required_relations {
        if !states.contains_key(relation) {
            return Err(residual_gap(
                PhysicalWriteStateGap::MissingPopulation,
                format!(
                    "missing complete constrained or referenced relation population: {relation}"
                ),
            ));
        }
    }

    for set in bundle.relation_constraints() {
        let Some(state) = states.get(set.relation()) else {
            continue;
        };
        let schema = bundle
            .source_schemas()
            .iter()
            .find(|schema| schema.relation() == set.relation())
            .ok_or_else(|| {
                residual_gap(
                    PhysicalWriteStateGap::MissingSchema,
                    format!("missing schema for constrained relation {}", set.relation()),
                )
            })?;
        let controlled = identity_columns
            .iter()
            .filter(|(relation, _)| relation == set.relation())
            .map(|(_, column)| column.clone())
            .collect::<BTreeSet<_>>();
        for row in &state.rows {
            super::generated::check_declared_sources(
                set.column_value_sources(),
                row,
                &controlled,
                schema,
            )?;
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
                let left = super::generated::evaluate(check.left(), &row.values, schema)?;
                let right = super::generated::evaluate(check.right(), &row.values, schema)?;
                let value = match right {
                    LiteralValue::Null => ConstraintValue::Null,
                    LiteralValue::Boolean(value) => ConstraintValue::Boolean(value),
                    LiteralValue::Number(value) => ConstraintValue::Number(value),
                    LiteralValue::Text(value) => ConstraintValue::String(value),
                };
                super::check_comparisons::validate_scalar_check(&left, check.operator(), &value)?;
            }
        }
        for check in set.computed_boolean_checks() {
            if check
                .evidence()
                .iter()
                .all(|e| e.enforcement() == ConstraintEnforcement::NotEnforced)
            {
                continue;
            }
            if !check
                .evidence()
                .iter()
                .any(|e| e.enforcement() == ConstraintEnforcement::Enforced)
            {
                return Err(residual_gap(
                    PhysicalWriteStateGap::UnprovedConstraint,
                    "Boolean CHECK enforcement is not attested",
                ));
            }
            for row in &state.rows {
                super::boolean_checks::check(check.predicate(), &row.values, schema)?;
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
                    .map(|expression| super::generated::evaluate(expression, &row.values, schema))
                    .collect::<StateResult<Vec<_>>>()?;
                if key.primary()
                    && values
                        .iter()
                        .any(|value| matches!(value, LiteralValue::Null))
                {
                    return Err(impossible("computed PRIMARY KEY cannot contain NULL"));
                }
                for prior in &seen {
                    match unique_key_collision(&values, prior, &[])? {
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
                    let binary_text_columns = key
                        .columns()
                        .iter()
                        .map(|column| attested_binary_text_law(bundle, set.relation(), column))
                        .collect::<Vec<_>>();
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
                            match unique_key_collision(&values, &existing, &binary_text_columns)? {
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
                            // An ambiguous candidate does not invalidate a later
                            // definitely matching tuple in the same complete bag.
                            match equal_key_rows(
                                bundle,
                                set.relation(),
                                row,
                                key.referenced_relation(),
                                other,
                                key.columns(),
                                key.referenced_columns(),
                            ) {
                                Ok(true) => {
                                    found = true;
                                    break;
                                }
                                Ok(false) | Err(StateFailure::Residual(_, _)) => {}
                                Err(error) => return Err(error),
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
                        super::check_comparisons::validate_scalar_check_with_binary_law(
                            value,
                            check.operator(),
                            check.value(),
                            attested_binary_text_law(bundle, state.relation(), check.column()),
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
                        let binary_text = bundle
                            .source_schemas()
                            .iter()
                            .find(|schema| schema.relation() == state.relation)
                            .and_then(|schema| {
                                schema
                                    .columns()
                                    .iter()
                                    .find(|column| column.name() == accepted.column())
                            })
                            .and_then(|column| column.text_comparison_law())
                            == Some(crate::relation::TextComparisonLaw::Binary);
                        let mut found = false;
                        let mut comparison_unknown = false;
                        for candidate in accepted.values() {
                            match constraint_scalar_equal(value, candidate, binary_text) {
                                Ok(true) => {
                                    found = true;
                                    break;
                                }
                                Ok(false) => {}
                                Err(StateFailure::Residual(_, _)) => comparison_unknown = true,
                                Err(error) => return Err(error),
                            }
                        }
                        if !found {
                            if comparison_unknown {
                                return Err(residual_gap(
                                    PhysicalWriteStateGap::UnprovedConstraint,
                                    "accepted-values candidates include an unproved equality law",
                                ));
                            }
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
    binary_text_columns: &[bool],
) -> StateResult<Option<bool>> {
    let mut unknown_null = false;
    let mut unknown_equality = false;
    for (index, (a, b)) in left.iter().zip(right).enumerate() {
        match (a, b) {
            (LiteralValue::Null, LiteralValue::Null) => unknown_null = true,
            (LiteralValue::Null, _) | (_, LiteralValue::Null) => return Ok(Some(false)),
            _ => match key_scalar_equality(a, b, binary_text_columns.get(index) == Some(&true)) {
                Ok(false) => return Ok(Some(false)),
                Ok(true) => {}
                Err(StateFailure::Residual(_, _)) => unknown_equality = true,
                Err(error) => return Err(error),
            },
        }
    }
    if unknown_equality {
        Err(residual_gap(
            PhysicalWriteStateGap::UnprovedConstraint,
            "composite key equality contains unproved comparisons",
        ))
    } else if unknown_null {
        Ok(None)
    } else {
        Ok(Some(true))
    }
}

fn equal_key_rows(
    bundle: &AnalysisBundle,
    left_relation: &str,
    left: &PhysicalWriteRow,
    right_relation: &str,
    right: &PhysicalWriteRow,
    left_columns: &[String],
    right_columns: &[String],
) -> StateResult<bool> {
    let mut unknown_equality = false;
    for (left_name, right_name) in left_columns.iter().zip(right_columns) {
        let a = left
            .values
            .get(left_name)
            .ok_or_else(|| residual("key column missing"))?;
        let b = right
            .values
            .get(right_name)
            .ok_or_else(|| residual("referenced key column missing"))?;
        let binary_text = attested_binary_text_law(bundle, left_relation, left_name)
            && attested_binary_text_law(bundle, right_relation, right_name);
        match key_scalar_equality(a, b, binary_text) {
            Ok(false) => return Ok(false),
            Ok(true) => {}
            Err(StateFailure::Residual(_, _)) => unknown_equality = true,
            Err(error) => return Err(error),
        }
    }
    if unknown_equality {
        Err(residual_gap(
            PhysicalWriteStateGap::UnprovedConstraint,
            "foreign-key tuple equality contains unproved comparisons",
        ))
    } else {
        Ok(true)
    }
}

/// Binary comparison is a canonical column fact, never an inference from the
/// collation name. Keys use the same attested law as accepted-values checking.
fn attested_binary_text_law(bundle: &AnalysisBundle, relation: &str, column: &str) -> bool {
    bundle
        .source_schemas()
        .iter()
        .find(|schema| schema.relation() == relation)
        .and_then(|schema| schema.columns().iter().find(|item| item.name() == column))
        .and_then(|item| item.text_comparison_law())
        == Some(crate::relation::TextComparisonLaw::Binary)
}

fn key_scalar_equality(a: &LiteralValue, b: &LiteralValue, binary_text: bool) -> StateResult<bool> {
    match (a, b) {
        (LiteralValue::Text(left), LiteralValue::Text(right)) if binary_text => Ok(left == right),
        _ => scalar_equality(a, b),
    }
}

fn constraint_scalar_equal(
    value: &LiteralValue,
    candidate: &ConstraintValue,
    binary_text: bool,
) -> StateResult<bool> {
    match (value, candidate) {
        (LiteralValue::Boolean(a), ConstraintValue::Boolean(b)) => Ok(a == b),
        (LiteralValue::Text(a), ConstraintValue::String(b)) => {
            if a == b {
                Ok(true)
            } else if binary_text {
                Ok(false)
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
    fn composite_keys_defer_unproved_text_until_all_components_are_considered() {
        let unknown = [
            LiteralValue::Text("first".into()),
            LiteralValue::Number("1".into()),
        ];
        let different = [
            LiteralValue::Text("FIRST".into()),
            LiteralValue::Number("2".into()),
        ];
        let same_numeric = [
            LiteralValue::Text("FIRST".into()),
            LiteralValue::Number("1".into()),
        ];
        assert_eq!(
            unique_key_collision(&unknown, &different, &[false, false]).unwrap(),
            Some(false),
        );
        assert!(matches!(
            unique_key_collision(&unknown, &same_numeric, &[false, false]),
            Err(StateFailure::Residual(_, _))
        ));
        assert_eq!(
            unique_key_collision(&unknown, &same_numeric, &[true, false]).unwrap(),
            Some(false),
        );

        let nullable = [LiteralValue::Null, LiteralValue::Number("7".into())];
        let disjoint = [LiteralValue::Null, LiteralValue::Number("8".into())];
        assert_eq!(
            unique_key_collision(&nullable, &disjoint, &[false, false]).unwrap(),
            Some(false),
        );
        assert_eq!(
            unique_key_collision(&nullable, &nullable, &[false, false]).unwrap(),
            None,
        );
    }

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
                constraint_scalar_equal(&LiteralValue::Number(actual.into()), &expected, false)
                    .unwrap(),
                "{actual}"
            );
        }
        assert!(!constraint_scalar_equal(
            &LiteralValue::Number("1.501".into()),
            &ConstraintValue::Number("1.5".into()),
            false,
        )
        .unwrap());
        assert!(matches!(
            constraint_scalar_equal(
                &LiteralValue::Number("NaN".into()),
                &ConstraintValue::Number("1.5".into()),
                false,
            ),
            Err(StateFailure::Residual(_, _))
        ));
    }
}
