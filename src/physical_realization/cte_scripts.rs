//! Compose an ordered, source-complete write script with its final CTE read.
//!
//! This deliberately consumes only normalized bundle facts and attested initial
//! physical rows. The read receives the *constructed* post-write target, never
//! an independently supplied witness for a materialized intermediate relation.

use std::collections::{BTreeMap, BTreeSet};

use crate::bundle::AnalysisBundle;
use crate::protocol::{LiteralValue, ProtocolStatement};

use super::{
    physical_finite_cte_bag_plan, physical_ordered_write_state_plan, PhysicalCteExact,
    PhysicalCteProof, PhysicalWriteState, PhysicalWriteStatePlan, PhysicalWriteTransition,
};

/// Exact ordered write transitions and terminal CTE bag from the same physical sources.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalOrderedCteExact {
    /// The complete, ordered materialization transitions, including intermediate bags.
    pub transitions: Vec<PhysicalWriteTransition>,
    /// Post-write complete physical states, including the produced target relation.
    pub final_states: Vec<PhysicalWriteState>,
    /// The exact terminal read, using those derived states rather than independent inputs.
    pub read: Box<PhysicalCteExact>,
}

/// Outcome of a joint write/materialization and terminal CTE proof.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhysicalOrderedCteProof {
    /// All transitions and the subsequent CTE output are source-complete.
    Exact(Box<PhysicalOrderedCteExact>),
    /// A contradiction in the write or requested terminal row is proved.
    Impossible { reason: String },
    /// Unsupported DDL, ordering, source evidence, or read semantics fail closed.
    Residual { reason: String },
}

/// Prove a terminal CTE read *after* all ordered writes in the analyzed bundle.
///
/// The caller supplies only initial physical source and target states, not
/// materialized intermediate rows. A read earlier than the final layer is not
/// supported, because reconstructing its correct historical preimage requires
/// a separate ordering witness. A query with an unsupported statement likewise
/// cannot be silently dropped by the underlying write interpreter.
pub fn physical_ordered_cte_read_plan(
    bundle: &AnalysisBundle,
    target_layer_id: &str,
    initial_states: &[PhysicalWriteState],
    wanted: Option<&BTreeMap<String, LiteralValue>>,
) -> PhysicalOrderedCteProof {
    let Some(layer) = bundle.layers().last() else {
        return PhysicalOrderedCteProof::Residual {
            reason: "missing terminal CTE layer".to_string(),
        };
    };
    if layer.id() != target_layer_id || layer.write_kind().is_some() {
        return PhysicalOrderedCteProof::Residual {
            reason: "CTE read must be the terminal non-write layer".to_string(),
        };
    }
    let Some(query) = bundle
        .inputs()
        .iter()
        .find(|input| input.id() == layer.input_id())
        .and_then(|input| input.statements().get(layer.statement_index()))
        .and_then(|statement| match statement {
            ProtocolStatement::Query(query) if query.write().is_none() => Some(query),
            _ => None,
        })
    else {
        return PhysicalOrderedCteProof::Residual {
            reason: "terminal layer has no canonical read query".to_string(),
        };
    };
    let source_names = query
        .dependencies()
        .iter()
        .map(|name| name.to_string())
        .collect::<BTreeSet<_>>();
    let (transitions, final_states) =
        match physical_ordered_write_state_plan(bundle, initial_states) {
            PhysicalWriteStatePlan::Feasible {
                transitions,
                final_states,
            } => (transitions, final_states),
            PhysicalWriteStatePlan::Impossible { reason, .. } => {
                return PhysicalOrderedCteProof::Impossible { reason }
            }
            PhysicalWriteStatePlan::Residual { reason, .. } => {
                return PhysicalOrderedCteProof::Residual { reason }
            }
        };
    if transitions.is_empty() {
        return PhysicalOrderedCteProof::Residual {
            reason: "no ordered materialization precedes the terminal CTE".to_string(),
        };
    }
    // Missing/duplicate source identities are left to the CTE evaluator,
    // which also validates every required canonical input schema.
    let source_states = final_states
        .iter()
        .filter(|state| source_names.contains(state.relation()))
        .cloned()
        .collect::<Vec<_>>();
    match physical_finite_cte_bag_plan(bundle, target_layer_id, &source_states, wanted) {
        PhysicalCteProof::Exact(read) => {
            PhysicalOrderedCteProof::Exact(Box::new(PhysicalOrderedCteExact {
                transitions,
                final_states,
                read,
            }))
        }
        PhysicalCteProof::Impossible { reason } => {
            PhysicalOrderedCteProof::Impossible { reason }
        }
        PhysicalCteProof::Residual { reason } => PhysicalOrderedCteProof::Residual { reason },
    }
}
