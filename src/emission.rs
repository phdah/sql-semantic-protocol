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
use crate::constraints::{
    ConstraintDiagnostic, ConstraintValue, RelationConstraint, RelationConstraintSet,
};
use crate::data_type::DataType;
use crate::protocol::{
    AggregateArgument, AggregateFunctionExpression, Aggregation, BetweenPredicate,
    BinaryExpression, Bound, CaseExpression, CaseSourceDomains, ColumnDomain, ColumnExpression,
    ColumnRef, ComparisonPredicate, ConditionExactness, Diagnostic, ExistsPredicate, Expression,
    FunctionExpression, GroupBy, GroupingExpression, InPredicate, InSubqueryPredicate,
    IsNullPredicate, Join, LineageSource, LiteralExpression, LiteralValue, LogicalPredicate,
    MergeAction, MergeClause, NotPredicate, Output, OutputColumn, Predicate, Predicates, Protocol,
    ProtocolStatement, QueryStatement, RelationRef, ScalarSubqueryExpression, SetOperand,
    SetOperation, SourceRelation, SubquerySemantics, UnaryExpression, UnknownSemantic,
    UnsupportedSemantic, UnsupportedStatement, ValueDomain, ValueRange, WindowFrame,
    WindowFrameBound, WindowFunctionExpression, WindowOrderExpression, WindowSpecification,
    WriteOperation, WriteValue,
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

    let mut value = json!({
        "protocol_version": bundle.protocol_version(),
        "inputs": inputs,
        "layers": layers,
        "graph": analysis_graph_to_value(bundle.graph())
    });

    if !bundle.comparison_declarations().is_empty() {
        value["declared_comparison_assumptions"] = json!(bundle
            .comparison_declarations()
            .iter()
            .map(|assumption| assumption.as_str())
            .collect::<Vec<_>>());
    }

    if !bundle.source_schemas().is_empty() {
        value["source_schemas"] = json!(bundle
            .source_schemas()
            .iter()
            .map(|schema| {
                let mut value = json!({
                    "relation": schema.relation(),
                    "columns": schema.columns().iter().map(|column| {
                        let mut column_value = json!({
                            "name": column.name(),
                            "data_type": data_type_to_value(column.data_type())
                        });
                        if let Some(zone) = column.timestamp_zone() {
                            column_value["timestamp_zone"] = json!(zone.as_str());
                        }
                        column_value
                    }).collect::<Vec<_>>()
                });
                if let Some(source_kind) = schema.source_kind() {
                    value["source_kind"] = json!(source_kind.as_str());
                }
                value
            })
            .collect::<Vec<_>>());
    }

    if !bundle.relation_constraints().is_empty() {
        value["relation_constraints"] = Value::Array(
            bundle
                .relation_constraints()
                .iter()
                .map(relation_constraint_set_to_value)
                .collect(),
        );
    }

    if !bundle.constraint_diagnostics().is_empty() {
        value["constraint_diagnostics"] = Value::Array(
            bundle
                .constraint_diagnostics()
                .iter()
                .map(constraint_diagnostic_to_value)
                .collect(),
        );
    }

    value
}

fn relation_constraint_set_to_value(set: &RelationConstraintSet) -> Value {
    let constraints = set
        .constraints()
        .iter()
        .map(|constraint| {
            let evidence = constraint
                .evidence()
                .iter()
                .map(|evidence| {
                    json!({
                        "source_kind": evidence.provenance().source_kind().as_str(),
                        "source_id": evidence.provenance().source_id(),
                        "enforcement": evidence.enforcement().as_str()
                    })
                })
                .collect::<Vec<_>>();
            match constraint {
                RelationConstraint::PrimaryKey(key) => json!({
                    "kind": "primary_key",
                    "columns": key.columns(),
                    "evidence": evidence
                }),
                RelationConstraint::UniqueKey(key) => json!({
                    "kind": "unique_key",
                    "columns": key.columns(),
                    "evidence": evidence
                }),
                RelationConstraint::ForeignKey(key) => json!({
                    "kind": "foreign_key",
                    "columns": key.columns(),
                    "referenced_relation": key.referenced_relation(),
                    "referenced_columns": key.referenced_columns(),
                    "evidence": evidence
                }),
                RelationConstraint::NotNull(constraint) => json!({
                    "kind": "not_null",
                    "column": constraint.column(),
                    "evidence": evidence
                }),
                RelationConstraint::AcceptedValues(constraint) => json!({
                    "kind": "accepted_values",
                    "column": constraint.column(),
                    "values": constraint
                        .values()
                        .iter()
                        .map(constraint_value_to_value)
                        .collect::<Vec<_>>(),
                    "quote": constraint.quote(),
                    "evidence": evidence
                }),
            }
        })
        .collect::<Vec<_>>();

    let mut value = json!({
        "relation": set.relation(),
        "constraints": constraints
    });
    if !set.diagnostics().is_empty() {
        value["diagnostics"] = json!(set
            .diagnostics()
            .iter()
            .map(constraint_diagnostic_to_value)
            .collect::<Vec<_>>());
    }
    value
}

fn constraint_diagnostic_to_value(diagnostic: &ConstraintDiagnostic) -> Value {
    json!({
        "code": diagnostic.code(),
        "message": diagnostic.message()
    })
}

fn constraint_value_to_value(value: &ConstraintValue) -> Value {
    match value {
        ConstraintValue::Null => json!({ "type": "null", "value": null }),
        ConstraintValue::Boolean(value) => json!({ "type": "boolean", "value": value }),
        ConstraintValue::Integer(value) => json!({ "type": "integer", "value": value }),
        ConstraintValue::UnsignedInteger(value) => {
            json!({ "type": "unsigned_integer", "value": value })
        }
        ConstraintValue::Number(value) => json!({ "type": "number", "value": value }),
        ConstraintValue::String(value) => json!({ "type": "string", "value": value }),
    }
}

fn data_type_to_value(data_type: &DataType) -> Value {
    match data_type {
        DataType::Boolean => json!({ "kind": "boolean" }),
        DataType::SignedInteger { bits } => {
            json!({ "kind": "signed_integer", "bits": bits })
        }
        DataType::UnsignedInteger { bits } => {
            json!({ "kind": "unsigned_integer", "bits": bits })
        }
        DataType::Decimal { precision, scale } => {
            json!({ "kind": "decimal", "precision": precision, "scale": scale })
        }
        DataType::FloatingPoint { bits } => {
            json!({ "kind": "floating_point", "bits": bits })
        }
        DataType::String { length, fixed } => {
            json!({ "kind": "string", "length": length, "fixed": fixed })
        }
        DataType::Binary { length, fixed } => {
            json!({ "kind": "binary", "length": length, "fixed": fixed })
        }
        DataType::Date => json!({ "kind": "date" }),
        DataType::Time { precision } => {
            json!({ "kind": "time", "precision": precision })
        }
        DataType::Timestamp { precision } => {
            json!({ "kind": "timestamp", "precision": precision })
        }
        DataType::Interval => json!({ "kind": "interval" }),
        DataType::Uuid => json!({ "kind": "uuid" }),
        DataType::Json => json!({ "kind": "json" }),
        DataType::BitString { length } => {
            json!({ "kind": "bit_string", "length": length })
        }
        DataType::Array { element, length } => json!({
            "kind": "array",
            "element": element.as_deref().map(data_type_to_value),
            "length": length
        }),
        DataType::Map { key, value } => json!({
            "kind": "map",
            "key": data_type_to_value(key),
            "value": data_type_to_value(value)
        }),
        DataType::Struct { fields } => json!({
            "kind": "struct",
            "fields": fields
                .iter()
                .map(|field| json!({
                    "name": field.name(),
                    "data_type": data_type_to_value(field.data_type())
                }))
                .collect::<Vec<_>>()
        }),
        DataType::Union { fields } => json!({
            "kind": "union",
            "fields": fields
                .iter()
                .map(|field| json!({
                    "name": field.name(),
                    "data_type": data_type_to_value(field.data_type())
                }))
                .collect::<Vec<_>>()
        }),
        DataType::Enum { values } => json!({
            "kind": "enum",
            "values": values
                .iter()
                .map(|value| json!({
                    "name": value.name(),
                    "value": value.value()
                }))
                .collect::<Vec<_>>()
        }),
        DataType::Set { values } => json!({
            "kind": "set",
            "values": values
        }),
        DataType::Table { name, fields } => json!({
            "kind": "table",
            "name": name,
            "fields": fields
                .iter()
                .map(|field| json!({
                    "name": field.name(),
                    "data_type": data_type_to_value(field.data_type())
                }))
                .collect::<Vec<_>>()
        }),
        DataType::Geometry { kind } => json!({
            "kind": "geometry",
            "geometry_kind": kind
        }),
        DataType::Regclass => json!({ "kind": "regclass" }),
        DataType::TextSearchVector => json!({ "kind": "text_search_vector" }),
        DataType::TextSearchQuery => json!({ "kind": "text_search_query" }),
        DataType::Nullable(inner) => json!({
            "kind": "nullable",
            "inner": data_type_to_value(inner)
        }),
        DataType::Any => json!({ "kind": "any" }),
        DataType::Unspecified => json!({ "kind": "unspecified" }),
        DataType::Trigger => json!({ "kind": "trigger" }),
        DataType::Custom { name, modifiers } => json!({
            "kind": "custom",
            "name": name,
            "modifiers": modifiers
        }),
    }
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
    let mut value = json!({
        "status": "resolved",
        "dependencies": semantics.dependencies(),
        "column_domains": semantics
            .column_domains()
            .iter()
            .map(column_domain_to_value)
            .collect::<Vec<_>>(),
        "join_equalities": semantics
            .join_equalities()
            .iter()
            .map(composed_join_equality_to_value)
            .collect::<Vec<_>>(),
        "condition_exactness": condition_exactness_to_value(semantics.condition_exactness()),
        "output": output_to_value(semantics.output()),
        "diagnostics": semantics
            .diagnostics()
            .iter()
            .map(composition_diagnostic_to_value)
            .collect::<Vec<_>>()
    });
    if !semantics.join_witnesses().is_empty() {
        value["join_witnesses"] = json!(semantics
            .join_witnesses()
            .iter()
            .map(join_witness_to_value)
            .collect::<Vec<_>>());
    }
    if !semantics.window_witnesses().is_empty() {
        value["window_witnesses"] = json!(semantics
            .window_witnesses()
            .iter()
            .map(|item| json!({
                "origin_layer_id": item.origin_layer_id(),
                "boundary_kind": item.boundary_kind().as_str(),
                "witness": window_witness_to_value(item.witness())
            }))
            .collect::<Vec<_>>());
    }
    if !semantics.group_witnesses().is_empty() {
        value["group_witnesses"] = json!(semantics
            .group_witnesses()
            .iter()
            .map(|item| json!({
                "origin_layer_id": item.origin_layer_id(),
                "boundary_kind": item.boundary_kind().as_str(),
                "witness": group_witness_to_value(item.witness())
            }))
            .collect::<Vec<_>>());
    }
    if !semantics.boolean_witnesses().is_empty() {
        value["boolean_witnesses"] = json!(semantics.boolean_witnesses().iter().map(|item| json!({
            "origin_layer_id": item.origin_layer_id(),
            "boundary_kind": item.boundary_kind().as_str(),
            "witness": boolean_witness_to_value(item.witness())
        })).collect::<Vec<_>>());
    }
    if !semantics.subquery_witnesses().is_empty() {
        value["subquery_witnesses"] = json!(semantics
            .subquery_witnesses()
            .iter()
            .map(|item| json!({
                "origin_layer_id": item.origin_layer_id(),
                "boundary_kind": item.boundary_kind().as_str(),
                "witness": subquery_membership_witness_to_value(item.witness())
            }))
            .collect::<Vec<_>>());
    }
    if !semantics.set_operations().is_empty() {
        value["set_operations"] = json!(semantics
            .set_operations()
            .iter()
            .map(|item| json!({
                "origin_layer_id": item.origin_layer_id(),
                "operation": set_operation_to_value(item.operation())
            }))
            .collect::<Vec<_>>());
    }
    value
}

fn join_witness_to_value(witness: &crate::JoinWitness) -> Value {
    let endpoint = |column: &crate::ComposedJoinColumn| {
        json!({
            "relation": column.relation(),
            "column": column.column(),
            "relation_instance": column.relation_instance()
        })
    };
    json!({
        "origin_layer_id": witness.origin_layer_id(),
        "join_kind": witness.kind().as_str(),
        "comparison": witness.comparison().map(|op| op.as_str()),
        "left": witness.left().map(endpoint),
        "right": witness.right().map(endpoint),
        "unknown_comparison_is_match": false,
        "qualifying": join_witness_direction_to_value(witness.qualifying()),
        "rejected": join_witness_direction_to_value(witness.rejected())
    })
}

fn join_witness_direction_to_value(direction: &crate::JoinWitnessDirection) -> Value {
    match direction {
        crate::JoinWitnessDirection::Impossible => json!({"status": "impossible"}),
        crate::JoinWitnessDirection::Residual { reason } => {
            json!({"status": "residual", "reason": reason})
        }
        crate::JoinWitnessDirection::Exact(cases) => json!({
            "status": "exact",
            "cases": cases.iter().map(|case| json!({
                "shape": case.shape().as_str(),
                "min_matches": case.shape().min_matches(),
                "max_matches": case.shape().max_matches(),
                "null_extended_side": case.null_extended_side().map(|side| side.as_str())
            })).collect::<Vec<_>>()
        }),
    }
}

fn composed_join_equality_to_value(equality: &crate::ComposedJoinEquality) -> Value {
    json!({
        "left": {
            "relation": equality.left().relation(),
            "column": equality.left().column(),
            "relation_instance": equality.left().relation_instance()
        },
        "right": {
            "relation": equality.right().relation(),
            "column": equality.right().column(),
            "relation_instance": equality.right().relation_instance()
        },
        "join_kind": equality.join_kind().as_str(),
        "origin_layer_id": equality.origin_layer_id()
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
        "condition_exactness": condition_exactness_to_value(statement.condition_exactness()),
        "output": output_to_value(statement.output()),
        "diagnostics": diagnostics
    });

    if let Some(aggregation) = statement.aggregation() {
        value["aggregation"] = aggregation_to_value(aggregation);
    }

    if let Some(group_witness) = statement.group_witness() {
        value["group_witness"] = group_witness_to_value(group_witness);
    }
    if let Some(witness) = statement.boolean_witness() {
        value["boolean_witness"] = boolean_witness_to_value(witness);
    }
    if !statement.subquery_witnesses().is_empty() {
        value["subquery_witnesses"] = json!(statement
            .subquery_witnesses()
            .iter()
            .map(subquery_membership_witness_to_value)
            .collect::<Vec<_>>());
    }
    if let Some(window_witness) = statement.window_witness() {
        value["window_witness"] = window_witness_to_value(window_witness);
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
                .map(|row| row.iter().map(write_value_to_value).collect::<Vec<_>>())
                .collect::<Vec<_>>()
        }),
        MergeAction::Update { assignments } => json!({
            "kind": "update",
            "assignments": assignments
                .iter()
                .map(|assignment| json!({
                    "target": assignment.target(),
                    "value": write_value_to_value(assignment.value())
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

fn write_value_to_value(value: &WriteValue) -> Value {
    json!({
        "expression": expression_to_value(value.expression()),
        "domain": value_domain_to_value(value.domain())
    })
}

fn boolean_witness_to_value(witness: &crate::BooleanWitness) -> Value {
    json!({
        "source_relation": witness.source_relation(),
        "condition": boolean_constraint_to_value(witness.condition()),
        "qualifying": boolean_witness_direction_to_value(witness.qualifying()),
        "rejected": boolean_witness_direction_to_value(witness.rejected())
    })
}

fn boolean_witness_direction_to_value(direction: &crate::BooleanWitnessDirection) -> Value {
    match direction {
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

fn boolean_constraint_to_value(constraint: &crate::BooleanRowConstraint) -> Value {
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
        crate::BooleanRowConstraint::IntegerComparison { column, operator, literal } => json!({
            "kind": "integer_comparison", "column": column_ref_to_value(column),
            "operator": operator.as_str(), "literal": literal
        }),
        crate::BooleanRowConstraint::Residual { reason } => json!({
            "kind": "residual", "reason": reason
        }),
    }
}

fn subquery_membership_witness_to_value(
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

fn subquery_membership_direction_to_value(
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

fn window_witness_to_value(witness: &crate::window_witness::WindowWitness) -> Value {
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

fn window_direction_to_value(direction: &crate::window_witness::WindowWitnessDirection) -> Value {
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

fn group_witness_to_value(witness: &crate::group_witness::GroupWitness) -> Value {
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

fn group_witness_direction_to_value(
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
            })).collect::<Vec<_>>(),
            "qualifying_witness": set_witness_direction_to_value(&qualifying),
            "non_qualifying_witness": set_witness_direction_to_value(&non_qualifying)
        }
    })
}

fn set_witness_direction_to_value(direction: &crate::protocol::SetWitnessDirection) -> Value {
    match direction {
        crate::protocol::SetWitnessDirection::Residual { reason, origin } => json!({
            "status": "residual",
            "reason": reason,
            "origin": origin
        }),
        crate::protocol::SetWitnessDirection::Exact(cases) => json!({
            "status": "exact",
            "cases": cases.iter().map(|case| json!({
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
            })).collect::<Vec<_>>()
        }),
    }
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

fn case_source_domains_to_value(domains: &CaseSourceDomains) -> Value {
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

fn condition_exactness_to_value(exactness: &ConditionExactness) -> Value {
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

fn residual_condition_to_value(residual: &crate::protocol::ResidualCondition) -> Value {
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
