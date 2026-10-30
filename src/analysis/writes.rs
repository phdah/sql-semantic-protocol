//! INSERT, UPDATE, DELETE and MERGE analysis.

use super::*;
use crate::protocol::{InsertConflictPolicy, InsertCteSource, JoinedMutationCteSource};

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

    // Conflict actions are distinct laws, not aliases for ordinary append.
    // Unnamed conflict targets, DO UPDATE, REPLACE and IGNORE require native
    // error/enforcement semantics that the parser cannot certify.
    if insert.overwrite {
        return unsupported_write_statement(
            "insert",
            "unproved_insert_overwrite_scope",
            "INSERT OVERWRITE needs attested partition and replacement scope (TASK-147)",
        );
    }
    if insert.replace_into || insert.ignore || insert.or.is_some() {
        return unsupported_write_statement(
            "insert",
            "unproved_insert_vendor_conflict_action",
            "INSERT IGNORE/REPLACE needs native constraint and error-handling laws (TASK-147)",
        );
    }
    let conflict_policy = match insert.on.as_ref() {
        None => None,
        Some(sqlparser::ast::OnInsert::OnConflict(conflict)) => {
            match (&conflict.conflict_target, &conflict.action) {
                (
                    Some(sqlparser::ast::ConflictTarget::Columns(columns)),
                    sqlparser::ast::OnConflictAction::DoNothing,
                ) if !columns.is_empty()
                    && columns
                        .iter()
                        .map(|column| &column.value)
                        .collect::<BTreeSet<_>>()
                        .len()
                        == columns.len() =>
                {
                    Some(InsertConflictPolicy::do_nothing(
                        columns.iter().map(|column| column.value.clone()).collect(),
                    ))
                }
                _ => {
                    return unsupported_write_statement(
                        "insert",
                        "unproved_insert_upsert_action",
                        "ON CONFLICT needs a named unique key and DO NOTHING; updates and inferred targets remain residual (TASK-147)",
                    );
                }
            }
        }
        Some(sqlparser::ast::OnInsert::DuplicateKeyUpdate(_)) => {
            return unsupported_write_statement(
                "insert",
                "unproved_insert_duplicate_key_update",
                "ON DUPLICATE KEY UPDATE needs native conflict and assignment laws (TASK-147)",
            );
        }
        Some(_) => {
            return unsupported_write_statement(
                "insert",
                "unproved_insert_vendor_conflict_action",
                "unrecognized INSERT conflict semantics cannot be certified (TASK-147)",
            );
        }
    };
    let returning_columns = match insert.returning.as_ref() {
        None => Vec::new(),
        Some(items) if !items.is_empty() => {
            let mut columns = Vec::new();
            for item in items {
                match item {
                    SelectItem::UnnamedExpr(Expr::Identifier(column)) => {
                        columns.push(column.value.clone());
                    }
                    _ => {
                        return unsupported_write_statement(
                            "insert",
                            "unproved_insert_returning_projection",
                            "RETURNING requires direct unqualified target columns (TASK-147)",
                        );
                    }
                }
            }
            if columns.iter().collect::<BTreeSet<_>>().len() != columns.len() {
                return unsupported_write_statement(
                    "insert",
                    "unproved_insert_returning_projection",
                    "duplicate RETURNING output names require positional identity (TASK-147)",
                );
            }
            columns
        }
        Some(_) => {
            return unsupported_write_statement(
                "insert",
                "unproved_insert_returning_projection",
                "empty RETURNING projection has no canonical row shape (TASK-147)",
            );
        }
    };
    if !insert.assignments.is_empty()
        || insert.partitioned.is_some()
        || !insert.after_columns.is_empty()
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
    let Some(source) = &insert.source else {
        if conflict_policy.is_some() || !returning_columns.is_empty() {
            return unsupported_write_statement(
                "insert",
                "unproved_insert_default_modifiers",
                "DEFAULT VALUES conflict handling and RETURNING require separate proof (TASK-147)",
            );
        }
        // sqlparser represents DEFAULT VALUES with no source after parsing.
        if !insert.columns.is_empty() {
            return unsupported_write_statement(
                "insert",
                "unsupported_insert_source",
                "DEFAULT VALUES cannot specify a target column list",
            );
        }
        return ProtocolStatement::Query(
            QueryStatement::new(
                Vec::new(),
                Vec::new(),
                Vec::new(),
                RowConditions::new(
                    Predicates::new(None, None, None),
                    Vec::new(),
                    ConditionExactness::from_residuals(Vec::new()),
                ),
                Output::new(Vec::new()),
                Vec::new(),
            )
            .with_produced_relation(Some(target.clone()))
            .with_write(Some(WriteOperation::append_default_values(target))),
        );
    };
    if !matches!(
        source.body.as_ref(),
        SetExpr::Select(_) | SetExpr::Query(_) | SetExpr::SetOperation { .. } | SetExpr::Values(_)
    ) {
        return unsupported_write_statement(
            "insert",
            "unsupported_insert_source",
            "only INSERT SELECT and literal VALUES source semantics are supported",
        );
    }
    if source.order_by.is_some() {
        return unsupported_write_statement(
            "insert",
            "unsupported_insert_source_order",
            "INSERT SELECT source ordering requires an attested execution-order law",
        );
    }
    // A single, nonrecursive CTE has one independently normalized producer
    // and one consumer. Neither stage may borrow an invented physical bag.
    // Other local scopes remain unsupported until their source-row laws
    // are explicitly represented in the canonical protocol.
    let insert_cte = if let Some(with) = &source.with {
        let [cte] = with.cte_tables.as_slice() else {
            return unsupported_write_statement(
                "insert",
                "unsupported_insert_cte",
                "INSERT CTE requires exactly one independently proved producer",
            );
        };
        if with.recursive || !cte.alias.columns.is_empty() || cte.query.order_by.is_some() {
            return unsupported_write_statement(
                "insert",
                "unsupported_insert_cte",
                "recursive and column-renamed CTE producers require additional scope evidence",
            );
        }
        let cte_name = cte.alias.name.to_string();
        let SetExpr::Select(outer) = source.body.as_ref() else {
            return unsupported_write_statement(
                "insert",
                "unsupported_insert_cte",
                "INSERT CTE consumer requires a direct SELECT",
            );
        };
        let [from] = outer.from.as_slice() else {
            return unsupported_write_statement(
                "insert",
                "unsupported_insert_cte",
                "INSERT CTE consumer must read exactly one local relation",
            );
        };
        if !from.joins.is_empty()
            || !matches!(&from.relation, TableFactor::Table { name, args: None, sample: None, .. }
                if name.to_string() == cte_name)
        {
            return unsupported_write_statement(
                "insert",
                "unsupported_insert_cte",
                "INSERT CTE consumer must refer only to its local producer",
            );
        }
        let producer = analyze_query(&cte.query, None, metadata);
        let mut outer_query = source.as_ref().clone();
        outer_query.with = None;
        let consumer = analyze_query(&outer_query, None, metadata);
        let proven = |query: &QueryStatement| {
            (query.row_preserving_projection() || query.filter_only_row_shape())
                && query.sources().len() == 1
                && query.set_operation().is_none()
                && query.aggregation().is_none()
                && query.joins().is_empty()
        };
        if !proven(&producer)
            || !proven(&consumer)
            || consumer
                .sources()
                .first()
                .is_none_or(|source| source.name() != cte_name)
        {
            return unsupported_write_statement(
                "insert",
                "unsupported_insert_cte",
                "INSERT CTE has unproved producer or consumer row-shaping",
            );
        }
        Some(InsertCteSource::new(cte_name, producer, consumer))
    } else {
        None
    };
    let query = analyze_query(source, Some(target.clone()), metadata);
    if matches!(source.body.as_ref(), SetExpr::Values(_)) && query.finite_table().is_none() {
        return unsupported_write_statement(
            "insert",
            "unsupported_insert_values",
            "VALUES has an expression without a provable finite literal value",
        );
    }
    let mut write =
        WriteOperation::append(target, target_columns).with_returning_columns(returning_columns);
    if let Some(conflict) = conflict_policy {
        write = write.with_insert_conflict(conflict);
    }
    if let Some(cte) = insert_cte {
        write = write.with_insert_cte_source(cte);
    }
    ProtocolStatement::Query(query.with_write(Some(write)))
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

fn normalize_mutation_returning(
    returning: Option<&[SelectItem]>,
    target: &str,
    target_alias: Option<&str>,
) -> Result<Vec<String>, &'static str> {
    let Some(items) = returning else {
        return Ok(Vec::new());
    };
    if items.is_empty() {
        return Err("RETURNING requires at least one direct target column");
    }
    let mut columns = Vec::new();
    let mut seen = BTreeSet::new();
    for item in items {
        // A target-qualified column and its unqualified form denote the same
        // canonical target column. Other qualifiers must not be silently
        // interpreted as references to this target's physical row.
        let column = match item {
            SelectItem::UnnamedExpr(Expr::Identifier(identifier)) => &identifier.value,
            SelectItem::UnnamedExpr(Expr::CompoundIdentifier(path)) if path.len() >= 2 => {
                let (qualifier, column) = path.split_at(path.len() - 1);
                let qualifier = qualifier
                    .iter()
                    .map(|part| part.value.as_str())
                    .collect::<Vec<_>>()
                    .join(".");
                if qualifier != target_alias.unwrap_or(target) {
                    return Err("RETURNING qualifier does not identify the mutation target");
                }
                &column[0].value
            }
            _ => {
                return Err(
                    "RETURNING requires direct target columns with no expression or wildcard",
                )
            }
        };
        if !seen.insert(column.clone()) {
            return Err("duplicate RETURNING column identities require positional evidence");
        }
        columns.push(column.clone());
    }
    Ok(columns)
}

/// Model a single read-only, nonrecursive CTE feeding UPDATE FROM or DELETE
/// USING. SQLParser wraps WITH DML in SetExpr::{Update,Delete}; accepting it as
/// an ordinary read-only Query would silently omit a state-changing statement.
pub(super) fn analyze_joined_cte_mutation(
    query: &SqlQuery,
    metadata: &AnalysisMetadata<'_>,
) -> ProtocolStatement {
    let Some(with) = &query.with else {
        return unsupported_write_statement(
            "query",
            "unproved_nested_dml",
            "nested DML without a local CTE cannot be certified as a write",
        );
    };
    let [cte] = with.cte_tables.as_slice() else {
        return unsupported_write_statement(
            "query",
            "unproved_joined_cte_shape",
            "joined mutation requires exactly one read-only CTE producer",
        );
    };
    if with.recursive
        || !cte.alias.columns.is_empty()
        || cte.query.with.is_some()
        || cte.query.order_by.is_some()
        || cte.query.limit_clause.is_some()
        || cte.query.fetch.is_some()
        || query.order_by.is_some()
        || query.limit_clause.is_some()
        || query.fetch.is_some()
        || !query.pipe_operators.is_empty()
    {
        return unsupported_write_statement(
            "query",
            "unproved_joined_cte_scope",
            "recursive, reordered or nested CTE write requires additional producer evidence",
        );
    }
    let name = cte.alias.name.to_string();
    let statement = match query.body.as_ref() {
        SetExpr::Update(statement) | SetExpr::Delete(statement) => statement,
        _ => {
            return unsupported_write_statement(
                "query",
                "unproved_joined_cte_action",
                "CTE write must be an UPDATE or DELETE",
            );
        }
    };
    let ProtocolStatement::Query(mutation) = analyze_statement(statement, metadata) else {
        return unsupported_write_statement(
            "query",
            "unproved_joined_cte_mutation",
            "CTE consumer must be an independently normalized joined mutation",
        );
    };
    let Some(write) = mutation.write() else {
        return unsupported_write_statement(
            "query",
            "unproved_joined_cte_action",
            "CTE query must contain an actual UPDATE or DELETE",
        );
    };
    if write
        .joined_source()
        .is_none_or(|source| source.name() != name)
        || mutation.sources().len() != 1
    {
        return unsupported_write_statement(
            "query",
            "unproved_joined_cte_consumer",
            "joined mutation must consume its one named local CTE source",
        );
    }
    let producer = analyze_query(&cte.query, None, metadata);
    if !matches!(cte.query.body.as_ref(), SetExpr::Select(_))
        || producer.sources().len() != 1
        || producer.sources()[0].name() == name
        || producer.sources()[0].name() == write.target()
        || !producer.joins().is_empty()
        || producer.set_operation().is_some()
        || producer.aggregation().is_some()
        || !(producer.row_preserving_projection() || producer.filter_only_row_shape())
        || producer.output().columns().is_empty()
        || producer
            .output()
            .columns()
            .iter()
            .map(|col| col.name())
            .collect::<BTreeSet<_>>()
            .len()
            != producer.output().columns().len()
    {
        return unsupported_write_statement(
            "query",
            "unproved_joined_cte_producer",
            "CTE producer requires one row-local, uniquely projected complete physical source",
        );
    }
    let physical_dependencies = producer.dependencies().to_vec();
    let write = write
        .clone()
        .with_joined_cte_source(JoinedMutationCteSource::new(name, producer));
    ProtocolStatement::Query(
        mutation
            .with_physical_dependencies(physical_dependencies)
            .with_write(Some(write)),
    )
}

/// Extract one named physical source; do not flatten unrelated JOIN inputs.
fn named_mutation_source(tables: &[TableWithJoins]) -> Option<SourceRelation> {
    let [source] = tables else {
        return None;
    };
    if !source.joins.is_empty() {
        return None;
    }
    merge_target_relation(&source.relation)
}

/// Shared target/source identities must be proved as a self-join in separate scope.
fn disjoint_mutation_sources(target: &SourceRelation, source: &SourceRelation) -> bool {
    let target_names = [target.name(), target.alias().unwrap_or(target.name())];
    let source_names = [source.name(), source.alias().unwrap_or(source.name())];
    !target_names.iter().any(|name| source_names.contains(name))
}

/// A bounded correlated EXISTS/NOT EXISTS reads one complete, independent
/// physical relation. This does not translate the subquery into UPDATE FROM:
/// each target row evaluates the nested WHERE against the same source bag.
fn bounded_mutation_exists(
    selection: &Expr,
    target: &SourceRelation,
    metadata: &AnalysisMetadata<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<(Predicate, SourceRelation)> {
    if !matches!(selection, Expr::Exists { .. }) {
        return None;
    }
    let active_qualifier = target.alias().unwrap_or(target.name());
    let scope = [OutputRelation {
        qualifiers: vec![active_qualifier.to_string()],
        source: OutputRelationSource::Physical {
            relation: target.name().to_string(),
            columns: metadata.schema_columns(target.name()),
        },
    }];
    let predicate =
        analyze_predicate_with_windows(selection, &[], &BTreeMap::new(), &scope, diagnostics);
    let Predicate::Exists(exists) = &predicate else {
        return None;
    };
    let nested = exists.subquery();
    let [source] = nested.sources() else {
        return None;
    };
    if !nested.row_shape_preserves_candidates()
        || !nested.joins().is_empty()
        || !nested.diagnostics().is_empty()
        || nested.predicates().where_predicate().is_none()
        || nested.correlations().is_empty()
        || nested
            .correlations()
            .iter()
            .any(|column| column.relation() != target.name())
        || !disjoint_mutation_sources(target, source)
    {
        return None;
    }
    let independent_source = source.clone();
    Some((predicate, independent_source))
}

pub(super) fn analyze_update(
    table: &TableWithJoins,
    assignments: &[sqlparser::ast::Assignment],
    from: Option<&sqlparser::ast::UpdateTableFromKind>,
    selection: Option<&Expr>,
    returning: Option<&[SelectItem]>,
    has_conflict_modifier: bool,
    metadata: &AnalysisMetadata<'_>,
) -> ProtocolStatement {
    if has_conflict_modifier || !table.joins.is_empty() || assignments.is_empty() {
        return unsupported_write_statement(
            "update",
            "unsupported_update_form",
            "UPDATE requires one named target, assignments, and no target JOIN or conflict modifier",
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
    let joined_source = match from {
        None => None,
        Some(sqlparser::ast::UpdateTableFromKind::AfterSet(tables))
        | Some(sqlparser::ast::UpdateTableFromKind::BeforeSet(tables)) => {
            named_mutation_source(tables)
        }
    };
    if from.is_some() && joined_source.is_none() {
        return unsupported_write_statement(
            "update",
            "unsupported_update_from",
            "UPDATE FROM requires exactly one named independent source without joins",
        );
    }
    if joined_source
        .as_ref()
        .is_some_and(|source| !disjoint_mutation_sources(&target_source, source))
    {
        return unsupported_write_statement(
            "update",
            "unproved_update_self_source",
            "UPDATE FROM cannot reuse or shadow the physical target without a joint alias proof",
        );
    }
    let mut diagnostics = Vec::new();
    let correlated = if from.is_none() {
        selection.and_then(|selection| {
            bounded_mutation_exists(selection, &target_source, metadata, &mut diagnostics)
        })
    } else {
        None
    };
    let mut domain_sources = vec![target_source.clone()];
    domain_sources.extend(joined_source.iter().cloned());
    let query_sources: Vec<SourceRelation> = joined_source
        .iter()
        .cloned()
        .chain(correlated.as_ref().map(|(_, source)| source.clone()))
        .collect();
    let dependencies = query_sources
        .iter()
        .map(|source| source.name().to_string())
        .collect();
    let returning_columns =
        match normalize_mutation_returning(returning, &target, target_alias.as_deref()) {
            Ok(columns) => columns,
            Err(reason) => {
                return unsupported_write_statement("update", "unproved_mutation_returning", reason)
            }
        };
    let mut derived_index = 0;
    let mut nonlocal_dependencies = BTreeSet::new();
    if correlated.is_none() {
        if let Some(selection) = selection {
            collect_expression_dependencies(
                selection,
                &BTreeSet::new(),
                &mut diagnostics,
                &mut derived_index,
                &mut nonlocal_dependencies,
            );
        }
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
    let predicate = correlated
        .as_ref()
        .map(|(predicate, _)| predicate.clone())
        .or_else(|| selection.map(|selection| analyze_predicate(selection, &mut diagnostics)));
    let domains = derive_column_domains(
        &Predicates::new(predicate.clone(), None, None),
        &domain_sources,
    );
    let assignments = assignments
        .iter()
        .map(|assignment| {
            let is_default = matches!(&assignment.value,
                Expr::Identifier(identifier) if identifier.quote_style.is_none()
                    && identifier.value.eq_ignore_ascii_case("DEFAULT"));
            // SQL DEFAULT is not the identifier of a physical source column.
            // The marker is evaluated only against authoritative target metadata.
            let expression = if is_default {
                Expression::Literal(LiteralExpression::new(
                    LiteralType::Null,
                    LiteralValue::Null,
                ))
            } else {
                analyze_expression_with_scope(&assignment.value, &[], &[], &mut diagnostics)
            };
            let domain = if is_default {
                ValueDomain::Unbounded
            } else {
                let derived = derive_expression_domain_with_column_domains(
                    &expression,
                    &domains,
                    &domain_sources,
                );
                refine_write_value_domain(&expression, derived, &domains, &domain_sources, metadata)
            };
            let assignment = MergeAssignment::new(
                assignment.target.to_string(),
                WriteValue::new(expression, domain),
            );
            if is_default {
                assignment.with_default()
            } else {
                assignment
            }
        })
        .collect();
    sort_diagnostics(&mut diagnostics);
    ProtocolStatement::Query(
        QueryStatement::new(
            query_sources,
            dependencies,
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
        .with_write(Some({
            let mut write =
                WriteOperation::update(target, target_alias, predicate, assignments, domains)
                    .with_returning_columns(returning_columns);
            if let Some(source) = joined_source {
                write = write.with_joined_source(source);
            }
            write
        })),
    )
}

pub(super) fn analyze_delete(
    delete: &SqlDelete,
    metadata: &AnalysisMetadata<'_>,
) -> ProtocolStatement {
    let tables = match &delete.from {
        SqlFromTable::WithFromKeyword(tables) | SqlFromTable::WithoutKeyword(tables) => tables,
    };
    if !delete.tables.is_empty()
        || !delete.order_by.is_empty()
        || delete.limit.is_some()
        || tables.len() != 1
        || !tables[0].joins.is_empty()
    {
        return unsupported_write_statement(
            "delete",
            "unsupported_delete_form",
            "DELETE requires one named target without target JOIN, ORDER BY, or LIMIT",
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
    let joined_source = delete
        .using
        .as_ref()
        .and_then(|tables| named_mutation_source(tables));
    if delete.using.is_some() && joined_source.is_none() {
        return unsupported_write_statement(
            "delete",
            "unsupported_delete_using",
            "DELETE USING requires exactly one named independent source without joins",
        );
    }
    if joined_source
        .as_ref()
        .is_some_and(|source| !disjoint_mutation_sources(&target_source, source))
    {
        return unsupported_write_statement(
            "delete",
            "unproved_delete_self_source",
            "DELETE USING cannot reuse or shadow the physical target without a joint alias proof",
        );
    }
    let mut diagnostics = Vec::new();
    let correlated = if delete.using.is_none() {
        delete.selection.as_ref().and_then(|selection| {
            bounded_mutation_exists(selection, &target_source, metadata, &mut diagnostics)
        })
    } else {
        None
    };
    let mut domain_sources = vec![target_source];
    domain_sources.extend(joined_source.iter().cloned());
    let query_sources: Vec<SourceRelation> = joined_source
        .iter()
        .cloned()
        .chain(correlated.as_ref().map(|(_, source)| source.clone()))
        .collect();
    let dependencies = query_sources
        .iter()
        .map(|source| source.name().to_string())
        .collect();
    let returning_columns = match normalize_mutation_returning(
        delete.returning.as_deref(),
        &target,
        target_alias.as_deref(),
    ) {
        Ok(columns) => columns,
        Err(reason) => {
            return unsupported_write_statement("delete", "unproved_mutation_returning", reason)
        }
    };
    let mut derived_index = 0;
    let mut nonlocal_dependencies = BTreeSet::new();
    if correlated.is_none() {
        if let Some(selection) = &delete.selection {
            collect_expression_dependencies(
                selection,
                &BTreeSet::new(),
                &mut diagnostics,
                &mut derived_index,
                &mut nonlocal_dependencies,
            );
        }
    }
    if !nonlocal_dependencies.is_empty() {
        return unsupported_write_statement(
            "delete",
            "unsupported_delete_subquery",
            "DELETE subqueries require source-row and correlation semantics that cannot yet be proved",
        );
    }
    let predicate = correlated
        .as_ref()
        .map(|(predicate, _)| predicate.clone())
        .or_else(|| {
            delete
                .selection
                .as_ref()
                .map(|selection| analyze_predicate(selection, &mut diagnostics))
        });
    let domains = derive_column_domains(
        &Predicates::new(predicate.clone(), None, None),
        &domain_sources,
    );
    sort_diagnostics(&mut diagnostics);
    ProtocolStatement::Query(
        QueryStatement::new(
            query_sources,
            dependencies,
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
        .with_write(Some({
            let mut write = WriteOperation::delete(target, target_alias, predicate, domains)
                .with_returning_columns(returning_columns);
            if let Some(source) = joined_source {
                write = write.with_joined_source(source);
            }
            write
        })),
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
            with_hints,
            index_hints,
            sample: None,
            version: None,
            json_path: None,
            with_ordinality: false,
            ..
        } if partitions.is_empty()
            && with_hints.is_empty()
            && index_hints.is_empty()
            // Some parser dialects treat vendor PARTITION syntax as a table
            // alias. A write with that ambiguous alias cannot safely claim
            // to target the whole physical relation.
            && !alias.as_ref().is_some_and(|alias|
                alias.name.value.eq_ignore_ascii_case("partition")) => {
            Some(SourceRelation::new(
                name.to_string(),
                alias.as_ref().map(|alias| alias.name.to_string()),
            ))
        }
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
