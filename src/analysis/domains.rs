//! Source and local-relational value-domain derivation.

use super::*;

pub(super) type LocalDomainMap = BTreeMap<String, Vec<ColumnDomain>>;

pub(super) fn analyze_query_column_domains(
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

pub(super) fn analyze_set_expr_column_domains(
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
                &scope,
                &relations.sources,
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

pub(super) fn derive_inner_join_column_domains(
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

pub(super) fn derive_inner_join_predicate_domains(
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
        Predicate::IsNull(predicate) if matches!(predicate.expression(), Expression::Column(_)) => {
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

pub(super) fn collect_select_local_domains(
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

pub(super) fn collect_table_factor_local_domains(
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

pub(super) fn remap_local_column_domains(
    domains: Vec<ColumnDomain>,
    scope: &[OutputRelation],
    sources: &[SourceRelation],
) -> Vec<ColumnDomain> {
    domains
        .into_iter()
        .flat_map(|column_domain| {
            let scoped_column = scoped_column_ref(column_domain.column(), sources);
            if let Ok(source) = resolve_plain_source_column(&scoped_column, scope) {
                return vec![ColumnDomain::new(
                    ColumnRef::new(
                        Some(source.relation().to_string()),
                        source.column().to_string(),
                    ),
                    column_domain.domain().clone(),
                )];
            }

            let local_matches = scope
                .iter()
                .filter(|relation| {
                    scoped_column.relation().is_none_or(|qualifier| {
                        relation
                            .qualifiers
                            .iter()
                            .any(|candidate| candidate == qualifier)
                    })
                })
                .filter_map(|relation| match &relation.source {
                    OutputRelationSource::Local(output) => {
                        let matches = output
                            .columns()
                            .iter()
                            .filter(|candidate| candidate.name() == scoped_column.name())
                            .collect::<Vec<_>>();
                        match matches.as_slice() {
                            [column] => Some(Ok(*column)),
                            [] => None,
                            _ => Some(Err(())),
                        }
                    }
                    OutputRelationSource::Physical { .. } => None,
                })
                .collect::<Vec<_>>();

            let [Ok(output_column)] = local_matches.as_slice() else {
                if local_matches.is_empty() {
                    return vec![column_domain];
                }
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
            };

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

            output_column
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
                .collect()
        })
        .collect()
}

pub(super) fn scoped_column_ref(column: &ColumnRef, sources: &[SourceRelation]) -> ColumnRef {
    let Some(relation) = column.relation() else {
        return column.clone();
    };
    let candidates = sources
        .iter()
        .filter(|source| source.name() == relation)
        .collect::<Vec<_>>();

    match candidates.as_slice() {
        [source] => match source.alias() {
            Some(alias) => ColumnRef::new(Some(alias.to_string()), column.name().to_string()),
            None => column.clone(),
        },
        _ => column.clone(),
    }
}

pub(super) fn remap_predicates_for_domain_derivation(
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

pub(super) fn remap_predicate_for_domain_derivation(
    predicate: &Predicate,
    scope: &[OutputRelation],
) -> Predicate {
    match predicate {
        Predicate::LikePrefix(predicate) => Predicate::LikePrefix(LikePrefixPredicate::new(
            remap_expression_for_domain_derivation(predicate.expression(), scope),
            predicate.prefix().to_string(),
            predicate.negated(),
        )),
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

pub(super) fn remap_logical_predicate_for_domain_derivation(
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

pub(super) fn remap_expression_for_domain_derivation(
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

pub(super) fn intersect_column_domain_sets(
    sets: impl IntoIterator<Item = Vec<ColumnDomain>>,
) -> Vec<ColumnDomain> {
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

pub(super) fn merge_set_operation_domains(
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
