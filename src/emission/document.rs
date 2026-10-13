//! Top-level protocol and bundle document serialization.

use super::*;

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

pub(super) fn bundle_to_value(bundle: &AnalysisBundle) -> Value {
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

    // Canonical nodes are stored only once; per-layer plans reference their identities.
    let plans = bundle
        .layers()
        .iter()
        .map(|layer| physical_source_plan(bundle, layer.id()))
        .collect::<Vec<_>>();
    let mut nodes = BTreeMap::<PhysicalPlanRef, PhysicalPlanNode>::new();
    for plan in &plans {
        for node in plan.nodes() {
            nodes
                .entry(node.id().clone())
                .or_insert_with(|| node.clone());
        }
    }
    value["graph"]["physical_nodes"] = json!(nodes
        .values()
        .map(physical_plan_node_to_value)
        .collect::<Vec<_>>());
    value["graph"]["physical_source_plans"] = json!(plans.iter().map(|plan| json!({
        "layer_id": plan.target_layer_id(),
        "node_refs": plan.nodes().iter().map(|node| physical_plan_ref_to_value(node.id())).collect::<Vec<_>>(),
        "physical_sources": plan.sources(),
        "qualifying": constructive_direction_to_value(plan.qualifying()),
        "rejected": constructive_direction_to_value(plan.rejected()),
        "zero_output": constructive_direction_to_value(plan.zero_output()),
        "gap": plan.gap().map(|gap| gap.as_str())
    })).collect::<Vec<_>>());

    // Optional cross-terminal count proof. Only row-only requests are
    // covered: emitting this for groups or histograms would incorrectly
    // suggest that their independent semantics were jointly certified.
    let row_only_goals = bundle.outcome_goals();
    if !row_only_goals.is_empty()
        && row_only_goals.iter().all(|evaluated| {
            let goal = evaluated.goal();
            goal.rows().is_some() && goal.groups().is_none() && goal.distributions().is_empty()
        })
    {
        let targets = row_only_goals
            .iter()
            .filter_map(|evaluated| {
                let goal = evaluated.goal();
                Some((goal.layer_id(), goal.rows()?))
            })
            .collect::<Vec<_>>();
        let joint = physical_joint_source_plan(bundle, &targets);
        value["graph"]["physical_joint_count_plan"] = json!({
            "targets": joint.targets().iter().map(|target| json!({
                "layer_id": target.layer_id(), "rows": target.rows()
            })).collect::<Vec<_>>(),
            "node_refs": joint.nodes().iter().map(|node| physical_plan_ref_to_value(node.id())).collect::<Vec<_>>(),
            "physical_sources": joint.sources(),
            "outcome": constructive_direction_to_value(joint.outcome()),
            "gap": joint.gap().map(|gap| gap.as_str())
        });
    }

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
                        if let Some(collation) = column.collation() {
                            column_value["collation"] = json!(collation);
                        }
                        if let Some(law) = column.text_comparison_law() {
                            column_value["text_comparison_law"] = json!(law.as_str());
                            column_value["comparison_evidence"] = json!(column
                                .comparison_evidence()
                                .iter()
                                .map(|evidence| json!({
                                    "source_kind": evidence.provenance().source_kind().as_str(),
                                    "source_id": evidence.provenance().source_id(),
                                    "enforcement": evidence.enforcement().as_str(),
                                }))
                                .collect::<Vec<_>>());
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

    let write_effects = bundle.write_state_effects();
    if !write_effects.is_empty() {
        value["write_effects"] = Value::Array(
            write_effects
                .iter()
                .map(layer_write_effect_to_value)
                .collect(),
        );
    }

    if !bundle.outcome_goals().is_empty() {
        value["outcome_goals"] = Value::Array(
            bundle
                .outcome_goals()
                .iter()
                .map(|item| {
                    let goal = item.goal();
                    let mut entry = json!({
                        "layer_id": goal.layer_id(),
                        "requested": {
                            "rows": goal.rows(),
                            "groups": goal.groups(),
                            "distributions": goal.distributions().iter().map(|distribution| json!({
                                "column": distribution.column(),
                                "values": distribution.values().iter().map(|entry| json!({
                                    "value": constraint_value_to_value(entry.value()),
                                    "rows": entry.rows()
                                })).collect::<Vec<_>>()
                            })).collect::<Vec<_>>()
                        },
                        "assessment": {
                            "status": item.status().as_str(),
                            "reason": item.reason(),
                            "min_rows": item.min_rows(),
                            "max_rows": item.max_rows()
                        }
                    });
                    if let Some(witness) = item.witness() {
                        entry["witness"] = outcome_witness_to_value(witness);
                    }
                    entry
                })
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
