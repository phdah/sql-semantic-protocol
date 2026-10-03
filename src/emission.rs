//! Protocol serialization boundary.
//!
//! Serialization depends only on parser-independent protocol values. It deliberately contains no
//! sqlparser AST handling.

use serde_json::{json, Value};

use crate::bundle::{
    AnalysisBundle, AnalysisGraph, ComposedSemantics, CompositionDiagnostic, DatasetRef,
    GraphComponent, GraphEdge, ResolvedComposedSemantics, SqlInputSource, TransformationLayer,
    UnresolvedComposedSemantics,
};
use crate::protocol::{
    AggregateArgument, AggregateFunctionExpression, Aggregation, BetweenPredicate,
    BinaryExpression, Bound, CaseExpression, ColumnDomain, ColumnExpression, ColumnRef,
    ComparisonPredicate, Diagnostic, ExistsPredicate, Expression, FunctionExpression, GroupBy,
    GroupingExpression, InPredicate, InSubqueryPredicate, IsNullPredicate, Join, LineageSource,
    LiteralExpression, LiteralValue, LogicalPredicate, MergeAction, MergeClause, NotPredicate,
    Output, OutputColumn, Predicate, Predicates, Protocol, ProtocolStatement, QueryStatement,
    RelationRef, ScalarSubqueryExpression, SetOperand, SetOperation, SourceRelation,
    SubquerySemantics, UnaryExpression, UnknownSemantic, UnsupportedSemantic, UnsupportedStatement,
    ValueDomain, ValueRange, WindowFrame, WindowFrameBound, WindowFunctionExpression,
    WindowOrderExpression, WindowSpecification, WriteOperation,
};

/// Serialize single-input analysis using the one active protocol document shape.
pub fn to_json(protocol: &Protocol) -> String {
    bundle_to_value(&AnalysisBundle::from_protocol(protocol)).to_string()
}

/// Serialize an analysis bundle using the one active protocol document shape.
///
/// Local transformation layers, transitive composed semantics, and the relation dependency graph
/// are serialized without exposing parser-specific values.
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
        "graph": analysis_graph_to_value(bundle.graph())
    })
}

fn transformation_layer_to_value(layer: &TransformationLayer) -> Value {
    let mut value = json!({
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
        "composed_semantics": composed_semantics_to_value(layer.composed_semantics())
    });

    if let Some(write_kind) = layer.write_kind() {
        value["write_kind"] = json!(write_kind.as_str());
    }

    value
}

fn composed_semantics_to_value(semantics: &ComposedSemantics) -> Value {
    match semantics {
        ComposedSemantics::Resolved(semantics) => resolved_composed_semantics_to_value(semantics),
        ComposedSemantics::Unresolved(semantics) => {
            unresolved_composed_semantics_to_value(semantics)
        }
    }
}

fn resolved_composed_semantics_to_value(semantics: &ResolvedComposedSemantics) -> Value {
    json!({
        "status": "resolved",
        "dependencies": semantics.dependencies(),
        "column_domains": semantics
            .column_domains()
            .iter()
            .map(column_domain_to_value)
            .collect::<Vec<_>>(),
        "output": output_to_value(semantics.output()),
        "diagnostics": semantics
            .diagnostics()
            .iter()
            .map(composition_diagnostic_to_value)
            .collect::<Vec<_>>()
    })
}

fn unresolved_composed_semantics_to_value(semantics: &UnresolvedComposedSemantics) -> Value {
    json!({
        "status": "unresolved",
        "reason": semantics.reason().as_str(),
        "diagnostics": semantics
            .diagnostics()
            .iter()
            .map(composition_diagnostic_to_value)
            .collect::<Vec<_>>()
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

fn analysis_graph_to_value(graph: &AnalysisGraph) -> Value {
    json!({
        "edges": graph
            .edges()
            .iter()
            .map(graph_edge_to_value)
            .collect::<Vec<_>>(),
        "components": graph
            .components()
            .iter()
            .map(graph_component_to_value)
            .collect::<Vec<_>>(),
        "diagnostics": graph
            .diagnostics()
            .iter()
            .map(composition_diagnostic_to_value)
            .collect::<Vec<_>>()
    })
}

fn graph_edge_to_value(edge: &GraphEdge) -> Value {
    json!({
        "consumer_layer_id": edge.consumer_layer_id(),
        "relation": edge.relation(),
        "resolution": edge.resolution().as_str(),
        "producer_layer_ids": edge.producer_layer_ids()
    })
}

fn graph_component_to_value(component: &GraphComponent) -> Value {
    json!({
        "id": component.id(),
        "layer_ids": component.layer_ids(),
        "final_outcomes": component
            .final_outcomes()
            .iter()
            .map(dataset_ref_to_value)
            .collect::<Vec<_>>(),
        "diagnostics": component
            .diagnostics()
            .iter()
            .map(composition_diagnostic_to_value)
            .collect::<Vec<_>>()
    })
}

fn composition_diagnostic_to_value(diagnostic: &CompositionDiagnostic) -> Value {
    let mut value = json!({
        "severity": diagnostic.severity().as_str(),
        "code": diagnostic.code(),
        "message": diagnostic.message()
    });

    if let Some(input_id) = diagnostic.input_id() {
        value["input_id"] = json!(input_id);
    }
    if let Some(layer_id) = diagnostic.layer_id() {
        value["layer_id"] = json!(layer_id);
    }
    if let Some(relation) = diagnostic.relation() {
        value["relation"] = json!(relation);
    }

    value
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

    let mut value = json!({
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
    });

    if let Some(aggregation) = statement.aggregation() {
        value["aggregation"] = aggregation_to_value(aggregation);
    }

    if let Some(set_operation) = statement.set_operation() {
        value["set_operation"] = set_operation_to_value(set_operation);
    }

    if let Some(write) = statement.write() {
        value["write"] = write_operation_to_value(write);
    }

    value
}

fn write_operation_to_value(write: &WriteOperation) -> Value {
    json!({
        "target": write.target(),
        "kind": write.kind().as_str(),
        "target_columns": write.target_columns(),
        "match_condition": write.match_condition().map_or(Value::Null, predicate_to_value),
        "merge_clauses": write
            .merge_clauses()
            .iter()
            .map(merge_clause_to_value)
            .collect::<Vec<_>>()
    })
}

fn merge_clause_to_value(clause: &MergeClause) -> Value {
    json!({
        "match_kind": clause.match_kind().as_str(),
        "predicate": clause.predicate().map_or(Value::Null, predicate_to_value),
        "action": merge_action_to_value(clause.action())
    })
}

fn merge_action_to_value(action: &MergeAction) -> Value {
    match action {
        MergeAction::Insert { columns, values } => json!({
            "kind": "insert",
            "columns": columns,
            "values": values
                .iter()
                .map(|row| row.iter().map(expression_to_value).collect::<Vec<_>>())
                .collect::<Vec<_>>()
        }),
        MergeAction::Update { assignments } => json!({
            "kind": "update",
            "assignments": assignments
                .iter()
                .map(|assignment| json!({
                    "target": assignment.target(),
                    "value": expression_to_value(assignment.value())
                }))
                .collect::<Vec<_>>()
        }),
        MergeAction::Delete => json!({ "kind": "delete" }),
        MergeAction::Unsupported(semantic) => json!({
            "kind": "unsupported",
            "semantic": unsupported_semantic_to_value(semantic)
        }),
    }
}

fn aggregation_to_value(aggregation: &Aggregation) -> Value {
    json!({
        "distinct": aggregation.distinct(),
        "distinct_on": aggregation.distinct_on().iter().map(expression_to_value).collect::<Vec<_>>(),
        "group_by": aggregation.group_by().map_or(Value::Null, group_by_to_value)
    })
}

fn group_by_to_value(group_by: &GroupBy) -> Value {
    match group_by {
        GroupBy::All => json!({ "kind": "all" }),
        GroupBy::Expressions(expressions) => json!({
            "kind": "expressions",
            "expressions": expressions.iter().map(grouping_expression_to_value).collect::<Vec<_>>()
        }),
    }
}

fn grouping_expression_to_value(expression: &GroupingExpression) -> Value {
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

fn grouping_sets_to_value(kind: &str, sets: &[Vec<crate::protocol::Expression>]) -> Value {
    json!({
        "kind": kind,
        "sets": sets.iter()
            .map(|set| set.iter().map(expression_to_value).collect::<Vec<_>>())
            .collect::<Vec<_>>()
    })
}

fn set_operation_to_value(operation: &SetOperation) -> Value {
    json!({
        "operator": operation.operator().as_str(),
        "quantifier": operation.quantifier().as_str(),
        "left": set_operand_to_value(operation.left()),
        "right": set_operand_to_value(operation.right())
    })
}

fn set_operand_to_value(operand: &SetOperand) -> Value {
    match operand {
        SetOperand::Query => json!({ "kind": "query" }),
        SetOperand::Operation(operation) => {
            let mut value = set_operation_to_value(operation);
            value["kind"] = json!("set_operation");
            value
        }
    }
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
        "domain": value_domain_to_value(column.domain()),
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

fn exists_predicate_to_value(predicate: &ExistsPredicate) -> Value {
    json!({
        "kind": "exists",
        "subquery": subquery_semantics_to_value(predicate.subquery()),
        "negated": predicate.negated()
    })
}

fn in_subquery_predicate_to_value(predicate: &InSubqueryPredicate) -> Value {
    json!({
        "kind": "in_subquery",
        "expression": expression_to_value(predicate.expression()),
        "subquery": subquery_semantics_to_value(predicate.subquery()),
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
        Expression::AggregateFunction(expression) => {
            aggregate_function_expression_to_value(expression)
        }
        Expression::WindowFunction(expression) => window_function_expression_to_value(expression),
        Expression::Case(expression) => case_expression_to_value(expression),
        Expression::BooleanPredicate(predicate) => json!({
            "kind": "boolean_predicate",
            "predicate": predicate_to_value(predicate)
        }),
        Expression::Unary(expression) => unary_expression_to_value(expression),
        Expression::Binary(expression) => binary_expression_to_value(expression),
        Expression::ScalarSubquery(expression) => scalar_subquery_expression_to_value(expression),
        Expression::Unknown(semantic) => unknown_semantic_to_value(semantic),
        Expression::Unsupported(semantic) => unsupported_semantic_to_value(semantic),
    }
}

fn case_expression_to_value(expression: &CaseExpression) -> Value {
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
                "result": expression_to_value(branch.result())
            }))
            .collect::<Vec<_>>(),
        "else_result": expression
            .else_result()
            .map_or(Value::Null, expression_to_value)
    })
}

fn scalar_subquery_expression_to_value(expression: &ScalarSubqueryExpression) -> Value {
    json!({
        "kind": "scalar_subquery",
        "subquery": subquery_semantics_to_value(expression.subquery())
    })
}

fn subquery_semantics_to_value(subquery: &SubquerySemantics) -> Value {
    json!({
        "dependencies": subquery.dependencies(),
        "correlations": subquery
            .correlations()
            .iter()
            .map(lineage_source_to_value)
            .collect::<Vec<_>>(),
        "output": output_to_value(subquery.output()),
        "predicates": predicates_to_value(subquery.predicates()),
        "diagnostics": subquery
            .diagnostics()
            .iter()
            .map(diagnostic_to_value)
            .collect::<Vec<_>>()
    })
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

fn aggregate_function_expression_to_value(expression: &AggregateFunctionExpression) -> Value {
    json!({
        "kind": "aggregate_function",
        "name": expression.name(),
        "arguments": expression.arguments().iter().map(aggregate_argument_to_value).collect::<Vec<_>>(),
        "distinct": expression.distinct(),
        "filter": expression.filter().map_or(Value::Null, predicate_to_value)
    })
}

fn aggregate_argument_to_value(argument: &AggregateArgument) -> Value {
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

fn window_function_expression_to_value(expression: &WindowFunctionExpression) -> Value {
    json!({
        "kind": "window_function",
        "function": function_expression_to_value(expression.function()),
        "window": window_specification_to_value(expression.window())
    })
}

fn window_specification_to_value(window: &WindowSpecification) -> Value {
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

fn window_order_expression_to_value(order: &WindowOrderExpression) -> Value {
    json!({
        "expression": expression_to_value(order.expression()),
        "ascending": order.ascending(),
        "nulls_first": order.nulls_first()
    })
}

fn window_frame_to_value(frame: &WindowFrame) -> Value {
    json!({
        "units": frame.units().as_str(),
        "start": window_frame_bound_to_value(frame.start_bound()),
        "end": window_frame_bound_to_value(frame.end_bound())
    })
}

fn window_frame_bound_to_value(bound: &WindowFrameBound) -> Value {
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
