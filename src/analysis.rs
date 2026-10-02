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
    BinaryOperator as SqlBinaryOperator, DuplicateTreatment, Expr, Function, FunctionArg,
    FunctionArgExpr, FunctionArguments, GroupByExpr, Join as SqlJoin, JoinConstraint, JoinOperator,
    Query as SqlQuery, Select, SelectItem, SetExpr, SetOperator as SqlSetOperator,
    SetQuantifier as SqlSetQuantifier, Statement as SqlStatement, TableFactor, TableWithJoins,
    UnaryOperator as SqlUnaryOperator, Value,
};

use crate::domain::derive_column_domains;
use crate::parser::ParsedSql;
use crate::protocol::{
    BetweenPredicate, BinaryExpression, BinaryOperator, ColumnDomain, ColumnExpression, ColumnRef,
    ComparisonOperator, ComparisonPredicate, Diagnostic, DiagnosticArea, DiagnosticSeverity,
    Expression, FunctionExpression, InPredicate, IsNullPredicate, Join as ProtocolJoin, JoinKind,
    LineageSource, LiteralExpression, LiteralType, LiteralValue, LogicalPredicate, NotPredicate,
    Output, OutputColumn, Predicate, Predicates, Protocol, ProtocolStatement, QueryStatement,
    RelationRef, SetOperand, SetOperation, SetOperator, SetQuantifier, SourceRelation,
    UnaryExpression, UnaryOperator, UnknownSemantic, UnsupportedSemantic, UnsupportedStatement,
    ValueDomain,
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
        SqlStatement::Query(query) => ProtocolStatement::Query(analyze_query(query, None)),
        SqlStatement::CreateTable(create_table) => match &create_table.query {
            Some(query) => {
                ProtocolStatement::Query(analyze_query(query, Some(create_table.name.to_string())))
            }
            None => unsupported_queryless_create_table(),
        },
        SqlStatement::CreateView { name, query, .. } => {
            ProtocolStatement::Query(analyze_query(query, Some(name.to_string())))
        }
        _ => unsupported_statement(),
    }
}

fn analyze_query(query: &SqlQuery, produced_relation: Option<String>) -> QueryStatement {
    let mut diagnostics = Vec::new();
    let mut derived_index = 0;
    let relation_analysis = analyze_query_relations(
        query,
        &BTreeSet::new(),
        &mut diagnostics,
        &mut derived_index,
    );

    let set_operation = analyze_set_operation(query.body.as_ref());
    let output = analyze_query_output(query, &BTreeMap::new(), &mut diagnostics);

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

    let column_domains = match query.body.as_ref() {
        SetExpr::Select(_) => derive_column_domains(&predicates, &relation_analysis.sources),
        SetExpr::SetOperation { .. } | SetExpr::Query(_) => {
            analyze_query_column_domains(query, &BTreeSet::new())
        }
        _ => Vec::new(),
    };

    if matches!(query.body.as_ref(), SetExpr::SetOperation { .. }) {
        inspect_set_expr_features(query.body.as_ref(), &mut diagnostics);
    }
    inspect_query_features(query, &mut diagnostics);
    sort_diagnostics(&mut diagnostics);

    QueryStatement::new(
        relation_analysis.sources,
        relation_analysis.dependencies.into_iter().collect(),
        relation_analysis.joins,
        predicates,
        column_domains,
        output,
        diagnostics,
    )
    .with_set_operation(set_operation)
    .with_produced_relation(produced_relation)
}

fn analyze_query_predicates(query: &SqlQuery, diagnostics: &mut Vec<Diagnostic>) -> Predicates {
    match query.body.as_ref() {
        SetExpr::Select(select) => analyze_select_predicates(select, diagnostics),
        SetExpr::Query(query) => analyze_query_predicates(query, diagnostics),
        SetExpr::SetOperation { .. } => Predicates::new(None, None, None),
        _ => Predicates::new(None, None, None),
    }
}

fn analyze_select(select: &Select, diagnostics: &mut Vec<Diagnostic>) -> Predicates {
    inspect_select_features(select, diagnostics);
    analyze_select_predicates(select, diagnostics)
}

fn analyze_select_predicates(select: &Select, diagnostics: &mut Vec<Diagnostic>) -> Predicates {
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
        | SetExpr::Merge(_)
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
        SqlSetOperator::Except => SetOperator::Except,
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

#[derive(Default)]
struct RelationAnalysis {
    sources: Vec<SourceRelation>,
    dependencies: BTreeSet<String>,
    joins: Vec<ProtocolJoin>,
}

struct AnalyzedRelation {
    source: SourceRelation,
    reference: RelationRef,
    dependencies: BTreeSet<String>,
}

fn analyze_query_relations(
    query: &SqlQuery,
    inherited_local_relations: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
    derived_index: &mut usize,
) -> RelationAnalysis {
    let mut analysis = RelationAnalysis::default();
    let mut local_relations = inherited_local_relations.clone();

    if let Some(with) = &query.with {
        for cte in &with.cte_tables {
            local_relations.insert(cte.alias.name.to_string());
        }

        for cte in &with.cte_tables {
            let nested =
                analyze_query_relations(&cte.query, &local_relations, diagnostics, derived_index);
            analysis.dependencies.extend(nested.dependencies);
        }
    }

    let body = analyze_set_expr_relations(
        query.body.as_ref(),
        &local_relations,
        diagnostics,
        derived_index,
    );
    merge_relation_analysis(&mut analysis, body);
    analysis
}

fn analyze_set_expr_relations(
    expression: &SetExpr,
    local_relations: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
    derived_index: &mut usize,
) -> RelationAnalysis {
    match expression {
        SetExpr::Select(select) => {
            analyze_select_relations(select, local_relations, diagnostics, derived_index)
        }
        SetExpr::Query(query) => {
            analyze_query_relations(query, local_relations, diagnostics, derived_index)
        }
        SetExpr::SetOperation { left, right, .. } => {
            let mut analysis =
                analyze_set_expr_relations(left, local_relations, diagnostics, derived_index);
            let right =
                analyze_set_expr_relations(right, local_relations, diagnostics, derived_index);
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
    let mut analysis = RelationAnalysis::default();

    for source in &select.from {
        analyze_table_with_joins(
            source,
            local_relations,
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
            } => collect_expression_dependencies(
                expression,
                local_relations,
                diagnostics,
                derived_index,
                &mut analysis.dependencies,
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
        collect_expression_dependencies(
            expression,
            local_relations,
            diagnostics,
            derived_index,
            &mut analysis.dependencies,
        );
    }

    analysis
}

fn analyze_table_with_joins(
    source: &TableWithJoins,
    local_relations: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
    derived_index: &mut usize,
    analysis: &mut RelationAnalysis,
) {
    let mut left = register_table_factor(
        &source.relation,
        local_relations,
        diagnostics,
        derived_index,
        analysis,
    );

    for join in &source.joins {
        let right = register_table_factor(
            &join.relation,
            local_relations,
            diagnostics,
            derived_index,
            analysis,
        );

        if let (Some(left_ref), Some(right_ref)) = (left.as_ref(), right.as_ref()) {
            analysis.joins.push(analyze_join(
                join,
                left_ref,
                right_ref,
                local_relations,
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
    local_relations: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
    derived_index: &mut usize,
    analysis: &mut RelationAnalysis,
) -> Option<RelationRef> {
    let relation = analyze_table_factor(factor, local_relations, diagnostics, derived_index)?;

    for dependency in relation.dependencies {
        analysis.dependencies.insert(dependency);
    }

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
    match factor {
        TableFactor::Table {
            name,
            alias,
            args: None,
            ..
        } => {
            let name = name.to_string();
            let alias = alias.as_ref().map(|alias| alias.name.to_string());
            let dependencies = if local_relations.contains(&name) {
                BTreeSet::new()
            } else {
                BTreeSet::from([name.clone()])
            };

            Some(AnalyzedRelation {
                source: SourceRelation::new(name.clone(), alias.clone()),
                reference: RelationRef::new(name, alias),
                dependencies,
            })
        }
        TableFactor::Derived {
            subquery, alias, ..
        } => {
            let nested =
                analyze_query_relations(subquery, local_relations, diagnostics, derived_index);
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

fn analyze_join(
    join: &SqlJoin,
    left: &RelationRef,
    right: &RelationRef,
    local_relations: &BTreeSet<String>,
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

    let (kind, constraint, exact_kind) = match &join.join_operator {
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
    };

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
            local_relations,
            diagnostics,
            derived_index,
            dependencies,
        );
    }

    let condition = constraint
        .and_then(|constraint| analyze_join_constraint(constraint, left, right, diagnostics));

    ProtocolJoin::new(kind, left.clone(), right.clone(), condition)
}

fn analyze_join_constraint(
    constraint: &JoinConstraint,
    left: &RelationRef,
    right: &RelationRef,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Predicate> {
    match constraint {
        JoinConstraint::On(expression) => Some(analyze_predicate(expression, diagnostics)),
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
            collect_expression_dependencies(
                expr,
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
            collect_expression_dependencies(
                left,
                local_relations,
                diagnostics,
                derived_index,
                dependencies,
            );
            collect_expression_dependencies(
                right,
                local_relations,
                diagnostics,
                derived_index,
                dependencies,
            );
        }
        Expr::UnaryOp { expr, .. } | Expr::Nested(expr) => collect_expression_dependencies(
            expr,
            local_relations,
            diagnostics,
            derived_index,
            dependencies,
        ),
        Expr::Between {
            expr, low, high, ..
        } => {
            collect_expression_dependencies(
                expr,
                local_relations,
                diagnostics,
                derived_index,
                dependencies,
            );
            collect_expression_dependencies(
                low,
                local_relations,
                diagnostics,
                derived_index,
                dependencies,
            );
            collect_expression_dependencies(
                high,
                local_relations,
                diagnostics,
                derived_index,
                dependencies,
            );
        }
        Expr::InList { expr, list, .. } => {
            collect_expression_dependencies(
                expr,
                local_relations,
                diagnostics,
                derived_index,
                dependencies,
            );
            for value in list {
                collect_expression_dependencies(
                    value,
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
                local_relations,
                diagnostics,
                derived_index,
                dependencies,
            );
            collect_function_argument_dependencies(
                &function.args,
                local_relations,
                diagnostics,
                derived_index,
                dependencies,
            );
            if let Some(filter) = &function.filter {
                collect_expression_dependencies(
                    filter,
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
                    collect_expression_dependencies(
                        expression,
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
            _ => match comparison_operator(op) {
                Some(operator) => normalize_comparison(
                    analyze_expression(left, diagnostics),
                    operator,
                    analyze_expression(right, diagnostics),
                ),
                None => Predicate::BooleanExpression(analyze_expression(expression, diagnostics)),
            },
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
        Expr::UnaryOp {
            op: SqlUnaryOperator::Not | SqlUnaryOperator::BangNot,
            expr,
        } => Predicate::Not(NotPredicate::new(analyze_predicate(expr, diagnostics))),
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

    match operator {
        Some(operator) => Expression::Unary(UnaryExpression::new(
            operator,
            analyze_expression(operand, diagnostics),
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
            analyze_expression(left, diagnostics),
            analyze_expression(right, diagnostics),
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

type LocalOutputMap = BTreeMap<String, BTreeMap<String, Vec<LineageSource>>>;

#[derive(Clone)]
struct OutputRelation {
    qualifiers: Vec<String>,
    source: OutputRelationSource,
}

#[derive(Clone)]
enum OutputRelationSource {
    Physical(String),
    Local(BTreeMap<String, Vec<LineageSource>>),
}

fn analyze_query_output(
    query: &SqlQuery,
    inherited_local_outputs: &LocalOutputMap,
    diagnostics: &mut Vec<Diagnostic>,
) -> Output {
    let mut local_outputs = inherited_local_outputs.clone();

    if let Some(with) = &query.with {
        for cte in &with.cte_tables {
            let output = analyze_query_output(&cte.query, &local_outputs, diagnostics);
            local_outputs.insert(cte.alias.name.to_string(), output_lineage_map(&output));
        }
    }

    analyze_set_expr_output(query.body.as_ref(), &local_outputs, diagnostics)
}

fn analyze_set_expr_output(
    expression: &SetExpr,
    local_outputs: &LocalOutputMap,
    diagnostics: &mut Vec<Diagnostic>,
) -> Output {
    match expression {
        SetExpr::Select(select) => analyze_select_output(select, local_outputs, diagnostics),
        SetExpr::Query(query) => analyze_query_output(query, local_outputs, diagnostics),
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

            let left_output = analyze_set_expr_output(left, local_outputs, diagnostics);
            let right_output = analyze_set_expr_output(right, local_outputs, diagnostics);
            merge_set_operation_output(left_output, right_output, diagnostics)
        }
        SetExpr::Values(_)
        | SetExpr::Insert(_)
        | SetExpr::Update(_)
        | SetExpr::Delete(_)
        | SetExpr::Merge(_)
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
                lineage,
            )
        })
        .collect();

    Output::new(columns)
}

fn analyze_select_output(
    select: &Select,
    local_outputs: &LocalOutputMap,
    diagnostics: &mut Vec<Diagnostic>,
) -> Output {
    let scope = build_output_scope(select, local_outputs, diagnostics);
    let columns = select
        .projection
        .iter()
        .map(|item| analyze_output_item(item, &scope, diagnostics))
        .collect();

    Output::new(columns)
}

fn analyze_output_item(
    item: &SelectItem,
    scope: &[OutputRelation],
    diagnostics: &mut Vec<Diagnostic>,
) -> OutputColumn {
    match item {
        SelectItem::UnnamedExpr(expression) => OutputColumn::new(
            output_name_for_expression(expression),
            analyze_expression(expression, diagnostics),
            lineage_for_expression(expression, scope, diagnostics),
        ),
        SelectItem::ExprWithAlias { expr, alias } => OutputColumn::new(
            alias.value.clone(),
            analyze_expression(expr, diagnostics),
            lineage_for_expression(expr, scope, diagnostics),
        ),
        SelectItem::Wildcard(_) => unresolved_wildcard_column("*".to_string(), diagnostics),
        SelectItem::QualifiedWildcard(prefix, _) => {
            unresolved_wildcard_column(format!("{prefix}.*"), diagnostics)
        }
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
        Vec::new(),
    )
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

fn output_lineage_map(output: &Output) -> BTreeMap<String, Vec<LineageSource>> {
    output
        .columns()
        .iter()
        .map(|column| (column.name().to_string(), column.lineage().to_vec()))
        .collect()
}

fn build_output_scope(
    select: &Select,
    local_outputs: &LocalOutputMap,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<OutputRelation> {
    let mut scope = Vec::new();

    for source in &select.from {
        register_output_table_factor(&source.relation, local_outputs, diagnostics, &mut scope);
        for join in &source.joins {
            register_output_table_factor(&join.relation, local_outputs, diagnostics, &mut scope);
        }
    }

    scope
}

fn register_output_table_factor(
    factor: &TableFactor,
    local_outputs: &LocalOutputMap,
    diagnostics: &mut Vec<Diagnostic>,
    scope: &mut Vec<OutputRelation>,
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
                Some(columns) => OutputRelationSource::Local(columns.clone()),
                None => OutputRelationSource::Physical(relation_name),
            };
            scope.push(OutputRelation { qualifiers, source });
        }
        TableFactor::Derived {
            subquery, alias, ..
        } => {
            let output = analyze_query_output(subquery, local_outputs, diagnostics);
            let qualifiers = alias
                .as_ref()
                .map(|alias| vec![alias.name.to_string()])
                .unwrap_or_default();
            scope.push(OutputRelation {
                qualifiers,
                source: OutputRelationSource::Local(output_lineage_map(&output)),
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
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<LineageSource> {
    let mut lineage = BTreeSet::new();
    collect_output_lineage(expression, scope, diagnostics, &mut lineage);
    lineage
        .into_iter()
        .map(|(relation, column)| LineageSource::new(relation, column))
        .collect()
}

fn collect_output_lineage(
    expression: &Expr,
    scope: &[OutputRelation],
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
        Expr::Function(function) => {
            collect_function_argument_lineage(&function.parameters, scope, diagnostics, lineage);
            collect_function_argument_lineage(&function.args, scope, diagnostics, lineage);
            if let Some(filter) = &function.filter {
                collect_output_lineage(filter, scope, diagnostics, lineage);
            }
        }
        Expr::UnaryOp { expr, .. }
        | Expr::Nested(expr)
        | Expr::IsNull(expr)
        | Expr::IsNotNull(expr) => {
            collect_output_lineage(expr, scope, diagnostics, lineage);
        }
        Expr::BinaryOp { left, right, .. }
        | Expr::AnyOp { left, right, .. }
        | Expr::AllOp { left, right, .. }
        | Expr::IsDistinctFrom(left, right)
        | Expr::IsNotDistinctFrom(left, right) => {
            collect_output_lineage(left, scope, diagnostics, lineage);
            collect_output_lineage(right, scope, diagnostics, lineage);
        }
        Expr::Between {
            expr, low, high, ..
        } => {
            collect_output_lineage(expr, scope, diagnostics, lineage);
            collect_output_lineage(low, scope, diagnostics, lineage);
            collect_output_lineage(high, scope, diagnostics, lineage);
        }
        Expr::InList { expr, list, .. } => {
            collect_output_lineage(expr, scope, diagnostics, lineage);
            for value in list {
                collect_output_lineage(value, scope, diagnostics, lineage);
            }
        }
        _ => {}
    }
}

fn collect_function_argument_lineage(
    arguments: &FunctionArguments,
    scope: &[OutputRelation],
    diagnostics: &mut Vec<Diagnostic>,
    lineage: &mut BTreeSet<(String, String)>,
) {
    if let FunctionArguments::List(arguments) = arguments {
        for argument in &arguments.args {
            if let FunctionArg::Unnamed(FunctionArgExpr::Expr(expression)) = argument {
                collect_output_lineage(expression, scope, diagnostics, lineage);
            }
        }
    }
}

fn resolve_output_column(
    qualifier: Option<&str>,
    column: &str,
    scope: &[OutputRelation],
    diagnostics: &mut Vec<Diagnostic>,
    lineage: &mut BTreeSet<(String, String)>,
) {
    let candidates = scope
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
            OutputRelationSource::Physical(relation) => Some(vec![LineageSource::new(
                relation.clone(),
                column.to_string(),
            )]),
            OutputRelationSource::Local(columns) => columns.get(column).cloned(),
        })
        .collect::<Vec<_>>();

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
