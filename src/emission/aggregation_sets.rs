//! Aggregation, GROUP BY, set operations and branch proofs.

use super::*;

pub(super) fn aggregation_to_value(aggregation: &Aggregation) -> Value {
    json!({
        "distinct": aggregation.distinct(),
        "distinct_on": aggregation.distinct_on().iter().map(expression_to_value).collect::<Vec<_>>(),
        "group_by": aggregation.group_by().map_or(Value::Null, group_by_to_value)
    })
}

pub(super) fn group_by_to_value(group_by: &GroupBy) -> Value {
    match group_by {
        GroupBy::All => json!({ "kind": "all" }),
        GroupBy::Expressions(expressions) => json!({
            "kind": "expressions",
            "expressions": expressions.iter().map(grouping_expression_to_value).collect::<Vec<_>>()
        }),
    }
}

pub(super) fn grouping_expression_to_value(expression: &GroupingExpression) -> Value {
    match expression {
        GroupingExpression::Expression(expression) => json!({
            "kind": "expression",
            "expression": expression_to_value(expression)
        }),
        GroupingExpression::GroupingSets(sets) => grouping_sets_to_value("grouping_sets", sets),
        GroupingExpression::Rollup(sets) => grouping_sets_to_value("rollup", sets),
        GroupingExpression::Cube(sets) => grouping_sets_to_value("cube", sets),
    }
}

pub(super) fn grouping_sets_to_value(kind: &str, sets: &[Vec<crate::protocol::Expression>]) -> Value {
    json!({
        "kind": kind,
        "sets": sets.iter()
            .map(|set| set.iter().map(expression_to_value).collect::<Vec<_>>())
            .collect::<Vec<_>>()
    })
}

pub(super) fn set_operation_to_value(operation: &SetOperation) -> Value {
    let (qualifying, non_qualifying) = operation.witness_directions();
    json!({
        "operator": operation.operator().as_str(),
        "quantifier": operation.quantifier().as_str(),
        "left": set_operand_to_value(operation.left()),
        "right": set_operand_to_value(operation.right()),
        "membership": {
            "tuple_equality": "not_distinct",
            "multiplicity_rule": operation.multiplicity_rule().map(|rule| rule.as_str()),
            "branches": operation.branches().iter().map(|branch| json!({
                "identity": branch.identity(),
                "sources": branch.sources().iter().map(source_relation_to_value).collect::<Vec<_>>(),
                "predicates": predicates_to_value(branch.predicates()),
                "column_domains": branch.column_domains().iter().map(column_domain_to_value).collect::<Vec<_>>(),
                "output": output_to_value(branch.output()),
                "condition_exactness": condition_exactness_to_value(branch.condition_exactness()),
                "empty_input_preserving": branch.empty_input_preserving(),
            })).collect::<Vec<_>>(),
            "qualifying_witness": set_witness_direction_to_value(&qualifying),
            "non_qualifying_witness": set_witness_direction_to_value(&non_qualifying)
        }
    })
}

pub(super) fn set_witness_direction_to_value(direction: &crate::protocol::SetWitnessDirection) -> Value {
    match direction {
        crate::protocol::SetWitnessDirection::Residual { reason, origin } => json!({
            "status": "residual",
            "reason": reason,
            "origin": origin
        }),
        crate::protocol::SetWitnessDirection::Exact(cases) => json!({
            "status": "exact",
            "cases": cases.iter().map(set_witness_case_to_value).collect::<Vec<_>>()
        }),
    }
}

pub(super) fn set_witness_case_to_value(case: &crate::protocol::SetWitnessCase) -> Value {
    json!({
        "output_tuple_count": case.output_tuple_count(),
        "obligations": case.obligations().iter().map(|obligation| json!({
            "branch_identity": obligation.branch_identity(),
            "boundary": {
                "kind": if obligation.boundary().is_intermediate() { "intermediate" } else { "physical" },
                "relation": obligation.boundary().relation(),
                "tuple_columns": obligation.boundary().tuple_columns()
            },
            "matching_tuple_count": obligation.matching_tuple_count()
        })).collect::<Vec<_>>()
    })
}

pub(super) fn set_operand_to_value(operand: &SetOperand) -> Value {
    match operand {
        SetOperand::Query => json!({ "kind": "query" }),
        SetOperand::Operation(operation) => {
            let mut value = set_operation_to_value(operation);
            value["kind"] = json!("set_operation");
            value
        }
    }
}

