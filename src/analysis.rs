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
    InSubqueryPredicate, IsNullPredicate, Join as ProtocolJoin, JoinKind, LikePrefixPredicate,
    LineageSource, LiteralExpression, LiteralType, LiteralValue, LogicalPredicate,
    MergeAction as ProtocolMergeAction, MergeAssignment, MergeClause as ProtocolMergeClause,
    MergeMatchKind, NotPredicate, Output, OutputColumn, Predicate, Predicates, Protocol,
    ProtocolStatement, QueryStatement, RelationRef, ResidualCondition, ResidualConditionReason,
    RowConditions, ScalarSubqueryExpression, SetBranch, SetMode, SetOperand, SetOperation,
    SetOperator, SetQuantifier, SetWitnessBoundary, SignedIntegerCastExpression, SourceRelation,
    SubquerySemantics, UnaryExpression, UnaryOperator, UnknownSemantic, UnsupportedSemantic,
    UnsupportedStatement, ValueDomain, ValueRange, WindowFrame, WindowFrameBound, WindowFrameUnits,
    WindowFunctionExpression, WindowOrderExpression, WindowSpecification, WriteDefinitionMode,
    WriteOperation, WriteValue,
};
use crate::relation::{RelationCatalog, RelationContext, SchemaColumn, TimestampZone};

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
                        if let Some(existing) = source_schemas.iter().find(|item| item.relation() == relation) {
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

fn analyze_statement(
    statement: &SqlStatement,
    metadata: &AnalysisMetadata<'_>,
) -> ProtocolStatement {
    match statement {
        SqlStatement::Query(query) => {
            ProtocolStatement::Query(analyze_query(query, None, metadata))
        }
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
                ProtocolStatement::Query(
                    analyze_query(query, Some(target.clone()), metadata)
                        .with_write(Some(WriteOperation::definition(target, mode))),
                )
            }
            None => unsupported_queryless_create_table(),
        },
        SqlStatement::CreateView {
            name,
            query,
            or_replace,
            ..
        } => {
            let target = name.to_string();
            let mode = if *or_replace {
                WriteDefinitionMode::ReplaceView
            } else {
                WriteDefinitionMode::CreateView
            };
            ProtocolStatement::Query(
                analyze_query(query, Some(target.clone()), metadata)
                    .with_write(Some(WriteOperation::definition(target, mode))),
            )
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
            from.is_some(),
            selection.as_ref(),
            returning.is_some(),
            or.is_some(),
        ),
        SqlStatement::Delete(delete) => analyze_delete(delete),
        SqlStatement::Merge {
            table,
            source,
            on,
            clauses,
            output,
            ..
        } => analyze_merge(table, source, on, clauses, output.is_some()),
        _ => unsupported_statement(),
    }
}

// SQL AST to canonical protocol conversion is split by semantic responsibility.
mod aggregation;
mod constraints;
mod dependencies;
mod diagnostics;
mod domains;
mod exactness;
mod expressions;
mod features;
mod functions;
mod joins;
mod lineage;
mod output;
mod predicates;
mod queries;
mod relations;
mod sets;
mod subqueries;
mod typing;
mod writes;

use aggregation::*;
use constraints::*;
use dependencies::*;
use diagnostics::*;
use domains::*;
use exactness::*;
use expressions::*;
use features::*;
use functions::*;
use joins::*;
use lineage::*;
use output::*;
use predicates::*;
use queries::*;
use relations::*;
use sets::*;
use subqueries::*;
use typing::*;
use writes::*;
