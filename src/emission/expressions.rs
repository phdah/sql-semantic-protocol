//! Expressions, scoped subqueries, and residual exactness.

use super::*;

pub(super) fn expression_to_value(expression: &Expression) -> Value {
    match expression {
        Expression::Column(expression) => column_expression_to_value(expression),
        Expression::Literal(expression) => literal_expression_to_value(expression),
        Expression::Function(expression) => function_expression_to_value(expression),
        Expression::AggregateFunction(expression) => {
            aggregate_function_expression_to_value(expression)
        }
        Expression::WindowFunction(expression) => window_function_expression_to_value(expression),
        Expression::Case(expression) => case_expression_to_value(expression),
        Expression::BooleanPredicate(predicate) => json!({
            "kind": "boolean_predicate",
            "predicate": predicate_to_value(predicate)
        }),
        Expression::SignedIntegerCast(expression) => json!({
            "kind": "signed_integer_cast",
            "expression": expression_to_value(expression.expression()),
            "target_bits": expression.target_bits(),
            "cast_kind": expression.cast_kind().as_str(),
            "on_failure": expression.cast_kind().failure_behavior().as_str()
        }),
        Expression::Unary(expression) => unary_expression_to_value(expression),
        Expression::Binary(expression) => binary_expression_to_value(expression),
        Expression::ScalarSubquery(expression) => scalar_subquery_expression_to_value(expression),
        Expression::Unknown(semantic) => unknown_semantic_to_value(semantic),
        Expression::Unsupported(semantic) => unsupported_semantic_to_value(semantic),
    }
}

pub(super) fn case_expression_to_value(expression: &CaseExpression) -> Value {
    json!({
        "kind": "case",
        "operand": expression
            .operand()
            .map_or(Value::Null, expression_to_value),
        "branches": expression
            .branches()
            .iter()
            .map(|branch| json!({
                "condition": expression_to_value(branch.condition()),
                "result": expression_to_value(branch.result()),
                "source_domains": case_source_domains_to_value(branch.source_domains())
            }))
            .collect::<Vec<_>>(),
        "else_result": expression
            .else_result()
            .map_or(Value::Null, expression_to_value),
        "else_source_domains": case_source_domains_to_value(expression.else_source_domains())
    })
}

pub(super) fn case_source_domains_to_value(domains: &CaseSourceDomains) -> Value {
    match domains {
        CaseSourceDomains::Reachable { alternatives } => json!({
            "status": "reachable",
            "alternatives": alternatives
                .iter()
                .map(|alternative| json!({
                    "column_domains": alternative
                        .column_domains()
                        .iter()
                        .map(column_domain_to_value)
                        .collect::<Vec<_>>()
                }))
                .collect::<Vec<_>>()
        }),
        CaseSourceDomains::Unreachable => json!({
            "status": "unreachable"
        }),
        CaseSourceDomains::Unknown(domain) => json!({
            "status": "unknown",
            "reason": domain.reason()
        }),
    }
}

pub(super) fn scalar_subquery_expression_to_value(expression: &ScalarSubqueryExpression) -> Value {
    json!({
        "kind": "scalar_subquery",
        "subquery": subquery_semantics_to_value(expression.subquery())
    })
}

pub(super) fn subquery_semantics_to_value(subquery: &SubquerySemantics) -> Value {
    json!({
        "dependencies": subquery.dependencies(),
        "correlations": subquery
            .correlations()
            .iter()
            .map(lineage_source_to_value)
            .collect::<Vec<_>>(),
        "joins": subquery.joins().iter().map(join_to_value).collect::<Vec<_>>(),
        "output": output_to_value(subquery.output()),
        "predicates": predicates_to_value(subquery.predicates()),
        "column_domains": subquery
            .column_domains()
            .iter()
            .map(column_domain_to_value)
            .collect::<Vec<_>>(),
        "condition_exactness": condition_exactness_to_value(subquery.condition_exactness()),
        "diagnostics": subquery
            .diagnostics()
            .iter()
            .map(diagnostic_to_value)
            .collect::<Vec<_>>()
    })
}

pub(super) fn condition_exactness_to_value(exactness: &ConditionExactness) -> Value {
    json!({
        "status": exactness.status().as_str(),
        "residual_conditions": exactness
            .residual_conditions()
            .iter()
            .map(residual_condition_to_value)
            .collect::<Vec<_>>(),
        "comparison_assumptions": exactness.required_assumptions().iter().map(|requirement| {
            let mut value = json!({
                "name": requirement.assumption().as_str(),
                "clause": requirement.clause().as_str(),
                "identity": requirement.identity(),
                "declared": exactness.declared_assumptions().contains(&requirement.assumption())
            });
            if let (Some(layer), Some(scope)) = (requirement.origin_layer_id(), requirement.origin_scope()) {
                value["origin"] = json!({"layer_id": layer, "scope": scope});
            }
            value
        }).collect::<Vec<_>>()
    })
}

pub(super) fn residual_condition_to_value(residual: &crate::protocol::ResidualCondition) -> Value {
    let mut value = json!({
        "reason": residual.reason().as_str(),
        "clause": residual.clause().as_str(),
        "identity": residual.identity()
    });

    if let (Some(layer_id), Some(scope)) = (residual.origin_layer_id(), residual.origin_scope()) {
        value["origin"] = json!({
            "layer_id": layer_id,
            "scope": scope
        });
    }

    value
}
