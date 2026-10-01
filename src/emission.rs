//! Protocol serialization boundary.
//!
//! Serialization depends only on parser-independent protocol values. It deliberately contains no
//! sqlparser AST handling.

use serde_json::{json, Value};

use crate::bundle::{AnalysisBundle, DatasetRef, SqlInputSource, TransformationLayer};
use crate::protocol::{
    BetweenPredicate, BinaryExpression, Bound, ColumnDomain, ColumnExpression, ColumnRef,
    ComparisonPredicate, Diagnostic, Expression, FunctionExpression, InPredicate, IsNullPredicate,
    Join, LineageSource, LiteralExpression, LiteralValue, LogicalPredicate, NotPredicate, Output,
    OutputColumn, Predicate, Predicates, Protocol, ProtocolStatement, QueryStatement, RelationRef,
    SourceRelation, UnaryExpression, UnknownSemantic, UnsupportedSemantic, UnsupportedStatement,
    ValueDomain, ValueRange,
};

/// Serialize single-input analysis using the one active protocol document shape.
pub fn to_json(protocol: &Protocol) -> String {
    bundle_to_value(&AnalysisBundle::from_protocol(protocol)).to_string()
}

/// Serialize an analysis bundle using the one active protocol document shape.
///
/// Local transformation layers and produced relation identities are populated. Cross-input graph
/// resolution and transitive semantic composition remain explicitly pending.
pub fn to_bundle_json(bundle: &AnalysisBundle) -> String {
    bundle_to_value(bundle).to_string()
}

fn bundle_to_value(bundle: &AnalysisBundle) -> Value {
    let inputs = bundle
        .inputs()
        .iter()
        .map(|input| {
            let source = match input.source() {
                SqlInputSource::Inline => json!({
                    "kind": "inline",
                    "label": Value::Null
                }),
                SqlInputSource::File { path } => json!({
                    "kind": "file",
                    "path": path
                }),
            };
            let statements = input
                .statements()
                .iter()
                .map(statement_to_value)
                .collect::<Vec<_>>();

            json!({
                "id": input.id(),
                "source": source,
                "dialect": input.dialect(),
                "statements": statements
            })
        })
        .collect::<Vec<_>>();

    let layers = bundle
        .layers()
        .iter()
        .map(transformation_layer_to_value)
        .collect::<Vec<_>>();

    json!({
        "protocol_version": bundle.protocol_version(),
        "inputs": inputs,
        "layers": layers,
        "graph": pending_graph_value()
    })
}

fn transformation_layer_to_value(layer: &TransformationLayer) -> Value {
    json!({
        "id": layer.id(),
        "statement": {
            "input_id": layer.input_id(),
            "statement_index": layer.statement_index()
        },
        "produces": layer
            .produces()
            .iter()
            .map(dataset_ref_to_value)
            .collect::<Vec<_>>(),
        "consumes": layer.consumes(),
        "composed_semantics": {
            "status": "unresolved",
            "reason": "unsupported",
            "diagnostics": [{
                "severity": "warning",
                "code": "semantic_composition_pending",
                "message": "cross-input semantic composition is not implemented yet",
                "input_id": layer.input_id(),
                "layer_id": layer.id()
            }]
        }
    })
}

fn dataset_ref_to_value(dataset: &DatasetRef) -> Value {
    match dataset {
        DatasetRef::Relation { name } => json!({
            "kind": "relation",
            "name": name
        }),
        DatasetRef::Anonymous { layer_id } => json!({
            "kind": "anonymous",
            "layer_id": layer_id
        }),
    }
}

fn pending_graph_value() -> Value {
    json!({
        "edges": [],
        "components": [],
        "diagnostics": [{
            "severity": "warning",
            "code": "multi_input_composition_pending",
            "message": "multi-input graph construction and semantic composition are not implemented yet"
        }]
    })
}

fn statement_to_value(statement: &ProtocolStatement) -> Value {
    match statement {
        ProtocolStatement::Query(statement) => query_statement_to_value(statement),
        ProtocolStatement::Unsupported(statement) => unsupported_statement_to_value(statement),
    }
}

fn query_statement_to_value(statement: &QueryStatement) -> Value {
    let sources = statement
        .sources()
        .iter()
        .map(source_relation_to_value)
        .collect::<Vec<_>>();
    let joins = statement
        .joins()
        .iter()
        .map(join_to_value)
        .collect::<Vec<_>>();
    let diagnostics = statement
        .diagnostics()
        .iter()
        .map(diagnostic_to_value)
        .collect::<Vec<_>>();

    json!({
        "kind": "query",
        "sources": sources,
        "dependencies": statement.dependencies(),
        "joins": joins,
        "predicates": predicates_to_value(statement.predicates()),
        "column_domains": statement
            .column_domains()
            .iter()
            .map(column_domain_to_value)
            .collect::<Vec<_>>(),
        "output": output_to_value(statement.output()),
        "diagnostics": diagnostics
    })
}

fn output_to_value(output: &Output) -> Value {
    json!({
        "columns": output
            .columns()
            .iter()
            .map(output_column_to_value)
            .collect::<Vec<_>>()
    })
}

fn output_column_to_value(column: &OutputColumn) -> Value {
    json!({
        "name": column.name(),
        "expression": expression_to_value(column.expression()),
        "lineage": column
            .lineage()
            .iter()
            .map(lineage_source_to_value)
            .collect::<Vec<_>>()
    })
}

fn lineage_source_to_value(source: &LineageSource) -> Value {
    json!({
        "relation": source.relation(),
        "column": source.column()
    })
}

fn column_domain_to_value(column_domain: &ColumnDomain) -> Value {
    json!({
        "column": column_ref_to_value(column_domain.column()),
        "domain": value_domain_to_value(column_domain.domain())
    })
}

fn column_ref_to_value(column: &ColumnRef) -> Value {
    json!({
        "relation": column.relation(),
        "name": column.name()
    })
}

fn value_domain_to_value(domain: &ValueDomain) -> Value {
    match domain {
        ValueDomain::Unbounded => json!({ "kind": "unbounded" }),
        ValueDomain::Ranges(domain) => json!({
            "kind": "ranges",
            "ranges": domain
                .ranges()
                .iter()
                .map(value_range_to_value)
                .collect::<Vec<_>>()
        }),
        ValueDomain::Set(domain) => json!({
            "kind": "set",
            "mode": domain.mode().as_str(),
            "values": domain
                .values()
                .iter()
                .map(literal_expression_to_value)
                .collect::<Vec<_>>()
        }),
        ValueDomain::Empty => json!({ "kind": "empty" }),
        ValueDomain::Unknown(domain) => json!({
            "kind": "unknown",
            "reason": domain.reason()
        }),
    }
}

fn value_range_to_value(range: &ValueRange) -> Value {
    json!({
        "lower": range.lower().map_or(Value::Null, bound_to_value),
        "upper": range.upper().map_or(Value::Null, bound_to_value)
    })
}

fn bound_to_value(bound: &Bound) -> Value {
    json!({
        "value": literal_expression_to_value(bound.value()),
        "inclusive": bound.inclusive()
    })
}

fn source_relation_to_value(source: &SourceRelation) -> Value {
    json!({
        "kind": "relation",
        "name": source.name(),
        "alias": source.alias()
    })
}

fn join_to_value(join: &Join) -> Value {
    json!({
        "kind": join.kind().as_str(),
        "left": relation_ref_to_value(join.left()),
        "right": relation_ref_to_value(join.right()),
        "condition": join
            .condition()
            .map_or(Value::Null, predicate_to_value)
    })
}

fn relation_ref_to_value(relation: &RelationRef) -> Value {
    json!({
        "relation": relation.relation(),
        "alias": relation.alias()
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
