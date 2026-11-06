//! Source-independent producer storage intent and execution-evidence boundary.
//!
//! A manifest's materialization config is a declaration, not an executed state.
//! Downstream consumers must independently attest physical source/target bags.

/// Storage/identity kind declared for a transformation producer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProducerMaterialization {
    /// An unnamed, inlined query; no physical relation is created.
    Inline,
    /// A named logical view declaration.
    LogicalView,
    /// A named physical table materialization intent.
    PhysicalTable,
    /// A stateful incremental target; model SQL alone does not specify the write.
    Incremental,
    /// No authoritative materialization kind was provided.
    Unspecified,
}

impl ProducerMaterialization {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Inline => "inline",
            Self::LogicalView => "logical_view",
            Self::PhysicalTable => "physical_table",
            Self::Incremental => "incremental",
            Self::Unspecified => "unspecified",
        }
    }
}

/// Evidence that a declared producer was actually executed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProducerExecution {
    /// An artifact declares a model but does not attest an engine execution or target bag.
    Unattested,
}

impl ProducerExecution {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Unattested => "unattested",
        }
    }
}

/// Whether a relation-less dependency's compiled expansion is independently proven.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InlineDependencyEvidence {
    /// This producer has no relation-less model dependencies.
    NotApplicable,
    /// Compiled downstream SQL is analyzed, but the manifest does not prove expansion equivalence.
    Unattested,
}

impl InlineDependencyEvidence {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::NotApplicable => "not_applicable",
            Self::Unattested => "unattested",
        }
    }
}

/// Canonical producer evidence, keyed by the same stable input ID as its analyzed SQL.
/// This is not a state transition witness and never invents a physical target population.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProducerEvidence {
    input_id: String,
    relation: Option<String>,
    materialization: ProducerMaterialization,
    execution: ProducerExecution,
    inline_dependencies: InlineDependencyEvidence,
}

impl ProducerEvidence {
    pub(crate) fn declared(
        input_id: String,
        relation: Option<String>,
        materialization: ProducerMaterialization,
        inline_dependencies: InlineDependencyEvidence,
    ) -> Self {
        Self {
            input_id,
            relation,
            materialization,
            execution: ProducerExecution::Unattested,
            inline_dependencies,
        }
    }

    /// Stable transformation input ID.
    pub fn input_id(&self) -> &str {
        &self.input_id
    }

    /// Named producer relation, absent for an inlined or anonymous producer.
    pub fn relation(&self) -> Option<&str> {
        self.relation.as_deref()
    }

    /// Declared storage/identity kind, distinct from executed SQL write semantics.
    pub fn materialization(&self) -> ProducerMaterialization {
        self.materialization
    }

    /// Execution evidence; manifest metadata alone is always unattested.
    pub fn execution(&self) -> ProducerExecution {
        self.execution
    }

    /// Proof status for relation-less compiled dependencies.
    pub fn inline_dependencies(&self) -> InlineDependencyEvidence {
        self.inline_dependencies
    }
}
