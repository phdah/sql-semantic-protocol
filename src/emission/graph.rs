//! Dataset identities, canonical relation graph, and physical plan nodes.

use super::*;

pub(super) fn dataset_ref_to_value(dataset: &DatasetRef) -> Value {
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

pub(super) fn analysis_graph_to_value(graph: &AnalysisGraph) -> Value {
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

pub(super) fn physical_plan_ref_to_value(reference: &PhysicalPlanRef) -> Value {
    match reference {
        PhysicalPlanRef::Source(name) => json!({ "kind": "source", "id": name }),
        PhysicalPlanRef::Layer(id) => json!({ "kind": "layer", "id": id }),
    }
}

pub(super) fn physical_plan_node_to_value(node: &PhysicalPlanNode) -> Value {
    let mut value = json!({
        "ref": physical_plan_ref_to_value(node.id()),
        "inputs": node.inputs().iter().map(physical_plan_ref_to_value).collect::<Vec<_>>(),
        "produced_relations": node.produced_relations(),
        "write_kind": node.write_kind().map(|kind| kind.as_str())
    });
    if !node.operator_witnesses().is_empty() {
        value["operator_witnesses"] = Value::Array(
            node.operator_witnesses()
                .iter()
                .map(constructive_witness_to_value)
                .collect(),
        );
    }
    if !node.pending_producers().is_empty() {
        value["pending_producers"] = Value::Array(
            node.pending_producers()
                .iter()
                .map(constructive_obligation_to_value)
                .collect(),
        );
    }
    value
}

pub(super) fn graph_edge_to_value(edge: &GraphEdge) -> Value {
    json!({
        "consumer_layer_id": edge.consumer_layer_id(),
        "relation": edge.relation(),
        "resolution": edge.resolution().as_str(),
        "producer_layer_ids": edge.producer_layer_ids()
    })
}

pub(super) fn graph_component_to_value(component: &GraphComponent) -> Value {
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

pub(super) fn composition_diagnostic_to_value(diagnostic: &CompositionDiagnostic) -> Value {
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
