//! Canonical row-local Boolean CHECK trees with SQL three-valued truth.
//!
//! A complete AND/OR/NOT predicate is one enforced obligation. Splitting an
//! OR into unrelated constraints would reject legitimate SQL rows.

use crate::column_sources::GeneratedValueExpression;
use crate::constraints::{CheckCompareOperator, ConstraintEvidence, ConstraintMetadataError};

/// SQL Boolean CHECK expression with only portable row-local leaves.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ComputedBooleanCheckPredicate {
    /// Compare two deterministic scalar expressions with SQL NULL semantics.
    Comparison {
        /// First scalar operand.
        left: GeneratedValueExpression,
        /// Portable comparison operation.
        operator: CheckCompareOperator,
        /// Second scalar operand.
        right: GeneratedValueExpression,
    },
    /// Exact NULL or NOT NULL test.
    IsNull {
        /// Tested row-local value.
        expression: GeneratedValueExpression,
        /// True means IS NOT NULL.
        negated: bool,
    },
    /// SQL AND, not a set of independently enforced CHECKs.
    And(Vec<Self>),
    /// SQL OR, with three-valued short-circuit proof.
    Or(Vec<Self>),
    /// SQL NOT, including UNKNOWN preservation.
    Not(Box<Self>),
}

impl ComputedBooleanCheckPredicate {
    /// Reject empty/invalid nested predicates at the canonical boundary.
    pub fn valid(&self) -> bool {
        match self {
            Self::Comparison { left, right, .. } => left.valid() && right.valid(),
            Self::IsNull { expression, .. } => expression.valid(),
            Self::And(children) | Self::Or(children) => {
                children.len() >= 2 && children.iter().all(Self::valid)
            }
            Self::Not(child) => child.valid(),
        }
    }

    /// Every referenced canonical physical column in deterministic order.
    pub fn referenced_columns(&self) -> Vec<&str> {
        let mut columns = Vec::new();
        self.collect_columns(&mut columns);
        columns.sort_unstable();
        columns.dedup();
        columns
    }

    fn collect_columns<'a>(&'a self, columns: &mut Vec<&'a str>) {
        match self {
            Self::Comparison { left, right, .. } => {
                columns.extend(left.referenced_columns());
                columns.extend(right.referenced_columns());
            }
            Self::IsNull { expression, .. } => columns.extend(expression.referenced_columns()),
            Self::And(children) | Self::Or(children) => {
                for child in children {
                    child.collect_columns(columns);
                }
            }
            Self::Not(child) => child.collect_columns(columns),
        }
    }
}

/// One enforced all-or-nothing SQL CHECK Boolean tree and its provenance.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ComputedBooleanCheckConstraint {
    predicate: ComputedBooleanCheckPredicate,
    evidence: Vec<ConstraintEvidence>,
}

impl ComputedBooleanCheckConstraint {
    /// Create a validated Boolean CHECK without collapsing its OR branches.
    pub fn new(
        predicate: ComputedBooleanCheckPredicate,
        mut evidence: Vec<ConstraintEvidence>,
    ) -> Result<Self, ConstraintMetadataError> {
        if !predicate.valid() {
            return Err(ConstraintMetadataError::InvalidColumns {
                message: "Boolean CHECK contains an empty/invalid expression".into(),
            });
        }
        if evidence.is_empty() {
            return Err(ConstraintMetadataError::InvalidEvidence {
                message: "Boolean CHECK requires provenance and enforcement evidence".into(),
            });
        }
        evidence.sort();
        evidence.dedup();
        Ok(Self { predicate, evidence })
    }

    /// Canonical predicate tree.
    pub fn predicate(&self) -> &ComputedBooleanCheckPredicate {
        &self.predicate
    }

    /// Proof provenance and enforcement.
    pub fn evidence(&self) -> &[ConstraintEvidence] {
        &self.evidence
    }

    pub(crate) fn same_semantics(&self, other: &Self) -> bool {
        self.predicate == other.predicate
    }

    pub(crate) fn merge_evidence(&mut self, other: &Self) {
        self.evidence.extend(other.evidence.iter().cloned());
        self.evidence.sort();
        self.evidence.dedup();
    }
}
