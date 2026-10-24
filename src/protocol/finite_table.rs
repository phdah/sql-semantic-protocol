//! Parser-independent, finite, source-free relational table values.
//!
//! Literal rows are the authoritative operator facts. SQL AST nodes never
//! escape the analysis boundary and consumers must not reconstruct a table
//! from an opaque expression or from per-column domains.

use super::*;
use std::collections::BTreeSet;

/// Supported deterministic constructor for a complete literal table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FiniteTableKind {
    /// An ordered source of literal VALUES tuples (SQL output remains a bag).
    Values,
    /// UNNEST of an attested, literal integer array.
    Unnest,
    /// EXPLODE of an attested, literal integer array.
    Explode,
    /// Schema-attested, row-preserving finite wildcard projection.
    Star,
    /// Fixed-key integer SUM pivot of a complete finite source bag.
    Pivot,
    /// Fixed-column unpivot of a complete finite source bag.
    Unpivot,
}

impl FiniteTableKind {
    /// Stable serialization identity for this row constructor.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Values => "values",
            Self::Unnest => "unnest",
            Self::Explode => "explode",
            Self::Star => "star",
            Self::Pivot => "pivot",
            Self::Unpivot => "unpivot",
        }
    }
}

/// An explicitly attested finite table output datatype.
///
/// Unknown means the row values are known (for example all NULL VALUES)
/// but there is no authoritative scalar datatype evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FiniteColumnType {
    /// An integer column, including a typed empty array's element column.
    Integer,
    /// Text values with exact literal identities.
    Text,
    /// No safe type evidence exists despite a completely known row bag.
    Unknown,
}

impl FiniteColumnType {
    /// Stable wire name of the canonical datatype evidence.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Integer => "integer",
            Self::Text => "text",
            Self::Unknown => "unknown",
        }
    }
}

/// A complete source-free relation with stable producer-row identities.
///
/// Rows are positional and retain multiplicity, even for identical tuples.
/// The caller must have verified that every expression is a typed integer or
/// SQL NULL and that no implicit type-coercion law is needed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FiniteTable {
    kind: FiniteTableKind,
    columns: Vec<String>,
    column_types: Vec<FiniteColumnType>,
    rows: Vec<Vec<LiteralExpression>>,
    // Complete input snapshot and positional contributors for finite transforms.
    source: Option<Box<FiniteTable>>,
    contributors: Vec<Vec<usize>>,
}

impl FiniteTable {
    pub(crate) fn new(
        kind: FiniteTableKind,
        columns: Vec<String>,
        column_types: Vec<FiniteColumnType>,
        rows: Vec<Vec<LiteralExpression>>,
    ) -> Option<Self> {
        let unique = columns.iter().collect::<BTreeSet<_>>();
        if columns.is_empty()
            || column_types.len() != columns.len()
            || unique.len() != columns.len()
            || columns.iter().any(|column| column.trim().is_empty())
            || rows.iter().any(|row| row.len() != columns.len())
            || rows.iter().any(|row| {
                row.iter()
                    .enumerate()
                    .any(|(index, literal)| match column_types[index] {
                        FiniteColumnType::Unknown => literal.literal_type() != LiteralType::Null,
                        FiniteColumnType::Integer => !matches!(
                            literal.literal_type(),
                            LiteralType::Integer | LiteralType::Null
                        ),
                        FiniteColumnType::Text => !matches!(
                            literal.literal_type(),
                            LiteralType::String | LiteralType::Null
                        ),
                    })
            })
            || rows.iter().flatten().any(|literal| {
                match (literal.literal_type(), literal.value()) {
                    (LiteralType::Null, LiteralValue::Null) => false,
                    (LiteralType::Integer, LiteralValue::Number(value)) => {
                        value.parse::<i128>().is_err()
                    }
                    (LiteralType::String, LiteralValue::Text(_)) => false,
                    _ => true,
                }
            })
        {
            return None;
        }
        Some(Self {
            kind,
            columns,
            column_types,
            rows,
            source: None,
            contributors: Vec::new(),
        })
    }

    /// Attach a complete canonical input bag and the source row indices that
    /// jointly produce each output row. Incomplete provenance cannot be exact.
    pub(crate) fn with_source(
        mut self,
        source: FiniteTable,
        contributors: Vec<Vec<usize>>,
    ) -> Option<Self> {
        if contributors.len() != self.rows.len()
            || contributors.iter().any(|indices| {
                indices.is_empty() || indices.iter().any(|index| *index >= source.rows.len())
            })
        {
            return None;
        }
        self.source = Some(Box::new(source));
        self.contributors = contributors;
        Some(self)
    }

    /// Original complete finite physical input, if this is a transformation.
    pub fn source(&self) -> Option<&FiniteTable> {
        self.source.as_deref()
    }

    /// Source positional row indices for each terminal row.
    pub fn contributors(&self) -> &[Vec<usize>] {
        &self.contributors
    }

    /// The canonical table-producing operator.
    pub fn kind(&self) -> FiniteTableKind {
        self.kind
    }

    /// Ordered, unambiguous output column identities.
    pub fn columns(&self) -> &[String] {
        &self.columns
    }

    /// Output datatype evidence retained even when the result bag is empty.
    pub fn column_types(&self) -> &[FiniteColumnType] {
        &self.column_types
    }

    /// Complete positional rows. Input order identifies producers, not SQL
    /// output ORDER BY semantics.
    pub fn rows(&self) -> &[Vec<LiteralExpression>] {
        &self.rows
    }
}
