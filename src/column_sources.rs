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

/// Canonical deterministic row-local expression for an attested generated column.
///
/// Opaque functions, casts and dialect-specific numeric evaluation are not
/// represented by this portable subset and remain explicitly unproved.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum GeneratedValueExpression {
    /// Value read from one declared column in the same physical row.
    Column(String),
    /// Exact canonical scalar constant.
    Literal(ConstraintValue),
    /// Checked integer addition.
    Add(Box<Self>, Box<Self>),
    /// Checked integer subtraction.
    Subtract(Box<Self>, Box<Self>),
    /// Checked integer multiplication.
    Multiply(Box<Self>, Box<Self>),
    /// Exact integer absolute value; minimum signed integer overflow is unproved.
    Abs(Box<Self>),
    /// First non-NULL portable value; implicit cross-type coercion is unproved.
    Coalesce(Vec<Self>),
}

impl GeneratedValueExpression {
    /// Check that all referenced columns have nonempty canonical identities.
    pub fn valid(&self) -> bool {
        match self {
            Self::Column(name) => !name.trim().is_empty(),
            Self::Literal(_) => true,
            Self::Abs(operand) => operand.valid(),
            Self::Coalesce(arguments) => {
                arguments.len() >= 2 && arguments.iter().all(Self::valid)
            }
            Self::Add(left, right) | Self::Subtract(left, right) | Self::Multiply(left, right) => {
                left.valid() && right.valid()
            }
        }
    }

    /// Every column referenced by this expression, in deterministic order.
    pub fn referenced_columns(&self) -> Vec<&str> {
        let mut columns = Vec::new();
        self.collect_references(&mut columns);
        columns.sort_unstable();
        columns.dedup();
        columns
    }

    fn collect_references<'a>(&'a self, columns: &mut Vec<&'a str>) {
        match self {
            Self::Column(name) => columns.push(name),
            Self::Literal(_) => {}
            Self::Abs(operand) => operand.collect_references(columns),
            Self::Coalesce(arguments) => {
                for argument in arguments {
                    argument.collect_references(columns);
                }
            }
            Self::Add(left, right) | Self::Subtract(left, right) | Self::Multiply(left, right) => {
                left.collect_references(columns);
                right.collect_references(columns);
            }
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
    expression: Option<GeneratedValueExpression>,
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
        Ok(Self {
            column,
            kind,
            value,
            expression: None,
            evidence,
        })
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

    /// Canonical portable generated expression when the DDL safely specifies one.
    pub fn expression(&self) -> Option<&GeneratedValueExpression> {
        self.expression.as_ref()
    }

    /// Attach a provable generated expression without replacing its provenance.
    pub fn with_generated_expression(
        mut self,
        expression: GeneratedValueExpression,
    ) -> Result<Self, ConstraintMetadataError> {
        if self.kind != ColumnValueKind::Generated || !expression.valid() {
            return Err(ConstraintMetadataError::InvalidEvidence {
                message: "only generated columns accept validated computation semantics"
                    .to_string(),
            });
        }
        self.expression = Some(expression);
        Ok(self)
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
        self.column == other.column
            && self.kind == other.kind
            && self.value == other.value
            && self.expression == other.expression
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constraints::{ConstraintEnforcement, ConstraintProvenance, ConstraintSourceKind};

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
        )
        .is_err());
        assert!(ColumnValueSource::new("id", ColumnValueKind::Default, None, evidence).is_ok());
    }
}
