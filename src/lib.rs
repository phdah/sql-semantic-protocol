//! Public library entry points for SQL Semantic Protocol.
//!
//! - analyze_sql parses one SQL string with a caller-supplied dialect.
//! - analyze_inputs analyzes SQL input units, links them, and composes transitive semantics.
//! - analyze_configured_inputs_with_catalog adds optional catalog/schema-aware relation resolution.
//! - select_targets projects a completed bundle onto named outcomes and their in-bundle ancestors.
//! - parse_analysis_manifest validates the versioned declarative analysis-manifest contract.
//! - to_json and to_bundle_json serialize the one active protocol contract without exposing parser AST types.
//! - to_openlineage_json exports representable dataset and field lineage as OpenLineage DatasetEvents.
//! - protocol contains the parser-independent public protocol model, including normalized
//!   expressions and predicates plus explicit unknown and unsupported semantic values.

mod analysis;
mod bundle;
mod composition;
mod domain;
mod emission;
mod manifest;
mod openlineage;
mod parser;
pub mod protocol;
mod relation;

use std::fmt;

use sqlparser::dialect::Dialect;

pub use analysis::AnalysisError;
pub use bundle::{
    analyze_configured_inputs, analyze_configured_inputs_with_catalog, analyze_inputs,
    select_targets, AnalysisBundle, AnalysisGraph,
    AnalyzedInput, ComposedSemantics, CompositionDiagnostic, CompositionFailureReason,
    ConfiguredInputAnalysisError, ConfiguredSqlInput, DatasetRef, GraphComponent, GraphEdge,
    InputAnalysisError, RelationResolution, ResolvedComposedSemantics, SqlInput, SqlInputSource,
    TargetSelectionError, TransformationLayer, UnresolvedComposedSemantics,
};
pub use emission::{to_bundle_json, to_json};
pub use manifest::{
    parse_analysis_manifest, AnalysisManifest, ManifestError, ManifestInput, ManifestInputSource,
    ManifestOutputScope, ANALYSIS_MANIFEST_VERSION,
};
pub use openlineage::{to_openlineage_json, OpenLineageExportError};
pub use parser::ParseError;
pub use relation::{
    RelationCatalog, RelationContext, RelationMetadataError, RelationResolutionError,
};
pub use protocol::{
    AggregateArgument, AggregateFunctionExpression, Aggregation, BetweenPredicate,
    BinaryExpression, BinaryOperator, Bound, CaseBranch, CaseExpression, ColumnDomain,
    ColumnExpression, ColumnRef, ComparisonOperator, ComparisonPredicate, Diagnostic,
    DiagnosticArea, DiagnosticSeverity, ExistsPredicate, Expression, FunctionExpression, GroupBy,
    GroupingExpression, InPredicate, InSubqueryPredicate, IsNullPredicate, Join, JoinKind,
    LineageSource, LiteralExpression, LiteralType, LiteralValue, LogicalPredicate, MergeAction,
    MergeAssignment, MergeClause, MergeMatchKind, NotPredicate, Output, OutputColumn, Predicate,
    Predicates, Protocol, ProtocolSource, ProtocolStatement, QueryStatement, RangesDomain,
    RelationRef, ScalarSubqueryExpression, SetDomain, SetMode, SetOperand, SetOperation,
    SetOperator, SetQuantifier, SourceRelation, SubquerySemantics, UnaryExpression, UnaryOperator,
    UnknownDomain, UnknownSemantic, UnsupportedSemantic, UnsupportedStatement, ValueDomain,
    ValueRange, WindowFrame, WindowFrameBound, WindowFrameUnits, WindowFunctionExpression,
    WindowOrderExpression, WindowSpecification, WriteKind, WriteOperation, WriteValue,
    PROTOCOL_VERSION,
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
