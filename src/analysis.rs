//! Semantic-analysis boundary.
//!
//! This module is the only layer that converts sqlparser AST statements into public protocol
//! values. Unsupported semantics are retained explicitly rather than silently discarded.

use std::fmt;

use sqlparser::ast::{
    Expr, GroupByExpr, Query as SqlQuery, Select, SelectItem, SetExpr, Statement as SqlStatement,
    TableFactor,
};

use crate::parser::ParsedSql;
use crate::protocol::{
    Diagnostic, DiagnosticArea, DiagnosticSeverity, Predicate, Predicates, Protocol,
    ProtocolStatement, QueryStatement, UnsupportedSemantic, UnsupportedStatement,
};

/// Error produced after parsing succeeds but protocol analysis cannot proceed.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum AnalysisError {
    /// The caller did not provide a non-empty dialect name for protocol metadata.
    EmptyDialectName,
    /// Parsing succeeded but produced no SQL statements to analyze.
    NoStatements,
}

impl fmt::Display for AnalysisError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyDialectName => {
                write!(formatter, "analysis requires a non-empty dialect name")
            }
            Self::NoStatements => write!(formatter, "analysis requires at least one SQL statement"),
        }
    }
}

impl std::error::Error for AnalysisError {}

pub(crate) fn analyze(parsed: ParsedSql, dialect_name: &str) -> Result<Protocol, AnalysisError> {
    if dialect_name.trim().is_empty() {
        return Err(AnalysisError::EmptyDialectName);
    }

    if parsed.statements.is_empty() {
        return Err(AnalysisError::NoStatements);
    }

    let statements = parsed.statements.iter().map(analyze_statement).collect();

    Ok(Protocol::new(dialect_name.to_string(), statements))
}

fn analyze_statement(statement: &SqlStatement) -> ProtocolStatement {
    match statement {
        SqlStatement::Query(query) => ProtocolStatement::Query(analyze_query(query)),
        _ => unsupported_statement(),
    }
}

fn analyze_query(query: &SqlQuery) -> QueryStatement {
    let mut diagnostics = Vec::new();

    let predicates = match query.body.as_ref() {
        SetExpr::Select(select) => analyze_select(select, &mut diagnostics),
        _ => {
            diagnostics.push(warning(
                "unsupported_query_body",
                DiagnosticArea::Statement,
                "the parsed query body is not supported by semantic analysis",
            ));
            Predicates::new(None, None, None)
        }
    };

    inspect_query_features(query, &mut diagnostics);
    sort_diagnostics(&mut diagnostics);

    QueryStatement::new(predicates, diagnostics)
}

fn analyze_select(select: &Select, diagnostics: &mut Vec<Diagnostic>) -> Predicates {
    inspect_projection(select, diagnostics);
    inspect_sources(select, diagnostics);
    inspect_select_features(select, diagnostics);

    let where_predicate = unsupported_predicate(
        "where_predicate",
        "WHERE predicate semantics are not implemented yet",
        select.selection.as_ref(),
        diagnostics,
    );
    let having_predicate = unsupported_predicate(
        "having_predicate",
        "HAVING predicate semantics are not implemented yet",
        select.having.as_ref(),
        diagnostics,
    );
    let qualify_predicate = unsupported_predicate(
        "qualify_predicate",
        "QUALIFY predicate semantics are not implemented yet",
        select.qualify.as_ref(),
        diagnostics,
    );

    Predicates::new(where_predicate, having_predicate, qualify_predicate)
}

fn inspect_projection(select: &Select, diagnostics: &mut Vec<Diagnostic>) {
    if !select.projection.is_empty() {
        diagnostics.push(warning(
            "output_analysis_pending",
            DiagnosticArea::Output,
            "output-column semantics are not implemented yet",
        ));
    }

    for item in &select.projection {
        match item {
            SelectItem::UnnamedExpr(expression)
            | SelectItem::ExprWithAlias {
                expr: expression, ..
            } => {
                inspect_expression(expression, diagnostics);
            }
            SelectItem::QualifiedWildcard(_, _) | SelectItem::Wildcard(_) => {
                diagnostics.push(warning(
                    "unresolved_wildcard",
                    DiagnosticArea::Output,
                    "wildcard output cannot be resolved without source schema information",
                ));
            }
        }
    }
}

fn inspect_expression(expression: &Expr, diagnostics: &mut Vec<Diagnostic>) {
    match expression {
        Expr::Identifier(_) | Expr::CompoundIdentifier(_) | Expr::Value(_) => {}
        Expr::Function(_) => diagnostics.push(warning(
            "unsupported_function",
            DiagnosticArea::Function,
            &format!(
                "function expression {expression} is parsed but function semantics are not implemented"
            ),
        )),
        _ => diagnostics.push(warning(
            "unsupported_expression",
            DiagnosticArea::Expression,
            &format!("expression {expression} is parsed but its semantics are not implemented"),
        )),
    }
}

fn inspect_sources(select: &Select, diagnostics: &mut Vec<Diagnostic>) {
    let mut has_regular_source = false;
    let mut has_joins = false;

    for source in &select.from {
        match &source.relation {
            TableFactor::Table { args: None, .. } => has_regular_source = true,
            factor => diagnostics.push(warning(
                "unsupported_table_factor",
                DiagnosticArea::Source,
                &format!(
                    "table factor {factor} is parsed but its source semantics are not implemented"
                ),
            )),
        }

        has_joins |= !source.joins.is_empty();
    }

    if has_regular_source {
        diagnostics.push(warning(
            "source_analysis_pending",
            DiagnosticArea::Source,
            "source-relation semantics are not implemented yet",
        ));
    }

    if has_joins {
        diagnostics.push(warning(
            "join_analysis_pending",
            DiagnosticArea::Join,
            "join semantics are not implemented yet",
        ));
    }
}

fn inspect_select_features(select: &Select, diagnostics: &mut Vec<Diagnostic>) {
    if select.distinct.is_some() {
        diagnostics.push(warning(
            "unsupported_distinct",
            DiagnosticArea::Output,
            "DISTINCT semantics are not implemented yet",
        ));
    }
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
    if has_group_by(&select.group_by) {
        diagnostics.push(warning(
            "unsupported_group_by",
            DiagnosticArea::Other,
            "GROUP BY semantics are not implemented yet",
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
    if !select.named_window.is_empty() {
        diagnostics.push(warning(
            "unsupported_named_window",
            DiagnosticArea::Other,
            "named WINDOW semantics are not implemented yet",
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

fn inspect_query_features(query: &SqlQuery, diagnostics: &mut Vec<Diagnostic>) {
    if query.with.is_some() {
        diagnostics.push(warning(
            "unsupported_cte",
            DiagnosticArea::Source,
            "CTE semantics are not implemented yet",
        ));
    }
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

fn unsupported_predicate(
    feature: &str,
    reason: &str,
    expression: Option<&Expr>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Predicate> {
    expression.map(|expression| {
        diagnostics.push(warning(
            &format!("unsupported_{feature}"),
            DiagnosticArea::Predicate,
            reason,
        ));
        inspect_expression(expression, diagnostics);

        Predicate::Unsupported(UnsupportedSemantic::new(
            feature.to_string(),
            Some(reason.to_string()),
        ))
    })
}

fn has_group_by(group_by: &GroupByExpr) -> bool {
    match group_by {
        GroupByExpr::All(_) => true,
        GroupByExpr::Expressions(expressions, modifiers) => {
            !expressions.is_empty() || !modifiers.is_empty()
        }
    }
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
