//! Output-scope selection for protocol emission.
//!
//! Scope selection is a rendering concern. Analysis always builds the complete input bundle,
//! dependency graph, and composed semantics before any transformation results are filtered.

use std::collections::BTreeSet;

use crate::{AnalysisBundle, DatasetRef, TransformationLayer};

/// Select which analyzed transformation results are exposed by protocol serialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputScope {
    /// Expose only layers that produce terminal outcomes from dependency-graph components.
    Final,
    /// Expose every analyzed transformation layer.
    AllLayers,
}

pub(crate) fn layers_for_scope(
    bundle: &AnalysisBundle,
    scope: OutputScope,
) -> Vec<&TransformationLayer> {
    match scope {
        OutputScope::Final => {
            let final_outcomes = bundle
                .graph()
                .components()
                .iter()
                .flat_map(|component| component.final_outcomes())
                .collect::<BTreeSet<&DatasetRef>>();

            bundle
                .layers()
                .iter()
                .filter(|layer| {
                    layer
                        .produces()
                        .iter()
                        .any(|dataset| final_outcomes.contains(dataset))
                })
                .collect()
        }
        OutputScope::AllLayers => bundle.layers().iter().collect(),
    }
}
