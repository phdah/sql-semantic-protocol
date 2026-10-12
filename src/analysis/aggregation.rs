//! WHERE, HAVING, QUALIFY and GROUP BY normalization.

use super::*;

pub(super) fn analyze_query_predicates(query: &SqlQuery, diagnostics: &mut Vec<Diagnostic>) -> Predicates {
    analyze_query_predicates_with_outer_scope(query, &[], diagnostics)
}

pub(super) fn analyze_query_predicates_with_outer_scope(
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

pub(super) fn analyze_select(select: &Select, diagnostics: &mut Vec<Diagnostic>) -> Predicates {
    inspect_select_features(select, diagnostics);
    let mut scope_diagnostics = Vec::new();
    let scope = build_output_scope(select, &BTreeMap::new(), &[], &mut scope_diagnostics, None);
    analyze_select_predicates_with_scope(select, &scope, diagnostics)
}

pub(super) fn analyze_select_predicates_with_scope(
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

pub(super) fn analyze_query_aggregation(
    query: &SqlQuery,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Aggregation> {
    match query.body.as_ref() {
        SetExpr::Select(select) => analyze_select_aggregation(select, diagnostics),
        SetExpr::Query(query) => analyze_query_aggregation(query, diagnostics),
        _ => None,
    }
}

pub(super) fn analyze_select_aggregation(
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

pub(super) fn analyze_grouping_expression(
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

pub(super) fn analyze_grouping_sets(
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

pub(super) fn diagnose_group_by_modifiers(
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
