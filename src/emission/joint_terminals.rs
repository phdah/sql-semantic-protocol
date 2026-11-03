//! Deterministic, source-attested sidecar for the TASK-167 joint terminal proof.
//!
//! The main SQL protocol root document is unchanged. This optional witness
//! sidecar is emitted only from an already evaluated canonical physical proof.

use super::*;
use crate::physical_realization::{
    PhysicalJointOtherExact, PhysicalJointTerminalProof, PhysicalSetColumnDomain,
    PhysicalWriteState,
};

fn exact_scalar(value: &LiteralValue) -> Value {
    match value {
        LiteralValue::Null => json!({"kind": "null"}),
        LiteralValue::Number(number) => json!({
            "kind": "number", "lexical": number
        }),
        LiteralValue::Text(text) => json!({
            "kind": "text", "value": text
        }),
        LiteralValue::Boolean(value) => json!({
            "kind": "boolean", "value": value
        }),
    }
}

fn row_to_value(row: &BTreeMap<String, LiteralValue>) -> Value {
    json!(row
        .iter()
        .map(|(column, value)| (column.clone(), exact_scalar(value)))
        .collect::<BTreeMap<_, _>>())
}

fn terminal_to_value(rows: &[BTreeMap<String, LiteralValue>]) -> Value {
    json!(rows.iter().map(row_to_value).collect::<Vec<_>>())
}

fn source_to_value(state: &PhysicalWriteState) -> Value {
    json!({
        "relation": state.relation(),
        "rows": state
            .rows()
            .iter()
            .map(|row| row_to_value(row.values()))
            .collect::<Vec<_>>()
    })
}

fn domains_to_value(domains: &BTreeMap<String, ValueDomain>) -> Value {
    json!(domains
        .iter()
        .map(|(name, domain)| (name.clone(), value_domain_to_value(domain)))
        .collect::<BTreeMap<_, _>>())
}

fn set_domain_to_value(domain: &PhysicalSetColumnDomain) -> Value {
    let bounds = domain.bounds();
    json!({
        "kind": "exact_signed_integer_set",
        "mode": "include",
        "values": domain.values.iter().map(ToString::to_string).collect::<Vec<_>>(),
        "contains_null": domain.contains_null,
        "lower": bounds.map(|(lower, _)| lower.to_string()),
        "upper": bounds.map(|(_, upper)| upper.to_string()),
        "lower_inclusive": bounds.is_some(),
        "upper_inclusive": bounds.is_some()
    })
}

/// Emit a canonical, deterministic physical witness sidecar without reparsing
/// SQL or claiming a joint constructive proof for unrelated source assignments.
///
/// The exact branch emits every physical row, pair membership, both terminal
/// full bags and their strongest exact domains. Impossible/Residual emit only
/// the classification and reason, never a fabricated output. This is a
/// separate optional evidence artifact, not a second protocol root version.
pub fn to_physical_joint_terminal_json(
    bundle: &AnalysisBundle,
    proof: &PhysicalJointTerminalProof,
) -> String {
    let evidence = match proof {
        PhysicalJointTerminalProof::Exact(exact) => {
            let secondary = match &exact.other {
                PhysicalJointOtherExact::Window(window) => json!({
                    "kind": "window",
                    "terminal": terminal_to_value(&window.terminal),
                    "domains": domains_to_value(&window.terminal_domains),
                    "source": window.source,
                    "source_row_ids": window.source_row_ids,
                    "terminal_row_ids": window.terminal_row_ids,
                    "rejected_row_ids": window.rejected_row_ids
                }),
                PhysicalJointOtherExact::Set(set) => json!({
                    "kind": "set_tree",
                    "terminal": terminal_to_value(&set.terminal),
                    "domains": set.terminal_domains.iter().map(set_domain_to_value).collect::<Vec<_>>(),
                    "sources": set.sources,
                    "tuples": set.tuples.iter().map(|tuple| json!({
                        "values": tuple.values,
                        "branch_row_ids": tuple.branch_row_ids,
                        "output_count": tuple.output_count
                    })).collect::<Vec<_>>()
                }),
            };
            json!({
                "status": "exact",
                "sources": exact.sources.iter().map(source_to_value).collect::<Vec<_>>(),
                "grouped": {
                    "layer_id": exact.grouped_layer_id,
                    "selected": exact.grouped.selected,
                    "not_true": exact.grouped.rejected,
                    "groups": exact.grouped.groups.iter().map(|group| json!({
                        "key": group.key,
                        "source_row_ids": group.source_row_ids,
                        "having_truth": format!("{:?}", group.having_truth)
                    })).collect::<Vec<_>>(),
                    "terminal": terminal_to_value(&exact.grouped.terminal),
                    "domains": domains_to_value(&exact.grouped.terminal_domains)
                },
                "secondary": {
                    "layer_id": exact.other_layer_id,
                    "proof": secondary
                }
            })
        }
        PhysicalJointTerminalProof::Impossible { reason } => {
            json!({"status":"impossible", "reason":reason})
        }
        PhysicalJointTerminalProof::Residual { reason } => {
            json!({"status":"residual", "reason":reason})
        }
    };
    json!({
        "protocol_version": bundle.protocol_version(),
        "kind": "physical_joint_terminal_evidence",
        "evidence": evidence
    })
    .to_string()
}
