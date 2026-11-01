//! Semantic-analysis boundary.
//!
//! This module is the only layer that converts sqlparser AST statements into public protocol
//! values. Unsupported semantics are retained explicitly rather than silently discarded.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    str::FromStr,
};

use serde_json::Number;
use sqlparser::ast::{
    BinaryOperator as SqlBinaryOperator, CastKind as SqlCastKind, ColumnOption,
    ConstraintCharacteristics, CreateTable as SqlCreateTable, Delete as SqlDelete,
    Distinct as SqlDistinct, DuplicateTreatment, Expr, FromTable as SqlFromTable, Function,
    FunctionArg, FunctionArgExpr, FunctionArguments, GroupByExpr,
    GroupByWithModifier as SqlGroupByWithModifier, IndexColumn, Insert as SqlInsert,
    Join as SqlJoin, JoinConstraint, JoinOperator, MergeAction as SqlMergeAction,
    MergeClause as SqlMergeClause, MergeClauseKind as SqlMergeClauseKind, MergeInsertKind,
    NamedWindowDefinition, NamedWindowExpr, Query as SqlQuery, Select, SelectItem, SetExpr,
    SetOperator as SqlSetOperator, SetQuantifier as SqlSetQuantifier, Statement as SqlStatement,
    TableConstraint, TableFactor, TableObject, TableWithJoins, UnaryOperator as SqlUnaryOperator,
    Value, WindowFrame as SqlWindowFrame, WindowFrameBound as SqlWindowFrameBound,
    WindowFrameUnits as SqlWindowFrameUnits, WindowSpec as SqlWindowSpec, WindowType,
};

use crate::constraints::{
    merge_relation_constraint_sets, ConstraintDiagnostic, ConstraintEnforcement,
    ConstraintEvidence, ConstraintMetadataError, ConstraintProvenance, ConstraintSourceKind,
    ConstraintValue, RelationConstraint, RelationConstraintSet,
};
use crate::data_type::DataType;
use crate::domain::{
    derive_case_source_domains, derive_column_domains, intersect_case_domain_values,
    intersect_domains, predicate_domains_contain_unknown, refine_column_domains_from_equalities,
    resolve_column, union_domains,
};
use crate::parser::ParsedSql;
use crate::protocol::{
    AggregateArgument, AggregateFunctionExpression, Aggregation, BetweenPredicate,
    BinaryExpression, BinaryOperator, Bound, CaseBranch, CaseExpression,
    CaseSourceDomainAlternative, CaseSourceDomains, ColumnDomain, ColumnExpression, ColumnRef,
    ComparisonAssumption, ComparisonOperator, ComparisonPredicate, ConditionClause,
    ConditionExactness, ConditionalCondition, Diagnostic, DiagnosticArea, DiagnosticSeverity,
    ExistsPredicate, Expression, FunctionExpression, GroupBy, GroupingExpression, InPredicate,
    InSubqueryOperand, InSubqueryPredicate, InSubqueryTuple, IsNullPredicate, Join as ProtocolJoin,
    JoinExecution, JoinKind, LikePrefixPredicate, LineageSource, LiteralExpression, LiteralType,
    LiteralValue, LogicalPredicate, MergeAction as ProtocolMergeAction, MergeAssignment,
    MergeClause as ProtocolMergeClause, MergeMatchKind, NotPredicate, Output, OutputColumn,
    Predicate, Predicates, Protocol, ProtocolStatement, QuantifiedSubqueryPredicate,
    QueryStatement, RelationRef, ResidualCondition, ResidualConditionReason, RowConditions,
    SamplingMethod, SamplingQuantity, SamplingSeed, SamplingSeedSyntax, SamplingSyntax,
    SamplingUnit, ScalarSubqueryExpression, SetBranch, SetMode, SetOperand, SetOperation,
    SetOperator, SetQuantifier, SetWitnessBoundary, SignedIntegerCastExpression,
    SignedIntegerCastKind, SignedIntegerWidth, SourceRelation, SubqueryQuantifier,
    SubquerySemantics, TableSampling, UnaryExpression, UnaryOperator, UnknownSemantic,
    UnsupportedSemantic, UnsupportedStatement, ValueDomain, ValueRange, WindowFrame,
    WindowFrameBound, WindowFrameUnits, WindowFunctionExpression, WindowOrderExpression,
    WindowSpecification, WriteDefinitionMode, WriteOperation, WriteValue,
};
use crate::relation::{RelationCatalog, RelationContext, SchemaColumn, TimestampZone};
use temporal_literals::normalize_temporal_identity_cast;

/// Error produced after parsing succeeds but protocol analysis cannot proceed.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum AnalysisError {
    /// The caller did not provide a non-empty dialect name for protocol metadata.
    EmptyDialectName,
    /// Parsing succeeded but produced no SQL statements to analyze.
    NoStatements,
    /// Parsed DDL contained internally inconsistent canonical constraint metadata.
    ConstraintMetadata {
        /// Explanation of the invalid constraint metadata.
        message: String,
    },
}

impl fmt::Display for AnalysisError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyDialectName => {
                write!(formatter, "analysis requires a non-empty dialect name")
            }
            Self::NoStatements => write!(formatter, "analysis requires at least one SQL statement"),
            Self::ConstraintMetadata { message } => {
                write!(formatter, "invalid relation constraint metadata: {message}")
            }
        }
    }
}

impl std::error::Error for AnalysisError {}

struct AnalysisMetadata<'a> {
    catalog: Option<&'a RelationCatalog>,
    dialect_name: &'a str,
    relation_context: Option<&'a RelationContext>,
}

impl AnalysisMetadata<'_> {
    fn schema_columns(&self, relation: &str) -> Option<Vec<String>> {
        let catalog = self.catalog?;
        let canonical = catalog
            .resolve(relation, self.dialect_name, self.relation_context)
            .ok()?;
        catalog
            .schemas()
            .iter()
            .find(|schema| schema.relation() == canonical)
            .map(|schema| {
                schema
                    .columns()
                    .iter()
                    .map(|column| column.name().to_string())
                    .collect()
            })
    }

    fn column_data_type(&self, column: &ColumnRef) -> Option<&DataType> {
        self.schema_column(column).map(SchemaColumn::data_type)
    }

    fn column_timestamp_zone(&self, column: &ColumnRef) -> Option<TimestampZone> {
        self.schema_column(column)
            .and_then(SchemaColumn::timestamp_zone)
    }

    fn schema_column(&self, column: &ColumnRef) -> Option<&SchemaColumn> {
        let relation = column.relation()?;
        let catalog = self.catalog?;
        let canonical = catalog
            .resolve(relation, self.dialect_name, self.relation_context)
            .ok()?;
        catalog
            .schemas()
            .iter()
            .find(|schema| schema.relation() == canonical)?
            .columns()
            .iter()
            .find(|schema_column| schema_column.name() == column.name())
    }
}

pub(crate) fn analyze(parsed: ParsedSql, dialect_name: &str) -> Result<Protocol, AnalysisError> {
    analyze_with_metadata(parsed, dialect_name, None, None)
}

pub(crate) fn analyze_with_catalog(
    parsed: ParsedSql,
    dialect_name: &str,
    catalog: &RelationCatalog,
    relation_context: Option<&RelationContext>,
) -> Result<Protocol, AnalysisError> {
    analyze_with_metadata(parsed, dialect_name, Some(catalog), relation_context)
}

fn analyze_with_metadata(
    parsed: ParsedSql,
    dialect_name: &str,
    catalog: Option<&RelationCatalog>,
    relation_context: Option<&RelationContext>,
) -> Result<Protocol, AnalysisError> {
    if dialect_name.trim().is_empty() {
        return Err(AnalysisError::EmptyDialectName);
    }

    if parsed.statements.is_empty() {
        return Err(AnalysisError::NoStatements);
    }

    let metadata = AnalysisMetadata {
        catalog,
        dialect_name,
        relation_context,
    };
    let statements = parsed
        .statements
        .iter()
        .map(|statement| analyze_statement(statement, &metadata))
        .collect();

    let mut relation_constraints = Vec::new();
    let mut source_schemas = Vec::<crate::relation::RelationSchema>::new();
    let mut invalid_schemas = BTreeSet::<String>::new();
    for statement in &parsed.statements {
        if let Some(constraints) = analyze_relation_constraints(statement).map_err(|error| {
            AnalysisError::ConstraintMetadata {
                message: error.to_string(),
            }
        })? {
            merge_relation_constraint_sets(&mut relation_constraints, &[constraints]);
        }
        if let SqlStatement::CreateTable(declared) = statement {
            let relation = declared.name.to_string();
            match analyze_create_table_schema(declared, dialect_name) {
                Ok(Some(schema)) => {
                    if !invalid_schemas.contains(&relation) {
                        if let Some(existing) = source_schemas
                            .iter()
                            .find(|item| item.relation() == relation)
                        {
                            if existing != &schema {
                                source_schemas.retain(|item| item.relation() != relation);
                                invalid_schemas.insert(relation.clone());
                            }
                        } else {
                            source_schemas.push(schema);
                        }
                    }
                }
                Ok(None) => {}
                Err(diagnostic) => {
                    let mut set = RelationConstraintSet::new(relation.clone(), vec![])
                        .expect("parsed relation cannot be empty");
                    set.add_diagnostic(diagnostic);
                    merge_relation_constraint_sets(&mut relation_constraints, &[set]);
                    invalid_schemas.insert(relation.clone());
                    source_schemas.retain(|item| item.relation() != relation);
                }
            }
        }
    }
    for relation in invalid_schemas {
        let mut set = RelationConstraintSet::new(relation.clone(), vec![])
            .expect("parsed relation cannot be empty");
        set.add_diagnostic(ConstraintDiagnostic::new(
            "unproved_ddl_schema",
            format!("SQL DDL '{relation}' has unprovable or conflicting schema facts"),
        ));
        merge_relation_constraint_sets(&mut relation_constraints, &[set]);
    }
    source_schemas.sort_by(|a, b| a.relation().cmp(b.relation()));

    Ok(Protocol::new(dialect_name.to_string(), statements)
        .with_relation_constraints(relation_constraints)
        .with_source_schemas(source_schemas))
}

/// A nested row-changing query is never an implicit read-only SELECT.
fn set_expr_contains_write(expr: &SetExpr) -> bool {
    match expr {
        SetExpr::Insert(_) | SetExpr::Update(_) | SetExpr::Delete(_) => true,
        SetExpr::Query(query) => set_expr_contains_write(query.body.as_ref()),
        SetExpr::SetOperation { left, right, .. } => {
            set_expr_contains_write(left) || set_expr_contains_write(right)
        }
        SetExpr::Select(_) | SetExpr::Values(_) | SetExpr::Table(_) => false,
    }
}

/// Exclude CREATE table dialect/storage/session options from ordinary
/// physical lifecycle proof. These AST facts cannot be inferred from a
/// row-preserving SELECT and need independent engine evidence.
fn create_table_execution_unknown(create_table: &sqlparser::ast::CreateTable) -> bool {
    create_table.temporary
        || create_table.external
        || create_table.transient
        || create_table.volatile
        || create_table.iceberg
        || create_table.global.is_some()
        || create_table.like.is_some()
        || create_table.clone.is_some()
        || !matches!(
            create_table.hive_distribution,
            sqlparser::ast::HiveDistributionStyle::NONE
        )
        || create_table.hive_formats.as_ref().is_some_and(|format| {
            format.row_format.is_some()
                || format.serde_properties.is_some()
                || format.storage.is_some()
                || format.location.is_some()
        })
        || create_table.file_format.is_some()
        || create_table.location.is_some()
        || create_table.without_rowid
        || create_table.comment.is_some()
        || create_table.on_commit.is_some()
        || create_table.on_cluster.is_some()
        || create_table.primary_key.is_some()
        || create_table.order_by.is_some()
        || create_table.partition_by.is_some()
        || create_table.cluster_by.is_some()
        || create_table.clustered_by.is_some()
        || create_table.inherits.is_some()
        || create_table.strict
        || create_table.copy_grants
        || create_table.enable_schema_evolution.is_some()
        || create_table.change_tracking.is_some()
        || create_table.data_retention_time_in_days.is_some()
        || create_table.max_data_extension_time_in_days.is_some()
        || create_table.default_ddl_collation.is_some()
        || create_table.with_aggregation_policy.is_some()
        || create_table.with_row_access_policy.is_some()
        || create_table.with_tags.is_some()
        || create_table.base_location.is_some()
        || create_table.external_volume.is_some()
        || create_table.catalog.is_some()
        || create_table.catalog_sync.is_some()
        || create_table.storage_serialization_policy.is_some()
        || !matches!(
            create_table.table_options,
            sqlparser::ast::CreateTableOptions::None
        )
}

fn analyze_statement(
    statement: &SqlStatement,
    metadata: &AnalysisMetadata<'_>,
) -> ProtocolStatement {
    match statement {
        SqlStatement::Query(query) => match query.body.as_ref() {
            SetExpr::Update(_) | SetExpr::Delete(_) => analyze_joined_cte_mutation(query, metadata),
            SetExpr::Insert(_) => unsupported_write_statement(
                "query",
                "unsupported_nested_write",
                "DML inside a query body has no proven ordered write semantics",
            ),
            other if set_expr_contains_write(other) => unsupported_write_statement(
                "query",
                "unsupported_nested_write",
                "Nested row-changing query bodies cannot be silently treated as SELECT",
            ),
            _ => ProtocolStatement::Query(analyze_query(query, None, metadata)),
        },
        SqlStatement::CreateTable(create_table) => match &create_table.query {
            Some(query) => {
                if create_table.if_not_exists {
                    return unsupported_statement();
                }
                let target = create_table.name.to_string();
                let mode = if create_table.or_replace {
                    WriteDefinitionMode::ReplaceTable
                } else {
                    WriteDefinitionMode::CreateTable
                };
                let uncertain_lifecycle = create_table_execution_unknown(create_table);
                let mut definition = WriteOperation::definition(target.clone(), mode);
                if uncertain_lifecycle {
                    definition = definition.with_definition_execution_unknown();
                }
                ProtocolStatement::Query(
                    analyze_query(query, Some(target), metadata).with_write(Some(definition)),
                )
            }
            None if !create_table.if_not_exists
                && !create_table.or_replace
                && !create_table_execution_unknown(create_table)
                && !create_table.columns.is_empty() =>
            {
                // A schema-only CREATE is an actual producer of an empty
                // physical relation. It must not be confused with a SELECT
                // missing source evidence or an independent physical bag.
                let target = create_table.name.to_string();
                ProtocolStatement::Query(
                    QueryStatement::new(
                        Vec::new(),
                        Vec::new(),
                        Vec::new(),
                        RowConditions::new(
                            Predicates::new(None, None, None),
                            Vec::new(),
                            ConditionExactness::from_residuals(Vec::new()),
                        ),
                        Output::new(Vec::new()),
                        Vec::new(),
                    )
                    .with_produced_relation(Some(target.clone()))
                    .with_write(Some(WriteOperation::definition(
                        target,
                        WriteDefinitionMode::CreateTable,
                    ))),
                )
            }
            None => unsupported_queryless_create_table(),
        },
        SqlStatement::CreateView {
            name,
            query,
            or_replace,
            or_alter,
            materialized,
            temporary,
            if_not_exists,
            with_no_schema_binding,
            columns,
            cluster_by,
            options,
            to,
            params,
            ..
        } => {
            // Only an unmodified persistent logical VIEW has a portable,
            // source-complete re-evaluation law. Vendor creation options,
            // replacement/alteration, security context and target routing
            // require independent engine evidence even if the SELECT parses.
            let execution_unknown = *or_alter
                || (*materialized && *or_replace)
                || *temporary
                || *if_not_exists
                || *with_no_schema_binding
                || !columns.is_empty()
                || !cluster_by.is_empty()
                || !matches!(options, sqlparser::ast::CreateTableOptions::None)
                || to.is_some()
                || params.is_some();
            let target = name.to_string();
            let mode = match (*materialized, *or_replace) {
                (false, false) => WriteDefinitionMode::CreateView,
                (false, true) => WriteDefinitionMode::ReplaceView,
                (true, false) => WriteDefinitionMode::CreateMaterializedView,
                (true, true) => WriteDefinitionMode::ReplaceMaterializedView,
            };
            ProtocolStatement::Query(
                analyze_query(query, Some(target.clone()), metadata).with_write(Some(
                    if execution_unknown {
                        WriteOperation::definition(target, mode).with_view_execution_unknown()
                    } else {
                        WriteOperation::definition(target, mode)
                    },
                )),
            )
        }
        SqlStatement::AlterTable {
            name,
            if_exists,
            only,
            operations,
            location,
            on_cluster,
            iceberg,
        } => {
            if *if_exists || *only || location.is_some() || on_cluster.is_some() || *iceberg {
                unsupported_write_statement(
                    "alter_table", "unsupported_alter_lifecycle",
                    "conditional, storage or vendor ALTER TABLE requires engine evidence",
                )
            } else {
                analyze_alter_columns(name, operations, metadata.dialect_name)
            }
        }
        SqlStatement::Insert(insert) => analyze_insert(insert, metadata),
        SqlStatement::Update {
            table,
            assignments,
            from,
            selection,
            returning,
            or,
        } => analyze_update(
            table,
            assignments,
            from.as_ref(),
            selection.as_ref(),
            returning.as_deref(),
            or.is_some(),
            metadata,
        ),
        SqlStatement::Delete(delete) => analyze_delete(delete, metadata),
        SqlStatement::Merge {
            table,
            source,
            on,
            clauses,
            output,
            ..
        } => analyze_merge(table, source, on, clauses, output.is_some(), metadata),
        _ => unsupported_statement(),
    }
}

// SQL AST to canonical protocol conversion is split by semantic responsibility.
mod alter_columns;
mod aggregation;
mod arithmetic_domains;
mod cast_domains;
pub(crate) mod constraints;
mod cte;
mod decimal_literals;
mod dependencies;
mod diagnostics;
mod domains;
mod exactness;
mod expressions;
mod features;
mod finite_tables;
mod functions;
mod joins;
mod lineage;
mod output;
mod predicates;
mod queries;
mod relations;
mod sampling;
mod sets;
mod star_projection;
mod subqueries;
mod temporal_literals;
mod typing;
mod writes;

use aggregation::*;
use alter_columns::*;
use arithmetic_domains::*;
use cast_domains::*;
use constraints::*;
use cte::*;
use decimal_literals::*;
use dependencies::*;
use diagnostics::*;
use domains::*;
use exactness::*;
use expressions::*;
use features::*;
use finite_tables::*;
use functions::*;
use joins::*;
use lineage::*;
use output::*;
use predicates::*;
use queries::*;
use relations::*;
use sampling::*;
use sets::*;
use subqueries::*;
use typing::*;
use writes::*;
