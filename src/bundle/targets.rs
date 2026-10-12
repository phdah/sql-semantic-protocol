//! Target selection with explicit ambiguity diagnostics.

use super::*;

/// Error returned when an explicit target relation cannot be selected safely.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TargetSelectionError {
    /// No supplied transformation produces the requested relation.
    UnknownTarget {
        /// Requested relation identifier.
        target: String,
    },
    /// More than one supplied transformation produces the requested relation.
    AmbiguousTarget {
        /// Requested relation identifier.
        target: String,
        /// Candidate producer layer identifiers in deterministic order.
        producer_layer_ids: Vec<String>,
    },
}

impl TargetSelectionError {
    /// Return the requested relation identifier.
    pub fn target(&self) -> &str {
        match self {
            Self::UnknownTarget { target } | Self::AmbiguousTarget { target, .. } => target,
        }
    }

    /// Return candidate producer layers when the requested relation is ambiguous.
    pub fn producer_layer_ids(&self) -> &[String] {
        match self {
            Self::UnknownTarget { .. } => &[],
            Self::AmbiguousTarget {
                producer_layer_ids, ..
            } => producer_layer_ids,
        }
    }
}

impl fmt::Display for TargetSelectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownTarget { target } => write!(
                formatter,
                "target relation '{target}' is not produced by any supplied transformation"
            ),
            Self::AmbiguousTarget {
                target,
                producer_layer_ids,
            } => write!(
                formatter,
                "target relation '{target}' is ambiguous; produced by layers {}",
                producer_layer_ids.join(", ")
            ),
        }
    }
}

impl std::error::Error for TargetSelectionError {}

/// Project a fully analyzed bundle onto explicit named target relations.
///
/// Target selection is deliberately applied after analysis and composition. The returned bundle
/// keeps the complete analyzed input list, while its transformation layers and dependency graph
/// are limited to the requested target producers plus every in-bundle ancestor needed to describe
/// them. Passing no targets returns the complete bundle unchanged.
pub fn select_targets(
    bundle: &AnalysisBundle,
    targets: &[String],
) -> Result<AnalysisBundle, TargetSelectionError> {
    if targets.is_empty() {
        return Ok(bundle.clone());
    }

    let producers = collect_producers(bundle.layers());
    let mut selected_layer_ids = BTreeSet::new();
    let mut pending = Vec::new();

    for target in targets {
        match producers.get(target) {
            None => {
                return Err(TargetSelectionError::UnknownTarget {
                    target: target.clone(),
                });
            }
            Some(producer_layer_ids) if producer_layer_ids.len() == 1 => {
                pending.push(producer_layer_ids[0].clone());
            }
            Some(producer_layer_ids) => {
                return Err(TargetSelectionError::AmbiguousTarget {
                    target: target.clone(),
                    producer_layer_ids: producer_layer_ids.clone(),
                });
            }
        }
    }

    while let Some(layer_id) = pending.pop() {
        if !selected_layer_ids.insert(layer_id.clone()) {
            continue;
        }

        for edge in bundle
            .graph()
            .edges()
            .iter()
            .filter(|edge| edge.consumer_layer_id() == layer_id)
        {
            for producer_layer_id in edge.producer_layer_ids().iter().rev() {
                if !selected_layer_ids.contains(producer_layer_id) {
                    pending.push(producer_layer_id.clone());
                }
            }
        }
    }

    let layers = bundle
        .layers()
        .iter()
        .filter(|layer| selected_layer_ids.contains(layer.id()))
        .cloned()
        .collect::<Vec<_>>();
    let graph = build_graph(&layers);

    Ok(AnalysisBundle {
        protocol_version: bundle.protocol_version,
        inputs: bundle.inputs.clone(),
        layers,
        graph,
        source_schemas: bundle.source_schemas.clone(),
        relation_constraints: bundle.relation_constraints.clone(),
        constraint_diagnostics: bundle.constraint_diagnostics.clone(),
        comparison_declarations: bundle.comparison_declarations.clone(),
        outcome_goals: bundle
            .outcome_goals
            .iter()
            .filter(|item| selected_layer_ids.contains(item.goal().layer_id()))
            .cloned()
            .collect(),
    })
}

pub(super) fn diagnostic_cmp(
    left: &CompositionDiagnostic,
    right: &CompositionDiagnostic,
) -> std::cmp::Ordering {
    (
        left.severity().as_str(),
        left.code(),
        left.input_id(),
        left.layer_id(),
        left.relation(),
        left.message(),
    )
        .cmp(&(
            right.severity().as_str(),
            right.code(),
            right.input_id(),
            right.layer_id(),
            right.relation(),
            right.message(),
        ))
}

