//! Semantic-analysis boundary.
//!
//! This module is the only layer that converts sqlparser AST statements into public protocol
//! values. Unsupported semantics are retained explicitly rather than silently discarded.

use std::{fmt, str::FromStr};

use serde_json::Number;
use sqlparser::ast::{
    BinaryOperator as SqlBinaryOperator, DuplicateTreatment, Expr, Function, FunctionArg,
    FunctionArgExpr, FunctionArguments, GroupByExpr, Query as SqlQuery, Select, SelectItem,
    SetExpr, Statement as SqlStatement, TableFactor, UnaryOperator as SqlUnaryOperator, Value,
};

use crate::parser::ParsedSql;
use crate::protocol::{
    BetweenPredicate, BinaryExpression, BinaryOperator, ColumnExpression, ComparisonOperator,
    ComparisonPredicate, Diagnostic, DiagnosticArea, DiagnosticSeverity, Expression,
    FunctionExpression, InPredicate, IsNullPredicate, LiteralExpression, LiteralType, LiteralValue,
    LogicalPredicate, NotPredicate, Predicate, Predicates, Protocol, ProtocolStatement,
    QueryStatement, UnaryExpression, UnaryOperator, UnsupportedSemantic, UnsupportedStatement,
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

    Predicates::new(
        select
            .selection
            .as_ref()
            .map(|expression| analyze_predicate(expression, diagnostics)),
        select
            .having
            .as_ref()
            .map(|expression| analyze_predicate(expression, diagnostics)),
        select
            .qualify
            .as_ref()
            .map(|expression| analyze_predicate(expression, diagnostics)),
    )
}

fn analyze_predicate(expression: &Expr, diagnostics: &mut Vec<Diagnostic>) -> Predicate {
    match expression {
        Expr::Nested(inner) => analyze_predicate(inner, diagnostics),
        Expr::BinaryOp { left, op, right } => match op {
            SqlBinaryOperator::And => Predicate::And(LogicalPredicate::pair(
                analyze_predicate(left, diagnostics),
                analyze_predicate(right, diagnostics),
            )),
            SqlBinaryOperator::Or => Predicate::Or(LogicalPredicate::pair(
                analyze_predicate(left, diagnostics),
                analyze_predicate(right, diagnostics),
            )),
            _ => comparison_operator(op).map_or_else(
                || Predicate::BooleanExpression(analyze_expression(expression, diagnostics)),
                |operator| {
                    normalize_comparison(
                        analyze_expression(left, diagnostics),
                        operator,
                        analyze_expression(right, diagnostics),
                    )
                },
            ),
        },
        Expr::IsDistinctFrom(left, right) => normalize_comparison(
            analyze_expression(left, diagnostics),
            ComparisonOperator::IsDistinctFrom,
            analyze_expression(right, diagnostics),
        ),
        Expr::IsNotDistinctFrom(left, right) => normalize_comparison(
            analyze_expression(left, diagnostics),
            ComparisonOperator::IsNotDistinctFrom,
            analyze_expression(right, diagnostics),
        ),
        Expr::IsNull(inner) => Predicate::IsNull(IsNullPredicate::new(
            analyze_expression(inner, diagnostics),
            false,
        )),
        Expr::IsNotNull(inner) => Predicate::IsNull(IsNullPredicate::new(
            analyze_expression(inner, diagnostics),
            true,
        )),
        Expr::InList {
            expr,
            list,
            negated,
        } => Predicate::In(InPredicate::new(
            analyze_expression(expr, diagnostics),
            list.iter()
                .map(|value| analyze_expression(value, diagnostics))
                .collect(),
            *negated,
        )),
        Expr::Between {
            expr,
            negated,
            low,
            high,
        } => Predicate::Between(BetweenPredicate::new(
            analyze_expression(expr, diagnostics),
            analyze_expression(low, diagnostics),
            analyze_expression(high, diagnostics),
            *negated,
        )),
        Expr::UnaryOp { op, expr }
            if matches!(op, SqlUnaryOperator::Not | SqlUnaryOperator::BangNot) =>
        {
            Predicate::Not(NotPredicate::new(analyze_predicate(expr, diagnostics)))
        }
        _ => Predicate::BooleanExpression(analyze_expression(expression, diagnostics)),
    }
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

fn analyze_expression(expression: &Expr, diagnostics: &mut Vec<Diagnostic>) -> Expression {
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
        Expr::Function(function) => analyze_function(function, expression, diagnostics),
        Expr::UnaryOp { op, expr } => analyze_unary_expression(op, expr, expression, diagnostics),
        Expr::BinaryOp { left, op, right } => {
            analyze_binary_expression(left, op, right, expression, diagnostics)
        }
        Expr::Nested(inner) => analyze_expression(inner, diagnostics),
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
        Some((column, relation_parts)) if relation_parts.is_empty() => {
            Expression::Column(ColumnExpression::new(None, column.value.clone()))
        }
        Some((column, relation_parts)) => Expression::Column(ColumnExpression::new(
            Some(
                relation_parts
                    .iter()
                    .map(|identifier| identifier.value.as_str())
                    .collect::<Vec<_>>()
                    .join("."),
            ),
            column.value.clone(),
        )),
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
    diagnostics: &mut Vec<Diagnostic>,
) -> Expression {
    if function.uses_odbc_syntax
        || !matches!(&function.parameters, FunctionArguments::None)
        || function.filter.is_some()
        || function.null_treatment.is_some()
        || function.over.is_some()
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
                        normalized_arguments.push(analyze_expression(argument, diagnostics));
                    }
                    _ => {
                        return unsupported_expression(
                            "function",
                            expression,
                            DiagnosticArea::Function,
                            diagnostics,
                        );
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

    Expression::Function(FunctionExpression::new(
        function.name.to_string(),
        arguments,
        distinct,
    ))
}

fn analyze_unary_expression(
    operator: &SqlUnaryOperator,
    operand: &Expr,
    expression: &Expr,
    diagnostics: &mut Vec<Diagnostic>,
) -> Expression {
    let operator = match operator {
        SqlUnaryOperator::Plus => Some(UnaryOperator::Plus),
        SqlUnaryOperator::Minus => Some(UnaryOperator::Minus),
        SqlUnaryOperator::PGBitwiseNot => Some(UnaryOperator::BitwiseNot),
        _ => None,
    };

    operator.map_or_else(
        || {
            unsupported_expression(
                "unary_expression",
                expression,
                DiagnosticArea::Expression,
                diagnostics,
            )
        },
        |operator| {
            Expression::Unary(UnaryExpression::new(
                operator,
                analyze_expression(operand, diagnostics),
            ))
        },
    )
}

fn analyze_binary_expression(
    left: &Expr,
    operator: &SqlBinaryOperator,
    right: &Expr,
    expression: &Expr,
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

    operator.map_or_else(
        || {
            unsupported_expression(
                "binary_expression",
                expression,
                DiagnosticArea::Expression,
                diagnostics,
            )
        },
        |operator| {
            Expression::Binary(BinaryExpression::new(
                operator,
                analyze_expression(left, diagnostics),
                analyze_expression(right, diagnostics),
            ))
        },
    )
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
    let _ = analyze_expression(expression, diagnostics);
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
