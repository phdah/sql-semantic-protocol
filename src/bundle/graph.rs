//! Canonical dataset references and relation graph structures.

use super::*;

/// Dataset produced by one transformation layer.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub enum DatasetRef {
    /// A named relation created by query-backed DDL.
    Relation {
        /// Deterministic SQL relation identity, preserving qualification and quoting.
        name: String,
    },
    /// An anonymous result produced by a bare query.
    Anonymous {
        /// Layer identifier used to address the anonymous result.
        layer_id: String,
    },
}

impl DatasetRef {
    /// Return the named relation identity when this dataset refers to a relation.
    pub fn relation_name(&self) -> Option<&str> {
        match self {
            Self::Relation { name } => Some(name),
            Self::Anonymous { .. } => None,
        }
    }

    /// Return the owning layer identifier when this dataset is anonymous.
    pub fn anonymous_layer_id(&self) -> Option<&str> {
        match self {
            Self::Relation { .. } => None,
            Self::Anonymous { layer_id } => Some(layer_id),
        }
    }
}

/// Resolution state for one consumed relation in the bundle dependency graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RelationResolution {
    /// Exactly one in-bundle producer matches the consumed relation.
    Resolved,
    /// No in-bundle producer exists, so the relation is an external dependency.
    External,
    /// A required producer is unavailable.
    Missing,
    /// Multiple in-bundle producers match and no producer can be selected safely.
    Ambiguous,
    /// The resolved producer relationship participates in a dependency cycle.
    Cycle,
    /// A producer writes only part of the relation, so it cannot define complete downstream semantics.
    Partial,
    /// Resolution could not be represented safely for another explicit reason.
    Unsupported,
}

impl RelationResolution {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Resolved => "resolved",
            Self::External => "external",
            Self::Missing => "missing",
            Self::Ambiguous => "ambiguous",
            Self::Cycle => "cycle",
            Self::Partial => "partial",
            Self::Unsupported => "unsupported",
        }
    }
}

/// One relation dependency edge from a consumer layer to zero or more candidate producers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphEdge {
    consumer_layer_id: String,
    relation: String,
    resolution: RelationResolution,
    producer_layer_ids: Vec<String>,
}

impl GraphEdge {
    /// Return the layer consuming this relation.
    pub fn consumer_layer_id(&self) -> &str {
        &self.consumer_layer_id
    }

    /// Return the normalized consumed relation identity.
    pub fn relation(&self) -> &str {
        &self.relation
    }

    /// Return how this relation was resolved.
    pub fn resolution(&self) -> RelationResolution {
        self.resolution
    }

    /// Return matching producer layer IDs in deterministic order.
    pub fn producer_layer_ids(&self) -> &[String] {
        &self.producer_layer_ids
    }
}

/// One connected component of transformation layers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphComponent {
    id: String,
    layer_ids: Vec<String>,
    final_outcomes: Vec<DatasetRef>,
    diagnostics: Vec<CompositionDiagnostic>,
}

impl GraphComponent {
    /// Return the deterministic component identifier.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Return component layers in deterministic dependency order when acyclic.
    pub fn layer_ids(&self) -> &[String] {
        &self.layer_ids
    }

    /// Return terminal datasets when the component has an unambiguous acyclic outcome.
    pub fn final_outcomes(&self) -> &[DatasetRef] {
        &self.final_outcomes
    }

    /// Return diagnostics affecting this component.
    pub fn diagnostics(&self) -> &[CompositionDiagnostic] {
        &self.diagnostics
    }
}

/// Deterministic dependency graph across all transformation layers in an analysis bundle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalysisGraph {
    edges: Vec<GraphEdge>,
    components: Vec<GraphComponent>,
    diagnostics: Vec<CompositionDiagnostic>,
}

impl AnalysisGraph {
    /// Return dependency edges in deterministic consumer/relation order.
    pub fn edges(&self) -> &[GraphEdge] {
        &self.edges
    }

    /// Return disconnected graph components ordered by their earliest layer.
    pub fn components(&self) -> &[GraphComponent] {
        &self.components
    }

    /// Return graph-level ambiguity and cycle diagnostics.
    pub fn diagnostics(&self) -> &[CompositionDiagnostic] {
        &self.diagnostics
    }
}

