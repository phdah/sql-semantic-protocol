//! Query statements, DML effects, and MERGE write operations.

use super::*;

pub(super) fn statement_to_value(statement: &ProtocolStatement) -> Value {
    match statement {
        ProtocolStatement::Query(statement) => query_statement_to_value(statement),
        ProtocolStatement::Unsupported(statement) => unsupported_statement_to_value(statement),
    }
}

pub(super) fn query_statement_to_value(statement: &QueryStatement) -> Value {
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

pub(super) fn layer_write_effect_to_value(write: &LayerWriteStateEffect) -> Value {
    json!({
        "layer_id": write.layer_id(),
        "target": write.target(),
        "sources": write.sources(),
        "state_effect": write_state_effect_to_value(write.effect()),
        "target_constraints": write.target_constraints()
            .map_or(Value::Null, relation_constraint_set_to_value),
    })
}

pub(super) fn write_state_effect_to_value(effect: &crate::WriteStateEffect) -> Value {
    json!({
        "initial_state": "caller_supplied",
        "target_columns": effect.target_columns(),
        "match_condition": effect.match_condition().map_or(Value::Null, predicate_to_value),
        "cardinality_rule": effect.cardinality_rule().as_str(),
        "affected_rows": { "minimum": effect.affected_rows().minimum(), "maximum": effect.affected_rows().maximum() },
        "post_state": match effect.post_state() { WritePostState::Empty => "empty", WritePostState::ApplyToInitial => "apply_to_initial" },
        "idempotence": match effect.idempotence() { WriteIdempotence::Proven => "proven", WriteIdempotence::Unproven => "unproven" },
        "branches": effect.branches().iter().map(|branch| json!({
            "match_kind": branch.match_kind().map(|kind| kind.as_str()),
            "predicate": branch.predicate().map_or(Value::Null, predicate_to_value),
            "domains": branch.domains().iter().map(column_domain_to_value).collect::<Vec<_>>(),
            "action": match branch.action() {
                WriteEffectAction::InsertQuery => json!({"kind": "insert_query"}),
                WriteEffectAction::Mutation(action) => merge_action_to_value(action),
            }
        })).collect::<Vec<_>>(),
        "residual_reasons": effect.reasons().iter().map(|reason| reason.as_str()).collect::<Vec<_>>(),
    })
}

pub(super) fn write_operation_to_value(write: &WriteOperation) -> Value {
    let state_effect = write
        .state_effect()
        .map(|effect| write_state_effect_to_value(&effect));
    json!({
        "target": write.target(),
        "kind": write.kind().as_str(),
        "target_columns": write.target_columns(),
        "match_condition": write.match_condition().map_or(Value::Null, predicate_to_value),
        "merge_clauses": write
            .merge_clauses()
            .iter()
            .map(merge_clause_to_value)
            .collect::<Vec<_>>(),
        "selection": write.selection().map_or(Value::Null, predicate_to_value),
        "selection_domains": write.selection_domains().iter().map(column_domain_to_value).collect::<Vec<_>>(),
        "assignments": write.assignments().iter().map(|assignment| json!({
            "target": assignment.target(),
            "value": write_value_to_value(assignment.value())
        })).collect::<Vec<_>>(),
        "state_effect": state_effect
    })
}

pub(super) fn merge_clause_to_value(clause: &MergeClause) -> Value {
    json!({
        "match_kind": clause.match_kind().as_str(),
        "predicate": clause.predicate().map_or(Value::Null, predicate_to_value),
        "domains": clause.domains().iter().map(column_domain_to_value).collect::<Vec<_>>(),
        "action": merge_action_to_value(clause.action())
    })
}

pub(super) fn merge_action_to_value(action: &MergeAction) -> Value {
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

pub(super) fn write_value_to_value(value: &WriteValue) -> Value {
    json!({
        "expression": expression_to_value(value.expression()),
        "domain": value_domain_to_value(value.domain())
    })
}
