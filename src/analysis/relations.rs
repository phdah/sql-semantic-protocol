//! CTE, local relation, table source and producer scope resolution.

use super::*;

#[derive(Clone, Default)]
pub(super) struct RelationAnalysis {
    pub(super) sources: Vec<SourceRelation>,
    pub(super) dependencies: BTreeSet<String>,
    pub(super) joins: Vec<ProtocolJoin>,
    pub(super) residual_conditions: Vec<ResidualCondition>,
}

#[derive(Clone)]
pub(super) struct LocalRelationAnalysis {
    pub(super) analysis: RelationAnalysis,
    pub(super) diagnostics: Vec<Diagnostic>,
}

pub(super) type LocalRelationAnalysisMap = BTreeMap<String, LocalRelationAnalysis>;

pub(super) struct RelationAnalysisScope<'a> {
    pub(super) local_relations: &'a BTreeSet<String>,
    pub(super) local_outputs: &'a LocalOutputMap,
    pub(super) local_analyses: &'a LocalRelationAnalysisMap,
    pub(super) output_scope: &'a [OutputRelation],
}

pub(super) struct AnalyzedRelation {
    pub(super) source: SourceRelation,
    pub(super) reference: RelationRef,
    pub(super) dependencies: BTreeSet<String>,
    pub(super) joins: Vec<ProtocolJoin>,
    pub(super) residual_conditions: Vec<ResidualCondition>,
}

pub(super) fn diagnose_local_query_predicates(
    query: &SqlQuery,
    inherited_local_outputs: &LocalOutputMap,
    diagnostics: &mut Vec<Diagnostic>,
    metadata: Option<&AnalysisMetadata<'_>>,
) {
    let mut local_outputs = inherited_local_outputs.clone();

    if let Some(with) = &query.with {
        for cte in &with.cte_tables {
            let mut output_diagnostics = Vec::new();
            let output = analyze_query_output(
                &cte.query,
                &local_outputs,
                &mut output_diagnostics,
                metadata,
            );
            local_outputs.insert(
                cte.alias.name.to_string(),
                apply_output_column_aliases(output, &cte.alias),
            );
        }
    }

    diagnose_local_set_expr_predicates(query.body.as_ref(), &local_outputs, diagnostics, metadata);
}

pub(super) fn diagnose_local_set_expr_predicates(
    expression: &SetExpr,
    local_outputs: &LocalOutputMap,
    diagnostics: &mut Vec<Diagnostic>,
    metadata: Option<&AnalysisMetadata<'_>>,
) {
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
            extend_unique_diagnostics(diagnostics, &predicate_diagnostics);
            diagnose_uncarried_local_predicates(&predicates, &scope, diagnostics);
        }
        SetExpr::Query(query) => {
            diagnose_local_query_predicates(query, local_outputs, diagnostics, metadata);
        }
        SetExpr::SetOperation { left, right, .. } => {
            diagnose_local_set_expr_predicates(left, local_outputs, diagnostics, metadata);
            diagnose_local_set_expr_predicates(right, local_outputs, diagnostics, metadata);
        }
        SetExpr::Values(_)
        | SetExpr::Insert(_)
        | SetExpr::Update(_)
        | SetExpr::Delete(_)
        | SetExpr::Table(_) => {}
    }
}

pub(super) fn diagnose_uncarried_local_predicates(
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

pub(super) fn uncarried_local_predicate_reason(
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
        Predicate::Or(logical) => {
            if let Some(reason) = logical
                .operands()
                .iter()
                .find_map(|predicate| uncarried_local_predicate_reason(predicate, scope))
            {
                return Some(reason);
            }

            let mut columns = BTreeSet::new();
            for operand in logical.operands() {
                collect_local_predicate_source_columns(operand, scope, &mut columns);
            }
            (columns.len() > 1).then(|| {
                "logical OR spans multiple physical source columns and cannot be preserved as independent domains"
                    .to_string()
            })
        }
        Predicate::LikePrefix(_) => Some(
            "LIKE prefix needs declared collation and cannot be reduced to independent domains"
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
                _ => Some(
                    "IN-list predicate targets a computed or unresolved expression".to_string(),
                ),
            }
        }
        Predicate::In(_) => {
            Some("IN-list predicate contains values that are not scalar literals".to_string())
        }
        Predicate::Exists(_) => {
            Some("EXISTS semantics depend on nested-row existence, not a scalar domain".to_string())
        }
        Predicate::QuantifiedSubquery(_) => Some(
            "quantified comparison depends on nested physical rows".to_string(),
        ),
        Predicate::InSubquery(_) => Some(
            "IN-subquery semantics depend on nested rows, not only a scalar domain".to_string(),
        ),
        Predicate::Between(predicate)
            if matches!(predicate.lower(), Expression::Literal(_))
                && matches!(predicate.upper(), Expression::Literal(_)) =>
        {
            match predicate.expression() {
                Expression::Column(column) => unresolved_domain_column_reason(column, scope),
                _ => Some(
                    "BETWEEN predicate targets a computed or unresolved expression".to_string(),
                ),
            }
        }
        Predicate::Between(_) => {
            Some("BETWEEN predicate bounds are not both scalar literals".to_string())
        }
        Predicate::BooleanExpression(_) => Some(
            "boolean predicate expression cannot be reduced safely to a scalar domain".to_string(),
        ),
        Predicate::Unknown(semantic) => Some(format!(
            "predicate semantics are unresolved: {}",
            semantic.reason()
        )),
        Predicate::Unsupported(semantic) => Some(match semantic.reason() {
            Some(reason) => format!(
                "unsupported {} predicate semantics: {reason}",
                semantic.feature()
            ),
            None => format!("unsupported {} predicate semantics", semantic.feature()),
        }),
    }
}

pub(super) fn collect_local_predicate_source_columns(
    predicate: &Predicate,
    scope: &[OutputRelation],
    columns: &mut BTreeSet<LineageSource>,
) {
    match predicate {
        Predicate::LikePrefix(predicate) => {
            collect_local_expression_source_column(predicate.expression(), scope, columns);
        }
        Predicate::Comparison(comparison) => {
            collect_local_expression_source_column(comparison.left(), scope, columns);
            collect_local_expression_source_column(comparison.right(), scope, columns);
        }
        Predicate::And(logical) | Predicate::Or(logical) => {
            for operand in logical.operands() {
                collect_local_predicate_source_columns(operand, scope, columns);
            }
        }
        Predicate::Not(not) => {
            collect_local_predicate_source_columns(not.operand(), scope, columns);
        }
        Predicate::IsNull(predicate) => {
            collect_local_expression_source_column(predicate.expression(), scope, columns);
        }
        Predicate::In(predicate) => {
            collect_local_expression_source_column(predicate.expression(), scope, columns);
        }
        Predicate::InSubquery(predicate) => {
            for expression in predicate.operand().expressions() {
                collect_local_expression_source_column(expression, scope, columns);
            }
        }
        Predicate::QuantifiedSubquery(predicate) => {
            collect_local_expression_source_column(predicate.operand(), scope, columns);
        }
        Predicate::Between(predicate) => {
            collect_local_expression_source_column(predicate.expression(), scope, columns);
        }
        Predicate::BooleanExpression(expression) => {
            collect_local_expression_source_column(expression, scope, columns);
        }
        Predicate::Exists(_) | Predicate::Unknown(_) | Predicate::Unsupported(_) => {}
    }
}

pub(super) fn collect_local_expression_source_column(
    expression: &Expression,
    scope: &[OutputRelation],
    columns: &mut BTreeSet<LineageSource>,
) {
    let Expression::Column(column) = expression else {
        return;
    };
    let reference = ColumnRef::new(
        column.relation().map(ToString::to_string),
        column.name().to_string(),
    );
    if let Ok(source) = resolve_plain_source_column(&reference, scope) {
        columns.insert(source);
    }
}

pub(super) fn unresolved_domain_column_reason(
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

pub(super) fn analyze_local_query_condition_exactness(
    query: &SqlQuery,
    local_outputs: &LocalOutputMap,
    relation_analysis: &RelationAnalysis,
    metadata: Option<&AnalysisMetadata<'_>>,
) -> ConditionExactness {
    let mut diagnostics = Vec::new();
    let predicates = analyze_query_predicates_with_local_outputs(
        query,
        local_outputs,
        &mut diagnostics,
        metadata,
    );
    diagnose_local_set_expr_predicates(
        query.body.as_ref(),
        local_outputs,
        &mut diagnostics,
        metadata,
    );
    if matches!(query.body.as_ref(), SetExpr::SetOperation { .. }) {
        inspect_set_expr_features(query.body.as_ref(), &mut diagnostics);
    }
    inspect_query_features(query, None, &mut diagnostics);

    let missing_columns = metadata.map_or_else(Vec::new, |metadata| {
        let output = analyze_query_output(query, local_outputs, &mut diagnostics, Some(metadata));
        validate_schema_column_references(
            query,
            &predicates,
            &output,
            &relation_analysis.joins,
            &relation_analysis.sources,
            metadata,
            &mut diagnostics,
        )
    });

    analyze_query_condition_exactness(
        query,
        &predicates,
        &relation_analysis.sources,
        &relation_analysis.dependencies,
        &relation_analysis.joins,
        &diagnostics,
        false,
    )
    .merged_with(&ConditionExactness::from_residuals(missing_columns))
}

pub(super) fn analyze_query_predicates_with_local_outputs(
    query: &SqlQuery,
    local_outputs: &LocalOutputMap,
    diagnostics: &mut Vec<Diagnostic>,
    metadata: Option<&AnalysisMetadata<'_>>,
) -> Predicates {
    match query.body.as_ref() {
        SetExpr::Select(select) => {
            let mut scope_diagnostics = Vec::new();
            let scope =
                build_output_scope(select, local_outputs, &[], &mut scope_diagnostics, metadata);
            extend_unique_diagnostics(diagnostics, &scope_diagnostics);
            analyze_select_predicates_with_scope(select, &scope, diagnostics)
        }
        SetExpr::Query(query) => {
            analyze_query_predicates_with_local_outputs(query, local_outputs, diagnostics, metadata)
        }
        SetExpr::SetOperation { .. } => Predicates::new(None, None, None),
        _ => Predicates::new(None, None, None),
    }
}

pub(super) fn analyze_query_relations(
    query: &SqlQuery,
    inherited_local_relations: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
    derived_index: &mut usize,
    metadata: Option<&AnalysisMetadata<'_>>,
) -> RelationAnalysis {
    analyze_query_relations_with_locals(
        query,
        inherited_local_relations,
        &BTreeMap::new(),
        &BTreeMap::new(),
        diagnostics,
        derived_index,
        metadata,
    )
}

pub(super) fn analyze_query_relations_with_locals(
    query: &SqlQuery,
    inherited_local_relations: &BTreeSet<String>,
    inherited_local_outputs: &LocalOutputMap,
    inherited_local_analyses: &LocalRelationAnalysisMap,
    diagnostics: &mut Vec<Diagnostic>,
    derived_index: &mut usize,
    metadata: Option<&AnalysisMetadata<'_>>,
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
            let mut nested = analyze_query_relations_with_locals(
                &cte.query,
                &local_relations,
                &local_outputs,
                &local_analyses,
                &mut cte_diagnostics,
                derived_index,
                metadata,
            );
            diagnose_local_query_predicates(
                &cte.query,
                &local_outputs,
                &mut cte_diagnostics,
                metadata,
            );
            let local_exactness = analyze_local_query_condition_exactness(
                &cte.query,
                &local_outputs,
                &nested,
                metadata,
            )
            .with_scope(format!("cte:{name}"));
            nested
                .residual_conditions
                .extend(local_exactness.residual_conditions().iter().cloned());
            let mut output_diagnostics = Vec::new();
            let output = analyze_query_output(
                &cte.query,
                &local_outputs,
                &mut output_diagnostics,
                metadata,
            );
            extend_unique_diagnostics(&mut cte_diagnostics, &output_diagnostics);
            local_outputs.insert(
                name.clone(),
                apply_output_column_aliases(output, &cte.alias),
            );
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
        metadata,
    )
}

pub(super) fn analyze_set_expr_relations_with_locals(
    expression: &SetExpr,
    local_relations: &BTreeSet<String>,
    local_outputs: &LocalOutputMap,
    local_analyses: &LocalRelationAnalysisMap,
    diagnostics: &mut Vec<Diagnostic>,
    derived_index: &mut usize,
    metadata: Option<&AnalysisMetadata<'_>>,
) -> RelationAnalysis {
    match expression {
        SetExpr::Select(select) => analyze_select_relations_with_locals(
            select,
            local_relations,
            local_outputs,
            local_analyses,
            diagnostics,
            derived_index,
            metadata,
        ),
        SetExpr::Query(query) => analyze_query_relations_with_locals(
            query,
            local_relations,
            local_outputs,
            local_analyses,
            diagnostics,
            derived_index,
            metadata,
        ),
        SetExpr::SetOperation { left, right, .. } => {
            let mut analysis = analyze_set_expr_relations_with_locals(
                left,
                local_relations,
                local_outputs,
                local_analyses,
                diagnostics,
                derived_index,
                metadata,
            );
            let right = analyze_set_expr_relations_with_locals(
                right,
                local_relations,
                local_outputs,
                local_analyses,
                diagnostics,
                derived_index,
                metadata,
            );
            merge_relation_analysis(&mut analysis, right);
            analysis
        }
        _ => RelationAnalysis::default(),
    }
}

pub(super) fn analyze_select_relations(
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
        None,
    )
}

pub(super) fn analyze_select_relations_with_locals(
    select: &Select,
    local_relations: &BTreeSet<String>,
    local_outputs: &LocalOutputMap,
    local_analyses: &LocalRelationAnalysisMap,
    diagnostics: &mut Vec<Diagnostic>,
    derived_index: &mut usize,
    metadata: Option<&AnalysisMetadata<'_>>,
) -> RelationAnalysis {
    let mut analysis = RelationAnalysis::default();
    let mut scope_diagnostics = Vec::new();
    let output_scope =
        build_output_scope(select, local_outputs, &[], &mut scope_diagnostics, metadata);
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
            metadata,
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

    if let Some(selection) = &select.selection {
        let mut ignored_diagnostics = Vec::new();
        let predicate = analyze_predicate_with_windows(
            selection,
            &select.named_window,
            &BTreeMap::new(),
            &output_scope,
            &mut ignored_diagnostics,
        );
        analysis.joins.extend(implicit_where_equality_joins(
            &predicate,
            &analysis.sources,
            &output_scope,
        ));
    }

    analysis
}

pub(super) fn implicit_where_equality_joins(
    predicate: &Predicate,
    sources: &[SourceRelation],
    scope: &[OutputRelation],
) -> Vec<ProtocolJoin> {
    let mut comparisons = Vec::new();
    collect_conjunctive_equality_comparisons(predicate, &mut comparisons);

    comparisons
        .into_iter()
        .filter_map(|comparison| {
            let (Expression::Column(left), Expression::Column(right)) =
                (comparison.left(), comparison.right())
            else {
                return None;
            };
            let (left_index, left_source) = source_for_column_expression(left, sources)?;
            let (right_index, right_source) = source_for_column_expression(right, sources)?;
            if left_index == right_index {
                return None;
            }

            let mut ignored_diagnostics = Vec::new();
            let condition = remap_join_equality_columns(
                Predicate::Comparison(comparison.clone()),
                scope,
                &mut ignored_diagnostics,
            );

            Some(ProtocolJoin::new(
                JoinKind::Inner,
                RelationRef::new(
                    left_source.name().to_string(),
                    left_source.alias().map(str::to_string),
                ),
                RelationRef::new(
                    right_source.name().to_string(),
                    right_source.alias().map(str::to_string),
                ),
                Some(condition),
            ))
        })
        .collect()
}

pub(super) fn collect_conjunctive_equality_comparisons<'a>(
    predicate: &'a Predicate,
    comparisons: &mut Vec<&'a ComparisonPredicate>,
) {
    match predicate {
        Predicate::Comparison(comparison) if comparison.operator() == ComparisonOperator::Eq => {
            comparisons.push(comparison);
        }
        Predicate::And(logical) => {
            for operand in logical.operands() {
                collect_conjunctive_equality_comparisons(operand, comparisons);
            }
        }
        _ => {}
    }
}

pub(super) fn source_for_column_expression<'a>(
    column: &ColumnExpression,
    sources: &'a [SourceRelation],
) -> Option<(usize, &'a SourceRelation)> {
    match column.relation() {
        Some(qualifier) => {
            let matches = sources
                .iter()
                .enumerate()
                .filter(|(_, source)| {
                    source.alias() == Some(qualifier)
                        || source.name() == qualifier
                        || (source.alias().is_none()
                            && source.name().rsplit('.').next() == Some(qualifier))
                })
                .collect::<Vec<_>>();
            let [source] = matches.as_slice() else {
                return None;
            };
            Some(*source)
        }
        None => match sources {
            [source] => Some((0, source)),
            _ => None,
        },
    }
}

pub(super) fn collect_grouping_dependencies(
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

pub(super) fn analyze_table_with_joins(
    source: &TableWithJoins,
    scope: &RelationAnalysisScope<'_>,
    diagnostics: &mut Vec<Diagnostic>,
    derived_index: &mut usize,
    analysis: &mut RelationAnalysis,
    metadata: Option<&AnalysisMetadata<'_>>,
) {
    let mut left = register_table_factor(
        &source.relation,
        scope,
        diagnostics,
        derived_index,
        analysis,
        metadata,
    );

    for join in &source.joins {
        let right = register_table_factor(
            &join.relation,
            scope,
            diagnostics,
            derived_index,
            analysis,
            metadata,
        );

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

pub(super) fn register_table_factor(
    factor: &TableFactor,
    scope: &RelationAnalysisScope<'_>,
    diagnostics: &mut Vec<Diagnostic>,
    derived_index: &mut usize,
    analysis: &mut RelationAnalysis,
    metadata: Option<&AnalysisMetadata<'_>>,
) -> Option<RelationRef> {
    let relation = analyze_table_factor_with_locals(
        factor,
        scope.local_relations,
        scope.local_outputs,
        scope.local_analyses,
        diagnostics,
        derived_index,
        metadata,
    )?;

    for dependency in relation.dependencies {
        analysis.dependencies.insert(dependency);
    }
    analysis.joins.extend(relation.joins);
    analysis
        .residual_conditions
        .extend(relation.residual_conditions);

    if !analysis.sources.iter().any(|existing| {
        existing.name() == relation.source.name() && existing.alias() == relation.source.alias()
    }) {
        analysis.sources.push(relation.source);
    }

    Some(relation.reference)
}

pub(super) fn analyze_table_factor(
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
        None,
    )
}

pub(super) fn analyze_table_factor_with_locals(
    factor: &TableFactor,
    local_relations: &BTreeSet<String>,
    local_outputs: &LocalOutputMap,
    local_analyses: &LocalRelationAnalysisMap,
    diagnostics: &mut Vec<Diagnostic>,
    derived_index: &mut usize,
    metadata: Option<&AnalysisMetadata<'_>>,
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
            let (dependencies, joins, residual_conditions) = if local_relations.contains(&name) {
                match local_analyses.get(&name) {
                    Some(local) => {
                        extend_unique_diagnostics(diagnostics, &local.diagnostics);
                        (
                            local.analysis.dependencies.clone(),
                            local.analysis.joins.clone(),
                            local.analysis.residual_conditions.clone(),
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
                        (BTreeSet::new(), Vec::new(), Vec::new())
                    }
                }
            } else {
                (BTreeSet::from([name.clone()]), Vec::new(), Vec::new())
            };

            Some(AnalyzedRelation {
                source: SourceRelation::new(name.clone(), alias.clone()),
                reference: RelationRef::new(name, alias),
                dependencies,
                joins,
                residual_conditions,
            })
        }
        TableFactor::Derived {
            subquery,
            alias,
            lateral,
            ..
        } => {
            // A lateral producer is evaluated for each row of the preceding
            // sources. Ordinary derived-table scopes cannot prove that bag;
            // retain its dependencies but never certify it as independent.
            if *lateral {
                diagnostics.push(warning(
                    "unsupported_lateral_row_scope",
                    DiagnosticArea::Source,
                    "LATERAL subquery needs per-left-row correlated population and complete output/absence proof",
                ));
            }
            let mut nested = analyze_query_relations_with_locals(
                subquery,
                local_relations,
                local_outputs,
                local_analyses,
                diagnostics,
                derived_index,
                metadata,
            );
            diagnose_local_query_predicates(subquery, local_outputs, diagnostics, metadata);
            let alias = alias.as_ref().map(|alias| alias.name.to_string());
            let name = match &alias {
                Some(_) => "subquery".to_string(),
                None => {
                    *derived_index += 1;
                    format!("subquery#{}", derived_index)
                }
            };
            let scope = alias.as_ref().map_or_else(
                || format!("derived:{name}"),
                |alias| format!("derived:{alias}"),
            );
            let local_exactness =
                analyze_local_query_condition_exactness(subquery, local_outputs, &nested, metadata)
                    .with_scope(scope);
            nested
                .residual_conditions
                .extend(local_exactness.residual_conditions().iter().cloned());

            Some(AnalyzedRelation {
                source: SourceRelation::new(name.clone(), alias.clone()),
                reference: RelationRef::new(name, alias),
                dependencies: nested.dependencies,
                joins: nested.joins,
                residual_conditions: nested.residual_conditions,
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

pub(super) fn extend_unique_diagnostics(target: &mut Vec<Diagnostic>, source: &[Diagnostic]) {
    for diagnostic in source {
        if !target.contains(diagnostic) {
            target.push(diagnostic.clone());
        }
    }
}
