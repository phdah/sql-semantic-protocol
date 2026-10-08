//! Canonical relation-constraint metadata shared by SQL and external metadata adapters.
//!
//! Constraint semantics are parser-independent. Evidence provenance and enforcement are preserved
//! separately so a declaration is never promoted into a stronger guarantee than its source proves.

use std::collections::BTreeSet;
use std::fmt;

/// Source category that supplied one piece of constraint evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub enum ConstraintSourceKind {
    /// A constraint declared directly in SQL DDL.
    SqlDdl,
    /// A constraint declared in dbt model or column metadata.
    DbtConstraint,
    /// A semantic assertion declared as a dbt generic data test.
    DbtTest,
    /// Vendor-neutral or caller-owned external metadata.
    ExternalMetadata,
}

impl ConstraintSourceKind {
    /// Return the stable protocol spelling for this source category.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SqlDdl => "sql_ddl",
            Self::DbtConstraint => "dbt_constraint",
            Self::DbtTest => "dbt_test",
            Self::ExternalMetadata => "external_metadata",
        }
    }
}

/// Strength of runtime enforcement established by one evidence source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub enum ConstraintEnforcement {
    /// The source explicitly establishes runtime enforcement.
    Enforced,
    /// The source explicitly establishes that the constraint is not runtime-enforced.
    NotEnforced,
    /// Runtime enforcement cannot be proven from the source evidence.
    Unknown,
}

impl ConstraintEnforcement {
    /// Return the stable protocol spelling for this enforcement state.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Enforced => "enforced",
            Self::NotEnforced => "not_enforced",
            Self::Unknown => "unknown",
        }
    }
}

/// Identity of the source that supplied constraint evidence.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ConstraintProvenance {
    source_kind: ConstraintSourceKind,
    source_id: String,
}

impl ConstraintProvenance {
    /// Construct provenance with a non-empty source identity.
    pub fn new(
        source_kind: ConstraintSourceKind,
        source_id: impl Into<String>,
    ) -> Result<Self, ConstraintMetadataError> {
        let source_id = source_id.into();
        if source_id.trim().is_empty() {
            return Err(ConstraintMetadataError::InvalidEvidence {
                message: "constraint provenance source_id cannot be empty".to_string(),
            });
        }
        Ok(Self {
            source_kind,
            source_id,
        })
    }

    /// Return the source category.
    pub fn source_kind(&self) -> ConstraintSourceKind {
        self.source_kind
    }

    /// Return the source identity within that category.
    pub fn source_id(&self) -> &str {
        &self.source_id
    }
}

/// One provenance/enforcement assertion supporting a canonical constraint.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ConstraintEvidence {
    provenance: ConstraintProvenance,
    enforcement: ConstraintEnforcement,
}

impl ConstraintEvidence {
    /// Construct one evidence item.
    pub fn new(provenance: ConstraintProvenance, enforcement: ConstraintEnforcement) -> Self {
        Self {
            provenance,
            enforcement,
        }
    }

    /// Return the evidence provenance.
    pub fn provenance(&self) -> &ConstraintProvenance {
        &self.provenance
    }

    /// Return the enforcement strength established by this evidence.
    pub fn enforcement(&self) -> ConstraintEnforcement {
        self.enforcement
    }
}

/// Scalar value accepted by a canonical column constraint.
///
/// The variant describes the canonical scalar literal after adapter normalization. Metadata
/// adapters must not place opaque SQL expressions in `String`; syntax that cannot be reduced to
/// one of these scalar values must remain explicit as unsupported metadata. Non-integral numeric
/// values retain normalized source text so emission is deterministic and does not introduce
/// floating point rounding.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub enum ConstraintValue {
    /// SQL/metadata null literal.
    Null,
    /// Boolean literal.
    Boolean(bool),
    /// Signed integer literal.
    Integer(i64),
    /// Unsigned integer literal outside the signed range.
    UnsignedInteger(u64),
    /// Non-integral JSON number in its normalized source representation.
    Number(String),
    /// String literal.
    String(String),
}

/// A canonical non-null constraint on one column.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct NotNullConstraint {
    column: String,
    evidence: Vec<ConstraintEvidence>,
}

impl NotNullConstraint {
    fn new(
        column: String,
        evidence: Vec<ConstraintEvidence>,
    ) -> Result<Self, ConstraintMetadataError> {
        validate_column(&column)?;
        validate_evidence(&evidence)?;
        Ok(Self {
            column,
            evidence: normalized_evidence(evidence),
        })
    }

    /// Return the constrained column.
    pub fn column(&self) -> &str {
        &self.column
    }

    /// Return every coalesced evidence item in deterministic order.
    pub fn evidence(&self) -> &[ConstraintEvidence] {
        &self.evidence
    }
}

/// A canonical finite accepted-values constraint on one column.
///
/// The finite set constrains non-NULL values. SQL NULL remains admissible unless a separate
/// `NotNull` constraint applies to the same column.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct AcceptedValuesConstraint {
    column: String,
    values: Vec<ConstraintValue>,
    quote: bool,
    evidence: Vec<ConstraintEvidence>,
}

impl AcceptedValuesConstraint {
    fn new(
        column: String,
        mut values: Vec<ConstraintValue>,
        quote: bool,
        evidence: Vec<ConstraintEvidence>,
    ) -> Result<Self, ConstraintMetadataError> {
        validate_column(&column)?;
        validate_evidence(&evidence)?;
        values.sort();
        values.dedup();
        Ok(Self {
            column,
            values,
            quote,
            evidence: normalized_evidence(evidence),
        })
    }

    /// Return the constrained column.
    pub fn column(&self) -> &str {
        &self.column
    }

    /// Return accepted values in deterministic order.
    ///
    /// An empty value set is valid only as an explicit unsatisfiable intersection and is paired
    /// with an `unsatisfiable_accepted_values` diagnostic on its relation metadata.
    pub fn values(&self) -> &[ConstraintValue] {
        &self.values
    }

    /// Return whether string-like values were declared with quoting enabled by the evidence.
    pub fn quote(&self) -> bool {
        self.quote
    }

    /// Return every coalesced evidence item in deterministic order.
    pub fn evidence(&self) -> &[ConstraintEvidence] {
        &self.evidence
    }

    fn intersect(&mut self, other: &Self) {
        self.values
            .retain(|value| other.values.binary_search(value).is_ok());
        self.evidence.extend(other.evidence.iter().cloned());
        self.evidence.sort();
        self.evidence.dedup();
    }
}

/// Ordered columns forming a primary or unique key.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct KeyConstraint {
    columns: Vec<String>,
    evidence: Vec<ConstraintEvidence>,
}

impl KeyConstraint {
    fn new(
        columns: Vec<String>,
        evidence: Vec<ConstraintEvidence>,
    ) -> Result<Self, ConstraintMetadataError> {
        validate_columns(&columns)?;
        validate_evidence(&evidence)?;
        Ok(Self {
            columns,
            evidence: normalized_evidence(evidence),
        })
    }

    /// Return key columns in declared composite-key order.
    pub fn columns(&self) -> &[String] {
        &self.columns
    }

    /// Return every coalesced evidence item in deterministic order.
    pub fn evidence(&self) -> &[ConstraintEvidence] {
        &self.evidence
    }
}

/// Ordered local and referenced columns forming a foreign key.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ForeignKeyConstraint {
    columns: Vec<String>,
    referenced_relation: String,
    referenced_columns: Vec<String>,
    evidence: Vec<ConstraintEvidence>,
}

impl ForeignKeyConstraint {
    fn new(
        columns: Vec<String>,
        referenced_relation: String,
        referenced_columns: Vec<String>,
        evidence: Vec<ConstraintEvidence>,
    ) -> Result<Self, ConstraintMetadataError> {
        validate_columns(&columns)?;
        validate_columns(&referenced_columns)?;
        validate_evidence(&evidence)?;
        if referenced_relation.trim().is_empty() {
            return Err(ConstraintMetadataError::InvalidForeignKey {
                message: "referenced relation cannot be empty".to_string(),
            });
        }
        if columns.len() != referenced_columns.len() {
            return Err(ConstraintMetadataError::InvalidForeignKey {
                message: format!(
                    "foreign key has {} local columns but {} referenced columns",
                    columns.len(),
                    referenced_columns.len()
                ),
            });
        }

        Ok(Self {
            columns,
            referenced_relation,
            referenced_columns,
            evidence: normalized_evidence(evidence),
        })
    }

    /// Return local foreign-key columns in declared order.
    pub fn columns(&self) -> &[String] {
        &self.columns
    }

    /// Return the referenced relation identity.
    pub fn referenced_relation(&self) -> &str {
        &self.referenced_relation
    }

    /// Return referenced columns in corresponding declared order.
    pub fn referenced_columns(&self) -> &[String] {
        &self.referenced_columns
    }

    /// Return every coalesced evidence item in deterministic order.
    pub fn evidence(&self) -> &[ConstraintEvidence] {
        &self.evidence
    }
}

/// One canonical constraint on a relation or one of its columns.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub enum RelationConstraint {
    /// Declared primary key. Primary-key columns do not admit SQL NULL.
    PrimaryKey(KeyConstraint),
    /// Declared unique key. SQL NULL is admitted unless a separate non-null constraint applies.
    UniqueKey(KeyConstraint),
    /// Declared foreign-key relationship. SQL NULL is admitted unless a separate non-null
    /// constraint applies.
    ForeignKey(ForeignKeyConstraint),
    /// Declared non-null column constraint.
    NotNull(NotNullConstraint),
    /// Declared finite accepted-values column constraint. SQL NULL is admitted unless a separate
    /// non-null constraint applies.
    AcceptedValues(AcceptedValuesConstraint),
}

impl RelationConstraint {
    /// Construct a primary-key constraint.
    pub fn primary_key(
        columns: Vec<String>,
        evidence: Vec<ConstraintEvidence>,
    ) -> Result<Self, ConstraintMetadataError> {
        Ok(Self::PrimaryKey(KeyConstraint::new(columns, evidence)?))
    }

    /// Construct a unique-key constraint.
    pub fn unique_key(
        columns: Vec<String>,
        evidence: Vec<ConstraintEvidence>,
    ) -> Result<Self, ConstraintMetadataError> {
        Ok(Self::UniqueKey(KeyConstraint::new(columns, evidence)?))
    }

    /// Construct a foreign-key constraint.
    pub fn foreign_key(
        columns: Vec<String>,
        referenced_relation: impl Into<String>,
        referenced_columns: Vec<String>,
        evidence: Vec<ConstraintEvidence>,
    ) -> Result<Self, ConstraintMetadataError> {
        Ok(Self::ForeignKey(ForeignKeyConstraint::new(
            columns,
            referenced_relation.into(),
            referenced_columns,
            evidence,
        )?))
    }

    /// Construct a non-null column constraint.
    pub fn not_null(
        column: impl Into<String>,
        evidence: Vec<ConstraintEvidence>,
    ) -> Result<Self, ConstraintMetadataError> {
        Ok(Self::NotNull(NotNullConstraint::new(
            column.into(),
            evidence,
        )?))
    }

    /// Construct a finite accepted-values column constraint.
    pub fn accepted_values(
        column: impl Into<String>,
        values: Vec<ConstraintValue>,
        quote: bool,
        evidence: Vec<ConstraintEvidence>,
    ) -> Result<Self, ConstraintMetadataError> {
        Ok(Self::AcceptedValues(AcceptedValuesConstraint::new(
            column.into(),
            values,
            quote,
            evidence,
        )?))
    }

    /// Return primary/unique key columns, local foreign-key columns, or the constrained column.
    pub fn columns(&self) -> &[String] {
        match self {
            Self::PrimaryKey(key) | Self::UniqueKey(key) => key.columns(),
            Self::ForeignKey(key) => key.columns(),
            Self::NotNull(constraint) => std::slice::from_ref(&constraint.column),
            Self::AcceptedValues(constraint) => std::slice::from_ref(&constraint.column),
        }
    }

    /// Return all supporting evidence in deterministic order.
    pub fn evidence(&self) -> &[ConstraintEvidence] {
        match self {
            Self::PrimaryKey(key) | Self::UniqueKey(key) => key.evidence(),
            Self::ForeignKey(key) => key.evidence(),
            Self::NotNull(constraint) => constraint.evidence(),
            Self::AcceptedValues(constraint) => constraint.evidence(),
        }
    }

    /// Return whether this constraint admits SQL NULL in its constrained column or columns.
    ///
    /// Unique keys, foreign keys, and accepted-values constraints describe non-NULL values and
    /// therefore admit NULL unless a separate `NotNull` constraint is also present. Primary keys
    /// and `NotNull` constraints reject NULL.
    pub const fn admits_null(&self) -> bool {
        match self {
            Self::PrimaryKey(_) | Self::NotNull(_) => false,
            Self::UniqueKey(_) | Self::ForeignKey(_) | Self::AcceptedValues(_) => true,
        }
    }

    fn same_semantics(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::PrimaryKey(left), Self::PrimaryKey(right))
            | (Self::UniqueKey(left), Self::UniqueKey(right)) => left.columns == right.columns,
            (Self::ForeignKey(left), Self::ForeignKey(right)) => {
                left.columns == right.columns
                    && left.referenced_relation == right.referenced_relation
                    && left.referenced_columns == right.referenced_columns
            }
            (Self::NotNull(left), Self::NotNull(right)) => left.column == right.column,
            (Self::AcceptedValues(left), Self::AcceptedValues(right)) => {
                left.column == right.column
                    && left.values == right.values
                    && left.quote == right.quote
            }
            _ => false,
        }
    }

    fn merge_evidence(&mut self, other: &Self) {
        let incoming = other.evidence().iter().cloned();
        match self {
            Self::PrimaryKey(key) | Self::UniqueKey(key) => {
                key.evidence.extend(incoming);
                key.evidence.sort();
                key.evidence.dedup();
            }
            Self::ForeignKey(key) => {
                key.evidence.extend(incoming);
                key.evidence.sort();
                key.evidence.dedup();
            }
            Self::NotNull(constraint) => {
                constraint.evidence.extend(incoming);
                constraint.evidence.sort();
                constraint.evidence.dedup();
            }
            Self::AcceptedValues(constraint) => {
                constraint.evidence.extend(incoming);
                constraint.evidence.sort();
                constraint.evidence.dedup();
            }
        }
    }

    fn is_primary_key(&self) -> bool {
        matches!(self, Self::PrimaryKey(_))
    }

    fn with_referenced_relation(&self, referenced_relation: String) -> Self {
        match self {
            Self::PrimaryKey(key) => Self::PrimaryKey(key.clone()),
            Self::UniqueKey(key) => Self::UniqueKey(key.clone()),
            Self::ForeignKey(key) => Self::ForeignKey(ForeignKeyConstraint {
                columns: key.columns.clone(),
                referenced_relation,
                referenced_columns: key.referenced_columns.clone(),
                evidence: key.evidence.clone(),
            }),
            Self::NotNull(constraint) => Self::NotNull(constraint.clone()),
            Self::AcceptedValues(constraint) => Self::AcceptedValues(constraint.clone()),
        }
    }
}

/// Explicit metadata conflict or unsupported constraint detail.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ConstraintDiagnostic {
    code: String,
    message: String,
}

impl ConstraintDiagnostic {
    pub(crate) fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }

    /// Return the stable diagnostic code.
    pub fn code(&self) -> &str {
        &self.code
    }

    /// Return the human-readable diagnostic explanation.
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// Canonical constraints and related diagnostics for one relation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationConstraintSet {
    relation: String,
    constraints: Vec<RelationConstraint>,
    diagnostics: Vec<ConstraintDiagnostic>,
}

impl RelationConstraintSet {
    /// Construct validated metadata for one relation.
    ///
    /// Identical constraints are coalesced while preserving all evidence. Multiple distinct
    /// primary keys remain visible and produce an explicit conflict diagnostic.
    pub fn new(
        relation: impl Into<String>,
        constraints: Vec<RelationConstraint>,
    ) -> Result<Self, ConstraintMetadataError> {
        let relation = relation.into();
        if relation.trim().is_empty() {
            return Err(ConstraintMetadataError::InvalidRelation);
        }

        let mut result = Self {
            relation,
            constraints: Vec::new(),
            diagnostics: Vec::new(),
        };
        for constraint in constraints {
            result.add_constraint(constraint);
        }
        result.normalize();
        Ok(result)
    }

    /// Return the relation identity described by this metadata.
    pub fn relation(&self) -> &str {
        &self.relation
    }

    /// Return canonical constraints in deterministic order.
    pub fn constraints(&self) -> &[RelationConstraint] {
        &self.constraints
    }

    /// Return explicit conflicts or unsupported metadata details.
    pub fn diagnostics(&self) -> &[ConstraintDiagnostic] {
        &self.diagnostics
    }

    /// Reject constraint facts disproved by available typed schema evidence.
    ///
    /// Missing schemas are not negative evidence. Unverifiable facts remain unchanged,
    /// whereas a known missing column or incompatible scalar produces a diagnostic.
    pub(crate) fn validated_against_schemas(
        &self,
        schemas: &[crate::relation::RelationSchema],
    ) -> Self {
        let local = schemas.iter().find(|schema| schema.relation() == self.relation);
        let mut valid = Vec::new();
        let mut diagnostics = self.diagnostics.clone();

        for constraint in &self.constraints {
            let missing_local = local.and_then(|schema| {
                constraint.columns().iter().find(|column| {
                    !schema.columns().iter().any(|declared| declared.name() == column.as_str())
                })
            });
            if let Some(column) = missing_local {
                diagnostics.push(ConstraintDiagnostic::new(
                    "invalid_constraint_column",
                    format!(
                        "constraint on relation '{}' references undeclared column '{}'",
                        self.relation, column
                    ),
                ));
                continue;
            }

            if let RelationConstraint::ForeignKey(key) = constraint {
                if let Some(target) = schemas
                    .iter()
                    .find(|schema| schema.relation() == key.referenced_relation())
                {
                    if let Some(column) = key.referenced_columns().iter().find(|column| {
                        !target.columns().iter().any(|declared| declared.name() == column.as_str())
                    }) {
                        diagnostics.push(ConstraintDiagnostic::new(
                            "invalid_constraint_column",
                            format!(
                                "foreign key from '{}' references undeclared column '{}.{}'",
                                self.relation, key.referenced_relation(), column
                            ),
                        ));
                        continue;
                    }
                }
            }

            if let (Some(schema), RelationConstraint::AcceptedValues(accepted)) =
                (local, constraint)
            {
                if let Some(column) = schema
                    .columns()
                    .iter()
                    .find(|column| column.name() == accepted.column())
                {
                    if accepted
                        .values()
                        .iter()
                        .any(|value| !constraint_value_fits_type(value, column.data_type()))
                    {
                        diagnostics.push(ConstraintDiagnostic::new(
                            "incompatible_accepted_value",
                            format!(
                                "accepted values for '{}.{}' are incompatible with declared datatype '{}'",
                                self.relation,
                                accepted.column(),
                                column.data_type().kind()
                            ),
                        ));
                        continue;
                    }
                }
            }

            valid.push(constraint.clone());
        }

        let mut result = Self::new(self.relation.clone(), valid)
            .expect("existing constraint relation has already been validated");
        for diagnostic in diagnostics {
            result.add_diagnostic(diagnostic);
        }
        result
    }

    pub(crate) fn add_diagnostic(&mut self, diagnostic: ConstraintDiagnostic) {
        self.diagnostics.push(diagnostic);
        self.normalize();
    }

    pub(crate) fn merge(&mut self, other: &Self) {
        for constraint in &other.constraints {
            self.add_constraint(constraint.clone());
        }
        self.diagnostics.extend(other.diagnostics.iter().cloned());
        self.normalize();
    }

    pub(crate) fn map_relations<E>(
        &self,
        relation: String,
        mut resolve_reference: impl FnMut(&str) -> Result<String, E>,
    ) -> Result<Self, E> {
        let mut constraints = Vec::with_capacity(self.constraints.len());
        for constraint in &self.constraints {
            let mapped = match constraint {
                RelationConstraint::ForeignKey(foreign_key) => constraint.with_referenced_relation(
                    resolve_reference(foreign_key.referenced_relation())?,
                ),
                RelationConstraint::PrimaryKey(_)
                | RelationConstraint::UniqueKey(_)
                | RelationConstraint::NotNull(_)
                | RelationConstraint::AcceptedValues(_) => constraint.clone(),
            };
            constraints.push(mapped);
        }

        Ok(Self {
            relation,
            constraints,
            diagnostics: self.diagnostics.clone(),
        })
    }

    fn add_constraint(&mut self, constraint: RelationConstraint) {
        if let RelationConstraint::AcceptedValues(incoming) = &constraint {
            let merged_empty = {
                let existing = self
                    .constraints
                    .iter_mut()
                    .find_map(|existing| match existing {
                        RelationConstraint::AcceptedValues(existing)
                            if existing.column == incoming.column
                                && existing.quote == incoming.quote =>
                        {
                            Some(existing)
                        }
                        _ => None,
                    });
                existing.map(|existing| {
                    existing.intersect(incoming);
                    existing.values.is_empty()
                })
            };
            if let Some(empty) = merged_empty {
                if empty {
                    self.diagnostics.push(ConstraintDiagnostic::new(
                        "unsatisfiable_accepted_values",
                        format!(
                            "relation '{}' column '{}' has accepted-values evidence with an empty intersection",
                            self.relation, incoming.column
                        ),
                    ));
                }
                return;
            }

            if self.constraints.iter().any(|existing| {
                matches!(
                    existing,
                    RelationConstraint::AcceptedValues(existing)
                        if existing.column == incoming.column && existing.quote != incoming.quote
                )
            }) {
                self.diagnostics.push(ConstraintDiagnostic::new(
                    "conflicting_accepted_values_quoting",
                    format!(
                        "relation '{}' column '{}' has accepted-values evidence with conflicting quote semantics",
                        self.relation, incoming.column
                    ),
                ));
            }
            if incoming.values.is_empty() {
                self.diagnostics.push(ConstraintDiagnostic::new(
                    "unsatisfiable_accepted_values",
                    format!(
                        "relation '{}' column '{}' declares no accepted values",
                        self.relation, incoming.column
                    ),
                ));
            }
        }

        if let Some(existing) = self
            .constraints
            .iter_mut()
            .find(|existing| existing.same_semantics(&constraint))
        {
            existing.merge_evidence(&constraint);
            return;
        }

        if constraint.is_primary_key() {
            let conflicting = self
                .constraints
                .iter()
                .filter(|existing| existing.is_primary_key())
                .map(|existing| existing.columns().join(", "))
                .collect::<Vec<_>>();
            if !conflicting.is_empty() {
                self.diagnostics.push(ConstraintDiagnostic::new(
                    "conflicting_primary_key",
                    format!(
                        "relation '{}' has conflicting primary-key definitions ({}) and ({})",
                        self.relation,
                        conflicting.join("); ("),
                        constraint.columns().join(", ")
                    ),
                ));
            }
        }

        self.constraints.push(constraint);
    }

    fn normalize(&mut self) {
        self.constraints.sort();
        self.diagnostics.sort();
        self.diagnostics.dedup();
    }
}

/// Merge relation metadata by exact relation identity.
///
/// This is the adapter-neutral enrichment boundary used by metadata producers. Existing facts are
/// preserved, identical facts coalesce their evidence, and contradictory primary keys remain
/// Test only canonical scalar representations whose compatibility is provable without a
/// warehouse's implicit casts or collation settings.
fn constraint_value_fits_type(
    value: &ConstraintValue,
    data_type: &crate::data_type::DataType,
) -> bool {
    use crate::data_type::DataType;
    if matches!(value, ConstraintValue::Null) {
        return true;
    }
    match data_type {
        DataType::Nullable(inner) => constraint_value_fits_type(value, inner),
        DataType::Boolean => matches!(value, ConstraintValue::Boolean(_)),
        DataType::SignedInteger { bits } => {
            let integer = match value {
                ConstraintValue::Integer(value) => i128::from(*value),
                ConstraintValue::UnsignedInteger(value) => i128::from(*value),
                _ => return false,
            };
            bits.is_none_or(|bits| {
                bits >= 128 || (integer >= -(1_i128 << (bits - 1))
                    && integer < (1_i128 << (bits - 1)))
            })
        }
        DataType::UnsignedInteger { bits } => {
            let integer = match value {
                ConstraintValue::Integer(value) if *value >= 0 => *value as u128,
                ConstraintValue::UnsignedInteger(value) => u128::from(*value),
                _ => return false,
            };
            bits.is_none_or(|bits| bits >= 128 || integer < (1_u128 << bits))
        }
        DataType::Decimal { .. } | DataType::FloatingPoint { .. } => match value {
            ConstraintValue::Integer(_) | ConstraintValue::UnsignedInteger(_) => true,
            ConstraintValue::Number(number) => number.parse::<f64>().is_ok_and(f64::is_finite),
            _ => false,
        },
        DataType::String { .. }
        | DataType::Enum { .. }
        | DataType::Set { .. }
        | DataType::Uuid
        | DataType::Date
        | DataType::Time { .. }
        | DataType::Timestamp { .. }
        | DataType::Interval => matches!(value, ConstraintValue::String(_)),
        // Opaque and non-scalar types have no portable value-validation contract yet.
        // Retain evidence rather than claiming that it is invalid.
        DataType::Any
        | DataType::Unspecified
        | DataType::Custom { .. }
        | DataType::Binary { .. }
        | DataType::BitString { .. }
        | DataType::Json
        | DataType::Array { .. }
        | DataType::Map { .. }
        | DataType::Struct { .. }
        | DataType::Union { .. }
        | DataType::Table { .. }
        | DataType::Geometry { .. }
        | DataType::Regclass
        | DataType::TextSearchVector
        | DataType::TextSearchQuery
        | DataType::Trigger => true,
    }
}

/// explicit diagnostics rather than being overwritten.
pub fn merge_relation_constraint_sets(
    target: &mut Vec<RelationConstraintSet>,
    incoming: &[RelationConstraintSet],
) {
    for set in incoming {
        match target
            .iter_mut()
            .find(|existing| existing.relation == set.relation)
        {
            Some(existing) => existing.merge(set),
            None => target.push(set.clone()),
        }
    }
    target.sort_by(|left, right| left.relation.cmp(&right.relation));
}

/// Invalid canonical constraint metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ConstraintMetadataError {
    /// Relation identity is empty.
    InvalidRelation,
    /// Key columns are empty, duplicated, or contain an empty identifier.
    InvalidColumns {
        /// Explanation of the invalid column list.
        message: String,
    },
    /// Foreign-key metadata is incomplete or has mismatched arity.
    InvalidForeignKey {
        /// Explanation of the invalid relationship.
        message: String,
    },
    /// Provenance/evidence metadata is invalid.
    InvalidEvidence {
        /// Explanation of the invalid evidence.
        message: String,
    },
}

impl fmt::Display for ConstraintMetadataError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRelation => write!(formatter, "constraint relation cannot be empty"),
            Self::InvalidColumns { message } => {
                write!(formatter, "invalid constraint columns: {message}")
            }
            Self::InvalidForeignKey { message } => {
                write!(formatter, "invalid foreign key: {message}")
            }
            Self::InvalidEvidence { message } => {
                write!(formatter, "invalid constraint evidence: {message}")
            }
        }
    }
}

impl std::error::Error for ConstraintMetadataError {}

fn validate_column(column: &str) -> Result<(), ConstraintMetadataError> {
    if column.trim().is_empty() {
        return Err(ConstraintMetadataError::InvalidColumns {
            message: "column names cannot be empty".to_string(),
        });
    }
    Ok(())
}

fn validate_columns(columns: &[String]) -> Result<(), ConstraintMetadataError> {
    if columns.is_empty() {
        return Err(ConstraintMetadataError::InvalidColumns {
            message: "at least one column is required".to_string(),
        });
    }

    let mut seen = BTreeSet::new();
    for column in columns {
        if column.trim().is_empty() {
            return Err(ConstraintMetadataError::InvalidColumns {
                message: "column names cannot be empty".to_string(),
            });
        }
        if !seen.insert(column.as_str()) {
            return Err(ConstraintMetadataError::InvalidColumns {
                message: format!("column '{}' is duplicated", column),
            });
        }
    }
    Ok(())
}

fn validate_evidence(evidence: &[ConstraintEvidence]) -> Result<(), ConstraintMetadataError> {
    if evidence.is_empty() {
        return Err(ConstraintMetadataError::InvalidEvidence {
            message: "at least one evidence item is required".to_string(),
        });
    }
    Ok(())
}

fn normalized_evidence(mut evidence: Vec<ConstraintEvidence>) -> Vec<ConstraintEvidence> {
    evidence.sort();
    evidence.dedup();
    evidence
}

#[cfg(test)]
mod tests {
    use super::*;

    fn evidence(source_id: &str) -> ConstraintEvidence {
        ConstraintEvidence::new(
            ConstraintProvenance::new(ConstraintSourceKind::SqlDdl, source_id)
                .expect("test source id is valid"),
            ConstraintEnforcement::Unknown,
        )
    }

    #[test]
    fn identical_constraints_coalesce_evidence() {
        let first = RelationConstraint::unique_key(vec!["id".to_string()], vec![evidence("one")])
            .expect("constraint");
        let second = RelationConstraint::unique_key(vec!["id".to_string()], vec![evidence("two")])
            .expect("constraint");
        let mut set =
            RelationConstraintSet::new("orders", vec![first]).expect("constraint metadata");
        set.merge(
            &RelationConstraintSet::new("orders", vec![second]).expect("constraint metadata"),
        );

        assert_eq!(set.constraints().len(), 1);
        assert_eq!(set.constraints()[0].evidence().len(), 2);
    }

    #[test]
    fn conflicting_primary_keys_remain_explicit() {
        let first = RelationConstraint::primary_key(vec!["id".to_string()], vec![evidence("one")])
            .expect("constraint");
        let second =
            RelationConstraint::primary_key(vec!["other_id".to_string()], vec![evidence("two")])
                .expect("constraint");
        let set =
            RelationConstraintSet::new("orders", vec![first, second]).expect("constraint metadata");

        assert_eq!(set.constraints().len(), 2);
        assert_eq!(set.diagnostics()[0].code(), "conflicting_primary_key");
    }

    #[test]
    fn foreign_key_requires_matching_composite_arity() {
        let error = RelationConstraint::foreign_key(
            vec!["a".to_string(), "b".to_string()],
            "parent",
            vec!["id".to_string()],
            vec![evidence("fk")],
        )
        .expect_err("mismatched arity must fail");

        assert!(matches!(
            error,
            ConstraintMetadataError::InvalidForeignKey { .. }
        ));
    }

    #[test]
    fn canonical_constraints_define_null_admission() {
        let primary =
            RelationConstraint::primary_key(vec!["id".to_string()], vec![evidence("primary")])
                .expect("primary key");
        let unique =
            RelationConstraint::unique_key(vec!["email".to_string()], vec![evidence("unique")])
                .expect("unique key");
        let foreign = RelationConstraint::foreign_key(
            vec!["parent_id".to_string()],
            "parents",
            vec!["id".to_string()],
            vec![evidence("foreign")],
        )
        .expect("foreign key");
        let not_null =
            RelationConstraint::not_null("email", vec![evidence("not-null")]).expect("not null");
        let accepted = RelationConstraint::accepted_values(
            "status",
            vec![ConstraintValue::String("ready".to_string())],
            true,
            vec![evidence("accepted")],
        )
        .expect("accepted values");

        assert!(!primary.admits_null());
        assert!(unique.admits_null());
        assert!(foreign.admits_null());
        assert!(!not_null.admits_null());
        assert!(accepted.admits_null());
    }
}
