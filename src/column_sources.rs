//! Canonical source of values for omitted or database-generated physical columns.
//!
//! This is declaration evidence, not permission to write arbitrary computed values.
//! Opaque expressions retain their provenance but cannot be executed by consumers.

use crate::constraints::{ConstraintEvidence, ConstraintMetadataError, ConstraintValue};

/// Kind of SQL column value production, independent of AST syntax.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ColumnValueKind {
    /// Insert default used when the column is omitted.
    Default,
    /// Value calculated by a generated expression, not independently writable.
    Generated,
    /// Value allocated by database identity/sequence state.
    Identity,
}

impl ColumnValueKind {
    /// Stable protocol spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Generated => "generated",
            Self::Identity => "identity",
        }
    }
}

/// A typed scalar default or an explicitly opaque generated value declaration.
///
/// `None` means the expression is known to exist but has not been safely
/// reduced to a deterministic scalar value. That is never an implicit NULL.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ColumnValueSource {
    column: String,
    kind: ColumnValueKind,
    value: Option<ConstraintValue>,
    evidence: Vec<ConstraintEvidence>,
}

impl ColumnValueSource {
    /// Construct a declaration with nonempty identity and provenance.
    pub fn new(
        column: impl Into<String>,
        kind: ColumnValueKind,
        value: Option<ConstraintValue>,
        mut evidence: Vec<ConstraintEvidence>,
    ) -> Result<Self, ConstraintMetadataError> {
        let column = column.into();
        if column.trim().is_empty() || evidence.is_empty() {
            return Err(ConstraintMetadataError::InvalidEvidence {
                message: "column value production requires a column and provenance".to_string(),
            });
        }
        if kind != ColumnValueKind::Default && value.is_some() {
            return Err(ConstraintMetadataError::InvalidEvidence {
                message: "generated/identity columns cannot be assigned a guessed literal"
                    .to_string(),
            });
        }
        evidence.sort();
        evidence.dedup();
        Ok(Self { column, kind, value, evidence })
    }

    /// Column to which this source belongs.
    pub fn column(&self) -> &str {
        &self.column
    }

    /// Value-production kind.
    pub fn kind(&self) -> ColumnValueKind {
        self.kind
    }

    /// Portable, exact scalar default, or None for an unproved expression.
    pub fn value(&self) -> Option<&ConstraintValue> {
        self.value.as_ref()
    }

    /// All source and enforcement attestations.
    pub fn evidence(&self) -> &[ConstraintEvidence] {
        &self.evidence
    }

    pub(crate) fn merge_evidence(&mut self, other: &Self) {
        self.evidence.extend(other.evidence.iter().cloned());
        self.evidence.sort();
        self.evidence.dedup();
    }

    pub(crate) fn same_semantics(&self, other: &Self) -> bool {
        self.column == other.column && self.kind == other.kind && self.value == other.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constraints::{
        ConstraintEnforcement, ConstraintProvenance, ConstraintSourceKind,
    };

    #[test]
    fn generated_expression_never_accepts_a_guessed_scalar() {
        let evidence = vec![ConstraintEvidence::new(
            ConstraintProvenance::new(ConstraintSourceKind::ExternalMetadata, "x").unwrap(),
            ConstraintEnforcement::Unknown,
        )];
        assert!(ColumnValueSource::new(
            "id",
            ColumnValueKind::Generated,
            Some(ConstraintValue::Integer(1)),
            evidence.clone(),
        ).is_err());
        assert!(ColumnValueSource::new("id", ColumnValueKind::Default, None, evidence).is_ok());
    }
}
