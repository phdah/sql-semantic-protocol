//! Canonical computed/functional key expressions independent of SQL parser dialects.

use crate::column_sources::GeneratedValueExpression;
use crate::constraints::{ConstraintEvidence, ConstraintMetadataError};

/// An attested ordered functional key, retaining all source and enforcement evidence.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ComputedKeyConstraint {
    expressions: Vec<GeneratedValueExpression>,
    primary: bool,
    evidence: Vec<ConstraintEvidence>,
}

impl ComputedKeyConstraint {
    /// Build a checked composite functional key from portable row-local expressions.
    pub fn new(
        expressions: Vec<GeneratedValueExpression>,
        primary: bool,
        mut evidence: Vec<ConstraintEvidence>,
    ) -> Result<Self, ConstraintMetadataError> {
        if expressions.is_empty() || expressions.iter().any(|expr| !expr.valid()) {
            return Err(ConstraintMetadataError::InvalidColumns {
                message: "functional key needs one or more valid scalar expressions".to_string(),
            });
        }
        if evidence.is_empty() {
            return Err(ConstraintMetadataError::InvalidEvidence {
                message: "functional key needs enforcement provenance".to_string(),
            });
        }
        evidence.sort();
        evidence.dedup();
        Ok(Self { expressions, primary, evidence })
    }

    /// Ordered computed and column terms jointly defining this key.
    pub fn expressions(&self) -> &[GeneratedValueExpression] {
        &self.expressions
    }

    /// Whether a NULL term is prohibited as a primary-key violation.
    pub fn primary(&self) -> bool {
        self.primary
    }

    /// Independent source attestations of this functional key's enforcement.
    pub fn evidence(&self) -> &[ConstraintEvidence] {
        &self.evidence
    }

    pub(crate) fn same_semantics(&self, other: &Self) -> bool {
        self.primary == other.primary && self.expressions == other.expressions
    }

    pub(crate) fn merge_evidence(&mut self, other: &Self) {
        self.evidence.extend(other.evidence.iter().cloned());
        self.evidence.sort();
        self.evidence.dedup();
    }
}
