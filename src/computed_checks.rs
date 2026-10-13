//! Portable expressions for an enforced cross-column SQL CHECK.
//!
//! Compare row-local typed scalar expressions exactly. SQL truth-table rules
//! admit a NULL/UNKNOWN CHECK, but unproved casts and collation stay residual.

use crate::column_sources::GeneratedValueExpression;
use crate::constraints::{CheckCompareOperator, ConstraintEvidence, ConstraintMetadataError};

/// One row-local comparison CHECK with provenance and execution attestation.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ComputedCheckConstraint {
    left: GeneratedValueExpression,
    operator: CheckCompareOperator,
    right: GeneratedValueExpression,
    evidence: Vec<ConstraintEvidence>,
}

impl ComputedCheckConstraint {
    /// Build a canonical CHECK over exact row-local scalar operands.
    pub fn new(
        left: GeneratedValueExpression,
        operator: CheckCompareOperator,
        right: GeneratedValueExpression,
        mut evidence: Vec<ConstraintEvidence>,
    ) -> Result<Self, ConstraintMetadataError> {
        if !left.valid() || !right.valid() {
            return Err(ConstraintMetadataError::InvalidColumns {
                message: "CHECK expression references an empty column identity".to_string(),
            });
        }
        if evidence.is_empty() {
            return Err(ConstraintMetadataError::InvalidEvidence {
                message: "CHECK needs one or more source attestations".to_string(),
            });
        }
        evidence.sort();
        evidence.dedup();
        Ok(Self { left, operator, right, evidence })
    }

    /// Left-hand row-local scalar expression.
    pub fn left(&self) -> &GeneratedValueExpression {
        &self.left
    }

    /// Portable comparison operation.
    pub fn operator(&self) -> CheckCompareOperator {
        self.operator
    }

    /// Right-hand row-local scalar expression.
    pub fn right(&self) -> &GeneratedValueExpression {
        &self.right
    }

    /// All source and enforcement attestations.
    pub fn evidence(&self) -> &[ConstraintEvidence] {
        &self.evidence
    }

    pub(crate) fn same_semantics(&self, other: &Self) -> bool {
        self.left == other.left && self.operator == other.operator && self.right == other.right
    }

    pub(crate) fn merge_evidence(&mut self, other: &Self) {
        self.evidence.extend(other.evidence.iter().cloned());
        self.evidence.sort();
        self.evidence.dedup();
    }
}
