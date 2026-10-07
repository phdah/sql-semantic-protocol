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
    BinaryOperator as SqlBinaryOperator, ColumnOption, ConstraintCharacteristics,
    CreateTable as SqlCreateTable, Distinct as SqlDistinct, DuplicateTreatment, Expr, Function,
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
use crate::domain::{
    derive_case_source_domains, derive_column_domains, intersect_case_domain_values,
    intersect_domains, refine_column_domains_from_equalities, resolve_column, union_domains,
};
use crate::parser::ParsedSql;
use crate::protocol::{
    AggregateArgument, AggregateFunctionExpression, Aggregation, BetweenPredicate,
    BinaryExpression, BinaryOperator, Bound, CaseBranch, CaseExpression,
    CaseSourceDomainAlternative, CaseSourceDomains, ColumnDomain, ColumnExpression, ColumnRef,
    ComparisonOperator, ComparisonPredicate, ConditionClause, ConditionExactness, Diagnostic,
    DiagnosticArea, DiagnosticSeverity,
    ExistsPredicate, Expression, FunctionExpression, GroupBy, GroupingExpression, InPredicate,
    InSubqueryPredicate, IsNullPredicate, Join as ProtocolJoin, JoinKind, LineageSource,
    LiteralExpression, LiteralType, LiteralValue, LogicalPredicate,
    MergeAction as ProtocolMergeAction, MergeAssignment, MergeClause as ProtocolMergeClause,
    MergeMatchKind, NotPredicate, Output, OutputColumn, Predicate, Predicates, Protocol,
    ProtocolStatement, QueryStatement, RelationRef, ResidualCondition, ResidualConditionReason,
    ScalarSubqueryExpression, SetMode, SetOperand, SetOperation, SetOperator, SetQuantifier,
    SourceRelation, SubquerySemantics, UnaryExpression,
    UnaryOperator, UnknownSemantic, UnsupportedSemantic, UnsupportedStatement, ValueDomain,
    ValueRange, WindowFrame, WindowFrameBound, WindowFrameUnits, WindowFunctionExpression,
    WindowOrderExpression, WindowSpecification, WriteOperation, WriteValue,
};
use crate::relation::{RelationCatalog, RelationContext};

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
    for statement in &parsed.statements {
        if let Some(constraints) = analyze_relation_constraints(statement).map_err(|error| {
            AnalysisError::ConstraintMetadata {
                message: error.to_string(),
            }
        })? {
            merge_relation_constraint_sets(&mut relation_constraints, &[constraints]);
        }
    }

    Ok(Protocol::new(dialect_name.to_string(), statements)
        .with_relation_constraints(relation_constraints))
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
                let target = create_table.name.to_string();
                ProtocolStatement::Query(
                    analyze_query(query, Some(target.clone()), metadata)
                        .with_write(Some(WriteOperation::definition(target))),
                )
            }
            None => unsupported_queryless_create_table(),
        },
        SqlStatement::CreateView { name, query, .. } => {
            let target = name.to_string();
            ProtocolStatement::Query(
                analyze_query(query, Some(target.clone()), metadata)
                    .with_write(Some(WriteOperation::definition(target))),
            )
        }
        SqlStatement::Insert(insert) => analyze_insert(insert, metadata),
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

fn analyze_insert(insert: &SqlInsert, metadata: &AnalysisMetadata<'_>) -> ProtocolStatement {
    let target = match &insert.table {
        TableObject::TableName(name) => name.to_string(),
        TableObject::TableFunction(_) => {
            return unsupported_write_statement(
                "insert",
                "unsupported_insert_target",
                "INSERT into a table function cannot be represented as a relation write",
            );
        }
    };

    let Some(source) = &insert.source else {
        return unsupported_write_statement(
            "insert",
            "unsupported_insert_source",
            "only INSERT INTO ... SELECT source semantics are supported",
        );
    };
    if !matches!(
        source.body.as_ref(),
        SetExpr::Select(_) | SetExpr::Query(_) | SetExpr::SetOperation { .. }
    ) {
        return unsupported_write_statement(
            "insert",
            "unsupported_insert_source",
            "only INSERT INTO ... SELECT source semantics are supported",
        );
    }

    if insert.overwrite
        || insert.replace_into
        || insert.ignore
        || insert.or.is_some()
        || !insert.assignments.is_empty()
        || insert.partitioned.is_some()
        || !insert.after_columns.is_empty()
        || insert.on.is_some()
        || insert.returning.is_some()
        || insert.priority.is_some()
        || insert.insert_alias.is_some()
        || insert.settings.is_some()
        || insert.format_clause.is_some()
    {
        return unsupported_write_statement(
            "insert",
            "unsupported_insert_write_semantics",
            "INSERT modifiers that can change append semantics are not supported",
        );
    }

    let target_columns = insert.columns.iter().map(ToString::to_string).collect();
    let query = analyze_query(source, Some(target.clone()), metadata)
        .with_write(Some(WriteOperation::append(target, target_columns)));
    ProtocolStatement::Query(query)
}

fn analyze_merge(
    table: &TableFactor,
    source: &TableFactor,
    on: &Expr,
    clauses: &[SqlMergeClause],
    has_output: bool,
) -> ProtocolStatement {
    let Some(target_source) = merge_target_relation(table) else {
        return unsupported_write_statement(
            "merge",
            "unsupported_merge_target",
            "MERGE target must be a named relation",
        );
    };
    let target = target_source.name().to_string();

    let mut diagnostics = Vec::new();
    if has_output {
        diagnostics.push(warning(
            "unsupported_merge_output",
            DiagnosticArea::Output,
            "MERGE OUTPUT semantics are not represented",
        ));
    }

    let mut derived_index = 0;
    let Some(source_relation) = analyze_table_factor(
        source,
        &BTreeSet::new(),
        &mut diagnostics,
        &mut derived_index,
    ) else {
        return unsupported_write_statement(
            "merge",
            "unsupported_merge_source",
            "MERGE source could not be represented safely",
        );
    };

    let mut dependencies = source_relation.dependencies.clone();
    collect_expression_dependencies(
        on,
        &BTreeSet::new(),
        &mut diagnostics,
        &mut derived_index,
        &mut dependencies,
    );
    let match_condition = analyze_predicate(on, &mut diagnostics);
    let merge_sources = vec![target_source, source_relation.source.clone()];
    let merge_clauses = clauses
        .iter()
        .map(|clause| {
            analyze_merge_clause(
                clause,
                &merge_sources,
                &match_condition,
                &mut diagnostics,
                &mut derived_index,
                &mut dependencies,
            )
        })
        .collect();

    sort_diagnostics(&mut diagnostics);
    let query = QueryStatement::new(
        vec![source_relation.source],
        dependencies.into_iter().collect(),
        Vec::new(),
        Predicates::new(None, None, None),
        Vec::new(),
        ConditionExactness::from_residuals(vec![ResidualCondition::new(
            ResidualConditionReason::AnalysisDiagnostic,
            ConditionClause::RowSetOperator,
            "merge",
        )]),
        Output::new(Vec::new()),
        diagnostics,
    )
    .with_produced_relation(Some(target.clone()))
    .with_write(Some(WriteOperation::conditional_mutation(
        target,
        match_condition,
        merge_clauses,
    )));

    ProtocolStatement::Query(query)
}

fn merge_target_relation(table: &TableFactor) -> Option<SourceRelation> {
    match table {
        TableFactor::Table {
            name,
            alias,
            args: None,
            ..
        } => Some(SourceRelation::new(
            name.to_string(),
            alias.as_ref().map(|alias| alias.name.to_string()),
        )),
        _ => None,
    }
}

fn analyze_merge_clause(
    clause: &SqlMergeClause,
    sources: &[SourceRelation],
    match_condition: &Predicate,
    diagnostics: &mut Vec<Diagnostic>,
    derived_index: &mut usize,
    dependencies: &mut BTreeSet<String>,
) -> ProtocolMergeClause {
    let match_kind = match clause.clause_kind {
        SqlMergeClauseKind::Matched => MergeMatchKind::Matched,
        SqlMergeClauseKind::NotMatched => MergeMatchKind::NotMatched,
        SqlMergeClauseKind::NotMatchedByTarget => MergeMatchKind::NotMatchedByTarget,
        SqlMergeClauseKind::NotMatchedBySource => MergeMatchKind::NotMatchedBySource,
    };

    let predicate = clause.predicate.as_ref().map(|predicate| {
        collect_expression_dependencies(
            predicate,
            &BTreeSet::new(),
            diagnostics,
            derived_index,
            dependencies,
        );
        analyze_predicate(predicate, diagnostics)
    });

    let branch_predicate = merge_branch_predicate(match_kind, match_condition, predicate.as_ref());
    let branch_domains = branch_predicate
        .as_ref()
        .map_or_else(Vec::new, |predicate| {
            let domains = derive_column_domains(
                &Predicates::new(Some(predicate.clone()), None, None),
                sources,
            );
            refine_column_domains_from_equalities(domains, predicate, sources)
        });

    let action = match &clause.action {
        SqlMergeAction::Delete => ProtocolMergeAction::Delete,
        SqlMergeAction::Update { assignments } => {
            let assignments = assignments
                .iter()
                .map(|assignment| {
                    collect_expression_dependencies(
                        &assignment.value,
                        &BTreeSet::new(),
                        diagnostics,
                        derived_index,
                        dependencies,
                    );
                    let expression =
                        analyze_expression_with_scope(&assignment.value, &[], &[], diagnostics);
                    let domain = derive_expression_domain_with_column_domains(
                        &expression,
                        &branch_domains,
                        sources,
                    );
                    MergeAssignment::new(
                        assignment.target.to_string(),
                        WriteValue::new(expression, domain),
                    )
                })
                .collect();
            ProtocolMergeAction::Update { assignments }
        }
        SqlMergeAction::Insert(insert) => match &insert.kind {
            MergeInsertKind::Values(values) => {
                let rows = values
                    .rows
                    .iter()
                    .map(|row| {
                        row.iter()
                            .map(|expression| {
                                collect_expression_dependencies(
                                    expression,
                                    &BTreeSet::new(),
                                    diagnostics,
                                    derived_index,
                                    dependencies,
                                );
                                let expression = analyze_expression_with_scope(
                                    expression,
                                    &[],
                                    &[],
                                    diagnostics,
                                );
                                let domain = derive_expression_domain_with_column_domains(
                                    &expression,
                                    &branch_domains,
                                    sources,
                                );
                                WriteValue::new(expression, domain)
                            })
                            .collect()
                    })
                    .collect();
                ProtocolMergeAction::Insert {
                    columns: insert.columns.iter().map(ToString::to_string).collect(),
                    values: rows,
                }
            }
            MergeInsertKind::Row => {
                diagnostics.push(warning(
                    "unsupported_merge_insert_row",
                    DiagnosticArea::Statement,
                    "MERGE INSERT ROW cannot be expanded without target schema information",
                ));
                ProtocolMergeAction::Unsupported(UnsupportedSemantic::new(
                    "merge_insert_row".to_string(),
                    Some("target schema is required to expand ROW".to_string()),
                ))
            }
        },
    };

    ProtocolMergeClause::new(match_kind, predicate, action)
}

fn merge_branch_predicate(
    match_kind: MergeMatchKind,
    match_condition: &Predicate,
    clause_predicate: Option<&Predicate>,
) -> Option<Predicate> {
    match match_kind {
        MergeMatchKind::Matched => Some(match clause_predicate {
            Some(predicate) => Predicate::And(LogicalPredicate::pair(
                match_condition.clone(),
                predicate.clone(),
            )),
            None => match_condition.clone(),
        }),
        MergeMatchKind::NotMatched
        | MergeMatchKind::NotMatchedByTarget
        | MergeMatchKind::NotMatchedBySource => clause_predicate.cloned(),
    }
}

fn analyze_query(
    query: &SqlQuery,
    produced_relation: Option<String>,
    metadata: &AnalysisMetadata<'_>,
) -> QueryStatement {
    let mut diagnostics = Vec::new();
    let mut derived_index = 0;
    let relation_analysis = analyze_query_relations(
        query,
        &BTreeSet::new(),
        &mut diagnostics,
        &mut derived_index,
    );

    let set_operation = analyze_set_operation(query.body.as_ref());
    let aggregation = analyze_query_aggregation(query, &mut diagnostics);
    let output = analyze_query_output(query, &BTreeMap::new(), &mut diagnostics, Some(metadata));

    let predicates = match query.body.as_ref() {
        SetExpr::Select(select) => analyze_select(select, &mut diagnostics),
        SetExpr::SetOperation { .. } => Predicates::new(None, None, None),
        SetExpr::Query(query) => analyze_query_predicates(query, &mut diagnostics),
        _ => {
            diagnostics.push(warning(
                "unsupported_query_body",
                DiagnosticArea::Statement,
                "the parsed query body is not supported by semantic analysis",
            ));
            Predicates::new(None, None, None)
        }
    };

    let column_domains = analyze_query_column_domains(
        query,
        &BTreeSet::new(),
        &BTreeMap::new(),
        &BTreeMap::new(),
        Some(metadata),
    );
    let output = refine_output_domains_from_column_domains(
        output,
        &column_domains,
        &relation_analysis.sources,
    );

    if matches!(query.body.as_ref(), SetExpr::SetOperation { .. }) {
        inspect_set_expr_features(query.body.as_ref(), &mut diagnostics);
    }
    inspect_query_features(query, &mut diagnostics);
    let condition_exactness = analyze_query_condition_exactness(
        query,
        &predicates,
        &relation_analysis.sources,
        &relation_analysis.joins,
        &diagnostics,
        false,
    );
    sort_diagnostics(&mut diagnostics);

    QueryStatement::new(
        relation_analysis.sources,
        relation_analysis.dependencies.into_iter().collect(),
        relation_analysis.joins,
        predicates,
        column_domains,
        condition_exactness,
        output,
        diagnostics,
    )
    .with_aggregation(aggregation)
    .with_set_operation(set_operation)
    .with_produced_relation(produced_relation)
}

fn analyze_query_condition_exactness(
    query: &SqlQuery,
    predicates: &Predicates,
    sources: &[SourceRelation],
    joins: &[ProtocolJoin],
    diagnostics: &[Diagnostic],
    correlated: bool,
) -> ConditionExactness {
    let mut residuals = Vec::new();

    if let Some(predicate) = predicates.where_predicate() {
        append_predicate_residuals(
            predicate,
            ConditionClause::Where,
            "where",
            sources,
            false,
            &mut residuals,
        );
    }

    if predicates.having_predicate().is_some() {
        residuals.push(ResidualCondition::new(
            ResidualConditionReason::Having,
            ConditionClause::Having,
            "having",
        ));
    }

    if predicates.qualify_predicate().is_some() {
        residuals.push(ResidualCondition::new(
            ResidualConditionReason::Qualify,
            ConditionClause::Qualify,
            "qualify",
        ));
    }

    for (index, join) in joins.iter().enumerate() {
        let identity = format!("join:{index}");
        match join.kind() {
            JoinKind::Inner => {
                if let Some(predicate) = join.condition() {
                    append_predicate_residuals(
                        predicate,
                        ConditionClause::JoinOn,
                        &identity,
                        sources,
                        true,
                        &mut residuals,
                    );
                }
            }
            JoinKind::Cross => {}
            JoinKind::Left | JoinKind::Right | JoinKind::Full => {
                residuals.push(ResidualCondition::new(
                    ResidualConditionReason::OuterJoin,
                    ConditionClause::JoinOn,
                    identity,
                ));
            }
            JoinKind::LeftSemi
            | JoinKind::RightSemi
            | JoinKind::LeftAnti
            | JoinKind::RightAnti
            | JoinKind::Unknown => {
                residuals.push(ResidualCondition::new(
                    ResidualConditionReason::UnsupportedJoinKind,
                    ConditionClause::JoinOn,
                    identity,
                ));
            }
        }
    }

    let mut source_counts = BTreeMap::new();
    for source in sources {
        *source_counts.entry(source.name().to_string()).or_insert(0usize) += 1;
    }
    for (source, count) in source_counts {
        if count > 1 {
            residuals.push(ResidualCondition::new(
                ResidualConditionReason::RepeatedSourceInstance,
                ConditionClause::RowSetOperator,
                format!("source:{source}"),
            ));
        }
    }

    collect_set_expr_exactness_residuals(query.body.as_ref(), "body", &mut residuals);

    if let Some(limit_clause) = &query.limit_clause {
        let text = limit_clause.to_string().to_ascii_uppercase();
        if text.contains("OFFSET") {
            residuals.push(ResidualCondition::new(
                ResidualConditionReason::Offset,
                ConditionClause::RowSetOperator,
                "offset",
            ));
        }
        if text.contains("LIMIT") || !text.contains("OFFSET") {
            residuals.push(ResidualCondition::new(
                ResidualConditionReason::Limit,
                ConditionClause::RowSetOperator,
                "limit",
            ));
        }
    }

    if query.fetch.is_some() {
        residuals.push(ResidualCondition::new(
            ResidualConditionReason::Fetch,
            ConditionClause::RowSetOperator,
            "fetch",
        ));
    }

    if correlated {
        residuals.push(ResidualCondition::new(
            ResidualConditionReason::CorrelatedSubquery,
            ConditionClause::Where,
            "correlation",
        ));
    }

    for diagnostic in diagnostics {
        if diagnostic_is_non_membership_or_already_classified(diagnostic.code()) {
            continue;
        }

        let clause = if diagnostic.code().contains("join") {
            ConditionClause::JoinOn
        } else if diagnostic.code().contains("predicate")
            || diagnostic.code() == "unsupported_query_body"
        {
            ConditionClause::Where
        } else {
            ConditionClause::RowSetOperator
        };
        residuals.push(ResidualCondition::new(
            ResidualConditionReason::AnalysisDiagnostic,
            clause,
            format!("diagnostic:{}", diagnostic.code()),
        ));
    }

    ConditionExactness::from_residuals(residuals)
}

fn diagnostic_is_non_membership_or_already_classified(code: &str) -> bool {
    matches!(
        code,
        "unsupported_order_by"
            | "unsupported_cluster_by"
            | "unsupported_distribute_by"
            | "unsupported_sort_by"
            | "unsupported_exclude"
            | "unsupported_select_into"
            | "unsupported_value_table_mode"
            | "ambiguous_output_lineage"
            | "unresolved_output_lineage"
            | "unresolved_wildcard"
            | "unsupported_expression"
            | "unsupported_function"
            | "ambiguous_named_window"
            | "cyclic_named_window"
            | "unresolved_named_window"
            | "unsupported_window_order_option"
            | "unsupported_window_override"
            | "unsupported_group_by_modifier"
            | "unsupported_lock"
            | "unsupported_for_clause"
            | "unsupported_settings"
            | "unsupported_format_clause"
            | "unsupported_merge_output"
            | "unsupported_merge_insert_row"
            | "unsupported_queryless_create_table"
            | "unsupported_top"
            | "unsupported_prewhere"
            | "unsupported_connect_by"
            | "unsupported_limit"
            | "unsupported_fetch"
            | "set_operation_arity_mismatch"
            | "unresolved_set_operation_output"
            | "unsupported_set_operation_alignment"
    )
}

fn collect_set_expr_exactness_residuals(
    expression: &SetExpr,
    identity: &str,
    residuals: &mut Vec<ResidualCondition>,
) {
    match expression {
        SetExpr::Select(select) => {
            if matches!(select.distinct, Some(SqlDistinct::On(_))) {
                residuals.push(ResidualCondition::new(
                    ResidualConditionReason::DistinctOn,
                    ConditionClause::RowSetOperator,
                    format!("{identity}:distinct_on"),
                ));
            }
            if select.top.is_some() {
                residuals.push(ResidualCondition::new(
                    ResidualConditionReason::Top,
                    ConditionClause::RowSetOperator,
                    format!("{identity}:top"),
                ));
            }
            if select.prewhere.is_some() {
                residuals.push(ResidualCondition::new(
                    ResidualConditionReason::Prewhere,
                    ConditionClause::Where,
                    format!("{identity}:prewhere"),
                ));
            }
            if select.connect_by.is_some() {
                residuals.push(ResidualCondition::new(
                    ResidualConditionReason::ConnectBy,
                    ConditionClause::RowSetOperator,
                    format!("{identity}:connect_by"),
                ));
            }
            collect_table_sample_residuals(select, identity, residuals);
        }
        SetExpr::Query(query) => {
            collect_set_expr_exactness_residuals(
                query.body.as_ref(),
                &format!("{identity}:query"),
                residuals,
            );
            if query.limit_clause.is_some() {
                residuals.push(ResidualCondition::new(
                    ResidualConditionReason::Limit,
                    ConditionClause::RowSetOperator,
                    format!("{identity}:query:limit"),
                ));
            }
            if query.fetch.is_some() {
                residuals.push(ResidualCondition::new(
                    ResidualConditionReason::Fetch,
                    ConditionClause::RowSetOperator,
                    format!("{identity}:query:fetch"),
                ));
            }
        }
        SetExpr::SetOperation {
            left,
            op,
            right,
            ..
        } => {
            residuals.push(ResidualCondition::new(
                ResidualConditionReason::SetOperation,
                ConditionClause::SetOperation,
                format!("{identity}:{}", analyze_set_operator(*op).as_str()),
            ));
            collect_set_expr_exactness_residuals(
                left,
                &format!("{identity}:left"),
                residuals,
            );
            collect_set_expr_exactness_residuals(
                right,
                &format!("{identity}:right"),
                residuals,
            );
        }
        SetExpr::Values(_)
        | SetExpr::Insert(_)
        | SetExpr::Update(_)
        | SetExpr::Delete(_)
        | SetExpr::Table(_) => {}
    }
}

fn collect_table_sample_residuals(
    select: &Select,
    identity: &str,
    residuals: &mut Vec<ResidualCondition>,
) {
    let mut index = 0usize;
    for source in &select.from {
        if table_factor_has_sample(&source.relation) {
            residuals.push(ResidualCondition::new(
                ResidualConditionReason::TableSample,
                ConditionClause::RowSetOperator,
                format!("{identity}:tablesample:{index}"),
            ));
        }
        index += 1;
        for join in &source.joins {
            if table_factor_has_sample(&join.relation) {
                residuals.push(ResidualCondition::new(
                    ResidualConditionReason::TableSample,
                    ConditionClause::RowSetOperator,
                    format!("{identity}:tablesample:{index}"),
                ));
            }
            index += 1;
        }
    }
}

fn table_factor_has_sample(factor: &TableFactor) -> bool {
    match factor {
        TableFactor::Table { sample, .. } | TableFactor::Derived { sample, .. } => sample.is_some(),
        _ => false,
    }
}

fn append_predicate_residuals(
    predicate: &Predicate,
    clause: ConditionClause,
    identity: &str,
    sources: &[SourceRelation],
    allow_join_equality: bool,
    residuals: &mut Vec<ResidualCondition>,
) {
    for reason in predicate_residual_reasons(predicate, clause, sources, allow_join_equality) {
        residuals.push(ResidualCondition::new(reason, clause, identity));
    }
}

fn predicate_residual_reasons(
    predicate: &Predicate,
    clause: ConditionClause,
    sources: &[SourceRelation],
    allow_join_equality: bool,
) -> Vec<ResidualConditionReason> {
    match predicate {
        Predicate::Comparison(comparison) => {
            match (comparison.left(), comparison.right()) {
                (Expression::Column(_), Expression::Literal(_))
                | (Expression::Literal(_), Expression::Column(_)) => Vec::new(),
                (Expression::Column(_), Expression::Column(_))
                    if allow_join_equality && comparison.operator() == ComparisonOperator::Eq =>
                {
                    Vec::new()
                }
                (Expression::Column(_), Expression::Column(_)) => {
                    vec![ResidualConditionReason::ColumnComparison]
                }
                _ => vec![ResidualConditionReason::ComputedExpression],
            }
        }
        Predicate::And(logical) => collect_logical_residual_reasons(
            logical,
            clause,
            sources,
            allow_join_equality,
        ),
        Predicate::Or(logical) => {
            if clause == ConditionClause::JoinOn {
                return vec![ResidualConditionReason::UnsupportedPredicate];
            }

            let child_reasons =
                collect_logical_residual_reasons(logical, clause, sources, allow_join_equality);
            if !child_reasons.is_empty() {
                return child_reasons;
            }

            let mut columns = BTreeSet::new();
            collect_predicate_column_refs(predicate, sources, &mut columns);
            if columns.len() > 1 {
                vec![ResidualConditionReason::CrossColumnDisjunction]
            } else {
                Vec::new()
            }
        }
        Predicate::Not(_) => vec![ResidualConditionReason::LogicalNot],
        Predicate::IsNull(predicate) => {
            if matches!(predicate.expression(), Expression::Column(_)) {
                Vec::new()
            } else {
                vec![ResidualConditionReason::ComputedExpression]
            }
        }
        Predicate::In(predicate) => {
            if matches!(predicate.expression(), Expression::Column(_))
                && predicate
                    .values()
                    .iter()
                    .all(|value| matches!(value, Expression::Literal(_)))
            {
                Vec::new()
            } else {
                vec![ResidualConditionReason::ComputedExpression]
            }
        }
        Predicate::Between(predicate) => {
            if matches!(predicate.expression(), Expression::Column(_))
                && matches!(predicate.lower(), Expression::Literal(_))
                && matches!(predicate.upper(), Expression::Literal(_))
            {
                Vec::new()
            } else {
                vec![ResidualConditionReason::ComputedExpression]
            }
        }
        Predicate::Exists(_) | Predicate::InSubquery(_) => {
            vec![ResidualConditionReason::SubqueryPredicate]
        }
        Predicate::BooleanExpression(Expression::Literal(literal)) => match literal.value() {
            LiteralValue::Boolean(true) => Vec::new(),
            LiteralValue::Boolean(false) | LiteralValue::Null => {
                vec![ResidualConditionReason::ConstantFalseOrNull]
            }
            LiteralValue::Number(_) | LiteralValue::Text(_) => {
                vec![ResidualConditionReason::UnsupportedPredicate]
            }
        },
        Predicate::BooleanExpression(_) => vec![ResidualConditionReason::ComputedExpression],
        Predicate::Unknown(_) | Predicate::Unsupported(_) => {
            vec![ResidualConditionReason::UnsupportedPredicate]
        }
    }
}

fn collect_logical_residual_reasons(
    logical: &LogicalPredicate,
    clause: ConditionClause,
    sources: &[SourceRelation],
    allow_join_equality: bool,
) -> Vec<ResidualConditionReason> {
    let mut reasons = Vec::new();
    for operand in logical.operands() {
        for reason in predicate_residual_reasons(operand, clause, sources, allow_join_equality) {
            if !reasons.contains(&reason) {
                reasons.push(reason);
            }
        }
    }
    reasons
}

fn collect_predicate_column_refs(
    predicate: &Predicate,
    sources: &[SourceRelation],
    columns: &mut BTreeSet<ColumnRef>,
) {
    match predicate {
        Predicate::Comparison(comparison) => {
            collect_expression_column_refs(comparison.left(), sources, columns);
            collect_expression_column_refs(comparison.right(), sources, columns);
        }
        Predicate::And(logical) | Predicate::Or(logical) => {
            for operand in logical.operands() {
                collect_predicate_column_refs(operand, sources, columns);
            }
        }
        Predicate::Not(predicate) => {
            collect_predicate_column_refs(predicate.operand(), sources, columns)
        }
        Predicate::IsNull(predicate) => {
            collect_expression_column_refs(predicate.expression(), sources, columns)
        }
        Predicate::In(predicate) => {
            collect_expression_column_refs(predicate.expression(), sources, columns);
            for value in predicate.values() {
                collect_expression_column_refs(value, sources, columns);
            }
        }
        Predicate::InSubquery(predicate) => {
            collect_expression_column_refs(predicate.expression(), sources, columns)
        }
        Predicate::Between(predicate) => {
            collect_expression_column_refs(predicate.expression(), sources, columns);
            collect_expression_column_refs(predicate.lower(), sources, columns);
            collect_expression_column_refs(predicate.upper(), sources, columns);
        }
        Predicate::BooleanExpression(expression) => {
            collect_expression_column_refs(expression, sources, columns)
        }
        Predicate::Exists(_) | Predicate::Unknown(_) | Predicate::Unsupported(_) => {}
    }
}

fn collect_expression_column_refs(
    expression: &Expression,
    sources: &[SourceRelation],
    columns: &mut BTreeSet<ColumnRef>,
) {
    match expression {
        Expression::Column(column) => {
            columns.insert(resolve_column(column, sources));
        }
        Expression::Function(function) => {
            for argument in function.arguments() {
                collect_expression_column_refs(argument, sources, columns);
            }
        }
        Expression::Case(case) => {
            if let Some(operand) = case.operand() {
                collect_expression_column_refs(operand, sources, columns);
            }
            for branch in case.branches() {
                collect_expression_column_refs(branch.condition(), sources, columns);
                collect_expression_column_refs(branch.result(), sources, columns);
            }
            if let Some(else_result) = case.else_result() {
                collect_expression_column_refs(else_result, sources, columns);
            }
        }
        Expression::BooleanPredicate(predicate) => {
            collect_predicate_column_refs(predicate, sources, columns)
        }
        Expression::Unary(unary) => {
            collect_expression_column_refs(unary.operand(), sources, columns)
        }
        Expression::Binary(binary) => {
            collect_expression_column_refs(binary.left(), sources, columns);
            collect_expression_column_refs(binary.right(), sources, columns);
        }
        Expression::AggregateFunction(_)
        | Expression::WindowFunction(_)
        | Expression::Literal(_)
        | Expression::ScalarSubquery(_)
        | Expression::Unknown(_)
        | Expression::Unsupported(_) => {}
    }
}

fn analyze_query_predicates(query: &SqlQuery, diagnostics: &mut Vec<Diagnostic>) -> Predicates {
    analyze_query_predicates_with_outer_scope(query, &[], diagnostics)
}

fn analyze_query_predicates_with_outer_scope(
    query: &SqlQuery,
    outer_scope: &[OutputRelation],
    diagnostics: &mut Vec<Diagnostic>,
) -> Predicates {
    match query.body.as_ref() {
        SetExpr::Select(select) => {
            let mut scope_diagnostics = Vec::new();
            let scope = build_output_scope(
                select,
                &BTreeMap::new(),
                outer_scope,
                &mut scope_diagnostics,
                None,
            );
            analyze_select_predicates_with_scope(select, &scope, diagnostics)
        }
        SetExpr::Query(query) => {
            analyze_query_predicates_with_outer_scope(query, outer_scope, diagnostics)
        }
        SetExpr::SetOperation { .. } => Predicates::new(None, None, None),
        _ => Predicates::new(None, None, None),
    }
}

fn analyze_select(select: &Select, diagnostics: &mut Vec<Diagnostic>) -> Predicates {
    inspect_select_features(select, diagnostics);
    let mut scope_diagnostics = Vec::new();
    let scope = build_output_scope(select, &BTreeMap::new(), &[], &mut scope_diagnostics, None);
    analyze_select_predicates_with_scope(select, &scope, diagnostics)
}

fn analyze_select_predicates_with_scope(
    select: &Select,
    scope: &[OutputRelation],
    diagnostics: &mut Vec<Diagnostic>,
) -> Predicates {
    let empty_aliases = BTreeMap::new();
    let output_aliases = select
        .projection
        .iter()
        .filter_map(|item| match item {
            SelectItem::ExprWithAlias { expr, alias } => Some((alias.value.clone(), expr)),
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();

    let where_predicate = select.selection.as_ref().map(|expression| {
        analyze_predicate_with_windows(
            expression,
            &select.named_window,
            &empty_aliases,
            scope,
            diagnostics,
        )
    });
    let having_predicate = select.having.as_ref().map(|expression| {
        analyze_predicate_with_windows(
            expression,
            &select.named_window,
            &output_aliases,
            scope,
            diagnostics,
        )
    });
    let qualify_predicate = select.qualify.as_ref().map(|expression| {
        analyze_predicate_with_windows(
            expression,
            &select.named_window,
            &output_aliases,
            scope,
            diagnostics,
        )
    });

    Predicates::new(where_predicate, having_predicate, qualify_predicate)
}

fn analyze_query_aggregation(
    query: &SqlQuery,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Aggregation> {
    match query.body.as_ref() {
        SetExpr::Select(select) => analyze_select_aggregation(select, diagnostics),
        SetExpr::Query(query) => analyze_query_aggregation(query, diagnostics),
        _ => None,
    }
}

fn analyze_select_aggregation(
    select: &Select,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Aggregation> {
    let (distinct, distinct_on) = match &select.distinct {
        None => (false, Vec::new()),
        Some(SqlDistinct::Distinct) => (true, Vec::new()),
        Some(SqlDistinct::On(expressions)) => (
            true,
            expressions
                .iter()
                .map(|expression| {
                    analyze_expression_with_windows(expression, &select.named_window, diagnostics)
                })
                .collect(),
        ),
    };

    let group_by = match &select.group_by {
        GroupByExpr::All(modifiers) => {
            diagnose_group_by_modifiers(modifiers, diagnostics);
            Some(GroupBy::All)
        }
        GroupByExpr::Expressions(expressions, modifiers) => {
            diagnose_group_by_modifiers(modifiers, diagnostics);
            if expressions.is_empty() {
                None
            } else {
                Some(GroupBy::Expressions(
                    expressions
                        .iter()
                        .map(|expression| {
                            analyze_grouping_expression(
                                expression,
                                &select.named_window,
                                diagnostics,
                            )
                        })
                        .collect(),
                ))
            }
        }
    };

    if !distinct && group_by.is_none() {
        None
    } else {
        Some(Aggregation::new(distinct, distinct_on, group_by))
    }
}

fn analyze_grouping_expression(
    expression: &Expr,
    named_windows: &[NamedWindowDefinition],
    diagnostics: &mut Vec<Diagnostic>,
) -> GroupingExpression {
    match expression {
        Expr::GroupingSets(sets) => GroupingExpression::GroupingSets(analyze_grouping_sets(
            sets,
            named_windows,
            diagnostics,
        )),
        Expr::Rollup(sets) => {
            GroupingExpression::Rollup(analyze_grouping_sets(sets, named_windows, diagnostics))
        }
        Expr::Cube(sets) => {
            GroupingExpression::Cube(analyze_grouping_sets(sets, named_windows, diagnostics))
        }
        _ => GroupingExpression::Expression(analyze_expression_with_windows(
            expression,
            named_windows,
            diagnostics,
        )),
    }
}

fn analyze_grouping_sets(
    sets: &[Vec<Expr>],
    named_windows: &[NamedWindowDefinition],
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<Vec<Expression>> {
    sets.iter()
        .map(|set| {
            set.iter()
                .map(|expression| {
                    analyze_expression_with_windows(expression, named_windows, diagnostics)
                })
                .collect()
        })
        .collect()
}

fn diagnose_group_by_modifiers(
    modifiers: &[SqlGroupByWithModifier],
    diagnostics: &mut Vec<Diagnostic>,
) {
    for modifier in modifiers {
        diagnostics.push(warning(
            "unsupported_group_by_modifier",
            DiagnosticArea::Other,
            &format!("GROUP BY modifier {modifier} is not represented safely"),
        ));
    }
}

fn analyze_set_operation(expression: &SetExpr) -> Option<SetOperation> {
    match expression {
        SetExpr::SetOperation {
            left,
            op,
            set_quantifier,
            right,
        } => Some(SetOperation::new(
            analyze_set_operator(*op),
            analyze_set_quantifier(*set_quantifier),
            analyze_set_operand(left),
            analyze_set_operand(right),
        )),
        SetExpr::Query(query) => analyze_set_operation(query.body.as_ref()),
        SetExpr::Select(_)
        | SetExpr::Values(_)
        | SetExpr::Insert(_)
        | SetExpr::Update(_)
        | SetExpr::Delete(_)
        | SetExpr::Table(_) => None,
    }
}

fn analyze_set_operand(expression: &SetExpr) -> SetOperand {
    match analyze_set_operation(expression) {
        Some(operation) => SetOperand::Operation(Box::new(operation)),
        None => SetOperand::Query,
    }
}

fn analyze_set_operator(operator: SqlSetOperator) -> SetOperator {
    match operator {
        SqlSetOperator::Union => SetOperator::Union,
        SqlSetOperator::Except | SqlSetOperator::Minus => SetOperator::Except,
        SqlSetOperator::Intersect => SetOperator::Intersect,
    }
}

fn analyze_set_quantifier(quantifier: SqlSetQuantifier) -> SetQuantifier {
    match quantifier {
        SqlSetQuantifier::All => SetQuantifier::All,
        SqlSetQuantifier::Distinct | SqlSetQuantifier::None => SetQuantifier::Distinct,
        SqlSetQuantifier::ByName => SetQuantifier::ByName,
        SqlSetQuantifier::AllByName => SetQuantifier::AllByName,
        SqlSetQuantifier::DistinctByName => SetQuantifier::DistinctByName,
    }
}

type LocalDomainMap = BTreeMap<String, Vec<ColumnDomain>>;

fn analyze_query_column_domains(
    query: &SqlQuery,
    inherited_local_relations: &BTreeSet<String>,
    inherited_local_outputs: &LocalOutputMap,
    inherited_local_domains: &LocalDomainMap,
    metadata: Option<&AnalysisMetadata<'_>>,
) -> Vec<ColumnDomain> {
    let mut local_relations = inherited_local_relations.clone();
    let mut local_outputs = inherited_local_outputs.clone();
    let mut local_domains = inherited_local_domains.clone();

    if let Some(with) = &query.with {
        for cte in &with.cte_tables {
            let name = cte.alias.name.to_string();
            let domains = analyze_query_column_domains(
                &cte.query,
                &local_relations,
                &local_outputs,
                &local_domains,
                metadata,
            );
            let mut output_diagnostics = Vec::new();
            let output = analyze_query_output(
                &cte.query,
                &local_outputs,
                &mut output_diagnostics,
                metadata,
            );

            local_relations.insert(name.clone());
            local_outputs.insert(name.clone(), output);
            local_domains.insert(name, domains);
        }
    }

    analyze_set_expr_column_domains(
        query.body.as_ref(),
        &local_relations,
        &local_outputs,
        &local_domains,
        metadata,
    )
}

fn analyze_set_expr_column_domains(
    expression: &SetExpr,
    local_relations: &BTreeSet<String>,
    local_outputs: &LocalOutputMap,
    local_domains: &LocalDomainMap,
    metadata: Option<&AnalysisMetadata<'_>>,
) -> Vec<ColumnDomain> {
    match expression {
        SetExpr::Select(select) => {
            let mut predicate_diagnostics = Vec::new();
            let scope = build_output_scope(
                select,
                local_outputs,
                &[],
                &mut predicate_diagnostics,
                metadata,
            );
            let predicates =
                analyze_select_predicates_with_scope(select, &scope, &mut predicate_diagnostics);
            let domain_predicates = remap_predicates_for_domain_derivation(&predicates, &scope);
            let mut relation_diagnostics = Vec::new();
            let mut derived_index = 0;
            let relations = analyze_select_relations(
                select,
                local_relations,
                &mut relation_diagnostics,
                &mut derived_index,
            );
            let predicate_domains = derive_column_domains(&domain_predicates, &relations.sources);
            let join_domains =
                derive_inner_join_column_domains(&relations.joins, &relations.sources);
            let own_domains = remap_local_column_domains(
                intersect_column_domain_sets([predicate_domains, join_domains]),
                local_outputs,
            );
            let source_domains = collect_select_local_domains(
                select,
                local_relations,
                local_outputs,
                local_domains,
                metadata,
            );
            intersect_column_domain_sets([source_domains, own_domains])
        }
        SetExpr::Query(query) => analyze_query_column_domains(
            query,
            local_relations,
            local_outputs,
            local_domains,
            metadata,
        ),
        SetExpr::SetOperation { left, right, .. } => merge_set_operation_domains(
            analyze_set_expr_column_domains(
                left,
                local_relations,
                local_outputs,
                local_domains,
                metadata,
            ),
            analyze_set_expr_column_domains(
                right,
                local_relations,
                local_outputs,
                local_domains,
                metadata,
            ),
        ),
        SetExpr::Values(_)
        | SetExpr::Insert(_)
        | SetExpr::Update(_)
        | SetExpr::Delete(_)
        | SetExpr::Table(_) => Vec::new(),
    }
}

fn derive_inner_join_column_domains(
    joins: &[ProtocolJoin],
    sources: &[SourceRelation],
) -> Vec<ColumnDomain> {
    intersect_column_domain_sets(
        joins
            .iter()
            .filter(|join| join.kind() == JoinKind::Inner)
            .filter_map(ProtocolJoin::condition)
            .map(|predicate| derive_inner_join_predicate_domains(predicate, sources)),
    )
}

fn derive_inner_join_predicate_domains(
    predicate: &Predicate,
    sources: &[SourceRelation],
) -> Vec<ColumnDomain> {
    match predicate {
        Predicate::And(logical) => intersect_column_domain_sets(
            logical
                .operands()
                .iter()
                .map(|operand| derive_inner_join_predicate_domains(operand, sources)),
        ),
        Predicate::Comparison(comparison)
            if matches!(
                (comparison.left(), comparison.right()),
                (Expression::Column(_), Expression::Literal(_))
                    | (Expression::Literal(_), Expression::Column(_))
            ) =>
        {
            derive_column_domains(
                &Predicates::new(Some(predicate.clone()), None, None),
                sources,
            )
        }
        Predicate::IsNull(predicate)
            if matches!(predicate.expression(), Expression::Column(_)) =>
        {
            derive_column_domains(
                &Predicates::new(Some(Predicate::IsNull(predicate.clone())), None, None),
                sources,
            )
        }
        Predicate::In(predicate)
            if matches!(predicate.expression(), Expression::Column(_))
                && predicate
                    .values()
                    .iter()
                    .all(|value| matches!(value, Expression::Literal(_))) =>
        {
            derive_column_domains(
                &Predicates::new(Some(Predicate::In(predicate.clone())), None, None),
                sources,
            )
        }
        Predicate::Between(predicate)
            if matches!(predicate.expression(), Expression::Column(_))
                && matches!(predicate.lower(), Expression::Literal(_))
                && matches!(predicate.upper(), Expression::Literal(_)) =>
        {
            derive_column_domains(
                &Predicates::new(Some(Predicate::Between(predicate.clone())), None, None),
                sources,
            )
        }
        _ => Vec::new(),
    }
}

fn collect_select_local_domains(
    select: &Select,
    local_relations: &BTreeSet<String>,
    local_outputs: &LocalOutputMap,
    local_domains: &LocalDomainMap,
    metadata: Option<&AnalysisMetadata<'_>>,
) -> Vec<ColumnDomain> {
    let mut domains = Vec::new();

    for source in &select.from {
        domains.extend(collect_table_factor_local_domains(
            &source.relation,
            local_relations,
            local_outputs,
            local_domains,
            metadata,
        ));
        for join in &source.joins {
            domains.extend(collect_table_factor_local_domains(
                &join.relation,
                local_relations,
                local_outputs,
                local_domains,
                metadata,
            ));
        }
    }

    intersect_column_domain_sets([domains])
}

fn collect_table_factor_local_domains(
    factor: &TableFactor,
    local_relations: &BTreeSet<String>,
    local_outputs: &LocalOutputMap,
    local_domains: &LocalDomainMap,
    metadata: Option<&AnalysisMetadata<'_>>,
) -> Vec<ColumnDomain> {
    match factor {
        TableFactor::Table {
            name, args: None, ..
        } => local_domains
            .get(&name.to_string())
            .cloned()
            .unwrap_or_default(),
        TableFactor::Derived { subquery, .. } => analyze_query_column_domains(
            subquery,
            local_relations,
            local_outputs,
            local_domains,
            metadata,
        ),
        _ => Vec::new(),
    }
}

fn remap_local_column_domains(
    domains: Vec<ColumnDomain>,
    local_outputs: &LocalOutputMap,
) -> Vec<ColumnDomain> {
    domains
        .into_iter()
        .flat_map(|column_domain| {
            let Some(relation) = column_domain.column().relation() else {
                return vec![column_domain];
            };
            let Some(output) = local_outputs.get(relation) else {
                return vec![column_domain];
            };
            let mut candidates = output
                .columns()
                .iter()
                .filter(|column| column.name() == column_domain.column().name());
            let Some(output_column) = candidates.next() else {
                return vec![column_domain];
            };
            if candidates.next().is_some() {
                return vec![ColumnDomain::new(
                    column_domain.column().clone(),
                    ValueDomain::unknown(format!(
                        "local column {} is ambiguous and cannot be mapped safely to physical source columns",
                        qualified_column_name(
                            column_domain.column().relation(),
                            column_domain.column().name()
                        )
                    )),
                )];
            }
            let Some(source) = output_column.plain_copy_source() else {
                let reason = format!(
                    "predicate on computed local column {} cannot be mapped safely to physical source columns",
                    qualified_column_name(
                        column_domain.column().relation(),
                        column_domain.column().name()
                    )
                );
                if output_column.lineage().is_empty() {
                    return vec![ColumnDomain::new(
                        column_domain.column().clone(),
                        ValueDomain::unknown(reason),
                    )];
                }
                return output_column
                    .lineage()
                    .iter()
                    .map(|source| {
                        ColumnDomain::new(
                            ColumnRef::new(
                                Some(source.relation().to_string()),
                                source.column().to_string(),
                            ),
                            ValueDomain::unknown(reason.clone()),
                        )
                    })
                    .collect();
            };

            vec![ColumnDomain::new(
                ColumnRef::new(
                    Some(source.relation().to_string()),
                    source.column().to_string(),
                ),
                column_domain.domain().clone(),
            )]
        })
        .collect()
}

fn remap_predicates_for_domain_derivation(
    predicates: &Predicates,
    scope: &[OutputRelation],
) -> Predicates {
    Predicates::new(
        predicates
            .where_predicate()
            .map(|predicate| remap_predicate_for_domain_derivation(predicate, scope)),
        predicates
            .having_predicate()
            .map(|predicate| remap_predicate_for_domain_derivation(predicate, scope)),
        predicates
            .qualify_predicate()
            .map(|predicate| remap_predicate_for_domain_derivation(predicate, scope)),
    )
}

fn remap_predicate_for_domain_derivation(
    predicate: &Predicate,
    scope: &[OutputRelation],
) -> Predicate {
    match predicate {
        Predicate::Comparison(comparison) => Predicate::Comparison(ComparisonPredicate::new(
            remap_expression_for_domain_derivation(comparison.left(), scope),
            comparison.operator(),
            remap_expression_for_domain_derivation(comparison.right(), scope),
        )),
        Predicate::And(logical) => {
            remap_logical_predicate_for_domain_derivation(logical, scope, Predicate::And)
        }
        Predicate::Or(logical) => {
            remap_logical_predicate_for_domain_derivation(logical, scope, Predicate::Or)
        }
        Predicate::Not(not) => Predicate::Not(NotPredicate::new(
            remap_predicate_for_domain_derivation(not.operand(), scope),
        )),
        Predicate::IsNull(predicate) => Predicate::IsNull(IsNullPredicate::new(
            remap_expression_for_domain_derivation(predicate.expression(), scope),
            predicate.negated(),
        )),
        Predicate::In(predicate) => Predicate::In(InPredicate::new(
            remap_expression_for_domain_derivation(predicate.expression(), scope),
            predicate
                .values()
                .iter()
                .map(|value| remap_expression_for_domain_derivation(value, scope))
                .collect(),
            predicate.negated(),
        )),
        Predicate::Exists(predicate) => Predicate::Exists(predicate.clone()),
        Predicate::InSubquery(predicate) => Predicate::InSubquery(InSubqueryPredicate::new(
            remap_expression_for_domain_derivation(predicate.expression(), scope),
            predicate.subquery().clone(),
            predicate.negated(),
        )),
        Predicate::Between(predicate) => Predicate::Between(BetweenPredicate::new(
            remap_expression_for_domain_derivation(predicate.expression(), scope),
            remap_expression_for_domain_derivation(predicate.lower(), scope),
            remap_expression_for_domain_derivation(predicate.upper(), scope),
            predicate.negated(),
        )),
        Predicate::BooleanExpression(expression) => {
            Predicate::BooleanExpression(remap_expression_for_domain_derivation(expression, scope))
        }
        Predicate::Unknown(semantic) => Predicate::Unknown(semantic.clone()),
        Predicate::Unsupported(semantic) => Predicate::Unsupported(semantic.clone()),
    }
}

fn remap_logical_predicate_for_domain_derivation(
    logical: &LogicalPredicate,
    scope: &[OutputRelation],
    constructor: fn(LogicalPredicate) -> Predicate,
) -> Predicate {
    match logical.operands() {
        [left, right] => constructor(LogicalPredicate::pair(
            remap_predicate_for_domain_derivation(left, scope),
            remap_predicate_for_domain_derivation(right, scope),
        )),
        _ => constructor(logical.clone()),
    }
}

fn remap_expression_for_domain_derivation(
    expression: &Expression,
    scope: &[OutputRelation],
) -> Expression {
    let Expression::Column(column) = expression else {
        return expression.clone();
    };
    let reference = ColumnRef::new(
        column.relation().map(ToString::to_string),
        column.name().to_string(),
    );
    match resolve_plain_source_column(&reference, scope) {
        Ok(source) => Expression::Column(ColumnExpression::new(
            Some(source.relation().to_string()),
            source.column().to_string(),
        )),
        Err(_) => expression.clone(),
    }
}

fn intersect_column_domain_sets<const N: usize>(sets: [Vec<ColumnDomain>; N]) -> Vec<ColumnDomain> {
    let mut domains = BTreeMap::<ColumnRef, ValueDomain>::new();
    for column_domain in sets.into_iter().flatten() {
        let column = column_domain.column().clone();
        let domain = column_domain.domain().clone();
        domains
            .entry(column)
            .and_modify(|existing| {
                *existing = intersect_domains(existing, &domain);
            })
            .or_insert(domain);
    }

    domains
        .into_iter()
        .map(|(column, domain)| ColumnDomain::new(column, domain))
        .collect()
}

fn merge_set_operation_domains(
    left: Vec<ColumnDomain>,
    right: Vec<ColumnDomain>,
) -> Vec<ColumnDomain> {
    let mut domains = BTreeMap::<ColumnRef, ValueDomain>::new();

    for column_domain in left.into_iter().chain(right) {
        let column = column_domain.column().clone();
        let domain = column_domain.domain().clone();

        match domains.get_mut(&column) {
            Some(existing) if *existing != domain => {
                *existing = ValueDomain::unknown(
                    "set-operation branches impose different constraints on the same source column",
                );
            }
            Some(_) => {}
            None => {
                domains.insert(column, domain);
            }
        }
    }

    domains
        .into_iter()
        .map(|(column, domain)| ColumnDomain::new(column, domain))
        .collect()
}

#[derive(Clone, Default)]
struct RelationAnalysis {
    sources: Vec<SourceRelation>,
    dependencies: BTreeSet<String>,
    joins: Vec<ProtocolJoin>,
}

#[derive(Clone)]
struct LocalRelationAnalysis {
    analysis: RelationAnalysis,
    diagnostics: Vec<Diagnostic>,
}

type LocalRelationAnalysisMap = BTreeMap<String, LocalRelationAnalysis>;

struct RelationAnalysisScope<'a> {
    local_relations: &'a BTreeSet<String>,
    local_outputs: &'a LocalOutputMap,
    local_analyses: &'a LocalRelationAnalysisMap,
    output_scope: &'a [OutputRelation],
}

struct AnalyzedRelation {
    source: SourceRelation,
    reference: RelationRef,
    dependencies: BTreeSet<String>,
    joins: Vec<ProtocolJoin>,
}

fn diagnose_local_query_predicates(
    query: &SqlQuery,
    inherited_local_outputs: &LocalOutputMap,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut local_outputs = inherited_local_outputs.clone();

    if let Some(with) = &query.with {
        for cte in &with.cte_tables {
            let mut output_diagnostics = Vec::new();
            let output =
                analyze_query_output(&cte.query, &local_outputs, &mut output_diagnostics, None);
            local_outputs.insert(cte.alias.name.to_string(), output);
        }
    }

    diagnose_local_set_expr_predicates(query.body.as_ref(), &local_outputs, diagnostics);
}

fn diagnose_local_set_expr_predicates(
    expression: &SetExpr,
    local_outputs: &LocalOutputMap,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match expression {
        SetExpr::Select(select) => {
            let mut predicate_diagnostics = Vec::new();
            let scope =
                build_output_scope(select, local_outputs, &[], &mut predicate_diagnostics, None);
            let predicates =
                analyze_select_predicates_with_scope(select, &scope, &mut predicate_diagnostics);
            extend_unique_diagnostics(diagnostics, &predicate_diagnostics);
            diagnose_uncarried_local_predicates(&predicates, &scope, diagnostics);
        }
        SetExpr::Query(query) => {
            diagnose_local_query_predicates(query, local_outputs, diagnostics);
        }
        SetExpr::SetOperation { left, right, .. } => {
            diagnose_local_set_expr_predicates(left, local_outputs, diagnostics);
            diagnose_local_set_expr_predicates(right, local_outputs, diagnostics);
        }
        SetExpr::Values(_)
        | SetExpr::Insert(_)
        | SetExpr::Update(_)
        | SetExpr::Delete(_)
        | SetExpr::Table(_) => {}
    }
}

fn diagnose_uncarried_local_predicates(
    predicates: &Predicates,
    scope: &[OutputRelation],
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (clause, predicate) in [
        ("WHERE", predicates.where_predicate()),
        ("HAVING", predicates.having_predicate()),
        ("QUALIFY", predicates.qualify_predicate()),
    ] {
        let Some(predicate) = predicate else {
            continue;
        };
        let Some(reason) = uncarried_local_predicate_reason(predicate, scope) else {
            continue;
        };
        let diagnostic = warning(
            "unresolved_local_predicate",
            DiagnosticArea::Predicate,
            &format!(
                "{clause} predicate inside a local relation cannot be carried completely through physical column domains: {reason}"
            ),
        );
        if !diagnostics.contains(&diagnostic) {
            diagnostics.push(diagnostic);
        }
    }
}

fn uncarried_local_predicate_reason(
    predicate: &Predicate,
    scope: &[OutputRelation],
) -> Option<String> {
    match predicate {
        Predicate::Comparison(comparison) => match (comparison.left(), comparison.right()) {
            (Expression::Column(column), Expression::Literal(_))
            | (Expression::Literal(_), Expression::Column(column)) => {
                unresolved_domain_column_reason(column, scope)
            }
            _ => Some(
                "comparison is not between one source column and one scalar literal".to_string(),
            ),
        },
        Predicate::And(logical) => logical
            .operands()
            .iter()
            .find_map(|predicate| uncarried_local_predicate_reason(predicate, scope)),
        Predicate::Or(_) => Some(
            "logical OR cannot be preserved as one conjunction of independent physical column domains"
                .to_string(),
        ),
        Predicate::Not(_) => Some(
            "logical NOT cannot always be reduced safely to independent physical column domains"
                .to_string(),
        ),
        Predicate::IsNull(predicate) => match predicate.expression() {
            Expression::Column(column) => unresolved_domain_column_reason(column, scope),
            _ => Some("null predicate targets a computed or unresolved expression".to_string()),
        },
        Predicate::In(predicate)
            if predicate
                .values()
                .iter()
                .all(|value| matches!(value, Expression::Literal(_))) =>
        {
            match predicate.expression() {
                Expression::Column(column) => unresolved_domain_column_reason(column, scope),
                _ => Some("IN-list predicate targets a computed or unresolved expression".to_string()),
            }
        }
        Predicate::In(_) => {
            Some("IN-list predicate contains values that are not scalar literals".to_string())
        }
        Predicate::Exists(_) => {
            Some("EXISTS semantics depend on nested-row existence, not a scalar domain".to_string())
        }
        Predicate::InSubquery(_) => {
            Some("IN-subquery semantics depend on nested rows, not only a scalar domain".to_string())
        }
        Predicate::Between(predicate)
            if matches!(predicate.lower(), Expression::Literal(_))
                && matches!(predicate.upper(), Expression::Literal(_)) =>
        {
            match predicate.expression() {
                Expression::Column(column) => unresolved_domain_column_reason(column, scope),
                _ => Some("BETWEEN predicate targets a computed or unresolved expression".to_string()),
            }
        }
        Predicate::Between(_) => {
            Some("BETWEEN predicate bounds are not both scalar literals".to_string())
        }
        Predicate::BooleanExpression(_) => {
            Some("boolean predicate expression cannot be reduced safely to a scalar domain".to_string())
        }
        Predicate::Unknown(semantic) => Some(format!(
            "predicate semantics are unresolved: {}",
            semantic.reason()
        )),
        Predicate::Unsupported(semantic) => Some(match semantic.reason() {
            Some(reason) => format!("unsupported {} predicate semantics: {reason}", semantic.feature()),
            None => format!("unsupported {} predicate semantics", semantic.feature()),
        }),
    }
}

fn unresolved_domain_column_reason(
    column: &ColumnExpression,
    scope: &[OutputRelation],
) -> Option<String> {
    let reference = ColumnRef::new(
        column.relation().map(ToString::to_string),
        column.name().to_string(),
    );
    resolve_plain_source_column(&reference, scope)
        .err()
        .map(|reason| {
            format!("source column cannot be mapped safely to physical lineage: {reason}")
        })
}

fn analyze_query_relations(
    query: &SqlQuery,
    inherited_local_relations: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
    derived_index: &mut usize,
) -> RelationAnalysis {
    analyze_query_relations_with_locals(
        query,
        inherited_local_relations,
        &BTreeMap::new(),
        &BTreeMap::new(),
        diagnostics,
        derived_index,
    )
}

fn analyze_query_relations_with_locals(
    query: &SqlQuery,
    inherited_local_relations: &BTreeSet<String>,
    inherited_local_outputs: &LocalOutputMap,
    inherited_local_analyses: &LocalRelationAnalysisMap,
    diagnostics: &mut Vec<Diagnostic>,
    derived_index: &mut usize,
) -> RelationAnalysis {
    let mut local_relations = inherited_local_relations.clone();
    let mut local_outputs = inherited_local_outputs.clone();
    let mut local_analyses = inherited_local_analyses.clone();

    if let Some(with) = &query.with {
        for cte in &with.cte_tables {
            local_relations.insert(cte.alias.name.to_string());
        }

        for cte in &with.cte_tables {
            let name = cte.alias.name.to_string();
            let mut cte_diagnostics = Vec::new();
            let nested = analyze_query_relations_with_locals(
                &cte.query,
                &local_relations,
                &local_outputs,
                &local_analyses,
                &mut cte_diagnostics,
                derived_index,
            );
            diagnose_local_query_predicates(&cte.query, &local_outputs, &mut cte_diagnostics);
            let mut output_diagnostics = Vec::new();
            let output =
                analyze_query_output(&cte.query, &local_outputs, &mut output_diagnostics, None);
            local_outputs.insert(name.clone(), output);
            local_analyses.insert(
                name,
                LocalRelationAnalysis {
                    analysis: nested,
                    diagnostics: cte_diagnostics,
                },
            );
        }
    }

    analyze_set_expr_relations_with_locals(
        query.body.as_ref(),
        &local_relations,
        &local_outputs,
        &local_analyses,
        diagnostics,
        derived_index,
    )
}

fn analyze_set_expr_relations_with_locals(
    expression: &SetExpr,
    local_relations: &BTreeSet<String>,
    local_outputs: &LocalOutputMap,
    local_analyses: &LocalRelationAnalysisMap,
    diagnostics: &mut Vec<Diagnostic>,
    derived_index: &mut usize,
) -> RelationAnalysis {
    match expression {
        SetExpr::Select(select) => analyze_select_relations_with_locals(
            select,
            local_relations,
            local_outputs,
            local_analyses,
            diagnostics,
            derived_index,
        ),
        SetExpr::Query(query) => analyze_query_relations_with_locals(
            query,
            local_relations,
            local_outputs,
            local_analyses,
            diagnostics,
            derived_index,
        ),
        SetExpr::SetOperation { left, right, .. } => {
            let mut analysis = analyze_set_expr_relations_with_locals(
                left,
                local_relations,
                local_outputs,
                local_analyses,
                diagnostics,
                derived_index,
            );
            let right = analyze_set_expr_relations_with_locals(
                right,
                local_relations,
                local_outputs,
                local_analyses,
                diagnostics,
                derived_index,
            );
            merge_relation_analysis(&mut analysis, right);
            analysis
        }
        _ => RelationAnalysis::default(),
    }
}

fn analyze_select_relations(
    select: &Select,
    local_relations: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
    derived_index: &mut usize,
) -> RelationAnalysis {
    analyze_select_relations_with_locals(
        select,
        local_relations,
        &BTreeMap::new(),
        &BTreeMap::new(),
        diagnostics,
        derived_index,
    )
}

fn analyze_select_relations_with_locals(
    select: &Select,
    local_relations: &BTreeSet<String>,
    local_outputs: &LocalOutputMap,
    local_analyses: &LocalRelationAnalysisMap,
    diagnostics: &mut Vec<Diagnostic>,
    derived_index: &mut usize,
) -> RelationAnalysis {
    let mut analysis = RelationAnalysis::default();
    let mut scope_diagnostics = Vec::new();
    let output_scope = build_output_scope(select, local_outputs, &[], &mut scope_diagnostics, None);
    let relation_scope = RelationAnalysisScope {
        local_relations,
        local_outputs,
        local_analyses,
        output_scope: &output_scope,
    };

    for source in &select.from {
        analyze_table_with_joins(
            source,
            &relation_scope,
            diagnostics,
            derived_index,
            &mut analysis,
        );
    }

    for item in &select.projection {
        match item {
            SelectItem::UnnamedExpr(expression)
            | SelectItem::ExprWithAlias {
                expr: expression, ..
            } => collect_expression_dependencies_with_windows(
                expression,
                &select.named_window,
                local_relations,
                diagnostics,
                derived_index,
                &mut analysis.dependencies,
            ),
            SelectItem::QualifiedWildcard(_, _) | SelectItem::Wildcard(_) => {}
        }
    }

    if let Some(SqlDistinct::On(expressions)) = &select.distinct {
        for expression in expressions {
            collect_expression_dependencies_with_windows(
                expression,
                &select.named_window,
                local_relations,
                diagnostics,
                derived_index,
                &mut analysis.dependencies,
            );
        }
    }

    if let GroupByExpr::Expressions(expressions, modifiers) = &select.group_by {
        for expression in expressions {
            collect_grouping_dependencies(
                expression,
                &select.named_window,
                local_relations,
                diagnostics,
                derived_index,
                &mut analysis.dependencies,
            );
        }
        for modifier in modifiers {
            if let SqlGroupByWithModifier::GroupingSets(expression) = modifier {
                collect_grouping_dependencies(
                    expression,
                    &select.named_window,
                    local_relations,
                    diagnostics,
                    derived_index,
                    &mut analysis.dependencies,
                );
            }
        }
    }

    for expression in [
        select.selection.as_ref(),
        select.having.as_ref(),
        select.qualify.as_ref(),
    ]
    .into_iter()
    .flatten()
    {
        collect_expression_dependencies_with_windows(
            expression,
            &select.named_window,
            local_relations,
            diagnostics,
            derived_index,
            &mut analysis.dependencies,
        );
    }

    analysis
}

fn collect_grouping_dependencies(
    expression: &Expr,
    named_windows: &[NamedWindowDefinition],
    local_relations: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
    derived_index: &mut usize,
    dependencies: &mut BTreeSet<String>,
) {
    match expression {
        Expr::GroupingSets(sets) | Expr::Rollup(sets) | Expr::Cube(sets) => {
            for set in sets {
                for expression in set {
                    collect_grouping_dependencies(
                        expression,
                        named_windows,
                        local_relations,
                        diagnostics,
                        derived_index,
                        dependencies,
                    );
                }
            }
        }
        _ => collect_expression_dependencies_with_windows(
            expression,
            named_windows,
            local_relations,
            diagnostics,
            derived_index,
            dependencies,
        ),
    }
}

fn analyze_table_with_joins(
    source: &TableWithJoins,
    scope: &RelationAnalysisScope<'_>,
    diagnostics: &mut Vec<Diagnostic>,
    derived_index: &mut usize,
    analysis: &mut RelationAnalysis,
) {
    let mut left = register_table_factor(
        &source.relation,
        scope,
        diagnostics,
        derived_index,
        analysis,
    );

    for join in &source.joins {
        let right =
            register_table_factor(&join.relation, scope, diagnostics, derived_index, analysis);

        if let (Some(left_ref), Some(right_ref)) = (left.as_ref(), right.as_ref()) {
            analysis.joins.push(analyze_join(
                join,
                left_ref,
                right_ref,
                scope,
                diagnostics,
                derived_index,
                &mut analysis.dependencies,
            ));
        } else {
            diagnostics.push(warning(
                "unsupported_join_relation",
                DiagnosticArea::Join,
                "join participants could not both be represented safely",
            ));
        }

        if right.is_some() {
            left = right;
        }
    }
}

fn register_table_factor(
    factor: &TableFactor,
    scope: &RelationAnalysisScope<'_>,
    diagnostics: &mut Vec<Diagnostic>,
    derived_index: &mut usize,
    analysis: &mut RelationAnalysis,
) -> Option<RelationRef> {
    let relation = analyze_table_factor_with_locals(
        factor,
        scope.local_relations,
        scope.local_outputs,
        scope.local_analyses,
        diagnostics,
        derived_index,
    )?;

    for dependency in relation.dependencies {
        analysis.dependencies.insert(dependency);
    }
    analysis.joins.extend(relation.joins);

    if !analysis.sources.iter().any(|existing| {
        existing.name() == relation.source.name() && existing.alias() == relation.source.alias()
    }) {
        analysis.sources.push(relation.source);
    }

    Some(relation.reference)
}

fn analyze_table_factor(
    factor: &TableFactor,
    local_relations: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
    derived_index: &mut usize,
) -> Option<AnalyzedRelation> {
    analyze_table_factor_with_locals(
        factor,
        local_relations,
        &BTreeMap::new(),
        &BTreeMap::new(),
        diagnostics,
        derived_index,
    )
}

fn analyze_table_factor_with_locals(
    factor: &TableFactor,
    local_relations: &BTreeSet<String>,
    local_outputs: &LocalOutputMap,
    local_analyses: &LocalRelationAnalysisMap,
    diagnostics: &mut Vec<Diagnostic>,
    derived_index: &mut usize,
) -> Option<AnalyzedRelation> {
    match factor {
        TableFactor::Table {
            name,
            alias,
            args: None,
            ..
        } => {
            let name = name.to_string();
            let alias = alias.as_ref().map(|alias| alias.name.to_string());
            let (dependencies, joins) = if local_relations.contains(&name) {
                match local_analyses.get(&name) {
                    Some(local) => {
                        extend_unique_diagnostics(diagnostics, &local.diagnostics);
                        (
                            local.analysis.dependencies.clone(),
                            local.analysis.joins.clone(),
                        )
                    }
                    None => {
                        diagnostics.push(warning(
                            "unresolved_local_relation_analysis",
                            DiagnosticArea::Source,
                            &format!(
                                "local relation {name} is referenced before its relation semantics can be resolved"
                            ),
                        ));
                        (BTreeSet::new(), Vec::new())
                    }
                }
            } else {
                (BTreeSet::from([name.clone()]), Vec::new())
            };

            Some(AnalyzedRelation {
                source: SourceRelation::new(name.clone(), alias.clone()),
                reference: RelationRef::new(name, alias),
                dependencies,
                joins,
            })
        }
        TableFactor::Derived {
            subquery, alias, ..
        } => {
            let nested = analyze_query_relations_with_locals(
                subquery,
                local_relations,
                local_outputs,
                local_analyses,
                diagnostics,
                derived_index,
            );
            diagnose_local_query_predicates(subquery, local_outputs, diagnostics);
            let alias = alias.as_ref().map(|alias| alias.name.to_string());
            let name = match &alias {
                Some(_) => "subquery".to_string(),
                None => {
                    *derived_index += 1;
                    format!("subquery#{}", derived_index)
                }
            };

            Some(AnalyzedRelation {
                source: SourceRelation::new(name.clone(), alias.clone()),
                reference: RelationRef::new(name, alias),
                dependencies: nested.dependencies,
                joins: nested.joins,
            })
        }
        _ => {
            diagnostics.push(warning(
                "unsupported_table_factor",
                DiagnosticArea::Source,
                &format!(
                    "table factor {factor} is parsed but its source semantics are not implemented"
                ),
            ));
            None
        }
    }
}

fn extend_unique_diagnostics(target: &mut Vec<Diagnostic>, source: &[Diagnostic]) {
    for diagnostic in source {
        if !target.contains(diagnostic) {
            target.push(diagnostic.clone());
        }
    }
}

fn analyze_join(
    join: &SqlJoin,
    left: &RelationRef,
    right: &RelationRef,
    scope: &RelationAnalysisScope<'_>,
    diagnostics: &mut Vec<Diagnostic>,
    derived_index: &mut usize,
    dependencies: &mut BTreeSet<String>,
) -> ProtocolJoin {
    if join.global {
        diagnostics.push(warning(
            "unsupported_global_join",
            DiagnosticArea::Join,
            "GLOBAL join modifier semantics are not implemented",
        ));
    }

    let (kind, constraint, exact_kind) = analyze_join_operator(&join.join_operator);

    if !exact_kind {
        diagnostics.push(warning(
            "unsupported_join_form",
            DiagnosticArea::Join,
            "join kind is parsed but is not represented precisely by protocol v0",
        ));
    }

    if let Some(JoinConstraint::On(expression)) = constraint {
        collect_expression_dependencies(
            expression,
            scope.local_relations,
            diagnostics,
            derived_index,
            dependencies,
        );
    }

    let condition = constraint.and_then(|constraint| {
        analyze_join_constraint(constraint, left, right, scope.output_scope, diagnostics)
    });

    ProtocolJoin::new(kind, left.clone(), right.clone(), condition)
}

fn analyze_join_operator(operator: &JoinOperator) -> (JoinKind, Option<&JoinConstraint>, bool) {
    match operator {
        JoinOperator::Join(constraint) | JoinOperator::Inner(constraint) => {
            (JoinKind::Inner, Some(constraint), true)
        }
        JoinOperator::Left(constraint) | JoinOperator::LeftOuter(constraint) => {
            (JoinKind::Left, Some(constraint), true)
        }
        JoinOperator::Right(constraint) | JoinOperator::RightOuter(constraint) => {
            (JoinKind::Right, Some(constraint), true)
        }
        JoinOperator::FullOuter(constraint) => (JoinKind::Full, Some(constraint), true),
        JoinOperator::CrossJoin => (JoinKind::Cross, None, true),
        JoinOperator::Semi(constraint) | JoinOperator::LeftSemi(constraint) => {
            (JoinKind::LeftSemi, Some(constraint), true)
        }
        JoinOperator::RightSemi(constraint) => (JoinKind::RightSemi, Some(constraint), true),
        JoinOperator::Anti(constraint) | JoinOperator::LeftAnti(constraint) => {
            (JoinKind::LeftAnti, Some(constraint), true)
        }
        JoinOperator::RightAnti(constraint) => (JoinKind::RightAnti, Some(constraint), true),
        JoinOperator::StraightJoin(constraint) => (JoinKind::Unknown, Some(constraint), false),
        JoinOperator::AsOf { constraint, .. } => (JoinKind::Unknown, Some(constraint), false),
        JoinOperator::CrossApply | JoinOperator::OuterApply => (JoinKind::Unknown, None, false),
    }
}

fn analyze_join_constraint(
    constraint: &JoinConstraint,
    left: &RelationRef,
    right: &RelationRef,
    scope: &[OutputRelation],
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Predicate> {
    let predicate = match constraint {
        JoinConstraint::On(expression) => Some(analyze_predicate_with_windows(
            expression,
            &[],
            &BTreeMap::new(),
            scope,
            diagnostics,
        )),
        JoinConstraint::Using(columns) => {
            let left_name = left.alias().unwrap_or(left.relation()).to_string();
            let right_name = right.alias().unwrap_or(right.relation()).to_string();
            let predicates = columns.iter().map(|column| {
                Predicate::Comparison(ComparisonPredicate::new(
                    Expression::Column(ColumnExpression::new(
                        Some(left_name.clone()),
                        column.to_string(),
                    )),
                    ComparisonOperator::Eq,
                    Expression::Column(ColumnExpression::new(
                        Some(right_name.clone()),
                        column.to_string(),
                    )),
                ))
            });
            combine_conjunction(predicates)
        }
        JoinConstraint::Natural => {
            diagnostics.push(warning(
                "unsupported_natural_join_condition",
                DiagnosticArea::Join,
                "NATURAL JOIN columns cannot be resolved without source schema information",
            ));
            None
        }
        JoinConstraint::None => None,
    };

    predicate.map(|predicate| remap_join_equality_columns(predicate, scope, diagnostics))
}

fn remap_join_equality_columns(
    predicate: Predicate,
    scope: &[OutputRelation],
    diagnostics: &mut Vec<Diagnostic>,
) -> Predicate {
    match predicate {
        Predicate::Comparison(comparison) if comparison.operator() == ComparisonOperator::Eq => {
            Predicate::Comparison(ComparisonPredicate::new(
                remap_join_equality_expression(comparison.left(), scope, diagnostics),
                comparison.operator(),
                remap_join_equality_expression(comparison.right(), scope, diagnostics),
            ))
        }
        Predicate::And(logical) => {
            remap_join_logical_predicate(logical.operands(), true, scope, diagnostics)
        }
        Predicate::Or(logical) => {
            remap_join_logical_predicate(logical.operands(), false, scope, diagnostics)
        }
        Predicate::Not(not) => Predicate::Not(NotPredicate::new(remap_join_equality_columns(
            not.operand().clone(),
            scope,
            diagnostics,
        ))),
        other => other,
    }
}

fn remap_join_logical_predicate(
    operands: &[Predicate],
    conjunction: bool,
    scope: &[OutputRelation],
    diagnostics: &mut Vec<Diagnostic>,
) -> Predicate {
    let mut mapped = operands
        .iter()
        .cloned()
        .map(|predicate| remap_join_equality_columns(predicate, scope, diagnostics));
    let Some(first) = mapped.next() else {
        return Predicate::Unknown(UnknownSemantic::new(
            "join logical predicate has no operands".to_string(),
        ));
    };

    mapped.fold(first, |left, right| {
        if conjunction {
            Predicate::And(LogicalPredicate::pair(left, right))
        } else {
            Predicate::Or(LogicalPredicate::pair(left, right))
        }
    })
}

fn remap_join_equality_expression(
    expression: &Expression,
    scope: &[OutputRelation],
    diagnostics: &mut Vec<Diagnostic>,
) -> Expression {
    let Expression::Column(column) = expression else {
        return expression.clone();
    };

    let column_ref = ColumnRef::new(
        column.relation().map(str::to_string),
        column.name().to_string(),
    );
    match resolve_plain_source_column(&column_ref, scope) {
        Ok(source) => Expression::Column(ColumnExpression::new(
            Some(source.relation().to_string()),
            source.column().to_string(),
        )),
        Err(reason) => {
            let reason = format!("join equality column {reason}");
            let diagnostic = warning(
                "unresolved_join_column_lineage",
                DiagnosticArea::Join,
                &reason,
            );
            if !diagnostics.contains(&diagnostic) {
                diagnostics.push(diagnostic);
            }
            Expression::Unknown(UnknownSemantic::new(reason))
        }
    }
}

fn combine_conjunction(predicates: impl IntoIterator<Item = Predicate>) -> Option<Predicate> {
    let mut predicates = predicates.into_iter();
    let first = predicates.next()?;
    Some(predicates.fold(first, |left, right| {
        Predicate::And(LogicalPredicate::pair(left, right))
    }))
}

fn collect_expression_dependencies(
    expression: &Expr,
    local_relations: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
    derived_index: &mut usize,
    dependencies: &mut BTreeSet<String>,
) {
    collect_expression_dependencies_with_windows(
        expression,
        &[],
        local_relations,
        diagnostics,
        derived_index,
        dependencies,
    );
}

fn collect_expression_dependencies_with_windows(
    expression: &Expr,
    named_windows: &[NamedWindowDefinition],
    local_relations: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
    derived_index: &mut usize,
    dependencies: &mut BTreeSet<String>,
) {
    match expression {
        Expr::Subquery(query)
        | Expr::Exists {
            subquery: query, ..
        } => {
            let nested =
                analyze_query_relations(query, local_relations, diagnostics, derived_index);
            dependencies.extend(nested.dependencies);
        }
        Expr::InSubquery { expr, subquery, .. } => {
            collect_expression_dependencies_with_windows(
                expr,
                named_windows,
                local_relations,
                diagnostics,
                derived_index,
                dependencies,
            );
            let nested =
                analyze_query_relations(subquery, local_relations, diagnostics, derived_index);
            dependencies.extend(nested.dependencies);
        }
        Expr::BinaryOp { left, right, .. }
        | Expr::AnyOp { left, right, .. }
        | Expr::AllOp { left, right, .. } => {
            collect_expression_dependencies_with_windows(
                left,
                named_windows,
                local_relations,
                diagnostics,
                derived_index,
                dependencies,
            );
            collect_expression_dependencies_with_windows(
                right,
                named_windows,
                local_relations,
                diagnostics,
                derived_index,
                dependencies,
            );
        }
        Expr::UnaryOp { expr, .. } | Expr::Nested(expr) => {
            collect_expression_dependencies_with_windows(
                expr,
                named_windows,
                local_relations,
                diagnostics,
                derived_index,
                dependencies,
            );
        }
        Expr::Between {
            expr, low, high, ..
        } => {
            for expression in [expr.as_ref(), low.as_ref(), high.as_ref()] {
                collect_expression_dependencies_with_windows(
                    expression,
                    named_windows,
                    local_relations,
                    diagnostics,
                    derived_index,
                    dependencies,
                );
            }
        }
        Expr::InList { expr, list, .. } => {
            collect_expression_dependencies_with_windows(
                expr,
                named_windows,
                local_relations,
                diagnostics,
                derived_index,
                dependencies,
            );
            for value in list {
                collect_expression_dependencies_with_windows(
                    value,
                    named_windows,
                    local_relations,
                    diagnostics,
                    derived_index,
                    dependencies,
                );
            }
        }
        Expr::Case {
            operand,
            conditions,
            else_result,
            ..
        } => {
            if let Some(operand) = operand {
                collect_expression_dependencies_with_windows(
                    operand,
                    named_windows,
                    local_relations,
                    diagnostics,
                    derived_index,
                    dependencies,
                );
            }
            for branch in conditions {
                collect_expression_dependencies_with_windows(
                    &branch.condition,
                    named_windows,
                    local_relations,
                    diagnostics,
                    derived_index,
                    dependencies,
                );
                collect_expression_dependencies_with_windows(
                    &branch.result,
                    named_windows,
                    local_relations,
                    diagnostics,
                    derived_index,
                    dependencies,
                );
            }
            if let Some(else_result) = else_result {
                collect_expression_dependencies_with_windows(
                    else_result,
                    named_windows,
                    local_relations,
                    diagnostics,
                    derived_index,
                    dependencies,
                );
            }
        }
        Expr::Function(function) => {
            collect_function_argument_dependencies(
                &function.parameters,
                named_windows,
                local_relations,
                diagnostics,
                derived_index,
                dependencies,
            );
            collect_function_argument_dependencies(
                &function.args,
                named_windows,
                local_relations,
                diagnostics,
                derived_index,
                dependencies,
            );
            if let Some(filter) = &function.filter {
                collect_expression_dependencies_with_windows(
                    filter,
                    named_windows,
                    local_relations,
                    diagnostics,
                    derived_index,
                    dependencies,
                );
            }
            if let Some(window) = &function.over {
                collect_window_dependencies(
                    window,
                    named_windows,
                    &mut BTreeSet::new(),
                    local_relations,
                    diagnostics,
                    derived_index,
                    dependencies,
                );
            }
        }
        _ => {}
    }
}

fn collect_function_argument_dependencies(
    arguments: &FunctionArguments,
    named_windows: &[NamedWindowDefinition],
    local_relations: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
    derived_index: &mut usize,
    dependencies: &mut BTreeSet<String>,
) {
    match arguments {
        FunctionArguments::None => {}
        FunctionArguments::Subquery(query) => {
            let nested =
                analyze_query_relations(query, local_relations, diagnostics, derived_index);
            dependencies.extend(nested.dependencies);
        }
        FunctionArguments::List(arguments) => {
            for argument in &arguments.args {
                if let FunctionArg::Unnamed(FunctionArgExpr::Expr(expression)) = argument {
                    collect_expression_dependencies_with_windows(
                        expression,
                        named_windows,
                        local_relations,
                        diagnostics,
                        derived_index,
                        dependencies,
                    );
                }
            }
        }
    }
}

fn collect_window_dependencies(
    window: &WindowType,
    named_windows: &[NamedWindowDefinition],
    visited: &mut BTreeSet<String>,
    local_relations: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
    derived_index: &mut usize,
    dependencies: &mut BTreeSet<String>,
) {
    match window {
        WindowType::WindowSpec(spec) => collect_window_spec_dependencies(
            spec,
            named_windows,
            visited,
            local_relations,
            diagnostics,
            derived_index,
            dependencies,
        ),
        WindowType::NamedWindow(name) => collect_named_window_dependencies(
            &name.value,
            named_windows,
            visited,
            local_relations,
            diagnostics,
            derived_index,
            dependencies,
        ),
    }
}

fn collect_named_window_dependencies(
    name: &str,
    named_windows: &[NamedWindowDefinition],
    visited: &mut BTreeSet<String>,
    local_relations: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
    derived_index: &mut usize,
    dependencies: &mut BTreeSet<String>,
) {
    if !visited.insert(name.to_string()) {
        return;
    }

    if let Some(NamedWindowDefinition(_, definition)) = named_windows
        .iter()
        .find(|definition| definition.0.value == name)
    {
        match definition {
            NamedWindowExpr::NamedWindow(base) => collect_named_window_dependencies(
                &base.value,
                named_windows,
                visited,
                local_relations,
                diagnostics,
                derived_index,
                dependencies,
            ),
            NamedWindowExpr::WindowSpec(spec) => collect_window_spec_dependencies(
                spec,
                named_windows,
                visited,
                local_relations,
                diagnostics,
                derived_index,
                dependencies,
            ),
        }
    }

    visited.remove(name);
}

fn collect_window_spec_dependencies(
    spec: &SqlWindowSpec,
    named_windows: &[NamedWindowDefinition],
    visited: &mut BTreeSet<String>,
    local_relations: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
    derived_index: &mut usize,
    dependencies: &mut BTreeSet<String>,
) {
    if let Some(name) = &spec.window_name {
        collect_named_window_dependencies(
            &name.value,
            named_windows,
            visited,
            local_relations,
            diagnostics,
            derived_index,
            dependencies,
        );
    }

    for expression in &spec.partition_by {
        collect_expression_dependencies_with_windows(
            expression,
            named_windows,
            local_relations,
            diagnostics,
            derived_index,
            dependencies,
        );
    }
    for order in &spec.order_by {
        collect_expression_dependencies_with_windows(
            &order.expr,
            named_windows,
            local_relations,
            diagnostics,
            derived_index,
            dependencies,
        );
    }
    if let Some(frame) = &spec.window_frame {
        for bound in [
            &frame.start_bound,
            frame
                .end_bound
                .as_ref()
                .unwrap_or(&SqlWindowFrameBound::CurrentRow),
        ] {
            match bound {
                SqlWindowFrameBound::Preceding(Some(expression))
                | SqlWindowFrameBound::Following(Some(expression)) => {
                    collect_expression_dependencies_with_windows(
                        expression,
                        named_windows,
                        local_relations,
                        diagnostics,
                        derived_index,
                        dependencies,
                    );
                }
                _ => {}
            }
        }
    }
}

fn merge_relation_analysis(target: &mut RelationAnalysis, source: RelationAnalysis) {
    for relation in source.sources {
        if !target.sources.iter().any(|existing| {
            existing.name() == relation.name() && existing.alias() == relation.alias()
        }) {
            target.sources.push(relation);
        }
    }
    target.dependencies.extend(source.dependencies);
    target.joins.extend(source.joins);
}

fn analyze_predicate(expression: &Expr, diagnostics: &mut Vec<Diagnostic>) -> Predicate {
    analyze_predicate_with_windows(expression, &[], &BTreeMap::new(), &[], diagnostics)
}

fn analyze_predicate_with_windows(
    expression: &Expr,
    named_windows: &[NamedWindowDefinition],
    output_aliases: &BTreeMap<String, &Expr>,
    scope: &[OutputRelation],
    diagnostics: &mut Vec<Diagnostic>,
) -> Predicate {
    match expression {
        Expr::Nested(inner) => {
            analyze_predicate_with_windows(inner, named_windows, output_aliases, scope, diagnostics)
        }
        Expr::BinaryOp { left, op, right } => match op {
            SqlBinaryOperator::And => Predicate::And(LogicalPredicate::pair(
                analyze_predicate_with_windows(
                    left,
                    named_windows,
                    output_aliases,
                    scope,
                    diagnostics,
                ),
                analyze_predicate_with_windows(
                    right,
                    named_windows,
                    output_aliases,
                    scope,
                    diagnostics,
                ),
            )),
            SqlBinaryOperator::Or => Predicate::Or(LogicalPredicate::pair(
                analyze_predicate_with_windows(
                    left,
                    named_windows,
                    output_aliases,
                    scope,
                    diagnostics,
                ),
                analyze_predicate_with_windows(
                    right,
                    named_windows,
                    output_aliases,
                    scope,
                    diagnostics,
                ),
            )),
            _ => match comparison_operator(op) {
                Some(operator) => normalize_comparison(
                    analyze_predicate_expression(
                        left,
                        named_windows,
                        output_aliases,
                        scope,
                        diagnostics,
                    ),
                    operator,
                    analyze_predicate_expression(
                        right,
                        named_windows,
                        output_aliases,
                        scope,
                        diagnostics,
                    ),
                ),
                None => Predicate::BooleanExpression(analyze_expression_with_scope(
                    expression,
                    scope,
                    named_windows,
                    diagnostics,
                )),
            },
        },
        Expr::IsDistinctFrom(left, right) => normalize_comparison(
            analyze_predicate_expression(left, named_windows, output_aliases, scope, diagnostics),
            ComparisonOperator::IsDistinctFrom,
            analyze_predicate_expression(right, named_windows, output_aliases, scope, diagnostics),
        ),
        Expr::IsNotDistinctFrom(left, right) => normalize_comparison(
            analyze_predicate_expression(left, named_windows, output_aliases, scope, diagnostics),
            ComparisonOperator::IsNotDistinctFrom,
            analyze_predicate_expression(right, named_windows, output_aliases, scope, diagnostics),
        ),
        Expr::IsNull(inner) => Predicate::IsNull(IsNullPredicate::new(
            analyze_predicate_expression(inner, named_windows, output_aliases, scope, diagnostics),
            false,
        )),
        Expr::IsNotNull(inner) => Predicate::IsNull(IsNullPredicate::new(
            analyze_predicate_expression(inner, named_windows, output_aliases, scope, diagnostics),
            true,
        )),
        Expr::InList {
            expr,
            list,
            negated,
        } => Predicate::In(InPredicate::new(
            analyze_predicate_expression(expr, named_windows, output_aliases, scope, diagnostics),
            list.iter()
                .map(|value| {
                    analyze_predicate_expression(
                        value,
                        named_windows,
                        output_aliases,
                        scope,
                        diagnostics,
                    )
                })
                .collect(),
            *negated,
        )),
        Expr::Exists { subquery, negated } => Predicate::Exists(ExistsPredicate::new(
            analyze_subquery_semantics(subquery, scope),
            *negated,
        )),
        Expr::InSubquery {
            expr,
            subquery,
            negated,
        } => Predicate::InSubquery(InSubqueryPredicate::new(
            analyze_predicate_expression(expr, named_windows, output_aliases, scope, diagnostics),
            analyze_subquery_semantics(subquery, scope),
            *negated,
        )),
        Expr::Between {
            expr,
            negated,
            low,
            high,
        } => Predicate::Between(BetweenPredicate::new(
            analyze_predicate_expression(expr, named_windows, output_aliases, scope, diagnostics),
            analyze_predicate_expression(low, named_windows, output_aliases, scope, diagnostics),
            analyze_predicate_expression(high, named_windows, output_aliases, scope, diagnostics),
            *negated,
        )),
        Expr::UnaryOp {
            op: SqlUnaryOperator::Not | SqlUnaryOperator::BangNot,
            expr,
        } => Predicate::Not(NotPredicate::new(analyze_predicate_with_windows(
            expr,
            named_windows,
            output_aliases,
            scope,
            diagnostics,
        ))),
        _ => Predicate::BooleanExpression(analyze_predicate_expression(
            expression,
            named_windows,
            output_aliases,
            scope,
            diagnostics,
        )),
    }
}

fn analyze_predicate_expression(
    expression: &Expr,
    named_windows: &[NamedWindowDefinition],
    output_aliases: &BTreeMap<String, &Expr>,
    scope: &[OutputRelation],
    diagnostics: &mut Vec<Diagnostic>,
) -> Expression {
    if let Expr::Identifier(identifier) = expression {
        if let Some(aliased_expression) = output_aliases.get(&identifier.value) {
            return analyze_expression_with_scope(
                aliased_expression,
                scope,
                named_windows,
                diagnostics,
            );
        }
    }

    analyze_expression_with_scope(expression, scope, named_windows, diagnostics)
}

fn normalize_comparison(
    left: Expression,
    operator: ComparisonOperator,
    right: Expression,
) -> Predicate {
    if matches!(left, Expression::Literal(_)) && matches!(right, Expression::Column(_)) {
        Predicate::Comparison(ComparisonPredicate::new(right, operator.reversed(), left))
    } else {
        Predicate::Comparison(ComparisonPredicate::new(left, operator, right))
    }
}

fn comparison_operator(operator: &SqlBinaryOperator) -> Option<ComparisonOperator> {
    match operator {
        SqlBinaryOperator::Eq => Some(ComparisonOperator::Eq),
        SqlBinaryOperator::NotEq => Some(ComparisonOperator::Neq),
        SqlBinaryOperator::Lt => Some(ComparisonOperator::Lt),
        SqlBinaryOperator::LtEq => Some(ComparisonOperator::Lte),
        SqlBinaryOperator::Gt => Some(ComparisonOperator::Gt),
        SqlBinaryOperator::GtEq => Some(ComparisonOperator::Gte),
        _ => None,
    }
}

fn analyze_expression_with_scope(
    expression: &Expr,
    scope: &[OutputRelation],
    named_windows: &[NamedWindowDefinition],
    diagnostics: &mut Vec<Diagnostic>,
) -> Expression {
    match expression {
        Expr::Subquery(query) => Expression::ScalarSubquery(Box::new(
            ScalarSubqueryExpression::new(analyze_subquery_semantics(query, scope)),
        )),
        Expr::Case {
            operand,
            conditions,
            else_result,
            ..
        } => analyze_case_expression(
            operand.as_deref(),
            conditions,
            else_result.as_deref(),
            scope,
            named_windows,
            diagnostics,
        ),
        _ if is_boolean_value_expression(expression) => {
            Expression::BooleanPredicate(Box::new(analyze_predicate_with_windows(
                expression,
                named_windows,
                &BTreeMap::new(),
                scope,
                diagnostics,
            )))
        }
        _ => analyze_expression_with_windows(expression, named_windows, diagnostics),
    }
}

fn is_boolean_value_expression(expression: &Expr) -> bool {
    match expression {
        Expr::BinaryOp { op, .. } => {
            comparison_operator(op).is_some()
                || matches!(op, SqlBinaryOperator::And | SqlBinaryOperator::Or)
        }
        Expr::IsDistinctFrom(_, _)
        | Expr::IsNotDistinctFrom(_, _)
        | Expr::IsNull(_)
        | Expr::IsNotNull(_)
        | Expr::InList { .. }
        | Expr::Exists { .. }
        | Expr::InSubquery { .. }
        | Expr::Between { .. } => true,
        Expr::Nested(inner) => is_boolean_value_expression(inner),
        _ => false,
    }
}

fn analyze_case_expression(
    operand: Option<&Expr>,
    conditions: &[sqlparser::ast::CaseWhen],
    else_result: Option<&Expr>,
    scope: &[OutputRelation],
    named_windows: &[NamedWindowDefinition],
    diagnostics: &mut Vec<Diagnostic>,
) -> Expression {
    let normalized_operand = operand
        .map(|value| analyze_expression_with_scope(value, scope, named_windows, diagnostics));
    let normalized_branches = conditions
        .iter()
        .map(|branch| {
            let condition = if operand.is_some() {
                analyze_expression_with_scope(&branch.condition, scope, named_windows, diagnostics)
            } else {
                Expression::BooleanPredicate(Box::new(analyze_predicate_with_windows(
                    &branch.condition,
                    named_windows,
                    &BTreeMap::new(),
                    scope,
                    diagnostics,
                )))
            };
            let result =
                analyze_expression_with_scope(&branch.result, scope, named_windows, diagnostics);
            (condition, result)
        })
        .collect::<Vec<_>>();
    let branch_predicates = normalized_branches
        .iter()
        .map(|(condition, _)| match &normalized_operand {
            Some(operand) => Predicate::Comparison(ComparisonPredicate::new(
                operand.clone(),
                ComparisonOperator::Eq,
                condition.clone(),
            )),
            None => match condition {
                Expression::BooleanPredicate(predicate) => predicate.as_ref().clone(),
                _ => Predicate::BooleanExpression(condition.clone()),
            },
        })
        .collect::<Vec<_>>();
    let case_sources = case_scope_sources(scope);
    let (branch_source_domains, else_source_domains) =
        derive_case_source_domains(&branch_predicates, &case_sources);
    let branch_source_domains = branch_source_domains
        .into_iter()
        .map(|domains| remap_case_source_domains_to_physical(domains, scope))
        .collect::<Vec<_>>();
    let else_source_domains = remap_case_source_domains_to_physical(else_source_domains, scope);
    let branches = normalized_branches
        .into_iter()
        .zip(branch_source_domains)
        .map(|((condition, result), source_domains)| {
            CaseBranch::new(condition, result, source_domains)
        })
        .collect();
    let normalized_else = else_result
        .map(|value| analyze_expression_with_scope(value, scope, named_windows, diagnostics));

    Expression::Case(CaseExpression::new(
        normalized_operand,
        branches,
        normalized_else,
        else_source_domains,
    ))
}

fn case_scope_sources(scope: &[OutputRelation]) -> Vec<SourceRelation> {
    scope
        .iter()
        .filter_map(|relation| {
            relation
                .qualifiers
                .first()
                .map(|qualifier| SourceRelation::new(qualifier.clone(), None))
        })
        .collect()
}

fn remap_case_source_domains_to_physical(
    source_domains: CaseSourceDomains,
    scope: &[OutputRelation],
) -> CaseSourceDomains {
    let alternatives = match source_domains {
        CaseSourceDomains::Reachable { alternatives } => alternatives,
        other => return other,
    };

    let mut mapped_alternatives = Vec::new();
    for alternative in alternatives {
        let mut mapped_domains = BTreeMap::<ColumnRef, ValueDomain>::new();
        for column_domain in alternative.column_domains() {
            let source = match resolve_plain_source_column(column_domain.column(), scope) {
                Ok(source) => source,
                Err(reason) => {
                    return CaseSourceDomains::unknown(format!("CASE branch source {reason}"));
                }
            };
            let column = ColumnRef::new(
                Some(source.relation().to_string()),
                source.column().to_string(),
            );
            let domain = column_domain.domain().clone();
            mapped_domains
                .entry(column)
                .and_modify(|existing| {
                    *existing = intersect_case_domain_values(existing, &domain);
                })
                .or_insert(domain);
        }

        if mapped_domains
            .values()
            .any(|domain| matches!(domain, ValueDomain::Empty))
        {
            continue;
        }
        mapped_domains.retain(|_, domain| !matches!(domain, ValueDomain::Unbounded));
        let mapped = CaseSourceDomainAlternative::new(
            mapped_domains
                .into_iter()
                .map(|(column, domain)| ColumnDomain::new(column, domain))
                .collect(),
        );
        if !mapped_alternatives.contains(&mapped) {
            mapped_alternatives.push(mapped);
        }
    }

    CaseSourceDomains::reachable(mapped_alternatives)
}

fn resolve_plain_source_column(
    column: &ColumnRef,
    scope: &[OutputRelation],
) -> Result<LineageSource, String> {
    let candidates = scope
        .iter()
        .filter(|relation| {
            column.relation().is_none_or(|qualifier| {
                relation
                    .qualifiers
                    .iter()
                    .any(|candidate| candidate == qualifier)
            })
        })
        .filter_map(|relation| match &relation.source {
            OutputRelationSource::Physical {
                relation,
                columns: Some(columns),
            } if !columns.iter().any(|candidate| candidate == column.name()) => None,
            OutputRelationSource::Physical { relation, .. } => Some(Ok(LineageSource::new(
                relation.clone(),
                column.name().to_string(),
            ))),
            OutputRelationSource::Local(output) => {
                let mut matches = output
                    .columns()
                    .iter()
                    .filter(|candidate| candidate.name() == column.name());
                let candidate = matches.next()?;
                if matches.next().is_some() {
                    return Some(Err(format!(
                        "column {} is ambiguous within the local relation",
                        qualified_column_name(column.relation(), column.name())
                    )));
                }
                Some(candidate.plain_copy_source().cloned().ok_or_else(|| {
                    format!(
                        "column {} is produced by a computed local expression and cannot be mapped safely to physical lineage",
                        qualified_column_name(column.relation(), column.name())
                    )
                }))
            }
        })
        .collect::<Vec<_>>();

    match candidates.as_slice() {
        [Ok(source)] => Ok(source.clone()),
        [Err(reason)] => Err(reason.clone()),
        [] => Err(format!(
            "column {} could not be resolved to physical lineage",
            qualified_column_name(column.relation(), column.name())
        )),
        _ => Err(format!(
            "column {} is ambiguous without source schema information",
            qualified_column_name(column.relation(), column.name())
        )),
    }
}

fn analyze_expression_with_windows(
    expression: &Expr,
    named_windows: &[NamedWindowDefinition],
    diagnostics: &mut Vec<Diagnostic>,
) -> Expression {
    match expression {
        Expr::Identifier(identifier) => {
            Expression::Column(ColumnExpression::new(None, identifier.value.clone()))
        }
        Expr::CompoundIdentifier(identifiers) => analyze_compound_identifier(identifiers),
        Expr::Value(value) => analyze_value(&value.value, expression, diagnostics),
        Expr::TypedString { data_type, value } => analyze_typed_string(
            data_type.to_string().as_str(),
            &value.value,
            expression,
            diagnostics,
        ),
        Expr::Function(function) => {
            analyze_function(function, expression, named_windows, diagnostics)
        }
        Expr::UnaryOp { op, expr } => {
            analyze_unary_expression(op, expr, expression, named_windows, diagnostics)
        }
        Expr::BinaryOp { left, op, right } => {
            analyze_binary_expression(left, op, right, expression, named_windows, diagnostics)
        }
        Expr::Nested(inner) => analyze_expression_with_windows(inner, named_windows, diagnostics),
        Expr::Case {
            operand,
            conditions,
            else_result,
            ..
        } => analyze_case_expression(
            operand.as_deref(),
            conditions,
            else_result.as_deref(),
            &[],
            named_windows,
            diagnostics,
        ),
        Expr::Subquery(query) => Expression::ScalarSubquery(Box::new(
            ScalarSubqueryExpression::new(analyze_subquery_semantics(query, &[])),
        )),
        _ => unsupported_expression(
            "expression",
            expression,
            DiagnosticArea::Expression,
            diagnostics,
        ),
    }
}

fn analyze_compound_identifier(identifiers: &[sqlparser::ast::Ident]) -> Expression {
    match identifiers.split_last() {
        Some((column, relation_parts)) => {
            let relation = if relation_parts.is_empty() {
                None
            } else {
                Some(
                    relation_parts
                        .iter()
                        .map(|identifier| identifier.value.as_str())
                        .collect::<Vec<_>>()
                        .join("."),
                )
            };
            Expression::Column(ColumnExpression::new(relation, column.value.clone()))
        }
        None => Expression::Unsupported(UnsupportedSemantic::new(
            "column_reference".to_string(),
            Some("empty compound identifier cannot be resolved".to_string()),
        )),
    }
}

fn analyze_value(
    value: &Value,
    expression: &Expr,
    diagnostics: &mut Vec<Diagnostic>,
) -> Expression {
    match value {
        Value::Number(value, _) => analyze_number(value, expression, diagnostics),
        Value::SingleQuotedString(value)
        | Value::TripleSingleQuotedString(value)
        | Value::TripleDoubleQuotedString(value)
        | Value::EscapedStringLiteral(value)
        | Value::UnicodeStringLiteral(value)
        | Value::SingleQuotedRawStringLiteral(value)
        | Value::DoubleQuotedRawStringLiteral(value)
        | Value::TripleSingleQuotedRawStringLiteral(value)
        | Value::TripleDoubleQuotedRawStringLiteral(value)
        | Value::NationalStringLiteral(value)
        | Value::DoubleQuotedString(value) => Expression::Literal(LiteralExpression::new(
            LiteralType::String,
            LiteralValue::Text(value.clone()),
        )),
        Value::Boolean(value) => Expression::Literal(LiteralExpression::new(
            LiteralType::Boolean,
            LiteralValue::Boolean(*value),
        )),
        Value::Null => Expression::Literal(LiteralExpression::new(
            LiteralType::Null,
            LiteralValue::Null,
        )),
        _ => unsupported_expression(
            "literal",
            expression,
            DiagnosticArea::Expression,
            diagnostics,
        ),
    }
}

fn analyze_number(value: &str, expression: &Expr, diagnostics: &mut Vec<Diagnostic>) -> Expression {
    match Number::from_str(value) {
        Ok(number) => {
            let literal_type = if value.contains('.') || value.contains('e') || value.contains('E')
            {
                LiteralType::Decimal
            } else {
                LiteralType::Integer
            };

            Expression::Literal(LiteralExpression::new(
                literal_type,
                LiteralValue::Number(number.to_string()),
            ))
        }
        Err(_) => unsupported_expression(
            "numeric_literal",
            expression,
            DiagnosticArea::Expression,
            diagnostics,
        ),
    }
}

fn analyze_typed_string(
    data_type: &str,
    value: &Value,
    expression: &Expr,
    diagnostics: &mut Vec<Diagnostic>,
) -> Expression {
    let data_type = data_type.to_ascii_uppercase();
    let literal_type = match data_type.as_str() {
        "DATE" => Some(LiteralType::Date),
        data_type if data_type.starts_with("TIMESTAMP") => Some(LiteralType::Timestamp),
        data_type if data_type.starts_with("TIME") => Some(LiteralType::Time),
        data_type if data_type.starts_with("INTERVAL") => Some(LiteralType::Interval),
        _ => None,
    };

    match (literal_type, string_literal_value(value)) {
        (Some(literal_type), Some(value)) => Expression::Literal(LiteralExpression::new(
            literal_type,
            LiteralValue::Text(value.to_string()),
        )),
        _ => unsupported_expression(
            "typed_literal",
            expression,
            DiagnosticArea::Expression,
            diagnostics,
        ),
    }
}

fn string_literal_value(value: &Value) -> Option<&str> {
    match value {
        Value::SingleQuotedString(value)
        | Value::TripleSingleQuotedString(value)
        | Value::TripleDoubleQuotedString(value)
        | Value::EscapedStringLiteral(value)
        | Value::UnicodeStringLiteral(value)
        | Value::SingleQuotedRawStringLiteral(value)
        | Value::DoubleQuotedRawStringLiteral(value)
        | Value::TripleSingleQuotedRawStringLiteral(value)
        | Value::TripleDoubleQuotedRawStringLiteral(value)
        | Value::NationalStringLiteral(value)
        | Value::DoubleQuotedString(value) => Some(value),
        _ => None,
    }
}

fn analyze_function(
    function: &Function,
    expression: &Expr,
    named_windows: &[NamedWindowDefinition],
    diagnostics: &mut Vec<Diagnostic>,
) -> Expression {
    if function.over.is_none() && is_aggregate_function(function) {
        return analyze_aggregate_function(function, expression, named_windows, diagnostics);
    }

    if function.uses_odbc_syntax
        || !matches!(&function.parameters, FunctionArguments::None)
        || function.filter.is_some()
        || function.null_treatment.is_some()
        || !function.within_group.is_empty()
    {
        return unsupported_expression(
            "function",
            expression,
            DiagnosticArea::Function,
            diagnostics,
        );
    }

    let (arguments, distinct) = match &function.args {
        FunctionArguments::None => (Vec::new(), false),
        FunctionArguments::List(arguments) if arguments.clauses.is_empty() => {
            let distinct = matches!(
                arguments.duplicate_treatment,
                Some(DuplicateTreatment::Distinct)
            );
            let mut normalized_arguments = Vec::with_capacity(arguments.args.len());

            for argument in &arguments.args {
                match argument {
                    FunctionArg::Unnamed(FunctionArgExpr::Expr(argument)) => {
                        normalized_arguments.push(analyze_expression_with_windows(
                            argument,
                            named_windows,
                            diagnostics,
                        ));
                    }
                    _ => {
                        return unsupported_expression(
                            "function",
                            expression,
                            DiagnosticArea::Function,
                            diagnostics,
                        )
                    }
                }
            }

            (normalized_arguments, distinct)
        }
        FunctionArguments::List(_) | FunctionArguments::Subquery(_) => {
            return unsupported_expression(
                "function",
                expression,
                DiagnosticArea::Function,
                diagnostics,
            );
        }
    };

    let function_expression =
        FunctionExpression::new(function.name.to_string(), arguments, distinct);

    match &function.over {
        Some(window) => match analyze_window_type(window, named_windows, diagnostics) {
            Some(window) => Expression::WindowFunction(WindowFunctionExpression::new(
                function_expression,
                window,
            )),
            None => Expression::Unsupported(UnsupportedSemantic::new(
                "window_function".to_string(),
                Some("window specification could not be resolved safely".to_string()),
            )),
        },
        None => Expression::Function(function_expression),
    }
}

fn analyze_aggregate_function(
    function: &Function,
    expression: &Expr,
    named_windows: &[NamedWindowDefinition],
    diagnostics: &mut Vec<Diagnostic>,
) -> Expression {
    if function.uses_odbc_syntax
        || !matches!(&function.parameters, FunctionArguments::None)
        || function.null_treatment.is_some()
        || !function.within_group.is_empty()
    {
        return unsupported_expression(
            "aggregate_function",
            expression,
            DiagnosticArea::Function,
            diagnostics,
        );
    }

    let (arguments, distinct) = match &function.args {
        FunctionArguments::None => (Vec::new(), false),
        FunctionArguments::List(arguments) if arguments.clauses.is_empty() => {
            let distinct = matches!(
                arguments.duplicate_treatment,
                Some(DuplicateTreatment::Distinct)
            );
            let mut normalized_arguments = Vec::with_capacity(arguments.args.len());

            for argument in &arguments.args {
                match argument {
                    FunctionArg::Unnamed(FunctionArgExpr::Expr(argument)) => {
                        normalized_arguments.push(AggregateArgument::Expression(
                            analyze_expression_with_windows(argument, named_windows, diagnostics),
                        ));
                    }
                    FunctionArg::Unnamed(FunctionArgExpr::Wildcard) => {
                        normalized_arguments.push(AggregateArgument::Wildcard);
                    }
                    FunctionArg::Unnamed(FunctionArgExpr::QualifiedWildcard(qualifier)) => {
                        normalized_arguments
                            .push(AggregateArgument::QualifiedWildcard(qualifier.to_string()));
                    }
                    _ => {
                        return unsupported_expression(
                            "aggregate_function",
                            expression,
                            DiagnosticArea::Function,
                            diagnostics,
                        )
                    }
                }
            }

            (normalized_arguments, distinct)
        }
        FunctionArguments::List(_) | FunctionArguments::Subquery(_) => {
            return unsupported_expression(
                "aggregate_function",
                expression,
                DiagnosticArea::Function,
                diagnostics,
            );
        }
    };

    let empty_aliases = BTreeMap::new();
    let filter = function.filter.as_ref().map(|filter| {
        analyze_predicate_with_windows(filter, named_windows, &empty_aliases, &[], diagnostics)
    });

    Expression::AggregateFunction(AggregateFunctionExpression::new(
        function.name.to_string(),
        arguments,
        distinct,
        filter,
    ))
}

fn is_aggregate_function(function: &Function) -> bool {
    let name = function.name.to_string();
    let unqualified = name.rsplit('.').next().unwrap_or(name.as_str());
    let normalized = unqualified
        .trim_matches(|character| matches!(character, '"' | '`' | '[' | ']'))
        .to_ascii_uppercase();

    matches!(
        normalized.as_str(),
        "ANY_VALUE"
            | "ARRAY_AGG"
            | "AVG"
            | "BIT_AND"
            | "BIT_OR"
            | "BIT_XOR"
            | "BOOL_AND"
            | "BOOL_OR"
            | "CORR"
            | "COUNT"
            | "COVAR_POP"
            | "COVAR_SAMP"
            | "EVERY"
            | "MAX"
            | "MIN"
            | "STDDEV"
            | "STDDEV_POP"
            | "STDDEV_SAMP"
            | "STRING_AGG"
            | "SUM"
            | "VAR_POP"
            | "VAR_SAMP"
            | "VARIANCE"
    )
}

fn analyze_window_type(
    window: &WindowType,
    named_windows: &[NamedWindowDefinition],
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<WindowSpecification> {
    match window {
        WindowType::WindowSpec(spec) => {
            analyze_window_spec(spec, named_windows, &mut BTreeSet::new(), diagnostics)
        }
        WindowType::NamedWindow(name) => {
            let resolved = resolve_named_window(
                &name.value,
                named_windows,
                &mut BTreeSet::new(),
                diagnostics,
            )?;
            Some(WindowSpecification::new(
                Some(name.value.clone()),
                resolved.partition_by().to_vec(),
                resolved.order_by().to_vec(),
                resolved.frame().cloned(),
            ))
        }
    }
}

fn resolve_named_window(
    name: &str,
    named_windows: &[NamedWindowDefinition],
    resolving: &mut BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<WindowSpecification> {
    let matches = named_windows
        .iter()
        .filter(|definition| definition.0.value == name)
        .collect::<Vec<_>>();

    let definition = match matches.as_slice() {
        [definition] => *definition,
        [] => {
            diagnostics.push(warning(
                "unresolved_named_window",
                DiagnosticArea::Function,
                &format!("named window {name} is not defined in the local query scope"),
            ));
            return None;
        }
        _ => {
            diagnostics.push(warning(
                "ambiguous_named_window",
                DiagnosticArea::Function,
                &format!("named window {name} is defined more than once in the local query scope"),
            ));
            return None;
        }
    };

    if !resolving.insert(name.to_string()) {
        diagnostics.push(warning(
            "cyclic_named_window",
            DiagnosticArea::Function,
            &format!("named window {name} participates in a reference cycle"),
        ));
        return None;
    }

    let resolved = match &definition.1 {
        NamedWindowExpr::NamedWindow(base) => {
            resolve_named_window(&base.value, named_windows, resolving, diagnostics)
        }
        NamedWindowExpr::WindowSpec(spec) => {
            analyze_window_spec(spec, named_windows, resolving, diagnostics)
        }
    };
    resolving.remove(name);

    resolved.map(|resolved| {
        WindowSpecification::new(
            Some(name.to_string()),
            resolved.partition_by().to_vec(),
            resolved.order_by().to_vec(),
            resolved.frame().cloned(),
        )
    })
}

fn analyze_window_spec(
    spec: &SqlWindowSpec,
    named_windows: &[NamedWindowDefinition],
    resolving: &mut BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<WindowSpecification> {
    let mut name = None;
    let mut partition_by = Vec::new();
    let mut order_by = Vec::new();
    let mut frame = None;

    if let Some(base_name) = &spec.window_name {
        let base = resolve_named_window(&base_name.value, named_windows, resolving, diagnostics)?;
        name = Some(base_name.value.clone());
        partition_by = base.partition_by().to_vec();
        order_by = base.order_by().to_vec();
        frame = base.frame().cloned();
    }

    if !spec.partition_by.is_empty() {
        if !partition_by.is_empty() {
            diagnostics.push(warning(
                "unsupported_window_override",
                DiagnosticArea::Function,
                "window PARTITION BY cannot safely override an inherited PARTITION BY clause",
            ));
            return None;
        }
        partition_by = spec
            .partition_by
            .iter()
            .map(|expression| {
                analyze_expression_with_windows(expression, named_windows, diagnostics)
            })
            .collect();
    }

    if !spec.order_by.is_empty() {
        if !order_by.is_empty() {
            diagnostics.push(warning(
                "unsupported_window_override",
                DiagnosticArea::Function,
                "window ORDER BY cannot safely override an inherited ORDER BY clause",
            ));
            return None;
        }
        order_by = spec
            .order_by
            .iter()
            .map(|order| {
                if order.with_fill.is_some() {
                    diagnostics.push(warning(
                        "unsupported_window_order_option",
                        DiagnosticArea::Function,
                        "window ORDER BY WITH FILL semantics are not represented",
                    ));
                }
                WindowOrderExpression::new(
                    analyze_expression_with_windows(&order.expr, named_windows, diagnostics),
                    order.options.asc,
                    order.options.nulls_first,
                )
            })
            .collect();
    }

    if let Some(window_frame) = &spec.window_frame {
        if frame.is_some() {
            diagnostics.push(warning(
                "unsupported_window_override",
                DiagnosticArea::Function,
                "window frame cannot safely override an inherited frame",
            ));
            return None;
        }
        frame = Some(analyze_window_frame(
            window_frame,
            named_windows,
            diagnostics,
        ));
    }

    Some(WindowSpecification::new(
        name,
        partition_by,
        order_by,
        frame,
    ))
}

fn analyze_window_frame(
    frame: &SqlWindowFrame,
    named_windows: &[NamedWindowDefinition],
    diagnostics: &mut Vec<Diagnostic>,
) -> WindowFrame {
    WindowFrame::new(
        match frame.units {
            SqlWindowFrameUnits::Rows => WindowFrameUnits::Rows,
            SqlWindowFrameUnits::Range => WindowFrameUnits::Range,
            SqlWindowFrameUnits::Groups => WindowFrameUnits::Groups,
        },
        analyze_window_frame_bound(&frame.start_bound, named_windows, diagnostics),
        frame
            .end_bound
            .as_ref()
            .map_or(WindowFrameBound::CurrentRow, |bound| {
                analyze_window_frame_bound(bound, named_windows, diagnostics)
            }),
    )
}

fn analyze_window_frame_bound(
    bound: &SqlWindowFrameBound,
    named_windows: &[NamedWindowDefinition],
    diagnostics: &mut Vec<Diagnostic>,
) -> WindowFrameBound {
    match bound {
        SqlWindowFrameBound::CurrentRow => WindowFrameBound::CurrentRow,
        SqlWindowFrameBound::Preceding(None) => WindowFrameBound::UnboundedPreceding,
        SqlWindowFrameBound::Following(None) => WindowFrameBound::UnboundedFollowing,
        SqlWindowFrameBound::Preceding(Some(expression)) => WindowFrameBound::Preceding(Box::new(
            analyze_expression_with_windows(expression, named_windows, diagnostics),
        )),
        SqlWindowFrameBound::Following(Some(expression)) => WindowFrameBound::Following(Box::new(
            analyze_expression_with_windows(expression, named_windows, diagnostics),
        )),
    }
}

fn analyze_unary_expression(
    operator: &SqlUnaryOperator,
    operand: &Expr,
    expression: &Expr,
    named_windows: &[NamedWindowDefinition],
    diagnostics: &mut Vec<Diagnostic>,
) -> Expression {
    let operator = match operator {
        SqlUnaryOperator::Plus => Some(UnaryOperator::Plus),
        SqlUnaryOperator::Minus => Some(UnaryOperator::Minus),
        SqlUnaryOperator::PGBitwiseNot => Some(UnaryOperator::BitwiseNot),
        _ => None,
    };

    match operator {
        Some(operator) => Expression::Unary(UnaryExpression::new(
            operator,
            analyze_expression_with_windows(operand, named_windows, diagnostics),
        )),
        None => unsupported_expression(
            "unary_expression",
            expression,
            DiagnosticArea::Expression,
            diagnostics,
        ),
    }
}

fn analyze_binary_expression(
    left: &Expr,
    operator: &SqlBinaryOperator,
    right: &Expr,
    expression: &Expr,
    named_windows: &[NamedWindowDefinition],
    diagnostics: &mut Vec<Diagnostic>,
) -> Expression {
    let operator = match operator {
        SqlBinaryOperator::Plus => Some(BinaryOperator::Add),
        SqlBinaryOperator::Minus => Some(BinaryOperator::Subtract),
        SqlBinaryOperator::Multiply => Some(BinaryOperator::Multiply),
        SqlBinaryOperator::Divide => Some(BinaryOperator::Divide),
        SqlBinaryOperator::Modulo => Some(BinaryOperator::Modulo),
        SqlBinaryOperator::StringConcat => Some(BinaryOperator::StringConcat),
        SqlBinaryOperator::BitwiseAnd => Some(BinaryOperator::BitwiseAnd),
        SqlBinaryOperator::BitwiseOr => Some(BinaryOperator::BitwiseOr),
        SqlBinaryOperator::BitwiseXor | SqlBinaryOperator::PGBitwiseXor => {
            Some(BinaryOperator::BitwiseXor)
        }
        _ => None,
    };

    match operator {
        Some(operator) => Expression::Binary(BinaryExpression::new(
            operator,
            analyze_expression_with_windows(left, named_windows, diagnostics),
            analyze_expression_with_windows(right, named_windows, diagnostics),
        )),
        None => unsupported_expression(
            "binary_expression",
            expression,
            DiagnosticArea::Expression,
            diagnostics,
        ),
    }
}

fn unsupported_expression(
    feature: &str,
    expression: &Expr,
    area: DiagnosticArea,
    diagnostics: &mut Vec<Diagnostic>,
) -> Expression {
    let code = if area == DiagnosticArea::Function {
        "unsupported_function"
    } else {
        "unsupported_expression"
    };
    let reason = format!("{feature} {expression} is parsed but its semantics are not implemented");

    diagnostics.push(warning(code, area, &reason));

    Expression::Unsupported(UnsupportedSemantic::new(feature.to_string(), Some(reason)))
}

type LocalOutputMap = BTreeMap<String, Output>;

#[derive(Clone)]
struct OutputRelation {
    qualifiers: Vec<String>,
    source: OutputRelationSource,
}

#[derive(Clone)]
enum OutputRelationSource {
    Physical {
        relation: String,
        columns: Option<Vec<String>>,
    },
    Local(Output),
}

fn analyze_query_output(
    query: &SqlQuery,
    inherited_local_outputs: &LocalOutputMap,
    diagnostics: &mut Vec<Diagnostic>,
    metadata: Option<&AnalysisMetadata<'_>>,
) -> Output {
    analyze_query_output_with_outer_scope(
        query,
        inherited_local_outputs,
        &[],
        diagnostics,
        metadata,
    )
}

fn analyze_query_output_with_outer_scope(
    query: &SqlQuery,
    inherited_local_outputs: &LocalOutputMap,
    outer_scope: &[OutputRelation],
    diagnostics: &mut Vec<Diagnostic>,
    metadata: Option<&AnalysisMetadata<'_>>,
) -> Output {
    let mut local_outputs = inherited_local_outputs.clone();

    if let Some(with) = &query.with {
        for cte in &with.cte_tables {
            let output = analyze_query_output_with_outer_scope(
                &cte.query,
                &local_outputs,
                &[],
                diagnostics,
                metadata,
            );
            local_outputs.insert(cte.alias.name.to_string(), output);
        }
    }

    analyze_set_expr_output_with_outer_scope(
        query.body.as_ref(),
        &local_outputs,
        outer_scope,
        diagnostics,
        metadata,
    )
}

fn analyze_set_expr_output_with_outer_scope(
    expression: &SetExpr,
    local_outputs: &LocalOutputMap,
    outer_scope: &[OutputRelation],
    diagnostics: &mut Vec<Diagnostic>,
    metadata: Option<&AnalysisMetadata<'_>>,
) -> Output {
    match expression {
        SetExpr::Select(select) => analyze_select_output_with_outer_scope(
            select,
            local_outputs,
            outer_scope,
            diagnostics,
            metadata,
        ),
        SetExpr::Query(query) => analyze_query_output_with_outer_scope(
            query,
            local_outputs,
            outer_scope,
            diagnostics,
            metadata,
        ),
        SetExpr::SetOperation {
            left,
            set_quantifier,
            right,
            ..
        } => {
            let quantifier = analyze_set_quantifier(*set_quantifier);
            if quantifier.uses_name_alignment() {
                diagnostics.push(warning(
                    "unsupported_set_operation_alignment",
                    DiagnosticArea::Output,
                    "BY NAME set-operation alignment is represented but output-column composition is not implemented",
                ));
                return Output::new(Vec::new());
            }

            let left_output = analyze_set_expr_output_with_outer_scope(
                left,
                local_outputs,
                outer_scope,
                diagnostics,
                metadata,
            );
            let right_output = analyze_set_expr_output_with_outer_scope(
                right,
                local_outputs,
                outer_scope,
                diagnostics,
                metadata,
            );
            merge_set_operation_output(left_output, right_output, diagnostics)
        }
        SetExpr::Values(_)
        | SetExpr::Insert(_)
        | SetExpr::Update(_)
        | SetExpr::Delete(_)
        | SetExpr::Table(_) => Output::new(Vec::new()),
    }
}

fn merge_set_operation_output(
    left: Output,
    right: Output,
    diagnostics: &mut Vec<Diagnostic>,
) -> Output {
    if left.columns().is_empty() || right.columns().is_empty() {
        diagnostics.push(warning(
            "unresolved_set_operation_output",
            DiagnosticArea::Output,
            "set-operation output cannot be resolved because at least one branch has no resolved output columns",
        ));
        return Output::new(Vec::new());
    }

    if left.columns().len() != right.columns().len() {
        diagnostics.push(warning(
            "set_operation_arity_mismatch",
            DiagnosticArea::Output,
            &format!(
                "set-operation branches expose different column counts: left has {}, right has {}",
                left.columns().len(),
                right.columns().len()
            ),
        ));
        return Output::new(Vec::new());
    }

    let columns = left
        .columns()
        .iter()
        .zip(right.columns())
        .map(|(left_column, right_column)| {
            let mut lineage = left_column.lineage().to_vec();
            lineage.extend_from_slice(right_column.lineage());

            OutputColumn::new(
                left_column.name().to_string(),
                Expression::Unknown(UnknownSemantic::new(
                    "set-operation output value is determined positionally by multiple query branches"
                        .to_string(),
                )),
                union_domains(left_column.domain(), right_column.domain()),
                lineage,
            )
        })
        .collect();

    Output::new(columns)
}

fn analyze_select_output_with_outer_scope(
    select: &Select,
    local_outputs: &LocalOutputMap,
    outer_scope: &[OutputRelation],
    diagnostics: &mut Vec<Diagnostic>,
    metadata: Option<&AnalysisMetadata<'_>>,
) -> Output {
    let scope = build_output_scope(select, local_outputs, outer_scope, diagnostics, metadata);
    let mut columns = Vec::new();
    for item in &select.projection {
        columns.extend(analyze_output_item(
            item,
            &scope,
            &select.named_window,
            diagnostics,
        ));
    }

    if let Some(qualify) = &select.qualify {
        refine_output_domains_from_predicate(&mut columns, qualify);
    }

    Output::new(columns)
}

fn analyze_output_item(
    item: &SelectItem,
    scope: &[OutputRelation],
    named_windows: &[NamedWindowDefinition],
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<OutputColumn> {
    match item {
        SelectItem::UnnamedExpr(expression) => {
            let normalized = inherit_local_case_expression(
                analyze_expression_with_scope(expression, scope, named_windows, diagnostics),
                scope,
            );
            let domain = derive_output_domain(&normalized);
            vec![OutputColumn::new(
                output_name_for_expression(expression),
                normalized,
                domain,
                lineage_for_expression(expression, scope, named_windows, diagnostics),
            )]
        }
        SelectItem::ExprWithAlias { expr, alias } => {
            let normalized = inherit_local_case_expression(
                analyze_expression_with_scope(expr, scope, named_windows, diagnostics),
                scope,
            );
            let domain = derive_output_domain(&normalized);
            vec![OutputColumn::new(
                alias.value.clone(),
                normalized,
                domain,
                lineage_for_expression(expr, scope, named_windows, diagnostics),
            )]
        }
        SelectItem::Wildcard(_) => expand_wildcard(None, scope, diagnostics),
        SelectItem::QualifiedWildcard(prefix, _) => {
            let qualifier = prefix.to_string();
            expand_wildcard(Some(&qualifier), scope, diagnostics)
        }
    }
}

fn inherit_local_case_expression(expression: Expression, scope: &[OutputRelation]) -> Expression {
    let Expression::Column(column) = &expression else {
        return expression;
    };

    let candidates = scope
        .iter()
        .filter(|relation| {
            column.relation().is_none_or(|qualifier| {
                relation
                    .qualifiers
                    .iter()
                    .any(|candidate| candidate == qualifier)
            })
        })
        .filter_map(|relation| match &relation.source {
            OutputRelationSource::Physical {
                columns: Some(columns),
                ..
            } if !columns.iter().any(|candidate| candidate == column.name()) => None,
            OutputRelationSource::Physical { .. } => Some(None),
            OutputRelationSource::Local(output) => {
                let mut matches = output
                    .columns()
                    .iter()
                    .filter(|candidate| candidate.name() == column.name());
                let candidate = matches.next()?;
                if matches.next().is_some() {
                    return Some(None);
                }

                Some(
                    matches!(candidate.expression(), Expression::Case(_))
                        .then(|| candidate.expression().clone()),
                )
            }
        })
        .collect::<Vec<_>>();

    match candidates.as_slice() {
        [Some(case_expression)] => case_expression.clone(),
        _ => expression,
    }
}

fn expand_wildcard(
    qualifier: Option<&str>,
    scope: &[OutputRelation],
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<OutputColumn> {
    let relations = scope
        .iter()
        .filter(|relation| {
            qualifier.is_none_or(|qualifier| {
                relation
                    .qualifiers
                    .iter()
                    .any(|candidate| candidate == qualifier)
            })
        })
        .collect::<Vec<_>>();

    if relations.is_empty() || (qualifier.is_some() && relations.len() != 1) {
        return vec![unresolved_wildcard_column(
            qualifier.map_or_else(|| "*".to_string(), |value| format!("{value}.*")),
            diagnostics,
        )];
    }

    let mut columns = Vec::new();
    for relation in relations {
        let expression_qualifier = relation.qualifiers.first().cloned();
        match &relation.source {
            OutputRelationSource::Physical {
                relation,
                columns: Some(schema_columns),
            } => {
                columns.extend(schema_columns.iter().map(|column| {
                    OutputColumn::new(
                        column.clone(),
                        Expression::Column(ColumnExpression::new(
                            expression_qualifier.clone(),
                            column.clone(),
                        )),
                        ValueDomain::Unbounded,
                        vec![LineageSource::new(relation.clone(), column.clone())],
                    )
                }));
            }
            OutputRelationSource::Physical { columns: None, .. } => {
                return vec![unresolved_wildcard_column(
                    qualifier.map_or_else(|| "*".to_string(), |value| format!("{value}.*")),
                    diagnostics,
                )];
            }
            OutputRelationSource::Local(output) => {
                columns.extend(output.columns().iter().map(|column| {
                    let expression = match column.expression() {
                        Expression::Case(_) => column.expression().clone(),
                        _ => Expression::Column(ColumnExpression::new(
                            expression_qualifier.clone(),
                            column.name().to_string(),
                        )),
                    };
                    OutputColumn::new(
                        column.name().to_string(),
                        expression,
                        column.domain().clone(),
                        column.lineage().to_vec(),
                    )
                }));
            }
        }
    }

    if columns.is_empty() {
        vec![unresolved_wildcard_column(
            qualifier.map_or_else(|| "*".to_string(), |value| format!("{value}.*")),
            diagnostics,
        )]
    } else {
        columns
    }
}

fn unresolved_wildcard_column(name: String, diagnostics: &mut Vec<Diagnostic>) -> OutputColumn {
    let reason = "wildcard output cannot be resolved without source schema information";
    diagnostics.push(warning(
        "unresolved_wildcard",
        DiagnosticArea::Output,
        reason,
    ));
    OutputColumn::new(
        name,
        Expression::Unknown(UnknownSemantic::new(reason.to_string())),
        ValueDomain::unknown(reason),
        Vec::new(),
    )
}

fn refine_output_domains_from_column_domains(
    output: Output,
    column_domains: &[ColumnDomain],
    sources: &[SourceRelation],
) -> Output {
    Output::new(
        output
            .columns()
            .iter()
            .cloned()
            .map(|column| {
                let derived = derive_expression_domain_with_column_domains(
                    column.expression(),
                    column_domains,
                    sources,
                );
                let lineage_domain =
                    column
                        .plain_copy_source()
                        .map_or(ValueDomain::Unbounded, |source| {
                            column_domains
                                .iter()
                                .find(|candidate| {
                                    candidate.column().relation() == Some(source.relation())
                                        && candidate.column().name() == source.column()
                                })
                                .map_or(ValueDomain::Unbounded, |candidate| {
                                    candidate.domain().clone()
                                })
                        });
                let domain = intersect_domains(column.domain(), &derived);
                let domain = intersect_domains(&domain, &lineage_domain);
                column.with_domain(domain)
            })
            .collect(),
    )
}

fn derive_expression_domain_with_column_domains(
    expression: &Expression,
    column_domains: &[ColumnDomain],
    sources: &[SourceRelation],
) -> ValueDomain {
    if let Expression::Column(column) = expression {
        let resolved = resolve_column(column, sources);
        if let Some(domain) = column_domains
            .iter()
            .find(|candidate| candidate.column() == &resolved)
        {
            return domain.domain().clone();
        }
    }

    derive_output_domain(expression)
}

fn derive_output_domain(expression: &Expression) -> ValueDomain {
    match expression {
        Expression::Column(_) => ValueDomain::Unbounded,
        Expression::Literal(literal) => ValueDomain::set(SetMode::Include, vec![literal.clone()]),
        Expression::BooleanPredicate(_) => boolean_domain(),
        Expression::WindowFunction(window)
            if window.function().name().eq_ignore_ascii_case("row_number") =>
        {
            integer_lower_bound_domain(1)
        }
        Expression::WindowFunction(_) => {
            ValueDomain::unknown("window-function output domain is not known safely")
        }
        Expression::AggregateFunction(function)
            if function.name().eq_ignore_ascii_case("count") =>
        {
            integer_lower_bound_domain(0)
        }
        Expression::AggregateFunction(_) => {
            ValueDomain::unknown("aggregate output domain is not known safely")
        }
        Expression::Case(case_expression) => {
            let mut domain = ValueDomain::Empty;
            for branch in case_expression.branches() {
                domain = union_domains(&domain, &derive_output_domain(branch.result()));
            }
            let else_domain = case_expression.else_result().map_or_else(
                || {
                    ValueDomain::set(
                        SetMode::Include,
                        vec![LiteralExpression::new(
                            LiteralType::Null,
                            LiteralValue::Null,
                        )],
                    )
                },
                derive_output_domain,
            );
            union_domains(&domain, &else_domain)
        }
        Expression::Unary(unary) => derive_unary_output_domain(unary),
        Expression::Binary(binary) => derive_binary_output_domain(binary),
        Expression::ScalarSubquery(subquery) => {
            let columns = subquery.subquery().output().columns();
            match columns {
                [column] => column.domain().clone(),
                _ => ValueDomain::unknown(
                    "scalar-subquery output domain requires exactly one resolved column",
                ),
            }
        }
        Expression::Function(_) => {
            ValueDomain::unknown("scalar-function output domain is not known safely")
        }
        Expression::Unknown(semantic) => ValueDomain::unknown(semantic.reason()),
        Expression::Unsupported(_) => {
            ValueDomain::unknown("unsupported expression has no safely known output domain")
        }
    }
}

fn boolean_domain() -> ValueDomain {
    ValueDomain::set(
        SetMode::Include,
        vec![
            LiteralExpression::new(LiteralType::Boolean, LiteralValue::Boolean(false)),
            LiteralExpression::new(LiteralType::Boolean, LiteralValue::Boolean(true)),
        ],
    )
}

fn integer_lower_bound_domain(value: i128) -> ValueDomain {
    ValueDomain::ranges(vec![ValueRange::new(
        Some(Bound::new(integer_literal(value), true)),
        None,
    )])
}

fn integer_literal(value: i128) -> LiteralExpression {
    LiteralExpression::new(
        LiteralType::Integer,
        LiteralValue::Number(value.to_string()),
    )
}

fn singleton_integer(domain: &ValueDomain) -> Option<i128> {
    let ValueDomain::Set(set) = domain else {
        return None;
    };
    if set.mode() != SetMode::Include {
        return None;
    }
    let [literal] = set.values() else {
        return None;
    };
    if literal.literal_type() != LiteralType::Integer {
        return None;
    }
    let LiteralValue::Number(value) = literal.value() else {
        return None;
    };
    value.parse().ok()
}

fn derive_unary_output_domain(unary: &UnaryExpression) -> ValueDomain {
    let operand = derive_output_domain(unary.operand());
    match unary.operator() {
        UnaryOperator::Plus => operand,
        UnaryOperator::Minus => singleton_integer(&operand)
            .and_then(i128::checked_neg)
            .map_or_else(
                || ValueDomain::unknown("unary minus cannot be bounded safely"),
                |value| ValueDomain::set(SetMode::Include, vec![integer_literal(value)]),
            ),
        UnaryOperator::BitwiseNot => {
            ValueDomain::unknown("bitwise output domain is not bounded safely")
        }
    }
}

fn derive_binary_output_domain(binary: &BinaryExpression) -> ValueDomain {
    let left = singleton_integer(&derive_output_domain(binary.left()));
    let right = singleton_integer(&derive_output_domain(binary.right()));
    let (Some(left), Some(right)) = (left, right) else {
        return ValueDomain::unknown("arithmetic output domain requires safely bounded operands");
    };

    let value = match binary.operator() {
        BinaryOperator::Add => left.checked_add(right),
        BinaryOperator::Subtract => left.checked_sub(right),
        BinaryOperator::Multiply => left.checked_mul(right),
        BinaryOperator::Divide => {
            if right == 0 {
                None
            } else {
                left.checked_div(right)
            }
        }
        BinaryOperator::Modulo => {
            if right == 0 {
                None
            } else {
                left.checked_rem(right)
            }
        }
        BinaryOperator::StringConcat
        | BinaryOperator::BitwiseAnd
        | BinaryOperator::BitwiseOr
        | BinaryOperator::BitwiseXor => None,
    };

    value.map_or_else(
        || ValueDomain::unknown("arithmetic output domain cannot be computed safely"),
        |value| ValueDomain::set(SetMode::Include, vec![integer_literal(value)]),
    )
}

fn refine_output_domains_from_predicate(columns: &mut [OutputColumn], predicate: &Expr) {
    for column in columns {
        if let Some(refinement) = output_alias_domain(predicate, column.name()) {
            let domain = intersect_domains(column.domain(), &refinement);
            *column = column.clone().with_domain(domain);
        }
    }
}

fn output_alias_domain(expression: &Expr, alias: &str) -> Option<ValueDomain> {
    match expression {
        Expr::Nested(inner) => output_alias_domain(inner, alias),
        Expr::BinaryOp {
            left,
            op: SqlBinaryOperator::And,
            right,
        } => match (
            output_alias_domain(left, alias),
            output_alias_domain(right, alias),
        ) {
            (Some(left), Some(right)) => Some(intersect_domains(&left, &right)),
            (Some(domain), None) | (None, Some(domain)) => Some(domain),
            (None, None) => None,
        },
        Expr::BinaryOp {
            left,
            op: SqlBinaryOperator::Or,
            right,
        } => match (
            output_alias_domain(left, alias),
            output_alias_domain(right, alias),
        ) {
            (Some(left), Some(right)) => Some(union_domains(&left, &right)),
            _ => None,
        },
        Expr::BinaryOp { left, op, right } => {
            let operator = comparison_operator(op)?;
            if expression_is_alias(left, alias) {
                literal_domain_for_comparison(right, operator)
            } else if expression_is_alias(right, alias) {
                literal_domain_for_comparison(left, operator.reversed())
            } else {
                None
            }
        }
        Expr::Between {
            expr,
            negated: false,
            low,
            high,
        } if expression_is_alias(expr, alias) => {
            let low = literal_expression(low)?;
            let high = literal_expression(high)?;
            Some(ValueDomain::ranges(vec![ValueRange::new(
                Some(Bound::new(low, true)),
                Some(Bound::new(high, true)),
            )]))
        }
        Expr::InList {
            expr,
            list,
            negated,
        } if expression_is_alias(expr, alias) => {
            let values = list
                .iter()
                .map(literal_expression)
                .collect::<Option<Vec<_>>>()?;
            Some(ValueDomain::set(
                if *negated {
                    SetMode::Exclude
                } else {
                    SetMode::Include
                },
                values,
            ))
        }
        _ => None,
    }
}

fn expression_is_alias(expression: &Expr, alias: &str) -> bool {
    matches!(expression, Expr::Identifier(identifier) if identifier.value == alias)
}

fn literal_expression(expression: &Expr) -> Option<LiteralExpression> {
    let mut diagnostics = Vec::new();
    match analyze_expression_with_windows(expression, &[], &mut diagnostics) {
        Expression::Literal(literal) => Some(literal),
        _ => None,
    }
}

fn literal_domain_for_comparison(
    expression: &Expr,
    operator: ComparisonOperator,
) -> Option<ValueDomain> {
    let literal = literal_expression(expression)?;
    match operator {
        ComparisonOperator::Eq => Some(ValueDomain::set(SetMode::Include, vec![literal])),
        ComparisonOperator::Neq => Some(ValueDomain::set(SetMode::Exclude, vec![literal])),
        ComparisonOperator::Gt => Some(ValueDomain::ranges(vec![ValueRange::new(
            Some(Bound::new(literal, false)),
            None,
        )])),
        ComparisonOperator::Gte => Some(ValueDomain::ranges(vec![ValueRange::new(
            Some(Bound::new(literal, true)),
            None,
        )])),
        ComparisonOperator::Lt => Some(ValueDomain::ranges(vec![ValueRange::new(
            None,
            Some(Bound::new(literal, false)),
        )])),
        ComparisonOperator::Lte => Some(ValueDomain::ranges(vec![ValueRange::new(
            None,
            Some(Bound::new(literal, true)),
        )])),
        ComparisonOperator::IsDistinctFrom | ComparisonOperator::IsNotDistinctFrom => None,
    }
}

fn analyze_subquery_semantics(
    query: &SqlQuery,
    outer_scope: &[OutputRelation],
) -> SubquerySemantics {
    let mut diagnostics = Vec::new();
    let mut derived_index = 0;
    let relations = analyze_query_relations(
        query,
        &BTreeSet::new(),
        &mut diagnostics,
        &mut derived_index,
    );
    let output = analyze_query_output_with_outer_scope(
        query,
        &BTreeMap::new(),
        outer_scope,
        &mut diagnostics,
        None,
    );
    let predicates =
        analyze_query_predicates_with_outer_scope(query, outer_scope, &mut diagnostics);
    let column_domains = analyze_query_column_domains(
        query,
        &BTreeSet::new(),
        &BTreeMap::new(),
        &BTreeMap::new(),
        None,
    );
    let correlations = collect_query_correlations(query, outer_scope);

    inspect_query_features(query, &mut diagnostics);
    let condition_exactness = analyze_query_condition_exactness(
        query,
        &predicates,
        &relations.sources,
        &relations.joins,
        &diagnostics,
        !correlations.is_empty(),
    );
    sort_diagnostics(&mut diagnostics);

    SubquerySemantics::new(
        relations.dependencies.into_iter().collect(),
        correlations,
        relations.joins,
        output,
        predicates,
        column_domains,
        condition_exactness,
        diagnostics,
    )
}

fn collect_query_correlations(
    query: &SqlQuery,
    outer_scope: &[OutputRelation],
) -> Vec<LineageSource> {
    let mut correlations = BTreeSet::new();
    collect_set_expr_correlations(query.body.as_ref(), outer_scope, &mut correlations);
    correlations
        .into_iter()
        .map(|(relation, column)| LineageSource::new(relation, column))
        .collect()
}

fn collect_set_expr_correlations(
    expression: &SetExpr,
    outer_scope: &[OutputRelation],
    correlations: &mut BTreeSet<(String, String)>,
) {
    match expression {
        SetExpr::Select(select) => {
            collect_select_correlations(select, outer_scope, correlations);
        }
        SetExpr::Query(query) => {
            collect_set_expr_correlations(query.body.as_ref(), outer_scope, correlations);
        }
        SetExpr::SetOperation { left, right, .. } => {
            collect_set_expr_correlations(left, outer_scope, correlations);
            collect_set_expr_correlations(right, outer_scope, correlations);
        }
        SetExpr::Values(_)
        | SetExpr::Insert(_)
        | SetExpr::Update(_)
        | SetExpr::Delete(_)
        | SetExpr::Table(_) => {}
    }
}

fn collect_select_correlations(
    select: &Select,
    outer_scope: &[OutputRelation],
    correlations: &mut BTreeSet<(String, String)>,
) {
    let local_qualifiers = select_local_qualifiers(select);

    for item in &select.projection {
        match item {
            SelectItem::UnnamedExpr(expression)
            | SelectItem::ExprWithAlias {
                expr: expression, ..
            } => collect_expression_correlations(
                expression,
                outer_scope,
                &local_qualifiers,
                correlations,
            ),
            SelectItem::QualifiedWildcard(_, _) | SelectItem::Wildcard(_) => {}
        }
    }

    for expression in [
        select.selection.as_ref(),
        select.having.as_ref(),
        select.qualify.as_ref(),
    ]
    .into_iter()
    .flatten()
    {
        collect_expression_correlations(expression, outer_scope, &local_qualifiers, correlations);
    }

    for source in &select.from {
        for join in &source.joins {
            if let (_, Some(JoinConstraint::On(expression)), _) =
                analyze_join_operator(&join.join_operator)
            {
                collect_expression_correlations(
                    expression,
                    outer_scope,
                    &local_qualifiers,
                    correlations,
                );
            }
        }
    }
}

fn select_local_qualifiers(select: &Select) -> BTreeSet<String> {
    let mut qualifiers = BTreeSet::new();
    for source in &select.from {
        collect_table_factor_qualifiers(&source.relation, &mut qualifiers);
        for join in &source.joins {
            collect_table_factor_qualifiers(&join.relation, &mut qualifiers);
        }
    }
    qualifiers
}

fn collect_table_factor_qualifiers(factor: &TableFactor, qualifiers: &mut BTreeSet<String>) {
    match factor {
        TableFactor::Table { name, alias, .. } => {
            if let Some(alias) = alias {
                qualifiers.insert(alias.name.to_string());
            } else {
                let relation = name.to_string();
                qualifiers.insert(relation.clone());
                if let Some(short) = relation.rsplit('.').next() {
                    qualifiers.insert(short.to_string());
                }
            }
        }
        TableFactor::Derived {
            alias: Some(alias), ..
        } => {
            qualifiers.insert(alias.name.to_string());
        }
        _ => {}
    }
}

fn collect_expression_correlations(
    expression: &Expr,
    outer_scope: &[OutputRelation],
    local_qualifiers: &BTreeSet<String>,
    correlations: &mut BTreeSet<(String, String)>,
) {
    match expression {
        Expr::CompoundIdentifier(identifiers) => {
            if let Some((column, relation_parts)) = identifiers.split_last() {
                let qualifier = relation_parts
                    .iter()
                    .map(|identifier| identifier.value.as_str())
                    .collect::<Vec<_>>()
                    .join(".");
                if !local_qualifiers.contains(&qualifier) {
                    collect_outer_column(&qualifier, &column.value, outer_scope, correlations);
                }
            }
        }
        Expr::BinaryOp { left, right, .. }
        | Expr::AnyOp { left, right, .. }
        | Expr::AllOp { left, right, .. }
        | Expr::IsDistinctFrom(left, right)
        | Expr::IsNotDistinctFrom(left, right) => {
            collect_expression_correlations(left, outer_scope, local_qualifiers, correlations);
            collect_expression_correlations(right, outer_scope, local_qualifiers, correlations);
        }
        Expr::UnaryOp { expr, .. }
        | Expr::Nested(expr)
        | Expr::IsNull(expr)
        | Expr::IsNotNull(expr) => {
            collect_expression_correlations(expr, outer_scope, local_qualifiers, correlations);
        }
        Expr::Between {
            expr, low, high, ..
        } => {
            for value in [expr.as_ref(), low.as_ref(), high.as_ref()] {
                collect_expression_correlations(value, outer_scope, local_qualifiers, correlations);
            }
        }
        Expr::InList { expr, list, .. } => {
            collect_expression_correlations(expr, outer_scope, local_qualifiers, correlations);
            for value in list {
                collect_expression_correlations(value, outer_scope, local_qualifiers, correlations);
            }
        }
        Expr::Case {
            operand,
            conditions,
            else_result,
            ..
        } => {
            if let Some(operand) = operand {
                collect_expression_correlations(
                    operand,
                    outer_scope,
                    local_qualifiers,
                    correlations,
                );
            }
            for branch in conditions {
                collect_expression_correlations(
                    &branch.condition,
                    outer_scope,
                    local_qualifiers,
                    correlations,
                );
                collect_expression_correlations(
                    &branch.result,
                    outer_scope,
                    local_qualifiers,
                    correlations,
                );
            }
            if let Some(else_result) = else_result {
                collect_expression_correlations(
                    else_result,
                    outer_scope,
                    local_qualifiers,
                    correlations,
                );
            }
        }
        Expr::Function(function) => {
            collect_function_argument_correlations(
                &function.parameters,
                outer_scope,
                local_qualifiers,
                correlations,
            );
            collect_function_argument_correlations(
                &function.args,
                outer_scope,
                local_qualifiers,
                correlations,
            );
            if let Some(filter) = &function.filter {
                collect_expression_correlations(
                    filter,
                    outer_scope,
                    local_qualifiers,
                    correlations,
                );
            }
        }
        Expr::Subquery(_) | Expr::Exists { .. } | Expr::InSubquery { .. } => {}
        _ => {}
    }
}

fn collect_function_argument_correlations(
    arguments: &FunctionArguments,
    outer_scope: &[OutputRelation],
    local_qualifiers: &BTreeSet<String>,
    correlations: &mut BTreeSet<(String, String)>,
) {
    if let FunctionArguments::List(arguments) = arguments {
        for argument in &arguments.args {
            if let FunctionArg::Unnamed(FunctionArgExpr::Expr(expression)) = argument {
                collect_expression_correlations(
                    expression,
                    outer_scope,
                    local_qualifiers,
                    correlations,
                );
            }
        }
    }
}

fn collect_outer_column(
    qualifier: &str,
    column: &str,
    outer_scope: &[OutputRelation],
    correlations: &mut BTreeSet<(String, String)>,
) {
    let candidates = output_column_candidates(Some(qualifier), column, outer_scope);
    if let [candidate] = candidates.as_slice() {
        correlations.extend(
            candidate
                .iter()
                .map(|source| (source.relation().to_string(), source.column().to_string())),
        );
    }
}

fn output_name_for_expression(expression: &Expr) -> String {
    match expression {
        Expr::Identifier(identifier) => identifier.value.clone(),
        Expr::CompoundIdentifier(identifiers) => identifiers.last().map_or_else(
            || expression.to_string(),
            |identifier| identifier.value.clone(),
        ),
        Expr::Nested(inner) => output_name_for_expression(inner),
        _ => expression.to_string(),
    }
}

fn build_output_scope(
    select: &Select,
    local_outputs: &LocalOutputMap,
    outer_scope: &[OutputRelation],
    diagnostics: &mut Vec<Diagnostic>,
    metadata: Option<&AnalysisMetadata<'_>>,
) -> Vec<OutputRelation> {
    let local_qualifiers = select_local_qualifiers(select);
    let mut scope = outer_scope
        .iter()
        .filter_map(|relation| {
            let qualifiers = relation
                .qualifiers
                .iter()
                .filter(|qualifier| !local_qualifiers.contains(*qualifier))
                .cloned()
                .collect::<Vec<_>>();
            (!qualifiers.is_empty()).then(|| OutputRelation {
                qualifiers,
                source: relation.source.clone(),
            })
        })
        .collect::<Vec<_>>();

    for source in &select.from {
        register_output_table_factor(
            &source.relation,
            local_outputs,
            diagnostics,
            &mut scope,
            metadata,
        );
        for join in &source.joins {
            register_output_table_factor(
                &join.relation,
                local_outputs,
                diagnostics,
                &mut scope,
                metadata,
            );
        }
    }

    scope
}

fn register_output_table_factor(
    factor: &TableFactor,
    local_outputs: &LocalOutputMap,
    diagnostics: &mut Vec<Diagnostic>,
    scope: &mut Vec<OutputRelation>,
    metadata: Option<&AnalysisMetadata<'_>>,
) {
    match factor {
        TableFactor::Table {
            name,
            alias,
            args: None,
            ..
        } => {
            let relation_name = name.to_string();
            let qualifiers =
                relation_qualifiers(&relation_name, alias.as_ref().map(|a| a.name.to_string()));
            let source = match local_outputs.get(&relation_name) {
                Some(output) => OutputRelationSource::Local(output.clone()),
                None => OutputRelationSource::Physical {
                    columns: metadata.and_then(|metadata| metadata.schema_columns(&relation_name)),
                    relation: relation_name,
                },
            };
            scope.push(OutputRelation { qualifiers, source });
        }
        TableFactor::Derived {
            lateral,
            subquery,
            alias,
            ..
        } => {
            let visible_outer_scope = if *lateral { scope.clone() } else { Vec::new() };
            let output = analyze_query_output_with_outer_scope(
                subquery,
                local_outputs,
                &visible_outer_scope,
                diagnostics,
                metadata,
            );
            let qualifiers = alias
                .as_ref()
                .map(|alias| vec![alias.name.to_string()])
                .unwrap_or_default();
            scope.push(OutputRelation {
                qualifiers,
                source: OutputRelationSource::Local(output),
            });
        }
        _ => {}
    }
}

fn relation_qualifiers(relation: &str, alias: Option<String>) -> Vec<String> {
    match alias {
        Some(alias) => vec![alias],
        None => {
            let mut qualifiers = vec![relation.to_string()];
            if let Some(last) = relation.rsplit('.').next() {
                if last != relation {
                    qualifiers.push(last.to_string());
                }
            }
            qualifiers
        }
    }
}

fn lineage_for_expression(
    expression: &Expr,
    scope: &[OutputRelation],
    named_windows: &[NamedWindowDefinition],
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<LineageSource> {
    let mut lineage = BTreeSet::new();
    collect_output_lineage(
        expression,
        scope,
        named_windows,
        &mut BTreeSet::new(),
        diagnostics,
        &mut lineage,
    );
    lineage
        .into_iter()
        .map(|(relation, column)| LineageSource::new(relation, column))
        .collect()
}

fn collect_output_lineage(
    expression: &Expr,
    scope: &[OutputRelation],
    named_windows: &[NamedWindowDefinition],
    visited_windows: &mut BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
    lineage: &mut BTreeSet<(String, String)>,
) {
    match expression {
        Expr::Identifier(identifier) => {
            resolve_output_column(None, &identifier.value, scope, diagnostics, lineage);
        }
        Expr::CompoundIdentifier(identifiers) => {
            if let Some((column, relation_parts)) = identifiers.split_last() {
                let relation = relation_parts
                    .iter()
                    .map(|identifier| identifier.value.as_str())
                    .collect::<Vec<_>>()
                    .join(".");
                resolve_output_column(
                    Some(relation.as_str()),
                    &column.value,
                    scope,
                    diagnostics,
                    lineage,
                );
            }
        }
        Expr::Subquery(query) => {
            let output = analyze_query_output_with_outer_scope(
                query,
                &BTreeMap::new(),
                scope,
                diagnostics,
                None,
            );
            for column in output.columns() {
                lineage.extend(
                    column
                        .lineage()
                        .iter()
                        .map(|source| (source.relation().to_string(), source.column().to_string())),
                );
            }
        }
        Expr::Case {
            operand,
            conditions,
            else_result,
            ..
        } => {
            if let Some(operand) = operand {
                collect_output_lineage(
                    operand,
                    scope,
                    named_windows,
                    visited_windows,
                    diagnostics,
                    lineage,
                );
            }
            for branch in conditions {
                collect_output_lineage(
                    &branch.condition,
                    scope,
                    named_windows,
                    visited_windows,
                    diagnostics,
                    lineage,
                );
                collect_output_lineage(
                    &branch.result,
                    scope,
                    named_windows,
                    visited_windows,
                    diagnostics,
                    lineage,
                );
            }
            if let Some(else_result) = else_result {
                collect_output_lineage(
                    else_result,
                    scope,
                    named_windows,
                    visited_windows,
                    diagnostics,
                    lineage,
                );
            }
        }
        Expr::Function(function) => {
            collect_function_argument_lineage(
                &function.parameters,
                scope,
                named_windows,
                visited_windows,
                diagnostics,
                lineage,
            );
            collect_function_argument_lineage(
                &function.args,
                scope,
                named_windows,
                visited_windows,
                diagnostics,
                lineage,
            );
            if let Some(filter) = &function.filter {
                collect_output_lineage(
                    filter,
                    scope,
                    named_windows,
                    visited_windows,
                    diagnostics,
                    lineage,
                );
            }
            if let Some(window) = &function.over {
                collect_window_lineage(
                    window,
                    scope,
                    named_windows,
                    visited_windows,
                    diagnostics,
                    lineage,
                );
            }
        }
        Expr::UnaryOp { expr, .. }
        | Expr::Nested(expr)
        | Expr::IsNull(expr)
        | Expr::IsNotNull(expr) => {
            collect_output_lineage(
                expr,
                scope,
                named_windows,
                visited_windows,
                diagnostics,
                lineage,
            );
        }
        Expr::BinaryOp { left, right, .. }
        | Expr::AnyOp { left, right, .. }
        | Expr::AllOp { left, right, .. }
        | Expr::IsDistinctFrom(left, right)
        | Expr::IsNotDistinctFrom(left, right) => {
            collect_output_lineage(
                left,
                scope,
                named_windows,
                visited_windows,
                diagnostics,
                lineage,
            );
            collect_output_lineage(
                right,
                scope,
                named_windows,
                visited_windows,
                diagnostics,
                lineage,
            );
        }
        Expr::Between {
            expr, low, high, ..
        } => {
            for expression in [expr.as_ref(), low.as_ref(), high.as_ref()] {
                collect_output_lineage(
                    expression,
                    scope,
                    named_windows,
                    visited_windows,
                    diagnostics,
                    lineage,
                );
            }
        }
        Expr::InList { expr, list, .. } => {
            collect_output_lineage(
                expr,
                scope,
                named_windows,
                visited_windows,
                diagnostics,
                lineage,
            );
            for value in list {
                collect_output_lineage(
                    value,
                    scope,
                    named_windows,
                    visited_windows,
                    diagnostics,
                    lineage,
                );
            }
        }
        _ => {}
    }
}

fn collect_function_argument_lineage(
    arguments: &FunctionArguments,
    scope: &[OutputRelation],
    named_windows: &[NamedWindowDefinition],
    visited_windows: &mut BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
    lineage: &mut BTreeSet<(String, String)>,
) {
    if let FunctionArguments::List(arguments) = arguments {
        for argument in &arguments.args {
            if let FunctionArg::Unnamed(FunctionArgExpr::Expr(expression)) = argument {
                collect_output_lineage(
                    expression,
                    scope,
                    named_windows,
                    visited_windows,
                    diagnostics,
                    lineage,
                );
            }
        }
    }
}

fn collect_window_lineage(
    window: &WindowType,
    scope: &[OutputRelation],
    named_windows: &[NamedWindowDefinition],
    visited_windows: &mut BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
    lineage: &mut BTreeSet<(String, String)>,
) {
    match window {
        WindowType::WindowSpec(spec) => collect_window_spec_lineage(
            spec,
            scope,
            named_windows,
            visited_windows,
            diagnostics,
            lineage,
        ),
        WindowType::NamedWindow(name) => collect_named_window_lineage(
            &name.value,
            scope,
            named_windows,
            visited_windows,
            diagnostics,
            lineage,
        ),
    }
}

fn collect_named_window_lineage(
    name: &str,
    scope: &[OutputRelation],
    named_windows: &[NamedWindowDefinition],
    visited_windows: &mut BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
    lineage: &mut BTreeSet<(String, String)>,
) {
    if !visited_windows.insert(name.to_string()) {
        return;
    }

    if let Some(NamedWindowDefinition(_, definition)) = named_windows
        .iter()
        .find(|definition| definition.0.value == name)
    {
        match definition {
            NamedWindowExpr::NamedWindow(base) => collect_named_window_lineage(
                &base.value,
                scope,
                named_windows,
                visited_windows,
                diagnostics,
                lineage,
            ),
            NamedWindowExpr::WindowSpec(spec) => collect_window_spec_lineage(
                spec,
                scope,
                named_windows,
                visited_windows,
                diagnostics,
                lineage,
            ),
        }
    }

    visited_windows.remove(name);
}

fn collect_window_spec_lineage(
    spec: &SqlWindowSpec,
    scope: &[OutputRelation],
    named_windows: &[NamedWindowDefinition],
    visited_windows: &mut BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
    lineage: &mut BTreeSet<(String, String)>,
) {
    if let Some(name) = &spec.window_name {
        collect_named_window_lineage(
            &name.value,
            scope,
            named_windows,
            visited_windows,
            diagnostics,
            lineage,
        );
    }
    for expression in &spec.partition_by {
        collect_output_lineage(
            expression,
            scope,
            named_windows,
            visited_windows,
            diagnostics,
            lineage,
        );
    }
    for order in &spec.order_by {
        collect_output_lineage(
            &order.expr,
            scope,
            named_windows,
            visited_windows,
            diagnostics,
            lineage,
        );
    }
    if let Some(frame) = &spec.window_frame {
        if let SqlWindowFrameBound::Preceding(Some(expression))
        | SqlWindowFrameBound::Following(Some(expression)) = &frame.start_bound
        {
            collect_output_lineage(
                expression,
                scope,
                named_windows,
                visited_windows,
                diagnostics,
                lineage,
            );
        }
        if let Some(
            SqlWindowFrameBound::Preceding(Some(expression))
            | SqlWindowFrameBound::Following(Some(expression)),
        ) = &frame.end_bound
        {
            collect_output_lineage(
                expression,
                scope,
                named_windows,
                visited_windows,
                diagnostics,
                lineage,
            );
        }
    }
}

fn output_column_candidates(
    qualifier: Option<&str>,
    column: &str,
    scope: &[OutputRelation],
) -> Vec<Vec<LineageSource>> {
    scope
        .iter()
        .filter(|relation| {
            qualifier.is_none_or(|qualifier| {
                relation
                    .qualifiers
                    .iter()
                    .any(|candidate| candidate == qualifier)
            })
        })
        .filter_map(|relation| match &relation.source {
            OutputRelationSource::Physical {
                relation,
                columns: Some(columns),
            } if !columns.iter().any(|candidate| candidate == column) => None,
            OutputRelationSource::Physical { relation, .. } => Some(vec![LineageSource::new(
                relation.clone(),
                column.to_string(),
            )]),
            OutputRelationSource::Local(output) => output
                .columns()
                .iter()
                .find(|candidate| candidate.name() == column)
                .map(|candidate| candidate.lineage().to_vec()),
        })
        .collect()
}

fn resolve_output_column(
    qualifier: Option<&str>,
    column: &str,
    scope: &[OutputRelation],
    diagnostics: &mut Vec<Diagnostic>,
    lineage: &mut BTreeSet<(String, String)>,
) {
    let candidates = output_column_candidates(qualifier, column, scope);

    match candidates.as_slice() {
        [candidate] => {
            lineage.extend(
                candidate
                    .iter()
                    .map(|source| (source.relation().to_string(), source.column().to_string())),
            );
        }
        [] => diagnostics.push(warning(
            "unresolved_output_lineage",
            DiagnosticArea::Output,
            &format!(
                "source lineage for column {} could not be resolved",
                qualified_column_name(qualifier, column)
            ),
        )),
        _ => diagnostics.push(warning(
            "ambiguous_output_lineage",
            DiagnosticArea::Output,
            &format!(
                "source lineage for column {} is ambiguous without source schema information",
                qualified_column_name(qualifier, column)
            ),
        )),
    }
}

fn qualified_column_name(qualifier: Option<&str>, column: &str) -> String {
    qualifier.map_or_else(
        || column.to_string(),
        |qualifier| format!("{qualifier}.{column}"),
    )
}

fn inspect_select_features(select: &Select, diagnostics: &mut Vec<Diagnostic>) {
    if select.top.is_some() {
        diagnostics.push(warning(
            "unsupported_top",
            DiagnosticArea::Other,
            "TOP semantics are not implemented yet",
        ));
    }
    if select.exclude.is_some() {
        diagnostics.push(warning(
            "unsupported_exclude",
            DiagnosticArea::Output,
            "SELECT EXCLUDE semantics are not implemented yet",
        ));
    }
    if select.into.is_some() {
        diagnostics.push(warning(
            "unsupported_select_into",
            DiagnosticArea::Other,
            "SELECT INTO semantics are not implemented yet",
        ));
    }
    if !select.lateral_views.is_empty() {
        diagnostics.push(warning(
            "unsupported_lateral_view",
            DiagnosticArea::Source,
            "LATERAL VIEW semantics are not implemented yet",
        ));
    }
    if select.prewhere.is_some() {
        diagnostics.push(warning(
            "unsupported_prewhere",
            DiagnosticArea::Predicate,
            "PREWHERE semantics are not represented by protocol v0",
        ));
    }
    if !select.cluster_by.is_empty() {
        diagnostics.push(warning(
            "unsupported_cluster_by",
            DiagnosticArea::Other,
            "CLUSTER BY semantics are not implemented yet",
        ));
    }
    if !select.distribute_by.is_empty() {
        diagnostics.push(warning(
            "unsupported_distribute_by",
            DiagnosticArea::Other,
            "DISTRIBUTE BY semantics are not implemented yet",
        ));
    }
    if !select.sort_by.is_empty() {
        diagnostics.push(warning(
            "unsupported_sort_by",
            DiagnosticArea::Other,
            "SORT BY semantics are not implemented yet",
        ));
    }
    if select.value_table_mode.is_some() {
        diagnostics.push(warning(
            "unsupported_value_table_mode",
            DiagnosticArea::Output,
            "value-table output semantics are not implemented yet",
        ));
    }
    if select.connect_by.is_some() {
        diagnostics.push(warning(
            "unsupported_connect_by",
            DiagnosticArea::Other,
            "CONNECT BY semantics are not implemented yet",
        ));
    }
}

fn inspect_set_expr_features(expression: &SetExpr, diagnostics: &mut Vec<Diagnostic>) {
    match expression {
        SetExpr::Select(select) => inspect_select_features(select, diagnostics),
        SetExpr::Query(query) => {
            inspect_set_expr_features(query.body.as_ref(), diagnostics);
            inspect_query_features(query, diagnostics);
        }
        SetExpr::SetOperation { left, right, .. } => {
            inspect_set_expr_features(left, diagnostics);
            inspect_set_expr_features(right, diagnostics);
        }
        SetExpr::Values(_)
        | SetExpr::Insert(_)
        | SetExpr::Update(_)
        | SetExpr::Delete(_)
        | SetExpr::Table(_) => {}
    }
}

fn inspect_query_features(query: &SqlQuery, diagnostics: &mut Vec<Diagnostic>) {
    if query.order_by.is_some() {
        diagnostics.push(warning(
            "unsupported_order_by",
            DiagnosticArea::Other,
            "ORDER BY semantics are not implemented yet",
        ));
    }
    if query.limit_clause.is_some() {
        diagnostics.push(warning(
            "unsupported_limit",
            DiagnosticArea::Other,
            "LIMIT semantics are not implemented yet",
        ));
    }
    if query.fetch.is_some() {
        diagnostics.push(warning(
            "unsupported_fetch",
            DiagnosticArea::Other,
            "FETCH semantics are not implemented yet",
        ));
    }
    if !query.locks.is_empty() {
        diagnostics.push(warning(
            "unsupported_lock",
            DiagnosticArea::Other,
            "query lock semantics are not implemented yet",
        ));
    }
    if query.for_clause.is_some() {
        diagnostics.push(warning(
            "unsupported_for_clause",
            DiagnosticArea::Other,
            "FOR clause semantics are not implemented yet",
        ));
    }
    if query.settings.is_some() {
        diagnostics.push(warning(
            "unsupported_settings",
            DiagnosticArea::Other,
            "query SETTINGS semantics are not implemented yet",
        ));
    }
    if query.format_clause.is_some() {
        diagnostics.push(warning(
            "unsupported_format_clause",
            DiagnosticArea::Other,
            "FORMAT clause semantics are not implemented yet",
        ));
    }
    if !query.pipe_operators.is_empty() {
        diagnostics.push(warning(
            "unsupported_pipe_operator",
            DiagnosticArea::Other,
            "pipe-operator semantics are not implemented yet",
        ));
    }
}

fn analyze_relation_constraints(
    statement: &SqlStatement,
) -> Result<Option<RelationConstraintSet>, ConstraintMetadataError> {
    let SqlStatement::CreateTable(create_table) = statement else {
        return Ok(None);
    };
    analyze_create_table_constraints(create_table)
}

fn analyze_create_table_constraints(
    create_table: &SqlCreateTable,
) -> Result<Option<RelationConstraintSet>, ConstraintMetadataError> {
    let relation = create_table.name.to_string();
    let mut constraints = Vec::new();
    let mut diagnostics = Vec::new();

    for (index, constraint) in create_table.constraints.iter().enumerate() {
        match constraint {
            TableConstraint::PrimaryKey {
                name,
                index_name,
                columns,
                characteristics,
                ..
            } => match index_column_names(columns) {
                Some(columns) => constraints.push(RelationConstraint::primary_key(
                    columns,
                    vec![sql_constraint_evidence(
                        &relation,
                        constraint_identity(
                            "primary_key",
                            index,
                            name.as_ref().map(ToString::to_string)
                                .or_else(|| index_name.as_ref().map(ToString::to_string)),
                        ),
                        characteristics.as_ref(),
                    )?],
                )?),
                None => diagnostics.push(ConstraintDiagnostic::new(
                    "unsupported_key_expression",
                    format!(
                        "primary key on relation '{relation}' contains a non-column expression"
                    ),
                )),
            },
            TableConstraint::Unique {
                name,
                index_name,
                columns,
                characteristics,
                ..
            } => match index_column_names(columns) {
                Some(columns) => constraints.push(RelationConstraint::unique_key(
                    columns,
                    vec![sql_constraint_evidence(
                        &relation,
                        constraint_identity(
                            "unique",
                            index,
                            name.as_ref().map(ToString::to_string)
                                .or_else(|| index_name.as_ref().map(ToString::to_string)),
                        ),
                        characteristics.as_ref(),
                    )?],
                )?),
                None => diagnostics.push(ConstraintDiagnostic::new(
                    "unsupported_key_expression",
                    format!(
                        "unique constraint on relation '{relation}' contains a non-column expression"
                    ),
                )),
            },
            TableConstraint::ForeignKey {
                name,
                columns,
                foreign_table,
                referred_columns,
                characteristics,
                ..
            } => {
                if referred_columns.is_empty() {
                    diagnostics.push(ConstraintDiagnostic::new(
                        "unresolved_foreign_key_columns",
                        format!(
                            "foreign key on relation '{relation}' does not declare referenced columns"
                        ),
                    ));
                    continue;
                }
                constraints.push(RelationConstraint::foreign_key(
                    columns.iter().map(ToString::to_string).collect(),
                    foreign_table.to_string(),
                    referred_columns.iter().map(ToString::to_string).collect(),
                    vec![sql_constraint_evidence(
                        &relation,
                        constraint_identity(
                            "foreign_key",
                            index,
                            name.as_ref().map(ToString::to_string),
                        ),
                        characteristics.as_ref(),
                    )?],
                )?);
            }
            TableConstraint::Check {
                name,
                expr,
                enforced,
            } => match accepted_values_from_check(expr, None) {
                Some((column, values, quote)) => {
                    constraints.push(RelationConstraint::accepted_values(
                        column,
                        values,
                        quote,
                        vec![sql_constraint_evidence_with_enforcement(
                            &relation,
                            constraint_identity(
                                "check",
                                index,
                                name.as_ref().map(ToString::to_string),
                            ),
                            *enforced,
                        )?],
                    )?);
                }
                None => diagnostics.push(ConstraintDiagnostic::new(
                    "unsupported_check_constraint",
                    format!(
                        "check constraint on relation '{relation}' cannot be represented safely as a finite accepted-values constraint"
                    ),
                )),
            },
            _ => {}
        }
    }

    for column in &create_table.columns {
        for (option_index, option) in column.options.iter().enumerate() {
            match &option.option {
                ColumnOption::Unique {
                    is_primary,
                    characteristics,
                } => {
                    let source_id = format!(
                        "{}:column:{}:{}:{}",
                        relation,
                        column.name,
                        if *is_primary { "primary_key" } else { "unique" },
                        option_index
                    );
                    let evidence = vec![sql_constraint_evidence(
                        &relation,
                        source_id,
                        characteristics.as_ref(),
                    )?];
                    let constraint = if *is_primary {
                        RelationConstraint::primary_key(vec![column.name.to_string()], evidence)?
                    } else {
                        RelationConstraint::unique_key(vec![column.name.to_string()], evidence)?
                    };
                    constraints.push(constraint);
                }
                ColumnOption::ForeignKey {
                    foreign_table,
                    referred_columns,
                    characteristics,
                    ..
                } => {
                    if referred_columns.is_empty() {
                        diagnostics.push(ConstraintDiagnostic::new(
                            "unresolved_foreign_key_columns",
                            format!(
                                "foreign key on '{}.{}' does not declare referenced columns",
                                relation, column.name
                            ),
                        ));
                        continue;
                    }
                    constraints.push(RelationConstraint::foreign_key(
                        vec![column.name.to_string()],
                        foreign_table.to_string(),
                        referred_columns.iter().map(ToString::to_string).collect(),
                        vec![sql_constraint_evidence(
                            &relation,
                            format!(
                                "{}:column:{}:foreign_key:{}",
                                relation, column.name, option_index
                            ),
                            characteristics.as_ref(),
                        )?],
                    )?);
                }
                ColumnOption::NotNull => {
                    constraints.push(RelationConstraint::not_null(
                        column.name.to_string(),
                        vec![sql_constraint_evidence(
                            &relation,
                            format!(
                                "{}:column:{}:not_null:{}",
                                relation, column.name, option_index
                            ),
                            None,
                        )?],
                    )?);
                }
                ColumnOption::Check(expr) => {
                    match accepted_values_from_check(expr, Some(column.name.value.as_str())) {
                        Some((column_name, values, quote)) => {
                            constraints.push(RelationConstraint::accepted_values(
                                column_name,
                                values,
                                quote,
                                vec![sql_constraint_evidence(
                                    &relation,
                                    format!(
                                        "{}:column:{}:check:{}",
                                        relation, column.name, option_index
                                    ),
                                    None,
                                )?],
                            )?);
                        }
                        None => diagnostics.push(ConstraintDiagnostic::new(
                            "unsupported_check_constraint",
                            format!(
                                "check constraint on '{}.{}' cannot be represented safely as a finite accepted-values constraint",
                                relation, column.name
                            ),
                        )),
                    }
                }
                _ => {}
            }
        }
    }

    if let Some(primary_key) = create_table.primary_key.as_deref() {
        match expression_key_columns(primary_key) {
            Some(columns) => constraints.push(RelationConstraint::primary_key(
                columns,
                vec![sql_constraint_evidence(
                    &relation,
                    format!("{relation}:clickhouse_primary_key"),
                    None,
                )?],
            )?),
            None => diagnostics.push(ConstraintDiagnostic::new(
                "unsupported_key_expression",
                format!("primary key on relation '{relation}' contains a non-column expression"),
            )),
        }
    }

    if constraints.is_empty() && diagnostics.is_empty() {
        return Ok(None);
    }

    let mut set = RelationConstraintSet::new(relation, constraints)?;
    for diagnostic in diagnostics {
        set.add_diagnostic(diagnostic);
    }
    Ok(Some(set))
}

fn accepted_values_from_check(
    expression: &Expr,
    expected_column: Option<&str>,
) -> Option<(String, Vec<ConstraintValue>, bool)> {
    let Expr::InList {
        expr,
        list,
        negated: false,
    } = unwrap_nested_expression(expression)
    else {
        return None;
    };

    let column = check_column_name(expr)?;
    if expected_column.is_some_and(|expected| expected != column) {
        return None;
    }

    let mut values = Vec::with_capacity(list.len());
    let mut quote = None;
    for expression in list {
        let (value, value_quote) = constraint_value_from_expression(expression)?;
        if let Some(value_quote) = value_quote {
            match quote {
                Some(current) if current != value_quote => return None,
                None => quote = Some(value_quote),
                _ => {}
            }
        }
        values.push(value);
    }

    Some((column.to_string(), values, quote.unwrap_or(false)))
}

fn unwrap_nested_expression(mut expression: &Expr) -> &Expr {
    while let Expr::Nested(inner) = expression {
        expression = inner;
    }
    expression
}

fn check_column_name(expression: &Expr) -> Option<&str> {
    match unwrap_nested_expression(expression) {
        Expr::Identifier(identifier) => Some(identifier.value.as_str()),
        Expr::CompoundIdentifier(identifiers) => identifiers
            .last()
            .map(|identifier| identifier.value.as_str()),
        _ => None,
    }
}

fn constraint_value_from_expression(expression: &Expr) -> Option<(ConstraintValue, Option<bool>)> {
    match unwrap_nested_expression(expression) {
        Expr::Value(value) => constraint_value_from_sql_value(&value.value),
        Expr::UnaryOp {
            op: SqlUnaryOperator::Plus,
            expr,
        } => constraint_number_from_expression(expr, false).map(|value| (value, Some(false))),
        Expr::UnaryOp {
            op: SqlUnaryOperator::Minus,
            expr,
        } => constraint_number_from_expression(expr, true).map(|value| (value, Some(false))),
        _ => None,
    }
}

fn constraint_value_from_sql_value(value: &Value) -> Option<(ConstraintValue, Option<bool>)> {
    match value {
        Value::Null => Some((ConstraintValue::Null, None)),
        Value::Boolean(value) => Some((ConstraintValue::Boolean(*value), Some(false))),
        Value::Number(value, _) => constraint_number(value).map(|value| (value, Some(false))),
        value => string_literal_value(value)
            .map(|value| (ConstraintValue::String(value.to_string()), Some(true))),
    }
}

fn constraint_number_from_expression(expression: &Expr, negative: bool) -> Option<ConstraintValue> {
    let Expr::Value(value) = unwrap_nested_expression(expression) else {
        return None;
    };
    let Value::Number(value, _) = &value.value else {
        return None;
    };
    if negative {
        constraint_number(&format!("-{value}"))
    } else {
        constraint_number(value)
    }
}

fn constraint_number(value: &str) -> Option<ConstraintValue> {
    let number = Number::from_str(value).ok()?;
    if let Some(value) = number.as_i64() {
        Some(ConstraintValue::Integer(value))
    } else if let Some(value) = number.as_u64() {
        Some(ConstraintValue::UnsignedInteger(value))
    } else {
        Some(ConstraintValue::Number(number.to_string()))
    }
}

fn sql_constraint_evidence(
    relation: &str,
    source_id: String,
    characteristics: Option<&ConstraintCharacteristics>,
) -> Result<ConstraintEvidence, ConstraintMetadataError> {
    sql_constraint_evidence_with_enforcement(
        relation,
        source_id,
        characteristics.and_then(|value| value.enforced),
    )
}

fn sql_constraint_evidence_with_enforcement(
    relation: &str,
    source_id: String,
    enforced: Option<bool>,
) -> Result<ConstraintEvidence, ConstraintMetadataError> {
    let enforcement = match enforced {
        Some(true) => ConstraintEnforcement::Enforced,
        Some(false) => ConstraintEnforcement::NotEnforced,
        None => ConstraintEnforcement::Unknown,
    };
    let provenance = ConstraintProvenance::new(
        ConstraintSourceKind::SqlDdl,
        if source_id.trim().is_empty() {
            format!("create_table:{relation}")
        } else {
            source_id
        },
    )?;
    Ok(ConstraintEvidence::new(provenance, enforcement))
}

fn constraint_identity(kind: &str, index: usize, name: Option<String>) -> String {
    match name {
        Some(name) => format!("{kind}:{name}"),
        None => format!("{kind}:{}", index + 1),
    }
}

fn index_column_names(columns: &[IndexColumn]) -> Option<Vec<String>> {
    columns
        .iter()
        .map(|column| match &column.column.expr {
            Expr::Identifier(identifier) => Some(identifier.to_string()),
            _ => None,
        })
        .collect()
}

fn expression_key_columns(expression: &Expr) -> Option<Vec<String>> {
    match expression {
        Expr::Identifier(identifier) => Some(vec![identifier.to_string()]),
        Expr::Tuple(expressions) => expressions
            .iter()
            .map(|expression| match expression {
                Expr::Identifier(identifier) => Some(identifier.to_string()),
                _ => None,
            })
            .collect(),
        _ => None,
    }
}

fn unsupported_queryless_create_table() -> ProtocolStatement {
    let diagnostic = warning(
        "unsupported_queryless_create_table",
        DiagnosticArea::Statement,
        "CREATE TABLE without an AS query does not define transformation semantics",
    );

    ProtocolStatement::Unsupported(UnsupportedStatement::new(
        "create_table".to_string(),
        vec![diagnostic],
    ))
}

fn unsupported_write_statement(category: &str, code: &str, message: &str) -> ProtocolStatement {
    ProtocolStatement::Unsupported(UnsupportedStatement::new(
        category.to_string(),
        vec![warning(code, DiagnosticArea::Statement, message)],
    ))
}

fn unsupported_statement() -> ProtocolStatement {
    let diagnostic = warning(
        "unsupported_statement",
        DiagnosticArea::Statement,
        "the parsed SQL statement is not supported by semantic analysis",
    );

    ProtocolStatement::Unsupported(UnsupportedStatement::new(
        "statement".to_string(),
        vec![diagnostic],
    ))
}

fn warning(code: &str, area: DiagnosticArea, message: &str) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Warning,
        code.to_string(),
        area,
        message.to_string(),
    )
}

fn sort_diagnostics(diagnostics: &mut [Diagnostic]) {
    diagnostics.sort_by(|left, right| {
        left.area()
            .as_str()
            .cmp(right.area().as_str())
            .then_with(|| left.code().cmp(right.code()))
            .then_with(|| left.message().cmp(right.message()))
    });
}
