//! Scalar literals, functions, windows, and unsupported diagnostics.

use super::*;

pub(super) fn column_expression_to_value(expression: &ColumnExpression) -> Value {
    json!({
        "kind": "column",
        "relation": expression.relation(),
        "name": expression.name()
    })
}

pub(super) fn literal_expression_to_value(expression: &LiteralExpression) -> Value {
    json!({
        "kind": "literal",
        "type": expression.literal_type().as_str(),
        "value": literal_value_to_value(expression.value())
    })
}

pub(super) fn literal_value_to_value(value: &LiteralValue) -> Value {
    match value {
        LiteralValue::Null => Value::Null,
        LiteralValue::Boolean(value) => Value::Bool(*value),
        LiteralValue::Number(value) => serde_json::from_str(value)
            .expect("numeric literal text is validated before protocol construction"),
        LiteralValue::Text(value) => Value::String(value.clone()),
    }
}

pub(super) fn function_expression_to_value(expression: &FunctionExpression) -> Value {
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

pub(super) fn aggregate_function_expression_to_value(
    expression: &AggregateFunctionExpression,
) -> Value {
    json!({
        "kind": "aggregate_function",
        "name": expression.name(),
        "arguments": expression.arguments().iter().map(aggregate_argument_to_value).collect::<Vec<_>>(),
        "distinct": expression.distinct(),
        "filter": expression.filter().map_or(Value::Null, predicate_to_value)
    })
}

pub(super) fn aggregate_argument_to_value(argument: &AggregateArgument) -> Value {
    match argument {
        AggregateArgument::Expression(expression) => json!({
            "kind": "expression",
            "expression": expression_to_value(expression)
        }),
        AggregateArgument::Wildcard => json!({ "kind": "wildcard" }),
        AggregateArgument::QualifiedWildcard(qualifier) => json!({
            "kind": "qualified_wildcard",
            "qualifier": qualifier
        }),
    }
}

pub(super) fn window_function_expression_to_value(expression: &WindowFunctionExpression) -> Value {
    json!({
        "kind": "window_function",
        "function": function_expression_to_value(expression.function()),
        "window": window_specification_to_value(expression.window())
    })
}

pub(super) fn window_specification_to_value(window: &WindowSpecification) -> Value {
    json!({
        "name": window.name(),
        "partition_by": window
            .partition_by()
            .iter()
            .map(expression_to_value)
            .collect::<Vec<_>>(),
        "order_by": window
            .order_by()
            .iter()
            .map(window_order_expression_to_value)
            .collect::<Vec<_>>(),
        "frame": window.frame().map_or(Value::Null, window_frame_to_value)
    })
}

pub(super) fn window_order_expression_to_value(order: &WindowOrderExpression) -> Value {
    json!({
        "expression": expression_to_value(order.expression()),
        "ascending": order.ascending(),
        "nulls_first": order.nulls_first()
    })
}

pub(super) fn window_frame_to_value(frame: &WindowFrame) -> Value {
    json!({
        "units": frame.units().as_str(),
        "start": window_frame_bound_to_value(frame.start_bound()),
        "end": window_frame_bound_to_value(frame.end_bound())
    })
}

pub(super) fn window_frame_bound_to_value(bound: &WindowFrameBound) -> Value {
    match bound {
        WindowFrameBound::CurrentRow => json!({ "kind": "current_row" }),
        WindowFrameBound::UnboundedPreceding => json!({ "kind": "unbounded_preceding" }),
        WindowFrameBound::Preceding(offset) => json!({
            "kind": "preceding",
            "offset": expression_to_value(offset)
        }),
        WindowFrameBound::UnboundedFollowing => json!({ "kind": "unbounded_following" }),
        WindowFrameBound::Following(offset) => json!({
            "kind": "following",
            "offset": expression_to_value(offset)
        }),
    }
}

pub(super) fn unary_expression_to_value(expression: &UnaryExpression) -> Value {
    json!({
        "kind": "unary",
        "operator": expression.operator().as_str(),
        "operand": expression_to_value(expression.operand())
    })
}

pub(super) fn binary_expression_to_value(expression: &BinaryExpression) -> Value {
    json!({
        "kind": "binary",
        "operator": expression.operator().as_str(),
        "left": expression_to_value(expression.left()),
        "right": expression_to_value(expression.right())
    })
}

pub(super) fn unknown_semantic_to_value(semantic: &UnknownSemantic) -> Value {
    json!({
        "kind": "unknown",
        "reason": semantic.reason()
    })
}

pub(super) fn unsupported_semantic_to_value(semantic: &UnsupportedSemantic) -> Value {
    json!({
        "kind": "unsupported",
        "feature": semantic.feature(),
        "reason": semantic.reason()
    })
}

pub(super) fn unsupported_statement_to_value(statement: &UnsupportedStatement) -> Value {
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

pub(super) fn diagnostic_to_value(diagnostic: &Diagnostic) -> Value {
    json!({
        "severity": diagnostic.severity().as_str(),
        "code": diagnostic.code(),
        "area": diagnostic.area().as_str(),
        "message": diagnostic.message()
    })
}
