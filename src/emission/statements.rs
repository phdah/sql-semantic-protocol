//! Query statements, DML effects, and MERGE write operations.

use super::*;

pub(super) fn statement_to_value(statement: &ProtocolStatement) -> Value {
    match statement {
        ProtocolStatement::Query(statement) => query_statement_to_value(statement),
        ProtocolStatement::Unsupported(statement) => unsupported_statement_to_value(statement),
    }
}

// Canonical finite transformations carry the complete original literal bag
// and row contributors, not inferred transitive source facts.
fn finite_table_to_value(table: &crate::protocol::FiniteTable) -> Value {
    let mut value = json!({
        "kind": table.kind().as_str(),
        "columns": table.columns(),
        "column_types": table.column_types().iter().map(|ty| ty.as_str()).collect::<Vec<_>>(),
        "rows": table.rows().iter().map(|row| {
            row.iter().map(literal_expression_to_value).collect::<Vec<_>>()
        }).collect::<Vec<_>>()
    });
    if let Some(source) = table.source() {
        value["source"] = finite_table_to_value(source);
        value["contributors"] = json!(table.contributors());
    }
    if let Some((left, right)) = table.joined_sources() {
        value["joined_sources"] =
            json!([finite_table_to_value(left), finite_table_to_value(right)]);
        value["join_pairs"] = json!(table.joined_pairs());
    }
    value
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

    if let Some(table) = statement.finite_table() {
        value["finite_table"] = finite_table_to_value(table);
    }

    if let Some(set_operation) = statement.set_operation() {
        value["set_operation"] = set_operation_to_value(set_operation);
    }
    if let Some(row_shaping) = statement.row_shaping() {
        value["row_shaping"] = row_shaping_to_value(row_shaping);
    }
    if !statement.sampling().is_empty() {
        value["sampling"] = json!(statement
            .sampling()
            .iter()
            .map(|sample| json!({
                "source": sample.source(),
                "source_instance": sample.source_instance(),
                "syntax": sample.syntax().as_str(),
                "method": sample.method().map(|method| method.as_str()),
                "quantity": sample.quantity().map(|quantity| json!({
                    "value": quantity.value().map(literal_value_to_value),
                    "unit": quantity.unit().map(|unit| unit.as_str())
                })),
                "seed": sample.seed().map(|seed| json!({
                    "syntax": seed.syntax().as_str(),
                    "value": seed.value().map(literal_value_to_value)
                })),
                "options_unproven": sample.has_bucket_or_offset(),
                "membership": "residual"
            }))
            .collect::<Vec<_>>());
    }
    if let Some(page) = statement.nested_page() {
        value["nested_page"] = json!({
            "source": page.source(),
            "projected_columns": page.projected_columns().iter().map(|(output, source)| json!({
                "output": output,
                "source": source,
            })).collect::<Vec<_>>(),
            "inner": row_shaping_to_value(page.inner()),
            "outer": row_shaping_to_value(page.outer()),
        });
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
                WriteEffectAction::InsertDefaultValues => json!({"kind": "insert_default_values"}),
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
    let mut value = json!({
        "target": write.target(),
        "target_alias": write.target_alias(),
        "definition_mode": write.definition_mode().map(|mode| mode.as_str()),
        "kind": write.kind().as_str(),
        "target_columns": write.target_columns(),
        "default_values": write.default_values(),
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
    });
    if let Some(conflict) = write.insert_conflict() {
        value["insert_conflict"] = json!({
            "action": "do_nothing",
            "target_columns": conflict.columns(),
        });
    }
    if !write.returning_columns().is_empty() {
        value["returning_columns"] = json!(write.returning_columns());
    }
    if let Some(cte) = write.insert_cte_source() {
        value["cte_source"] = json!({
            "name": cte.name(),
            "producer": query_statement_to_value(cte.producer()),
            "consumer": query_statement_to_value(cte.consumer())
        });
    }
    value
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
