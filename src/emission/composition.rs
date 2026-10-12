//! Layer, composed semantics, and join proof serialization.

use super::*;

pub(super) fn transformation_layer_to_value(layer: &TransformationLayer) -> Value {
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

pub(super) fn composed_semantics_to_value(semantics: &ComposedSemantics) -> Value {
    match semantics {
        ComposedSemantics::Resolved(semantics) => resolved_composed_semantics_to_value(semantics),
        ComposedSemantics::Unresolved(semantics) => {
            unresolved_composed_semantics_to_value(semantics)
        }
    }
}

pub(super) fn resolved_composed_semantics_to_value(semantics: &ResolvedComposedSemantics) -> Value {
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
    let constructive = crate::constructive::local_constructive_witnesses(semantics);
    if !constructive.is_empty() {
        value["constructive_witnesses"] = Value::Array(
            constructive
                .iter()
                .map(constructive_witness_to_value)
                .collect(),
        );
    }
    let pending = crate::constructive::local_pending_producers(semantics);
    if !pending.is_empty() {
        value["constructive_pending_producers"] = Value::Array(
            pending
                .iter()
                .map(constructive_obligation_to_value)
                .collect(),
        );
    }
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
        value["boolean_witnesses"] = json!(semantics
            .boolean_witnesses()
            .iter()
            .map(|item| json!({
                "origin_layer_id": item.origin_layer_id(),
                "boundary_kind": item.boundary_kind().as_str(),
                "witness": boolean_witness_to_value(item.witness())
            }))
            .collect::<Vec<_>>());
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

pub(super) fn join_witness_to_value(witness: &crate::JoinWitness) -> Value {
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

pub(super) fn join_witness_direction_to_value(direction: &crate::JoinWitnessDirection) -> Value {
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

pub(super) fn composed_join_equality_to_value(equality: &crate::ComposedJoinEquality) -> Value {
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

pub(super) fn unresolved_composed_semantics_to_value(
    semantics: &UnresolvedComposedSemantics,
) -> Value {
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
