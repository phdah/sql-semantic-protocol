//! Root query orchestration and SELECT-level filtering.

use crate::protocol::WindowGroupedCountSource;
use super::*;

/// A plain derived SELECT preserves physical row identity only when it adds
/// neither filters nor row-shaping. Use this narrow certified path exclusively
/// for a uniquely expanded CASE alias NULL test.
fn direct_derived_case_witness_source(
    query: &SqlQuery,
    predicate: Option<&Predicate>,
    dependencies: &BTreeSet<String>,
) -> Option<SourceRelation> {
    let Some(Predicate::IsNull(test)) = predicate else {
        return None;
    };
    if !matches!(test.expression(), Expression::Column(_)) || query.with.is_some() {
        return None;
    }
    let SetExpr::Select(select) = query.body.as_ref() else {
        return None;
    };
    let [outer] = select.from.as_slice() else {
        return None;
    };
    if !outer.joins.is_empty() {
        return None;
    }
    let TableFactor::Derived { subquery, .. } = &outer.relation else {
        return None;
    };
    if subquery.with.is_some() || subquery.limit_clause.is_some() || subquery.fetch.is_some() {
        return None;
    }
    let SetExpr::Select(inner) = subquery.body.as_ref() else {
        return None;
    };
    let [inner_from] = inner.from.as_slice() else {
        return None;
    };
    // Only an unambiguous SQL alias reference may inherit the producer's
    // CASE. Direct expressions in the outer WHERE are not interchangeable
    // with projected columns and must not borrow this physical identity.
    let target_alias = match select.selection.as_ref()? {
        Expr::IsNull(expression) | Expr::IsNotNull(expression) => {
            let Expr::Identifier(name) = expression.as_ref() else {
                return None;
            };
            &name.value
        }
        _ => return None,
    };
    if inner
        .projection
        .iter()
        .filter(|item| {
            matches!(
                item,
                SelectItem::ExprWithAlias {
                    expr: Expr::Case { .. },
                    alias,
                } if &alias.value == target_alias
            )
        })
        .count()
        != 1
        || inner
            .projection
            .iter()
            .filter(|item| {
                matches!(
                    item,
                    SelectItem::ExprWithAlias { alias, .. } if &alias.value == target_alias
                )
            })
            .count()
            != 1
    {
        return None;
    }
    if !matches!(
        &inner_from.relation,
        TableFactor::Table {
            alias: None,
            sample: None,
            ..
        }
    ) || !inner_from.joins.is_empty()
        || inner.selection.is_some()
        || inner.having.is_some()
        || inner.qualify.is_some()
        || inner.prewhere.is_some()
        || inner.distinct.is_some()
        || inner.top.is_some()
        || !inner.lateral_views.is_empty()
        || !matches!(&inner.group_by, GroupByExpr::Expressions(items, modifiers)
            if items.is_empty() && modifiers.is_empty())
        || dependencies.len() != 1
    {
        return None;
    }
    Some(SourceRelation::new(
        dependencies.iter().next()?.clone(),
        None,
    ))
}

/// Retain one precisely validated derived GROUP BY COUNT(*) producer in the
/// canonical protocol. Never infer aggregate semantics from a projected alias.
fn qualify_grouped_count_source(
    query: &SqlQuery,
    dependencies: &BTreeSet<String>,
) -> Option<WindowGroupedCountSource> {
    if dependencies.len() != 1 { return None; }
    let source = dependencies.iter().next()?;
    let SetExpr::Select(outer) = query.body.as_ref() else { return None; };
    let [from] = outer.from.as_slice() else { return None; };
    if !from.joins.is_empty() { return None; }
    let TableFactor::Derived { subquery, .. } = &from.relation else { return None; };
    if subquery.with.is_some() || subquery.limit_clause.is_some() || subquery.fetch.is_some() {
        return None;
    }
    let SetExpr::Select(inner) = subquery.body.as_ref() else { return None; };
    let [inner_from] = inner.from.as_slice() else { return None; };
    if !matches!(&inner_from.relation, TableFactor::Table { sample: None, .. })
        || !inner_from.joins.is_empty() || inner.selection.is_some()
        || inner.qualify.is_some() || inner.having.is_some()
        || inner.distinct.is_some() || inner.top.is_some()
        || inner.prewhere.is_some() || inner.connect_by.is_some()
        || !inner.lateral_views.is_empty()
    { return None; }
    let GroupByExpr::Expressions(group, modifiers) = &inner.group_by else { return None; };
    let [Expr::Identifier(group_column)] = group.as_slice() else { return None; };
    if !modifiers.is_empty() || inner.projection.len() != 2 { return None; }
    let group_matches = inner.projection.iter().filter(|projection| match projection {
        SelectItem::UnnamedExpr(Expr::Identifier(name)) => name.value == group_column.value,
        SelectItem::ExprWithAlias { expr: Expr::Identifier(name), alias } =>
            name.value == group_column.value && alias.value == group_column.value,
        _ => false,
    }).count() == 1;
    if !group_matches { return None; }
    let mut count_alias = None;
    for projection in &inner.projection {
        let SelectItem::ExprWithAlias { expr: Expr::Function(function), alias } = projection
            else { continue; };
        if !function.name.to_string().eq_ignore_ascii_case("count")
            || function.over.is_some() || function.filter.is_some() || function.null_treatment.is_some()
        { continue; }
        let FunctionArguments::List(args) = &function.args else { continue; };
        if !args.clauses.is_empty() || args.duplicate_treatment.is_some()
            || !matches!(args.args.as_slice(), [FunctionArg::Unnamed(FunctionArgExpr::Wildcard)])
        { continue; }
        count_alias = Some(alias.value.clone());
    }
    let count_alias = count_alias?;
    if count_alias == group_column.value { return None; }
    Some(WindowGroupedCountSource::new(
        source.clone(), group_column.value.clone(), count_alias
    ))
}

pub(super) fn analyze_query(
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
        Some(metadata),
    );

    let set_operation = analyze_set_operation(query.body.as_ref()).map(|operation| {
        let mut branches = Vec::new();
        collect_set_branch_evidence(
            query.body.as_ref(),
            query,
            "body",
            metadata,
            true,
            &mut branches,
        );
        let row_shaping = analyze_set_row_shaping(query, &branches);
        populate_set_operation_evidence(
            query.body.as_ref(),
            operation,
            &branches,
            query.limit_clause.is_none() && query.fetch.is_none(),
        )
        .with_row_shaping(row_shaping)
    });
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

    let (column_domains, typing_failures) = type_column_domains(
        analyze_query_column_domains(
            query,
            &BTreeSet::new(),
            &BTreeMap::new(),
            &BTreeMap::new(),
            Some(metadata),
        ),
        metadata,
    );
    // The output expression for a set operation is an Unknown placeholder. Its
    // actual domain is the recursive composition of the independently analyzed
    // branch outputs, not a refinement of that placeholder.
    let output = match set_operation.as_ref() {
        Some(operation) => refine_set_operation_output_domains(output, operation),
        None => refine_catalog_arithmetic_output_domains(
            refine_output_domains_from_column_domains(
                output,
                &column_domains,
                &relation_analysis.sources,
            ),
            &column_domains,
            &relation_analysis.sources,
            metadata,
        ),
    };

    if matches!(query.body.as_ref(), SetExpr::SetOperation { .. }) {
        inspect_set_expr_features(query.body.as_ref(), &mut diagnostics);
    }
    inspect_query_features(query, set_operation.as_ref(), &mut diagnostics);
    let missing_column_residuals = validate_schema_column_references(
        query,
        &predicates,
        &output,
        &relation_analysis.joins,
        &relation_analysis.sources,
        metadata,
        &mut diagnostics,
    );
    let predicate_exactness = analyze_query_condition_exactness(
        query,
        &predicates,
        &relation_analysis.sources,
        &relation_analysis.dependencies,
        &relation_analysis.joins,
        &diagnostics,
        false,
    );
    let condition_exactness = predicate_exactness
        .merged_with(&unknown_column_domain_exactness(
            &column_domains,
            &typing_failures,
            &predicates,
            &relation_analysis.joins,
            &relation_analysis.sources,
            &predicate_exactness,
        ))
        .merged_with(&comparison_domain_exactness(&column_domains, metadata))
        .merged_with(&ConditionExactness::from_residuals(
            relation_analysis.residual_conditions.clone(),
        ))
        .merged_with(&ConditionExactness::from_residuals(
            missing_column_residuals,
        ));
    // An unfiltered, row-preserving derived SELECT over one physical table
    // can expose a computed CASE alias without creating a new row identity.
    // Rebind that single expression's witness to its actual physical source;
    // all other derived shapes remain explicit residual boundaries.
    let local_case_witness_source = direct_derived_case_witness_source(
        query,
        predicates.where_predicate(),
        &relation_analysis.dependencies,
    );
    // Expand only the certified witness's computed alias. The canonical
    // predicate and independent source-column domains must keep the
    // intermediate alias reference and its uncertainty provenance.
    let local_case_witness_predicate = local_case_witness_source.as_ref().and_then(|_| {
        let SetExpr::Select(select) = query.body.as_ref() else {
            return None;
        };
        let Some(Predicate::IsNull(test)) = predicates.where_predicate() else {
            return None;
        };
        let mut local_diagnostics = Vec::new();
        let scope = build_output_scope(
            select,
            &BTreeMap::new(),
            &[],
            &mut local_diagnostics,
            Some(metadata),
        );
        let expression = inherit_local_case_expression(test.expression().clone(), &scope);
        matches!(expression, Expression::Case(_))
            .then(|| Predicate::IsNull(IsNullPredicate::new(expression, test.negated())))
    });
    let witness_sources = if local_case_witness_predicate.is_some() {
        local_case_witness_source.into_iter().collect::<Vec<_>>()
    } else {
        relation_analysis.sources.clone()
    };
    let witness_predicate = local_case_witness_predicate
        .as_ref()
        .or_else(|| predicates.where_predicate());
    let boolean_witness = crate::boolean_witness::analyze(
        witness_predicate,
        &witness_sources,
        |column| {
            let original_type = metadata.column_data_type(column)?;
            let data_type = match original_type {
                DataType::Nullable(inner) => inner.as_ref(),
                other => other,
            };
            match data_type {
                DataType::SignedInteger { bits: Some(bits) } if *bits > 0 && *bits <= 64 => {
                    let magnitude = 1_i128 << (u32::from(*bits) - 1);
                    Some(crate::boolean_witness::SignedIntegerEvidence {
                        minimum: -magnitude,
                        maximum: magnitude - 1,
                    })
                }
                _ => None,
            }
        },
        |column| {
            let original_type = metadata.column_data_type(column)?;
            let data_type = match original_type {
                DataType::Nullable(inner) => inner.as_ref(),
                other => other,
            };
            match data_type {
                DataType::String {
                    length,
                    fixed: false,
                } => Some(crate::boolean_witness::StringEvidence { max_chars: *length }),
                _ => None,
            }
        },
        |column| metadata.column_data_type(column).cloned(),
    );
    let (column_domains, output) =
        refine_boolean_qualifying_outcomes(column_domains, output, boolean_witness.as_ref());
    // Typed scalar projections depend on the *final* qualifying source range:
    // Boolean witness inversion may prove bounds absent from predicate-only
    // domain analysis. Re-derive computed results from that same row-local
    // source evidence without refining unrelated or residual witnesses.
    let output = if set_operation.is_none() {
        refine_catalog_arithmetic_output_domains(
            output,
            &column_domains,
            &relation_analysis.sources,
            metadata,
        )
    } else {
        output
    };
    sort_diagnostics(&mut diagnostics);

    // Only a plain one-to-one projection preserves all source-row truth
    // assignments when a downstream boolean witness is mapped to its inputs.
    // Filtered, limited, grouped, sampled, and joined producers must remain
    // intermediate witness boundaries, even if their columns are plain copies.
    let row_preserving_projection = query.with.is_none()
        && query.limit_clause.is_none()
        && query.fetch.is_none()
        && matches!(query.body.as_ref(), SetExpr::Select(select)
            if select.from.len() == 1
                && matches!(&select.from[0].relation, TableFactor::Table { sample: None, .. })
                && select.from[0].joins.is_empty()
                && select.distinct.is_none()
                && select.top.is_none()
                && select.selection.is_none()
                && select.having.is_none()
                && select.qualify.is_none()
                && select.prewhere.is_none()
                && select.lateral_views.is_empty()
                && select.connect_by.is_none()
                && matches!(&select.group_by, GroupByExpr::Expressions(items, modifiers)
                    if items.is_empty() && modifiers.is_empty()));

    // A plain single-source WHERE filters source rows without other row shaping.
    let filter_only_row_shape = query.with.is_none()
        && query.limit_clause.is_none()
        && query.fetch.is_none()
        && matches!(query.body.as_ref(), SetExpr::Select(select)
            if select.from.len() == 1
                && matches!(&select.from[0].relation, TableFactor::Table { sample: None, .. })
                && select.from[0].joins.is_empty()
                && select.distinct.is_none()
                && select.top.is_none()
                && select.selection.is_some()
                && select.having.is_none()
                && select.qualify.is_none()
                && select.prewhere.is_none()
                && select.lateral_views.is_empty()
                && select.connect_by.is_none()
                && matches!(&select.group_by, GroupByExpr::Expressions(items, modifiers)
                    if items.is_empty() && modifiers.is_empty()));

    // Prove singleton output only for source-free literal SELECTs and global
    // aggregates without grouping or any row-limiting, HAVING, or QUALIFY clause.
    // This is a result-row fact, not a source-row cardinality assumption.
    let proven_single_row_output = query.with.is_none()
        && query.limit_clause.is_none()
        && query.fetch.is_none()
        && diagnostics.is_empty()
        && matches!(query.body.as_ref(), SetExpr::Select(select)
            if select.top.is_none()
                && select.having.is_none()
                && select.qualify.is_none()
                && select.prewhere.is_none()
                && select.connect_by.is_none()
                && select.lateral_views.is_empty()
                && matches!(&select.group_by, GroupByExpr::Expressions(items, modifiers)
                    if items.is_empty() && modifiers.is_empty())
                && ((select.from.is_empty()
                        && select.selection.is_none()
                        && output.columns().iter().all(|column| matches!(column.expression(), Expression::Literal(_))))
                    || (output.columns().iter().any(|column| matches!(column.expression(), Expression::AggregateFunction(_)))
                        && output.columns().iter().all(|column| matches!(column.expression(), Expression::AggregateFunction(_) | Expression::Literal(_))))));

    // GROUP BY normally emits one final row per surviving group. Additional
    // output-shaping (DISTINCT, QUALIFY, TOP, LIMIT, FETCH) can change that count.
    let group_rows_match_surviving_groups = query.limit_clause.is_none()
        && query.fetch.is_none()
        && matches!(query.body.as_ref(), SetExpr::Select(select)
            if select.distinct.is_none()
                && select.qualify.is_none()
                && select.top.is_none()
                && matches!(&select.group_by, GroupByExpr::Expressions(items, modifiers)
                    if !items.is_empty() && modifiers.is_empty()
                        && items.iter().all(|item| !matches!(item, Expr::GroupingSets(_) | Expr::Cube(_) | Expr::Rollup(_)))));

    // These are the only upstream row-shapes whose physical QUALIFY
    // composition is currently certified. Any unrelated row shaping fails closed.
    let composed_rank_outer = query.with.is_none()
        && query.limit_clause.is_none() && query.fetch.is_none()
        && matches!(query.body.as_ref(), SetExpr::Select(select)
            if select.qualify.is_some() && select.top.is_none()
                && select.distinct.is_none() && select.selection.is_none()
                && select.having.is_none() && select.prewhere.is_none()
                && select.connect_by.is_none() && select.lateral_views.is_empty()
                && matches!(&select.group_by, GroupByExpr::Expressions(items, mods)
                    if items.is_empty() && mods.is_empty())
                && select.from.len() == 1);
    let grouped_count_source = composed_rank_outer
        .then(|| qualify_grouped_count_source(query, &relation_analysis.dependencies))
        .flatten();
    let joined_qualify_shape = composed_rank_outer
        && matches!(query.body.as_ref(), SetExpr::Select(select)
            if select.from.len() == 1
                && matches!(&select.from[0].relation, TableFactor::Table { sample: None, .. })
                && select.from[0].joins.len() == 1);
    let ranked_composed_shape = joined_qualify_shape || grouped_count_source.is_some();

    let (plain_goal_output_shape, ranked_goal_output_shape) =
        if query.with.is_none() && query.limit_clause.is_none() && query.fetch.is_none() {
            match query.body.as_ref() {
                SetExpr::Select(select)
                    if select.top.is_none()
                        && select.distinct.is_none()
                        && select.selection.is_none()
                        && select.having.is_none()
                        && select.prewhere.is_none()
                        && select.connect_by.is_none()
                        && select.lateral_views.is_empty()
                        && matches!(&select.group_by, GroupByExpr::Expressions(items, modifiers)
                            if items.is_empty() && modifiers.is_empty())
                        && select.from.len() == 1
                        && matches!(
                            &select.from[0].relation,
                            TableFactor::Table { sample: None, .. }
                        ) =>
                {
                    (
                        select.qualify.is_none(),
                        select.qualify.is_some() && select.from[0].joins.is_empty(),
                    )
                }
                _ => (false, false),
            }
        } else {
            (false, false)
        };

    // A WHERE-bearing JOIN cannot use plain_goal_output_shape, but it
    // must still prove the absence of DISTINCT, LIMIT, TOP and other
    // row-shaping syntax before its physical row-count construction.
    let filtered_join_goal_output_shape = query.with.is_none()
        && query.limit_clause.is_none()
        && query.fetch.is_none()
        && matches!(query.body.as_ref(), SetExpr::Select(select)
            if select.top.is_none()
                && select.distinct.is_none()
                && select.selection.is_some()
                && select.having.is_none()
                && select.qualify.is_none()
                && select.prewhere.is_none()
                && select.connect_by.is_none()
                && select.lateral_views.is_empty()
                && matches!(&select.group_by, GroupByExpr::Expressions(items, modifiers)
                    if items.is_empty() && modifiers.is_empty())
                && select.from.len() == 1
                && matches!(&select.from[0].relation, TableFactor::Table { sample: None, .. }));

    let special_join_goal_output_shape = query.with.is_none()
        && query.limit_clause.is_none()
        && query.fetch.is_none()
        && matches!(query.body.as_ref(), SetExpr::Select(select)
            if select.from.len() == 1
                && select.from[0].joins.len() == 1
                && matches!(&select.from[0].relation, TableFactor::Table { sample: None, .. })
                && select.distinct.is_none()
                && select.top.is_none()
                && select.selection.is_none()
                && select.having.is_none()
                && select.qualify.is_none()
                && select.prewhere.is_none()
                && select.lateral_views.is_empty()
                && select.connect_by.is_none()
                && matches!(&select.group_by, GroupByExpr::Expressions(items, modifiers)
                    if items.is_empty() && modifiers.is_empty()));

    QueryStatement::new(
        relation_analysis.sources,
        relation_analysis.dependencies.into_iter().collect(),
        relation_analysis.joins,
        RowConditions::new(predicates, column_domains, condition_exactness),
        output,
        diagnostics,
    )
    .with_aggregation(aggregation)
    .with_set_operation(set_operation)
    .with_subquery_witnesses()
    .with_boolean_witness(boolean_witness)
    .with_row_preserving_projection(row_preserving_projection)
    .with_filter_only_row_shape(filter_only_row_shape)
    .with_proven_single_row_output(proven_single_row_output)
    .with_group_row_correspondence(group_rows_match_surviving_groups)
    .with_goal_shapes(plain_goal_output_shape, ranked_goal_output_shape)
    .with_ranked_composition(ranked_composed_shape, grouped_count_source)
    .with_filtered_join_goal_shape(filtered_join_goal_output_shape)
    .with_special_join_goal_shape(special_join_goal_output_shape)
    .with_group_witness()
    .with_window_witness()
    .with_projected_window_witness(projected_window_filter(query, metadata))
    .with_produced_relation(produced_relation)
}

// Exact same-row conjunctions may prove source scalar bounds even where
// general expression-domain analysis conservatively returned Unknown. Never
// distribute OR branches into independently sampled per-column intervals.
pub(super) fn refine_boolean_qualifying_outcomes(
    column_domains: Vec<ColumnDomain>,
    output: Output,
    witness: Option<&crate::boolean_witness::BooleanWitness>,
) -> (Vec<ColumnDomain>, Output) {
    // A symbolically impossible joint filter admits no qualifying rows, so
    // every projected output is empty even when no independent per-column
    // domain can be narrowed.
    if witness.is_some_and(|item| {
        matches!(
            item.qualifying(),
            crate::boolean_witness::BooleanWitnessDirection::Impossible
        )
    }) {
        let output = Output::new(
            output
                .columns()
                .iter()
                .cloned()
                .map(|column| column.with_domain(ValueDomain::Empty))
                .collect(),
        );
        return (column_domains, output);
    }
    let proven = witness
        .map(crate::boolean_witness::BooleanWitness::qualifying_conjunctive_domains)
        .unwrap_or_default();
    if proven.is_empty() {
        return (column_domains, output);
    }

    let mut domains = column_domains
        .into_iter()
        .map(|item| (item.column().clone(), item.domain().clone()))
        .collect::<BTreeMap<_, _>>();
    for item in proven {
        let column = item.column().clone();
        let domain = item.domain().clone();
        domains
            .entry(column)
            .and_modify(|existing| {
                if matches!(existing, ValueDomain::Unknown(_)) {
                    *existing = domain.clone();
                } else {
                    *existing = intersect_domains(existing, &domain);
                }
            })
            .or_insert(domain);
    }

    let output = Output::new(
        output
            .columns()
            .iter()
            .cloned()
            .map(|column| {
                let Some(source) = column.plain_copy_source() else {
                    return column;
                };
                let reference = ColumnRef::new(
                    Some(source.relation().to_string()),
                    source.column().to_string(),
                );
                let Some(domain) = domains.get(&reference) else {
                    return column;
                };
                let refined = match column.domain() {
                    ValueDomain::Unknown(_) => domain.clone(),
                    other => intersect_domains(other, domain),
                };
                column.with_domain(refined)
            })
            .collect(),
    );

    (
        domains
            .into_iter()
            .map(|(column, domain)| ColumnDomain::new(column, domain))
            .collect(),
        output,
    )
}

/// Resolve one immediately projected ROW_NUMBER filter through a direct derived
/// table or one local CTE. Refuse joins and other row-set shaping at this boundary.
pub(super) fn projected_window_filter(
    query: &SqlQuery,
    metadata: &AnalysisMetadata<'_>,
) -> Option<(crate::window_witness::WindowWitness, String)> {
    let SetExpr::Select(select) = query.body.as_ref() else {
        return None;
    };
    let [source] = select.from.as_slice() else {
        return None;
    };
    if !source.joins.is_empty()
        || select.having.is_some()
        || select.qualify.is_some()
        || query.limit_clause.is_some()
        || query.fetch.is_some()
        || select.distinct.is_some()
        || select.top.is_some()
        || select.prewhere.is_some()
        || !select.lateral_views.is_empty()
        || select.connect_by.is_some()
        || !matches!(
            &select.group_by,
            GroupByExpr::Expressions(expressions, modifiers)
                if expressions.is_empty() && modifiers.is_empty()
        )
    {
        return None;
    }
    let nested = match &source.relation {
        TableFactor::Derived { subquery, .. } => subquery.as_ref(),
        TableFactor::Table { name, .. } => {
            let with = query.with.as_ref()?;
            let [cte] = with.cte_tables.as_slice() else {
                return None;
            };
            if cte.alias.name.value != name.to_string() {
                return None;
            }
            cte.query.as_ref()
        }
        _ => return None,
    };
    let nested_analysis = analyze_query(nested, None, metadata);
    let mut diagnostics = Vec::new();
    let scope = build_output_scope(select, &BTreeMap::new(), &[], &mut diagnostics, None);
    let predicates = analyze_select_predicates_with_scope(select, &scope, &mut diagnostics);
    let filter = predicates.where_predicate()?;
    let outer_source = SourceRelation::new(
        match &source.relation {
            TableFactor::Table { name, .. } => name.to_string(),
            TableFactor::Derived { .. } => source.relation.to_string(),
            _ => return None,
        },
        match &source.relation {
            TableFactor::Table { alias, .. } | TableFactor::Derived { alias, .. } => {
                alias.as_ref().map(|alias| alias.name.value.clone())
            }
            _ => None,
        },
    );
    crate::window_witness::analyze_projected(&nested_analysis, filter, &outer_source)
}
