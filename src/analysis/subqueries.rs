//! Nested query semantics, correlations and subquery predicates.

use super::*;

pub(super) fn analyze_subquery_semantics(
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
        None,
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

    inspect_query_features(query, None, &mut diagnostics);
    let condition_exactness = analyze_query_condition_exactness(
        query,
        &predicates,
        &relations.sources,
        &relations.dependencies,
        &relations.joins,
        &diagnostics,
        !correlations.is_empty(),
    );
    sort_diagnostics(&mut diagnostics);

    let row_shape_preserves_candidates = query.with.is_none()
        && query.limit_clause.is_none()
        && query.fetch.is_none()
        && matches!(query.body.as_ref(), SetExpr::Select(select)
            if select.from.len() == 1
                && matches!(
                    &select.from[0].relation,
                    TableFactor::Table {
                        args: None,
                        sample: None,
                        version: None,
                        json_path: None,
                        with_ordinality: false,
                        partitions,
                        with_hints,
                        index_hints,
                        ..
                    } if partitions.is_empty() && with_hints.is_empty() && index_hints.is_empty()
                )
                && select.from[0].joins.is_empty()
                // DISTINCT preserves Boolean membership, but removes candidate rows.
                && select.distinct.is_none()
                && select.having.is_none()
                && select.qualify.is_none()
                && select.top.is_none()
                && select.prewhere.is_none()
                && select.lateral_views.is_empty()
                && select.connect_by.is_none()
                && matches!(&select.group_by, GroupByExpr::Expressions(expressions, modifiers)
                    if expressions.is_empty() && modifiers.is_empty()))
        && output
            .columns()
            .iter()
            .all(|column| nested_projection_preserves_candidate_rows(column.expression()));

    // A nonempty GROUP BY over local source columns preserves EXISTS
    // emptiness, although it does not preserve the physical candidate bag.
    // Never reuse this certification for IN, scalar, or aggregate outputs.
    let inner_qualifier = relations
        .sources
        .first()
        .map(|source| source.alias().unwrap_or(source.name()));
    let grouped_exists_equivalent = query.with.is_none()
        && query.limit_clause.is_none()
        && query.fetch.is_none()
        && matches!(query.body.as_ref(), SetExpr::Select(select)
            if select.from.len() == 1
                && matches!(&select.from[0].relation, TableFactor::Table {
                    args: None, sample: None, version: None, json_path: None,
                    with_ordinality: false, partitions, with_hints, index_hints, ..
                } if partitions.is_empty() && with_hints.is_empty() && index_hints.is_empty())
                && select.from[0].joins.is_empty()
                && select.distinct.is_none()
                && select.having.is_none()
                && select.qualify.is_none()
                && select.top.is_none()
                && select.prewhere.is_none()
                && select.lateral_views.is_empty()
                && select.connect_by.is_none()
                && matches!(&select.group_by, GroupByExpr::Expressions(expressions, modifiers)
                    if !expressions.is_empty() && modifiers.is_empty()
                        && expressions.iter().all(|expression|
                            matches!(expression, Expr::CompoundIdentifier(parts)
                                if parts.len() == 2 && Some(parts[0].value.as_str()) == inner_qualifier))))
        && output.columns().iter().all(|column|
            nested_projection_preserves_candidate_rows(column.expression()));

    // Keep joined EXISTS certification independent from row-preserving and
    // grouped claims. The physical planner must still prove *every* ON and
    // WHERE comparison against the same complete physical source bags.
    let joined_exists_equivalent = query.with.is_none()
        && query.limit_clause.is_none()
        && query.fetch.is_none()
        && relations.joins.len() == 1
        && relations.joins[0].kind() == JoinKind::Inner
        && relations.joins[0].execution().is_none()
        && matches!(query.body.as_ref(), SetExpr::Select(select)
            if select.from.len() == 1
                && matches!(&select.from[0].relation, TableFactor::Table {
                    args: None, sample: None, version: None, json_path: None,
                    with_ordinality: false, partitions, with_hints, index_hints, ..
                } if partitions.is_empty() && with_hints.is_empty() && index_hints.is_empty())
                && select.from[0].joins.len() == 1
                && matches!(&select.from[0].joins[0].relation, TableFactor::Table {
                    args: None, sample: None, version: None, json_path: None,
                    with_ordinality: false, partitions, with_hints, index_hints, ..
                } if partitions.is_empty() && with_hints.is_empty() && index_hints.is_empty())
                && select.distinct.is_none()
                && select.having.is_none()
                && select.qualify.is_none()
                && select.top.is_none()
                && select.prewhere.is_none()
                && select.lateral_views.is_empty()
                && select.connect_by.is_none()
                && matches!(&select.group_by, GroupByExpr::Expressions(expressions, modifiers)
                    if expressions.is_empty() && modifiers.is_empty()))
        && output.columns().iter().all(|column|
            nested_projection_preserves_candidate_rows(column.expression()));

    SubquerySemantics::new(
        relations.sources,
        relations.dependencies.into_iter().collect(),
        correlations,
        relations.joins,
        output,
        RowConditions::new(predicates, column_domains, condition_exactness),
        diagnostics,
    )
    .with_row_shape_preserves_candidates(row_shape_preserves_candidates)
    .with_grouped_exists_equivalent(grouped_exists_equivalent)
    .with_joined_exists_equivalent(joined_exists_equivalent)
}

/// Avoid calling an aggregate or window query candidate-preserving merely
/// because the row-shaping operation is nested in a scalar expression.
/// Unknown expression forms default to residual membership evidence.
pub(super) fn nested_projection_preserves_candidate_rows(expression: &Expression) -> bool {
    match expression {
        Expression::Column(_) | Expression::Literal(_) => true,
        Expression::Function(function) => function
            .arguments()
            .iter()
            .all(nested_projection_preserves_candidate_rows),
        Expression::Unary(unary) => nested_projection_preserves_candidate_rows(unary.operand()),
        Expression::Binary(binary) => {
            nested_projection_preserves_candidate_rows(binary.left())
                && nested_projection_preserves_candidate_rows(binary.right())
        }
        Expression::AggregateFunction(_)
        | Expression::WindowFunction(_)
        | Expression::SignedIntegerCast(_)
        | Expression::Case(_)
        | Expression::BooleanPredicate(_)
        | Expression::ScalarSubquery(_)
        | Expression::Unknown(_)
        | Expression::Unsupported(_) => false,
    }
}

pub(super) fn collect_query_correlations(
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

pub(super) fn collect_set_expr_correlations(
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

pub(super) fn collect_select_correlations(
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

pub(super) fn select_local_qualifiers(select: &Select) -> BTreeSet<String> {
    let mut qualifiers = BTreeSet::new();
    for source in &select.from {
        collect_table_factor_qualifiers(&source.relation, &mut qualifiers);
        for join in &source.joins {
            collect_table_factor_qualifiers(&join.relation, &mut qualifiers);
        }
    }
    qualifiers
}

pub(super) fn collect_table_factor_qualifiers(
    factor: &TableFactor,
    qualifiers: &mut BTreeSet<String>,
) {
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

pub(super) fn collect_expression_correlations(
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

pub(super) fn collect_function_argument_correlations(
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

pub(super) fn collect_outer_column(
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

pub(super) fn output_name_for_expression(expression: &Expr) -> String {
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
