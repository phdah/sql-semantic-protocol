//! Boolean, membership, window, and grouping witness directions.

use super::*;

pub(super) fn boolean_witness_to_value(witness: &crate::BooleanWitness) -> Value {
    json!({
        "source_relation": witness.source_relation(),
        "condition": boolean_constraint_to_value(witness.condition()),
        "qualifying": boolean_witness_direction_to_value(witness.qualifying()),
        "rejected": boolean_witness_direction_to_value(witness.rejected())
    })
}

pub(super) fn boolean_witness_direction_to_value(
    direction: &crate::BooleanWitnessDirection,
) -> Value {
    match direction {
        crate::BooleanWitnessDirection::Impossible => json!({"status": "impossible"}),
        crate::BooleanWitnessDirection::Exact(case) => json!({
            "status": "exact",
            "truth": case.as_str()
        }),
        crate::BooleanWitnessDirection::Residual { reason } => json!({
            "status": "residual",
            "reason": reason
        }),
    }
}

pub(super) fn boolean_constraint_to_value(constraint: &crate::BooleanRowConstraint) -> Value {
    match constraint {
        crate::BooleanRowConstraint::All(children) => json!({
            "kind": "all", "operands": children.iter().map(boolean_constraint_to_value).collect::<Vec<_>>()
        }),
        crate::BooleanRowConstraint::Any(children) => json!({
            "kind": "any", "operands": children.iter().map(boolean_constraint_to_value).collect::<Vec<_>>()
        }),
        crate::BooleanRowConstraint::NullTest { column, negated } => json!({
            "kind": "null_test", "column": column_ref_to_value(column), "negated": negated
        }),
        crate::BooleanRowConstraint::UnknownNullComparison { column } => json!({
            "kind": "unknown_null_comparison", "column": column_ref_to_value(column)
        }),
        crate::BooleanRowConstraint::IntegerComparison {
            column,
            operator,
            literal,
        } => json!({
            "kind": "integer_comparison", "column": column_ref_to_value(column),
            "operator": operator.as_str(), "literal": literal
        }),
        crate::BooleanRowConstraint::IntegerColumnComparison {
            left,
            operator,
            right,
        } => json!({
            "kind": "integer_column_comparison",
            "left": column_ref_to_value(left),
            "operator": operator.as_str(),
            "right": column_ref_to_value(right)
        }),
        crate::BooleanRowConstraint::StringEquality {
            column,
            operator,
            literal,
        } => json!({
            "kind": "string_equality",
            "column": column_ref_to_value(column),
            "operator": operator.as_str(),
            "literal": literal
        }),
        crate::BooleanRowConstraint::StringPrefix {
            column,
            prefix,
            negated,
        } => json!({
            "kind": "string_prefix", "column": column_ref_to_value(column),
            "prefix": prefix, "negated": negated
        }),
        crate::BooleanRowConstraint::Residual { reason } => json!({
            "kind": "residual", "reason": reason
        }),
    }
}

pub(super) fn subquery_membership_witness_to_value(
    witness: &crate::subquery_witness::SubqueryMembershipWitness,
) -> Value {
    json!({
        "operator": witness.kind().as_str(),
        "outer_relation": witness.outer_relation(),
        "inner_relation": witness.inner_relation(),
        "correlations": witness.correlations().iter().map(|key| json!({
            "outer": column_ref_to_value(key.outer()),
            "inner": column_ref_to_value(key.inner())
        })).collect::<Vec<_>>(),
        "membership_key": witness.membership_key().map(|key| json!({
            "outer": column_ref_to_value(key.outer()),
            "inner": column_ref_to_value(key.inner())
        })),
        "inner_column_domains": witness.inner_column_domains().iter().map(column_domain_to_value).collect::<Vec<_>>(),
        "qualifying": subquery_membership_direction_to_value(witness.qualifying()),
        "rejected": subquery_membership_direction_to_value(witness.rejected())
    })
}

pub(super) fn subquery_membership_direction_to_value(
    direction: &crate::subquery_witness::SubqueryMembershipDirection,
) -> Value {
    match direction {
        crate::subquery_witness::SubqueryMembershipDirection::Exact(cases) => json!({
            "status": "exact",
            "cases": cases.iter().map(|case| case.as_str()).collect::<Vec<_>>()
        }),
        crate::subquery_witness::SubqueryMembershipDirection::Residual { reason } => json!({
            "status": "residual",
            "reason": reason
        }),
    }
}

pub(super) fn window_witness_to_value(witness: &crate::window_witness::WindowWitness) -> Value {
    json!({
        "boundary": witness.boundary(),
        "partition_by": witness.partition_by().iter().map(column_ref_to_value).collect::<Vec<_>>(),
        "order_by": witness.order_by().iter().map(|key| json!({
            "column": column_ref_to_value(key.column()),
            "ascending": key.ascending(),
            "nulls_first": key.nulls_first(),
            "strict_unique": true
        })).collect::<Vec<_>>(),
        "predicate": witness.operator().zip(witness.limit()).map(|(operator, limit)| json!({
            "operator": operator.as_str(), "limit": limit
        })),
        "qualifying": window_direction_to_value(witness.qualifying()),
        "rejected": window_direction_to_value(witness.rejected())
    })
}

pub(super) fn window_direction_to_value(
    direction: &crate::window_witness::WindowWitnessDirection,
) -> Value {
    match direction {
        crate::window_witness::WindowWitnessDirection::Residual { reason } => {
            json!({"status": "residual", "reason": reason})
        }
        crate::window_witness::WindowWitnessDirection::Impossible => {
            json!({"status": "impossible"})
        }
        crate::window_witness::WindowWitnessDirection::Exact(case) => {
            json!({"status": "exact", "min_preceding": case.min_preceding(),
                "max_preceding": case.max_preceding()})
        }
    }
}

pub(super) fn group_witness_to_value(witness: &crate::group_witness::GroupWitness) -> Value {
    json!({
        "boundary": witness.boundary(),
        "group_keys": witness.group_keys().iter().map(column_ref_to_value).collect::<Vec<_>>(),
        "aggregate": witness.aggregate().map(|aggregate| aggregate.as_str()),
        "distinct": witness.distinct(),
        "argument": witness.argument().map_or(Value::Null, column_ref_to_value),
        "predicate": witness.predicate().map(|(operator, bound)| json!({
            "operator": operator.as_str(), "bound": literal_expression_to_value(bound)
        })),
        "qualifying": group_witness_direction_to_value(witness.qualifying()),
        "rejected": group_witness_direction_to_value(witness.rejected())
    })
}

pub(super) fn group_witness_direction_to_value(
    direction: &crate::group_witness::GroupWitnessDirection,
) -> Value {
    match direction {
        crate::group_witness::GroupWitnessDirection::Residual { reason } => json!({
            "status": "residual", "reason": reason
        }),
        crate::group_witness::GroupWitnessDirection::Exact(cases) => json!({
            "status": "exact",
            "cases": cases.iter().map(|case| json!({
                "min_rows": case.min_rows(),
                "max_rows": case.max_rows(),
                "min_non_null": case.min_non_null(),
                "max_non_null": case.max_non_null(),
                "tests": case.tests().iter().map(|test| json!({
                    "kind": test.kind(),
                    "operator": test.operator().as_str(),
                    "bound": literal_expression_to_value(test.bound())
                })).collect::<Vec<_>>()
            })).collect::<Vec<_>>()
        }),
    }
}
