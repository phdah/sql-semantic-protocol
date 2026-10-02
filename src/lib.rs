//! Public library entry points for SQL Semantic Protocol.
//!
//! - analyze_sql parses one SQL string with a caller-supplied dialect.
//! - analyze_inputs analyzes SQL input units and builds their deterministic relation dependency graph.
//! - to_json and to_bundle_json serialize the one active protocol contract without exposing parser AST types.
//! - protocol contains the parser-independent public protocol model, including normalized
//!   expressions and predicates plus explicit unknown and unsupported semantic values.

mod analysis;
mod bundle;
mod domain;
mod emission;
mod parser;
pub mod protocol;

use std::fmt;

use sqlparser::dialect::Dialect;

pub use analysis::AnalysisError;
pub use bundle::{
    analyze_inputs, AnalysisBundle, AnalysisGraph, AnalyzedInput, CompositionDiagnostic,
    DatasetRef, GraphComponent, GraphEdge, InputAnalysisError, RelationResolution, SqlInput,
    SqlInputSource, TransformationLayer,
};
pub use emission::{to_bundle_json, to_json};
pub use parser::ParseError;
pub use protocol::{
    BetweenPredicate, BinaryExpression, BinaryOperator, Bound, ColumnDomain, ColumnExpression,
    ColumnRef, ComparisonOperator, ComparisonPredicate, Diagnostic, DiagnosticArea,
    DiagnosticSeverity, Expression, FunctionExpression, InPredicate, IsNullPredicate, Join,
    JoinKind, LineageSource, LiteralExpression, LiteralType, LiteralValue, LogicalPredicate,
    NotPredicate, Output, OutputColumn, Predicate, Predicates, Protocol, ProtocolSource,
    ProtocolStatement, QueryStatement, RangesDomain, RelationRef, SetDomain, SetMode,
    SourceRelation, UnaryExpression, UnaryOperator, UnknownDomain, UnknownSemantic,
    UnsupportedSemantic, UnsupportedStatement, ValueDomain, ValueRange, PROTOCOL_VERSION,
};

/// Error returned when SQL cannot be converted into protocol domain values.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// SQL parsing failed before semantic analysis could begin.
    Parse(ParseError),
    /// Parsing succeeded but semantic analysis could not produce a protocol value.
    Analysis(AnalysisError),
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => write!(formatter, "{error}"),
            Self::Analysis(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Parse(error) => Some(error),
            Self::Analysis(error) => Some(error),
        }
    }
}

/// Parse and analyze SQL using the dialect selected by the caller.
///
/// The dialect name is copied into the protocol source metadata. The dialect implementation is
/// used only at the parsing boundary; sqlparser AST values never appear in the returned protocol.
pub fn analyze_sql(
    sql: &str,
    dialect_name: &str,
    dialect: &dyn Dialect,
) -> Result<Protocol, Error> {
    let parsed = parser::parse_sql(sql, dialect).map_err(Error::Parse)?;
    analysis::analyze(parsed, dialect_name).map_err(Error::Analysis)
}
