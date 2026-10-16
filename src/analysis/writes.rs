//! INSERT, UPDATE, DELETE and MERGE analysis.

use super::*;

pub(super) fn analyze_insert(
    insert: &SqlInsert,
    metadata: &AnalysisMetadata<'_>,
) -> ProtocolStatement {
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

fn refine_write_value_domain(
    expression: &Expression,
    domain: ValueDomain,
    column_domains: &[ColumnDomain],
    sources: &[SourceRelation],
    metadata: &AnalysisMetadata<'_>,
) -> ValueDomain {
    refine_catalog_arithmetic_domain(expression, &domain, column_domains, sources, metadata)
        .unwrap_or(domain)
}

pub(super) fn analyze_update(
    table: &TableWithJoins,
    assignments: &[sqlparser::ast::Assignment],
    has_from: bool,
    selection: Option<&Expr>,
    has_returning: bool,
    has_conflict_modifier: bool,
    metadata: &AnalysisMetadata<'_>,
) -> ProtocolStatement {
    if has_from
        || has_returning
        || has_conflict_modifier
        || !table.joins.is_empty()
        || assignments.is_empty()
    {
        return unsupported_write_statement(
            "update",
            "unsupported_update_form",
            "only single-target UPDATE SET without FROM, JOIN, RETURNING, or conflict modifiers is supported",
        );
    }
    let Some(target_source) = merge_target_relation(&table.relation) else {
        return unsupported_write_statement(
            "update",
            "unsupported_update_target",
            "UPDATE requires one unpartitioned named target relation",
        );
    };
    let target = target_source.name().to_string();
    let target_alias = target_source.alias().map(str::to_string);
    let mut diagnostics = Vec::new();
    let mut derived_index = 0;
    let mut nonlocal_dependencies = BTreeSet::new();
    if let Some(selection) = selection {
        collect_expression_dependencies(
            selection,
            &BTreeSet::new(),
            &mut diagnostics,
            &mut derived_index,
            &mut nonlocal_dependencies,
        );
    }
    for assignment in assignments {
        collect_expression_dependencies(
            &assignment.value,
            &BTreeSet::new(),
            &mut diagnostics,
            &mut derived_index,
            &mut nonlocal_dependencies,
        );
    }
    if !nonlocal_dependencies.is_empty() {
        return unsupported_write_statement(
            "update",
            "unsupported_update_subquery",
            "UPDATE subqueries require source-row and correlation semantics that cannot yet be proved",
        );
    }
    let predicate = selection.map(|selection| analyze_predicate(selection, &mut diagnostics));
    let domains = derive_column_domains(
        &Predicates::new(predicate.clone(), None, None),
        std::slice::from_ref(&target_source),
    );
    let assignments = assignments
        .iter()
        .map(|assignment| {
            let expression =
                analyze_expression_with_scope(&assignment.value, &[], &[], &mut diagnostics);
            let domain = derive_expression_domain_with_column_domains(
                &expression,
                &domains,
                std::slice::from_ref(&target_source),
            );
            let domain = refine_write_value_domain(
                &expression,
                domain,
                &domains,
                std::slice::from_ref(&target_source),
                metadata,
            );
            MergeAssignment::new(
                assignment.target.to_string(),
                WriteValue::new(expression, domain),
            )
        })
        .collect();
    sort_diagnostics(&mut diagnostics);
    ProtocolStatement::Query(
        QueryStatement::new(
            Vec::new(),
            Vec::new(),
            Vec::new(),
            RowConditions::new(
                Predicates::new(predicate.clone(), None, None),
                domains.clone(),
                ConditionExactness::from_residuals(vec![ResidualCondition::new(
                    ResidualConditionReason::AnalysisDiagnostic,
                    ConditionClause::RowSetOperator,
                    "update_requires_initial_state",
                )]),
            ),
            Output::new(Vec::new()),
            diagnostics,
        )
        .with_produced_relation(Some(target.clone()))
        .with_write(Some(WriteOperation::update(
            target,
            target_alias,
            predicate,
            assignments,
            domains,
        ))),
    )
}

pub(super) fn analyze_delete(delete: &SqlDelete) -> ProtocolStatement {
    let tables = match &delete.from {
        SqlFromTable::WithFromKeyword(tables) | SqlFromTable::WithoutKeyword(tables) => tables,
    };
    if !delete.tables.is_empty()
        || delete.using.is_some()
        || delete.returning.is_some()
        || !delete.order_by.is_empty()
        || delete.limit.is_some()
        || tables.len() != 1
        || !tables[0].joins.is_empty()
    {
        return unsupported_write_statement(
            "delete",
            "unsupported_delete_form",
            "only single-target DELETE FROM without USING, JOIN, RETURNING, ORDER BY, or LIMIT is supported",
        );
    }
    let Some(target_source) = merge_target_relation(&tables[0].relation) else {
        return unsupported_write_statement(
            "delete",
            "unsupported_delete_target",
            "DELETE requires one unpartitioned named target relation",
        );
    };
    let target = target_source.name().to_string();
    let target_alias = target_source.alias().map(str::to_string);
    let mut diagnostics = Vec::new();
    let mut derived_index = 0;
    let mut nonlocal_dependencies = BTreeSet::new();
    if let Some(selection) = &delete.selection {
        collect_expression_dependencies(
            selection,
            &BTreeSet::new(),
            &mut diagnostics,
            &mut derived_index,
            &mut nonlocal_dependencies,
        );
    }
    if !nonlocal_dependencies.is_empty() {
        return unsupported_write_statement(
            "delete",
            "unsupported_delete_subquery",
            "DELETE subqueries require source-row and correlation semantics that cannot yet be proved",
        );
    }
    let predicate = delete
        .selection
        .as_ref()
        .map(|selection| analyze_predicate(selection, &mut diagnostics));
    let domains = derive_column_domains(
        &Predicates::new(predicate.clone(), None, None),
        &[target_source],
    );
    sort_diagnostics(&mut diagnostics);
    ProtocolStatement::Query(
        QueryStatement::new(
            Vec::new(),
            Vec::new(),
            Vec::new(),
            RowConditions::new(
                Predicates::new(predicate.clone(), None, None),
                domains.clone(),
                ConditionExactness::from_residuals(vec![ResidualCondition::new(
                    ResidualConditionReason::AnalysisDiagnostic,
                    ConditionClause::RowSetOperator,
                    "delete_requires_initial_state",
                )]),
            ),
            Output::new(Vec::new()),
            diagnostics,
        )
        .with_produced_relation(Some(target.clone()))
        .with_write(Some(WriteOperation::delete(
            target,
            target_alias,
            predicate,
            domains,
        ))),
    )
}

pub(super) fn analyze_merge(
    table: &TableFactor,
    source: &TableFactor,
    on: &Expr,
    clauses: &[SqlMergeClause],
    has_output: bool,
    metadata: &AnalysisMetadata<'_>,
) -> ProtocolStatement {
    let Some(target_source) = merge_target_relation(table) else {
        return unsupported_write_statement(
            "merge",
            "unsupported_merge_target",
            "MERGE target must be an unpartitioned named relation",
        );
    };
    let target = target_source.name().to_string();
    let target_alias = target_source.alias().map(str::to_string);

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
                metadata,
            )
        })
        .collect();

    sort_diagnostics(&mut diagnostics);
    let query = QueryStatement::new(
        vec![source_relation.source],
        dependencies.into_iter().collect(),
        Vec::new(),
        RowConditions::new(
            Predicates::new(None, None, None),
            Vec::new(),
            ConditionExactness::from_residuals(vec![ResidualCondition::new(
                ResidualConditionReason::AnalysisDiagnostic,
                ConditionClause::RowSetOperator,
                "merge",
            )]),
        ),
        Output::new(Vec::new()),
        diagnostics,
    )
    .with_produced_relation(Some(target.clone()))
    .with_write(Some(WriteOperation::conditional_mutation(
        target,
        target_alias,
        match_condition,
        merge_clauses,
    )));

    ProtocolStatement::Query(query)
}

pub(super) fn merge_target_relation(table: &TableFactor) -> Option<SourceRelation> {
    match table {
        TableFactor::Table {
            name,
            alias,
            args: None,
            partitions,
            ..
        } if partitions.is_empty() => Some(SourceRelation::new(
            name.to_string(),
            alias.as_ref().map(|alias| alias.name.to_string()),
        )),
        _ => None,
    }
}

pub(super) fn analyze_merge_clause(
    clause: &SqlMergeClause,
    sources: &[SourceRelation],
    match_condition: &Predicate,
    diagnostics: &mut Vec<Diagnostic>,
    derived_index: &mut usize,
    dependencies: &mut BTreeSet<String>,
    metadata: &AnalysisMetadata<'_>,
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
    let mut branch_domains = predicate.as_ref().map_or_else(Vec::new, |predicate| {
        derive_column_domains(
            &Predicates::new(Some(predicate.clone()), None, None),
            sources,
        )
    });
    if let Some(branch_predicate) = &branch_predicate {
        branch_domains =
            refine_column_domains_from_equalities(branch_domains, branch_predicate, sources);
    }

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
                    let domain = refine_write_value_domain(
                        &expression,
                        domain,
                        &branch_domains,
                        sources,
                        metadata,
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
                                let domain = refine_write_value_domain(
                                    &expression,
                                    domain,
                                    &branch_domains,
                                    sources,
                                    metadata,
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

    ProtocolMergeClause::new(match_kind, predicate, branch_domains, action)
}

pub(super) fn merge_branch_predicate(
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
