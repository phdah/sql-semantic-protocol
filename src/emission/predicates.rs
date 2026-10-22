//! Normalized SQL predicates and boolean conditions.

use super::*;

pub(super) fn predicates_to_value(predicates: &Predicates) -> Value {
    json!({
        "where": optional_predicate_to_value(predicates.where_predicate()),
        "having": optional_predicate_to_value(predicates.having_predicate()),
        "qualify": optional_predicate_to_value(predicates.qualify_predicate())
    })
}

pub(super) fn optional_predicate_to_value(predicate: Option<&Predicate>) -> Value {
    predicate.map_or(Value::Null, predicate_to_value)
}

pub(super) fn predicate_to_value(predicate: &Predicate) -> Value {
    match predicate {
        Predicate::Comparison(predicate) => comparison_predicate_to_value(predicate),
        Predicate::LikePrefix(predicate) => json!({
            "kind": "like_prefix",
            "expression": expression_to_value(predicate.expression()),
            "prefix": predicate.prefix(),
            "negated": predicate.negated()
        }),
        Predicate::And(predicate) => logical_predicate_to_value("and", predicate),
        Predicate::Or(predicate) => logical_predicate_to_value("or", predicate),
        Predicate::Not(predicate) => not_predicate_to_value(predicate),
        Predicate::IsNull(predicate) => is_null_predicate_to_value(predicate),
        Predicate::In(predicate) => in_predicate_to_value(predicate),
        Predicate::Exists(predicate) => exists_predicate_to_value(predicate),
        Predicate::InSubquery(predicate) => in_subquery_predicate_to_value(predicate),
        Predicate::Between(predicate) => between_predicate_to_value(predicate),
        Predicate::BooleanExpression(expression) => json!({
            "kind": "boolean_expression",
            "expression": expression_to_value(expression)
        }),
        Predicate::Unknown(semantic) => unknown_semantic_to_value(semantic),
        Predicate::Unsupported(semantic) => unsupported_semantic_to_value(semantic),
    }
}

pub(super) fn comparison_predicate_to_value(predicate: &ComparisonPredicate) -> Value {
    json!({
        "kind": "comparison",
        "left": expression_to_value(predicate.left()),
        "operator": predicate.operator().as_str(),
        "right": expression_to_value(predicate.right())
    })
}

pub(super) fn logical_predicate_to_value(kind: &str, predicate: &LogicalPredicate) -> Value {
    json!({
        "kind": kind,
        "operands": predicate
            .operands()
            .iter()
            .map(predicate_to_value)
            .collect::<Vec<_>>()
    })
}

pub(super) fn not_predicate_to_value(predicate: &NotPredicate) -> Value {
    json!({
        "kind": "not",
        "operand": predicate_to_value(predicate.operand())
    })
}

pub(super) fn is_null_predicate_to_value(predicate: &IsNullPredicate) -> Value {
    json!({
        "kind": "is_null",
        "expression": expression_to_value(predicate.expression()),
        "negated": predicate.negated()
    })
}

pub(super) fn in_predicate_to_value(predicate: &InPredicate) -> Value {
    json!({
        "kind": "in",
        "expression": expression_to_value(predicate.expression()),
        "values": predicate
            .values()
            .iter()
            .map(expression_to_value)
            .collect::<Vec<_>>(),
        "negated": predicate.negated()
    })
}

pub(super) fn exists_predicate_to_value(predicate: &ExistsPredicate) -> Value {
    json!({
        "kind": "exists",
        "subquery": subquery_semantics_to_value(predicate.subquery()),
        "negated": predicate.negated()
    })
}

pub(super) fn in_subquery_predicate_to_value(predicate: &InSubqueryPredicate) -> Value {
    let expression = match predicate.operand() {
        InSubqueryOperand::Scalar(expression) => expression_to_value(expression),
        InSubqueryOperand::Tuple(tuple) => json!({
            "kind": "tuple",
            "expressions": tuple.expressions().iter().map(expression_to_value).collect::<Vec<_>>(),
        }),
    };
    json!({
        "kind": "in_subquery",
        "expression": expression,
        "subquery": subquery_semantics_to_value(predicate.subquery()),
        "negated": predicate.negated()
    })
}

pub(super) fn between_predicate_to_value(predicate: &BetweenPredicate) -> Value {
    json!({
        "kind": "between",
        "expression": expression_to_value(predicate.expression()),
        "lower": expression_to_value(predicate.lower()),
        "upper": expression_to_value(predicate.upper()),
        "negated": predicate.negated()
    })
}
