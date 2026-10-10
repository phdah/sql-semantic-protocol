//! Public library entry points for SQL Semantic Protocol.
//!
//! - analyze_sql parses one SQL string with a caller-supplied dialect.
//! - analyze_inputs analyzes SQL input units, links them, and composes transitive semantics.
//! - analyze_configured_inputs_with_catalog adds optional catalog/schema-aware relation resolution and typed source schemas.
//! - parse_dbt_manifest, parse_dbt_catalog, analyze_dbt_artifacts, and analyze_dbt_manifest_with_schemas adapt dbt artifacts, including canonical key and column constraints, into the same core analysis path.
//! - With the `odcs` feature, parse_odcs_yaml and parse_odcs_documents adapt ODCS v3.2 YAML contracts into canonical schema and constraint evidence.
//! - canonical constraint types expose primary, unique, foreign-key, not-null, and accepted-values metadata with provenance.
//! - select_targets projects a completed bundle onto named outcomes and their in-bundle ancestors.
//! - parse_analysis_manifest validates the versioned declarative analysis-manifest contract.
//! - to_json and to_bundle_json serialize the one active protocol contract without exposing parser AST types.
//! - to_openlineage_json exports representable dataset and field lineage as OpenLineage DatasetEvents.
//! - dialect_from_name resolves built-in dialects for consumers without a direct sqlparser dependency.
//! - protocol contains the parser-independent public protocol model, including normalized
//!   expressions, predicates, row-condition exactness, and explicit unknown/unsupported values.
//! - JoinWitness and JoinWitnessDirection describe matched, unmatched and null-extended input obligations.
//! - SubqueryMembershipWitness describes EXISTS and IN source-row membership and NULL behavior.
//! - BooleanWitness describes coupled single-row boolean obligations and their proof boundaries.
//! - WindowWitness, WindowOrderKey, WindowRankCase, and WindowWitnessDirection describe
//!   source-partition and strict-order obligations for ranked-row membership.
//! - OutcomeGoal, OutputDistribution, and EvaluatedOutcomeGoal describe optional caller
//!   requests and their independently proven output-row feasibility.
//! - OutcomeWitness describes source-complete integer-key, group, join, window and set
//!   cardinality constructions derived from existing operator witness contracts.
//! - WriteStateEffect describes conservative INSERT, UPDATE, DELETE and MERGE before/after obligations.
//! - ConstructiveWitness and local_constructive_witnesses normalize proven operator-local
//!   source obligations without pretending to solve full physical-source DAGs.
//! - BagLaw transfers closed-world count bounds through proven bag operators without
//!   claiming source construction or guessing unsupported NULL/key semantics.
//! - physical_source_plan builds a reference-based physical dependency DAG and conservatively proves single-row WHERE witnesses across transparent producer chains.
//! - BagKeyHistogram and equijoin_key_histogram prove fully controlled per-key
//!   multiplicities with NULL, duplicates and shared physical aliases.

mod analysis;
mod bag_histogram;
mod bag_semantics;
mod boolean_witness;
mod bundle;
mod composition;
mod constraints;
mod constructive;
mod data_type;
mod dbt;
mod domain;
mod emission;
mod group_witness;
mod join_witness;
mod manifest;
#[cfg(feature = "odcs")]
mod odcs;
mod openlineage;
mod outcome_goals;
mod outcome_proofs;
mod parser;
mod physical_realization;
pub mod protocol;
mod relation;
mod subquery_witness;
mod window_witness;

use std::fmt;

use sqlparser::dialect::{dialect_from_str, Dialect};

pub use analysis::AnalysisError;
pub use bag_histogram::{
    equijoin_key_histogram, BagHistogramProof, BagKeyExpressionIdentity, BagKeyHistogram,
};
pub use bag_semantics::{
    BagCountProof, BagCountTarget, BagEvidence, BagJoinKeys, BagLaw, BagPopulationIdentity,
    BagScope, BagSourceIdentity, BagTupleIdentity,
};
pub use boolean_witness::{
    BooleanOperands, BooleanRowConstraint, BooleanTruthCase, BooleanWitness,
    BooleanWitnessDirection,
};
pub use bundle::{
    analyze_configured_inputs, analyze_configured_inputs_with_catalog,
    analyze_configured_inputs_with_resolver, analyze_inputs, select_targets, AnalysisBundle,
    AnalysisGraph, AnalyzedInput, ComposedBooleanWitness, ComposedGroupWitness, ComposedJoinColumn,
    ComposedJoinEquality, ComposedSemantics, ComposedSetOperation, ComposedSubqueryWitness,
    ComposedWindowWitness, CompositionDiagnostic, CompositionFailureReason,
    ConfiguredInputAnalysisError, ConfiguredSqlInput, DatasetRef, GraphComponent, GraphEdge,
    GroupBoundaryKind, InputAnalysisError, LayerWriteStateEffect, RelationResolution,
    ResolvedComposedSemantics, SqlInput, SqlInputSource, TargetSelectionError, TransformationLayer,
    UnresolvedComposedSemantics,
};
pub use constraints::{
    merge_relation_constraint_sets, AcceptedValuesConstraint, ConstraintDiagnostic,
    ConstraintEnforcement, ConstraintEvidence, ConstraintMetadataError, ConstraintProvenance,
    ConstraintSourceKind, ConstraintValue, ForeignKeyConstraint, KeyConstraint, NotNullConstraint,
    RelationConstraint, RelationConstraintSet,
};
pub use constructive::{
    local_constructive_witnesses, local_pending_producers, ClosedWorldCoverage,
    ConstructiveWitness, CountBounds, ProofStrength, RowQuantifier, RowVariable, WitnessBoundary,
    WitnessCase, WitnessDirection, WitnessFormula, WitnessObligation, WitnessOperator, WitnessTerm,
};
pub use data_type::{parse_data_type, DataType, DataTypeField, DataTypeParseError, EnumValue};
pub use dbt::{
    analyze_dbt_artifacts, analyze_dbt_manifest, analyze_dbt_manifest_with_schemas,
    parse_dbt_catalog, parse_dbt_manifest, DbtArtifactsError, DbtCatalog, DbtCatalogError,
    DbtManifest, DbtManifestError, SUPPORTED_DBT_CATALOG_VERSIONS, SUPPORTED_DBT_MANIFEST_VERSIONS,
};
pub use emission::{to_bundle_json, to_json};
pub use group_witness::{
    GroupAggregate, GroupValueTest, GroupWitness, GroupWitnessCase, GroupWitnessDirection,
};
pub use join_witness::{
    JoinSide, JoinWitness, JoinWitnessCase, JoinWitnessDirection, JoinWitnessShape,
};
pub use manifest::{
    parse_analysis_manifest, AnalysisManifest, ManifestError, ManifestInput, ManifestInputSource,
    ManifestOutputScope, ManifestRelationContext, ANALYSIS_MANIFEST_VERSION,
};
#[cfg(feature = "odcs")]
pub use odcs::{
    parse_odcs_documents, parse_odcs_yaml, OdcsDiagnostic, OdcsDocument, OdcsError, OdcsMetadata,
    SUPPORTED_ODCS_API_VERSION,
};
pub use openlineage::{to_openlineage_json, OpenLineageExportError};
pub use outcome_goals::{
    EvaluatedOutcomeGoal, OutcomeGoal, OutcomeGoalError, OutcomeGoalStatus, OutputDistribution,
    OutputValueCount,
};
pub use outcome_proofs::{OutcomeWitness, SourceColumnValues};
pub use parser::ParseError;
pub use physical_realization::{
    physical_joint_row_count_plan, physical_rejected_row_count_plan, physical_row_count_plan,
    physical_source_plan, PhysicalPlanNode, PhysicalPlanRef, PhysicalProofGap, PhysicalSourcePlan,
};
pub use protocol::{
    AggregateArgument, AggregateFunctionExpression, Aggregation, BetweenPredicate,
    BinaryExpression, BinaryOperator, Bound, CaseBranch, CaseExpression,
    CaseSourceDomainAlternative, CaseSourceDomains, ColumnDomain, ColumnExpression, ColumnRef,
    ComparisonAssumption, ComparisonOperator, ComparisonPredicate, ConditionClause,
    ConditionExactness, ConditionExactnessStatus, ConditionalCondition, Diagnostic, DiagnosticArea,
    DiagnosticSeverity, ExistsPredicate, Expression, FunctionExpression, GroupBy,
    GroupingExpression, InPredicate, InSubqueryPredicate, IsNullPredicate, Join, JoinKind,
    LikePrefixPredicate, LineageSource, LiteralExpression, LiteralType, LiteralValue,
    LogicalPredicate, MergeAction, MergeAssignment, MergeClause, MergeMatchKind, NotPredicate,
    Output, OutputColumn, Predicate, Predicates, Protocol, ProtocolSource, ProtocolStatement,
    QueryStatement, RangesDomain, RelationRef, ResidualCondition, ResidualConditionReason,
    ScalarSubqueryExpression, SetBranch, SetDomain, SetMode, SetMultiplicityRule, SetOperand,
    SetOperation, SetOperator, SetQuantifier, SetWitnessBoundary, SetWitnessCase,
    SetWitnessDirection, SetWitnessObligation, SignedIntegerCastExpression, SourceRelation,
    SubquerySemantics, UnaryExpression, UnaryOperator, UnknownDomain, UnknownSemantic,
    UnsupportedSemantic, UnsupportedStatement, ValueDomain, ValueRange, WindowFrame,
    WindowFrameBound, WindowFrameUnits, WindowFunctionExpression, WindowOrderExpression,
    WindowSpecification, WriteAffectedRows, WriteCardinalityRule, WriteCountError,
    WriteEffectAction, WriteEffectBranch, WriteIdempotence, WriteInitialState, WriteKind,
    WriteOperation, WritePostState, WriteRowCounts, WriteStateEffect, WriteUncertainty, WriteValue,
    PROTOCOL_VERSION,
};
pub use relation::{
    RelationCatalog, RelationContext, RelationMetadataError, RelationResolutionError,
    RelationResolver, RelationSchema, SchemaColumn, SchemaSourceKind, TimestampZone,
};
pub use subquery_witness::{
    SubqueryCorrelation, SubqueryMembershipCase, SubqueryMembershipDirection,
    SubqueryMembershipKind, SubqueryMembershipWitness,
};
pub use window_witness::{WindowOrderKey, WindowRankCase, WindowWitness, WindowWitnessDirection};

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
