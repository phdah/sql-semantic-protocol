//! Local transformation layers and write-state effects.

use super::*;

pub struct TransformationLayer {
    id: String,
    input_id: String,
    statement_index: usize,
    produces: Vec<DatasetRef>,
    consumes: Vec<String>,
    relation_identities: BTreeMap<String, String>,
    write_kind: Option<WriteKind>,
    composed_semantics: ComposedSemantics,
}

impl TransformationLayer {
    /// Return the deterministic layer identifier.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Return the input containing the statement represented by this layer.
    pub fn input_id(&self) -> &str {
        &self.input_id
    }

    /// Return the zero-based statement index within the input.
    pub fn statement_index(&self) -> usize {
        self.statement_index
    }

    /// Return datasets produced by this layer.
    pub fn produces(&self) -> &[DatasetRef] {
        &self.produces
    }

    /// Return normalized physical relations consumed directly by this layer.
    pub fn consumes(&self) -> &[String] {
        &self.consumes
    }

    /// Return how this layer writes its named relation, if it performs a relation write.
    pub fn write_kind(&self) -> Option<WriteKind> {
        self.write_kind
    }

    /// Return transitive semantics composed through in-bundle producers.
    pub fn composed_semantics(&self) -> &ComposedSemantics {
        &self.composed_semantics
    }

    pub(crate) fn canonical_relation(&self, relation: &str) -> String {
        self.relation_identities
            .get(relation)
            .cloned()
            .unwrap_or_else(|| relation.to_string())
    }
}

/// One partial write's canonical effect, target identity and constraint evidence.
///
/// Absent constraint evidence is not a proof that a target has no keys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayerWriteStateEffect {
    layer_id: String,
    target: String,
    sources: Vec<String>,
    effect: WriteStateEffect,
    target_constraints: Option<RelationConstraintSet>,
}

impl LayerWriteStateEffect {
    /// Stable layer identity of the mutation.
    pub fn layer_id(&self) -> &str {
        &self.layer_id
    }
    /// Resolved target relation identity.
    pub fn target(&self) -> &str {
        &self.target
    }
    /// Resolved input relations; the pre-existing target is separate.
    pub fn sources(&self) -> &[String] {
        &self.sources
    }
    /// Typed mutation obligations, including row-count conservation.
    pub fn effect(&self) -> &WriteStateEffect {
        &self.effect
    }
    /// Available target constraints with enforcement/provenance.
    pub fn target_constraints(&self) -> Option<&RelationConstraintSet> {
        self.target_constraints.as_ref()
    }
}

/// Multi-input analysis result with deterministic local layers and relation dependency graph.
#[derive(Debug, Clone, PartialEq, Eq)]
