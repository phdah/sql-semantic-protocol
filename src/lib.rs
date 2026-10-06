//! Public library entry points for SQL Semantic Protocol.
//!
//! - analyze_sql parses one SQL string with a caller-supplied dialect.
//! - analyze_inputs analyzes SQL input units, links them, and composes transitive semantics.
//! - analyze_configured_inputs_with_catalog adds optional catalog/schema-aware relation resolution and typed source schemas.
//! - parse_dbt_manifest, parse_dbt_catalog, and analyze_dbt_artifacts adapt dbt artifacts, including canonical key and column constraints, into the same core analysis path.
//! - canonical constraint types expose primary, unique, foreign-key, not-null, and accepted-values metadata with provenance.
//! - select_targets projects a completed bundle onto named outcomes and their in-bundle ancestors.
//! - parse_analysis_manifest validates the versioned declarative analysis-manifest contract.
//! - to_json and to_bundle_json serialize the one active protocol contract without exposing parser AST types.
//! - to_openlineage_json exports representable dataset and field lineage as OpenLineage DatasetEvents.
//! - dialect_from_name resolves built-in dialects for consumers without a direct sqlparser dependency.
//! - protocol contains the parser-independent public protocol model, including normalized
//!   expressions and predicates plus explicit unknown and unsupported semantic values.

mod analysis;
mod bundle;
mod composition;
mod constraints;
mod data_type;
mod dbt;
mod domain;
mod emission;
mod manifest;
mod openlineage;
mod parser;
pub mod protocol;
mod relation;

use std::fmt;

use sqlparser::dialect::{dialect_from_str, Dialect};

pub use analysis::AnalysisError;
pub use bundle::{
    analyze_configured_inputs, analyze_configured_inputs_with_catalog,
    analyze_configured_inputs_with_resolver, analyze_inputs, select_targets, AnalysisBundle,
    AnalysisGraph, AnalyzedInput, ComposedSemantics, CompositionDiagnostic,
    CompositionFailureReason, ConfiguredInputAnalysisError, ConfiguredSqlInput, DatasetRef,
    GraphComponent, GraphEdge, InputAnalysisError, RelationResolution, ResolvedComposedSemantics,
    SqlInput, SqlInputSource, TargetSelectionError, TransformationLayer,
    UnresolvedComposedSemantics,
};
pub use constraints::{
    merge_relation_constraint_sets, AcceptedValuesConstraint, ConstraintDiagnostic,
    ConstraintEnforcement, ConstraintEvidence, ConstraintMetadataError, ConstraintProvenance,
    ConstraintSourceKind, ConstraintValue, ForeignKeyConstraint, KeyConstraint, NotNullConstraint,
    RelationConstraint, RelationConstraintSet,
};
pub use data_type::{parse_data_type, DataType, DataTypeField, DataTypeParseError, EnumValue};
pub use dbt::{
    analyze_dbt_artifacts, analyze_dbt_manifest, parse_dbt_catalog, parse_dbt_manifest,
    DbtArtifactsError, DbtCatalog, DbtCatalogError, DbtManifest, DbtManifestError,
    SUPPORTED_DBT_CATALOG_VERSIONS, SUPPORTED_DBT_MANIFEST_VERSIONS,
};
pub use emission::{to_bundle_json, to_json};
pub use manifest::{
    parse_analysis_manifest, AnalysisManifest, ManifestError, ManifestInput, ManifestInputSource,
    ManifestOutputScope, ManifestRelationContext, ANALYSIS_MANIFEST_VERSION,
};
pub use openlineage::{to_openlineage_json, OpenLineageExportError};
pub use parser::ParseError;
pub use protocol::{
    AggregateArgument, AggregateFunctionExpression, Aggregation, BetweenPredicate,
    BinaryExpression, BinaryOperator, Bound, CaseBranch, CaseExpression,
    CaseSourceDomainAlternative, CaseSourceDomains, ColumnDomain, ColumnExpression, ColumnRef,
    ComparisonOperator, ComparisonPredicate, Diagnostic, DiagnosticArea, DiagnosticSeverity,
    ExistsPredicate, Expression, FunctionExpression, GroupBy, GroupingExpression, InPredicate,
    InSubqueryPredicate, IsNullPredicate, Join, JoinKind, LineageSource, LiteralExpression,
    LiteralType, LiteralValue, LogicalPredicate, MergeAction, MergeAssignment, MergeClause,
    MergeMatchKind, NotPredicate, Output, OutputColumn, Predicate, Predicates, Protocol,
    ProtocolSource, ProtocolStatement, QueryStatement, RangesDomain, RelationRef,
    ScalarSubqueryExpression, SetDomain, SetMode, SetOperand, SetOperation, SetOperator,
    SetQuantifier, SourceRelation, SubquerySemantics, UnaryExpression, UnaryOperator,
    UnknownDomain, UnknownSemantic, UnsupportedSemantic, UnsupportedStatement, ValueDomain,
    ValueRange, WindowFrame, WindowFrameBound, WindowFrameUnits, WindowFunctionExpression,
    WindowOrderExpression, WindowSpecification, WriteKind, WriteOperation, WriteValue,
    PROTOCOL_VERSION,
};
pub use relation::{
    RelationCatalog, RelationContext, RelationMetadataError, RelationResolutionError,
    RelationResolver, RelationSchema, SchemaColumn,
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

pub(crate) fn analyze_sql_with_catalog(
    sql: &str,
    dialect_name: &str,
    dialect: &dyn Dialect,
    catalog: &RelationCatalog,
    relation_context: Option<&RelationContext>,
) -> Result<Protocol, Error> {
    let parsed = parser::parse_sql(sql, dialect).map_err(Error::Parse)?;
    analysis::analyze_with_catalog(parsed, dialect_name, catalog, relation_context)
        .map_err(Error::Analysis)
}

/// Resolve a built-in sqlparser dialect by name for library consumers.
///
/// This keeps consumers from depending directly on sqlparser only to select a dialect before
/// calling the protocol analyzer.
pub fn dialect_from_name(name: &str) -> Option<Box<dyn Dialect>> {
    dialect_from_str(name.to_ascii_lowercase())
}
