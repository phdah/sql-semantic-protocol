//! Protocol serialization boundary.
//!
//! Serialization depends only on parser-independent protocol values. It deliberately contains no
//! sqlparser AST handling.

use serde_json::{json, Value};

use crate::protocol::{
    BetweenPredicate, BinaryExpression, ColumnExpression, ComparisonPredicate, Diagnostic,
    Expression, FunctionExpression, InPredicate, IsNullPredicate, LiteralExpression, LiteralValue,
    LogicalPredicate, NotPredicate, Predicate, Predicates, Protocol, ProtocolStatement,
    QueryStatement, UnaryExpression, UnknownSemantic, UnsupportedSemantic, UnsupportedStatement,
};

/// Serialize protocol domain values to JSON.
pub fn to_json(protocol: &Protocol) -> String {
    protocol_to_value(protocol).to_string()
}

fn protocol_to_value(protocol: &Protocol) -> Value {
    let statements = protocol
        .statements()
        .iter()
        .map(statement_to_value)
        .collect::<Vec<_>>();

    json!({
        "protocol_version": protocol.protocol_version(),
        "source": {
            "dialect": protocol.source().dialect()
        },
        "statements": statements
    })
}

fn statement_to_value(statement: &ProtocolStatement) -> Value {
    match statement {
        ProtocolStatement::Query(statement) => query_statement_to_value(statement),
        ProtocolStatement::Unsupported(statement) => unsupported_statement_to_value(statement),
    }
}

fn query_statement_to_value(statement: &QueryStatement) -> Value {
    let diagnostics = statement
        .diagnostics()
        .iter()
        .map(diagnostic_to_value)
        .collect::<Vec<_>>();

    json!({
        "kind": "query",
        "sources": [],
        "dependencies": [],
        "joins": [],
        "predicates": predicates_to_value(statement.predicates()),
        "column_domains": [],
        "output": {
            "columns": []
        },
        "diagnostics": diagnostics
    })
}

fn predicates_to_value(predicates: &Predicates) -> Value {
    json!({
        "where": optional_predicate_to_value(predicates.where_predicate()),
        "having": optional_predicate_to_value(predicates.having_predicate()),
        "qualify": optional_predicate_to_value(predicates.qualify_predicate())
    })
}

fn optional_predicate_to_value(predicate: Option<&Predicate>) -> Value {
    predicate.map_or(Value::Null, predicate_to_value)
}

fn predicate_to_value(predicate: &Predicate) -> Value {
    match predicate {
        Predicate::Comparison(predicate) => comparison_predicate_to_value(predicate),
        Predicate::And(predicate) => logical_predicate_to_value("and", predicate),
        Predicate::Or(predicate) => logical_predicate_to_value("or", predicate),
        Predicate::Not(predicate) => not_predicate_to_value(predicate),
        Predicate::IsNull(predicate) => is_null_predicate_to_value(predicate),
        Predicate::In(predicate) => in_predicate_to_value(predicate),
        Predicate::Between(predicate) => between_predicate_to_value(predicate),
        Predicate::BooleanExpression(expression) => json!({
            "kind": "boolean_expression",
            "expression": expression_to_value(expression)
        }),
        Predicate::Unknown(semantic) => unknown_semantic_to_value(semantic),
        Predicate::Unsupported(semantic) => unsupported_semantic_to_value(semantic),
    }
}

fn comparison_predicate_to_value(predicate: &ComparisonPredicate) -> Value {
    json!({
        "kind": "comparison",
        "left": expression_to_value(predicate.left()),
        "operator": predicate.operator().as_str(),
        "right": expression_to_value(predicate.right())
    })
}

fn logical_predicate_to_value(kind: &str, predicate: &LogicalPredicate) -> Value {
    json!({
        "kind": kind,
        "operands": predicate
            .operands()
            .iter()
            .map(predicate_to_value)
            .collect::<Vec<_>>()
    })
}

fn not_predicate_to_value(predicate: &NotPredicate) -> Value {
    json!({
        "kind": "not",
        "operand": predicate_to_value(predicate.operand())
    })
}

fn is_null_predicate_to_value(predicate: &IsNullPredicate) -> Value {
    json!({
        "kind": "is_null",
        "expression": expression_to_value(predicate.expression()),
        "negated": predicate.negated()
    })
}

fn in_predicate_to_value(predicate: &InPredicate) -> Value {
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

fn between_predicate_to_value(predicate: &BetweenPredicate) -> Value {
    json!({
        "kind": "between",
        "expression": expression_to_value(predicate.expression()),
        "lower": expression_to_value(predicate.lower()),
        "upper": expression_to_value(predicate.upper()),
        "negated": predicate.negated()
    })
}

fn expression_to_value(expression: &Expression) -> Value {
    match expression {
        Expression::Column(expression) => column_expression_to_value(expression),
        Expression::Literal(expression) => literal_expression_to_value(expression),
        Expression::Function(expression) => function_expression_to_value(expression),
        Expression::Unary(expression) => unary_expression_to_value(expression),
        Expression::Binary(expression) => binary_expression_to_value(expression),
        Expression::Unknown(semantic) => unknown_semantic_to_value(semantic),
        Expression::Unsupported(semantic) => unsupported_semantic_to_value(semantic),
    }
}

fn column_expression_to_value(expression: &ColumnExpression) -> Value {
    json!({
        "kind": "column",
        "relation": expression.relation(),
        "name": expression.name()
    })
}

fn literal_expression_to_value(expression: &LiteralExpression) -> Value {
    json!({
        "kind": "literal",
        "type": expression.literal_type().as_str(),
        "value": literal_value_to_value(expression.value())
    })
}

fn literal_value_to_value(value: &LiteralValue) -> Value {
    match value {
        LiteralValue::Null => Value::Null,
        LiteralValue::Boolean(value) => Value::Bool(*value),
        LiteralValue::Number(value) => serde_json::from_str(value)
            .expect("numeric literal text is validated before protocol construction"),
        LiteralValue::Text(value) => Value::String(value.clone()),
    }
}

fn function_expression_to_value(expression: &FunctionExpression) -> Value {
    json!({
        "kind": "function",
        "name": expression.name(),
        "arguments": expression
            .arguments()
            .iter()
            .map(expression_to_value)
            .collect::<Vec<_>>(),
        "distinct": expression.distinct()
    })
}

fn unary_expression_to_value(expression: &UnaryExpression) -> Value {
    json!({
        "kind": "unary",
        "operator": expression.operator().as_str(),
        "operand": expression_to_value(expression.operand())
    })
}

fn binary_expression_to_value(expression: &BinaryExpression) -> Value {
    json!({
        "kind": "binary",
        "operator": expression.operator().as_str(),
        "left": expression_to_value(expression.left()),
        "right": expression_to_value(expression.right())
    })
}

fn unknown_semantic_to_value(semantic: &UnknownSemantic) -> Value {
    json!({
        "kind": "unknown",
        "reason": semantic.reason()
    })
}

fn unsupported_semantic_to_value(semantic: &UnsupportedSemantic) -> Value {
    json!({
        "kind": "unsupported",
        "feature": semantic.feature(),
        "reason": semantic.reason()
    })
}

fn unsupported_statement_to_value(statement: &UnsupportedStatement) -> Value {
    let diagnostics = statement
        .diagnostics()
        .iter()
        .map(diagnostic_to_value)
        .collect::<Vec<_>>();

    json!({
        "kind": "unsupported",
        "category": statement.category(),
        "diagnostics": diagnostics
    })
}

fn diagnostic_to_value(diagnostic: &Diagnostic) -> Value {
    json!({
        "severity": diagnostic.severity().as_str(),
        "code": diagnostic.code(),
        "area": diagnostic.area().as_str(),
        "message": diagnostic.message()
    })
}
