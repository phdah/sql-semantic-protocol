//! Comparison exactness and residual condition classification.

use super::*;

// Preserve representable domains while making warehouse-dependent comparisons conditional.
pub(super) fn comparison_domain_exactness(
    column_domains: &[ColumnDomain],
    metadata: &AnalysisMetadata<'_>,
) -> ConditionExactness {
    let mut requirements = Vec::new();
    for column_domain in column_domains {
        let domain = column_domain.domain();
        let literal = match domain {
            ValueDomain::Ranges(ranges) => ranges
                .ranges()
                .iter()
                .flat_map(|range| {
                    [range.lower(), range.upper()]
                        .into_iter()
                        .flatten()
                        .map(|bound| bound.value())
                })
                .next(),
            ValueDomain::Set(set) => set
                .values()
                .iter()
                .find(|value| value.literal_type() != LiteralType::Null),
            ValueDomain::Unbounded | ValueDomain::Empty | ValueDomain::Unknown(_) => None,
        };
        let Some(literal) = literal else { continue };
        let data_type = metadata.column_data_type(column_domain.column());
        let data_type = match data_type {
            Some(DataType::Nullable(inner)) => Some(inner.as_ref()),
            other => other,
        };
        let mut assumptions = BTreeSet::new();
        match data_type {
            Some(DataType::String { fixed, .. }) => {
                assumptions.insert(ComparisonAssumption::BinaryCollation);
                if *fixed {
                    assumptions.insert(ComparisonAssumption::NoCharPadding);
                }
            }
            Some(DataType::FloatingPoint { .. }) => {
                assumptions.insert(ComparisonAssumption::NoNan);
                assumptions.insert(ComparisonAssumption::SignedZeroEquivalent);
            }
            Some(DataType::Timestamp { .. }) => {
                let zone = metadata.column_timestamp_zone(column_domain.column());
                if zone != Some(TimestampZone::WithoutTimeZone)
                    && (zone != Some(TimestampZone::WithTimeZone)
                        || !timestamp_literal_has_offset(literal))
                {
                    assumptions.insert(ComparisonAssumption::SessionTimeZone);
                }
            }
            None if literal.literal_type() == LiteralType::String => {
                assumptions.insert(ComparisonAssumption::BinaryCollation);
            }
            None if literal.literal_type() == LiteralType::Timestamp => {
                assumptions.insert(ComparisonAssumption::SessionTimeZone);
            }
            _ => {}
        }
        for assumption in assumptions {
            requirements.push(ConditionalCondition::new(
                assumption,
                ConditionClause::Where,
                format!(
                    "column_domain:{}",
                    qualified_column_name(
                        column_domain.column().relation(),
                        column_domain.column().name()
                    )
                ),
            ));
        }
    }
    ConditionExactness::from_requirements(requirements)
}

pub(super) fn unknown_column_domain_exactness(
    column_domains: &[ColumnDomain],
    typing_failures: &[(ColumnRef, ResidualConditionReason)],
    predicates: &Predicates,
    joins: &[ProtocolJoin],
    sources: &[SourceRelation],
    predicate_exactness: &ConditionExactness,
) -> ConditionExactness {
    let mut residuals = Vec::new();
    for column_domain in column_domains {
        if !matches!(column_domain.domain(), ValueDomain::Unknown(_)) {
            continue;
        }
        let typed_failure = typing_failures
            .iter()
            .find(|(column, _)| column == column_domain.column())
            .map(|(_, reason)| *reason);
        let mut locations =
            condition_locations_for_column(column_domain.column(), predicates, joins, sources);
        if locations.is_empty() {
            // The domain may have been remapped from a computed local column to
            // physical lineage. Such a column no longer matches the local
            // predicate's reference, but its unknown outcome still blocks exactness.
            for (clause, identity, predicate) in [
                (
                    ConditionClause::Where,
                    "where",
                    predicates.where_predicate(),
                ),
                (
                    ConditionClause::Having,
                    "having",
                    predicates.having_predicate(),
                ),
                (
                    ConditionClause::Qualify,
                    "qualify",
                    predicates.qualify_predicate(),
                ),
            ] {
                if predicate.is_some() {
                    locations.push((clause, identity.to_string()));
                }
            }
            if locations.is_empty() {
                for (index, join) in joins.iter().enumerate() {
                    if join.condition().is_some() {
                        locations.push((ConditionClause::JoinOn, format!("join:{index}")));
                    }
                }
            }
        }
        for (clause, identity) in locations {
            // A structural unknown explained by the predicate classifier is already
            // represented with the predicate's own identity. Do not report it again.
            if typed_failure.is_none()
                && predicate_exactness
                    .residual_conditions()
                    .iter()
                    .any(|residual| residual.clause() == clause)
            {
                continue;
            }
            residuals.push(ResidualCondition::new(
                typed_failure.unwrap_or(ResidualConditionReason::ComputedExpression),
                clause,
                format!(
                    "{identity}:column_domain:{}",
                    qualified_column_name(
                        column_domain.column().relation(),
                        column_domain.column().name()
                    )
                ),
            ));
        }
    }
    ConditionExactness::from_residuals(residuals)
}

pub(super) fn analyze_query_condition_exactness(
    query: &SqlQuery,
    predicates: &Predicates,
    sources: &[SourceRelation],
    dependencies: &BTreeSet<String>,
    joins: &[ProtocolJoin],
    diagnostics: &[Diagnostic],
    correlated: bool,
) -> ConditionExactness {
    let mut residuals = Vec::new();

    // Local relations carry inherited physical joins, but the enclosing scope
    // only names the CTE/derived-table sources. Include proven dependencies so
    // normalized physical equalities are not misclassified as column comparisons.
    let mut equality_sources = sources.to_vec();
    for dependency in dependencies {
        if !equality_sources
            .iter()
            .any(|source| source.name() == dependency)
        {
            equality_sources.push(SourceRelation::new(dependency.clone(), None));
        }
    }

    if let Some(predicate) = predicates.where_predicate() {
        append_predicate_residuals(
            predicate,
            ConditionClause::Where,
            "where",
            &equality_sources,
            true,
            &mut residuals,
        );
    }

    if predicates.having_predicate().is_some() {
        residuals.push(ResidualCondition::new(
            ResidualConditionReason::Having,
            ConditionClause::Having,
            "having",
        ));
    }

    if predicates.qualify_predicate().is_some() {
        residuals.push(ResidualCondition::new(
            ResidualConditionReason::Qualify,
            ConditionClause::Qualify,
            "qualify",
        ));
    }

    for (index, join) in joins.iter().enumerate() {
        let identity = format!("join:{index}");
        match join.kind() {
            JoinKind::Inner => {
                if let Some(predicate) = join.condition() {
                    append_predicate_residuals(
                        predicate,
                        ConditionClause::JoinOn,
                        &identity,
                        &equality_sources,
                        true,
                        &mut residuals,
                    );
                }
            }
            JoinKind::Cross => {}
            JoinKind::Left | JoinKind::Right | JoinKind::Full => {
                residuals.push(ResidualCondition::new(
                    ResidualConditionReason::OuterJoin,
                    ConditionClause::JoinOn,
                    identity,
                ));
            }
            JoinKind::LeftSemi
            | JoinKind::RightSemi
            | JoinKind::LeftAnti
            | JoinKind::RightAnti
            | JoinKind::Unknown => {
                residuals.push(ResidualCondition::new(
                    ResidualConditionReason::UnsupportedJoinKind,
                    ConditionClause::JoinOn,
                    identity,
                ));
            }
        }
    }

    let mut source_counts = BTreeMap::new();
    for source in sources {
        *source_counts
            .entry(source.name().to_string())
            .or_insert(0usize) += 1;
    }
    for (source, count) in source_counts {
        if count > 1 {
            residuals.push(ResidualCondition::new(
                ResidualConditionReason::RepeatedSourceInstance,
                ConditionClause::RowSetOperator,
                format!("source:{source}"),
            ));
        }
    }

    collect_set_expr_exactness_residuals(query.body.as_ref(), "body", &mut residuals);

    if let Some(limit_clause) = &query.limit_clause {
        let text = limit_clause.to_string().to_ascii_uppercase();
        if text.contains("OFFSET") {
            residuals.push(ResidualCondition::new(
                ResidualConditionReason::Offset,
                ConditionClause::RowSetOperator,
                "offset",
            ));
        }
        if text.contains("LIMIT") || !text.contains("OFFSET") {
            residuals.push(ResidualCondition::new(
                ResidualConditionReason::Limit,
                ConditionClause::RowSetOperator,
                "limit",
            ));
        }
    }

    if query.fetch.is_some() {
        residuals.push(ResidualCondition::new(
            ResidualConditionReason::Fetch,
            ConditionClause::RowSetOperator,
            "fetch",
        ));
    }

    if correlated {
        residuals.push(ResidualCondition::new(
            ResidualConditionReason::CorrelatedSubquery,
            ConditionClause::Where,
            "correlation",
        ));
    }

    for diagnostic in diagnostics {
        if diagnostic_is_non_membership_or_already_classified(diagnostic.code()) {
            continue;
        }

        let clause = if diagnostic.code().contains("join") {
            ConditionClause::JoinOn
        } else if diagnostic.code().contains("predicate")
            || diagnostic.code() == "unsupported_query_body"
        {
            ConditionClause::Where
        } else {
            ConditionClause::RowSetOperator
        };
        residuals.push(ResidualCondition::new(
            ResidualConditionReason::AnalysisDiagnostic,
            clause,
            format!("diagnostic:{}", diagnostic.code()),
        ));
    }

    ConditionExactness::from_residuals(residuals)
}

pub(super) fn diagnostic_is_non_membership_or_already_classified(code: &str) -> bool {
    matches!(
        code,
        "unsupported_order_by"
            | "unsupported_cluster_by"
            | "unsupported_distribute_by"
            | "unsupported_sort_by"
            | "unsupported_exclude"
            | "unsupported_select_into"
            | "unsupported_value_table_mode"
            | "ambiguous_output_lineage"
            | "unresolved_output_lineage"
            | "unresolved_wildcard"
            | "unsupported_expression"
            | "unsupported_function"
            | "ambiguous_named_window"
            | "cyclic_named_window"
            | "unresolved_named_window"
            | "unsupported_window_order_option"
            | "unsupported_window_override"
            | "unsupported_group_by_modifier"
            | "unsupported_lock"
            | "unsupported_for_clause"
            | "unsupported_settings"
            | "unsupported_format_clause"
            | "unsupported_merge_output"
            | "unsupported_merge_insert_row"
            | "unsupported_queryless_create_table"
            | "unsupported_top"
            | "unsupported_prewhere"
            | "unsupported_connect_by"
            | "unsupported_limit"
            | "unsupported_fetch"
            | "set_operation_arity_mismatch"
            | "unresolved_set_operation_output"
            | "unsupported_set_operation_alignment"
            | "unknown_schema_column"
    )
}

pub(super) fn collect_set_expr_exactness_residuals(
    expression: &SetExpr,
    identity: &str,
    residuals: &mut Vec<ResidualCondition>,
) {
    match expression {
        SetExpr::Select(select) => {
            if matches!(select.distinct, Some(SqlDistinct::On(_))) {
                residuals.push(ResidualCondition::new(
                    ResidualConditionReason::DistinctOn,
                    ConditionClause::RowSetOperator,
                    format!("{identity}:distinct_on"),
                ));
            }
            if select.top.is_some() {
                residuals.push(ResidualCondition::new(
                    ResidualConditionReason::Top,
                    ConditionClause::RowSetOperator,
                    format!("{identity}:top"),
                ));
            }
            if select.prewhere.is_some() {
                residuals.push(ResidualCondition::new(
                    ResidualConditionReason::Prewhere,
                    ConditionClause::Where,
                    format!("{identity}:prewhere"),
                ));
            }
            if select.connect_by.is_some() {
                residuals.push(ResidualCondition::new(
                    ResidualConditionReason::ConnectBy,
                    ConditionClause::RowSetOperator,
                    format!("{identity}:connect_by"),
                ));
            }
            collect_table_sample_residuals(select, identity, residuals);
        }
        SetExpr::Query(query) => {
            collect_set_expr_exactness_residuals(
                query.body.as_ref(),
                &format!("{identity}:query"),
                residuals,
            );
            if query.limit_clause.is_some() {
                residuals.push(ResidualCondition::new(
                    ResidualConditionReason::Limit,
                    ConditionClause::RowSetOperator,
                    format!("{identity}:query:limit"),
                ));
            }
            if query.fetch.is_some() {
                residuals.push(ResidualCondition::new(
                    ResidualConditionReason::Fetch,
                    ConditionClause::RowSetOperator,
                    format!("{identity}:query:fetch"),
                ));
            }
        }
        SetExpr::SetOperation {
            left, op, right, ..
        } => {
            residuals.push(ResidualCondition::new(
                ResidualConditionReason::SetOperation,
                ConditionClause::SetOperation,
                format!("{identity}:{}", analyze_set_operator(*op).as_str()),
            ));
            collect_set_expr_exactness_residuals(left, &format!("{identity}:left"), residuals);
            collect_set_expr_exactness_residuals(right, &format!("{identity}:right"), residuals);
        }
        SetExpr::Values(_)
        | SetExpr::Insert(_)
        | SetExpr::Update(_)
        | SetExpr::Delete(_)
        | SetExpr::Table(_) => {}
    }
}

pub(super) fn collect_table_sample_residuals(
    select: &Select,
    identity: &str,
    residuals: &mut Vec<ResidualCondition>,
) {
    let mut index = 0usize;
    for source in &select.from {
        if table_factor_has_sample(&source.relation) {
            residuals.push(ResidualCondition::new(
                ResidualConditionReason::TableSample,
                ConditionClause::RowSetOperator,
                format!("{identity}:tablesample:{index}"),
            ));
        }
        index += 1;
        for join in &source.joins {
            if table_factor_has_sample(&join.relation) {
                residuals.push(ResidualCondition::new(
                    ResidualConditionReason::TableSample,
                    ConditionClause::RowSetOperator,
                    format!("{identity}:tablesample:{index}"),
                ));
            }
            index += 1;
        }
    }
}

pub(super) fn table_factor_has_sample(factor: &TableFactor) -> bool {
    match factor {
        TableFactor::Table { sample, .. } => sample.is_some(),
        _ => false,
    }
}

pub(super) fn append_predicate_residuals(
    predicate: &Predicate,
    clause: ConditionClause,
    identity: &str,
    sources: &[SourceRelation],
    allow_join_equality: bool,
    residuals: &mut Vec<ResidualCondition>,
) {
    // A path through the logical tree identifies a condition, rather than only its
    // reason. Two unsupported siblings must not collapse into one residual.
    match predicate {
        Predicate::And(logical) => {
            for (index, operand) in logical.operands().iter().enumerate() {
                append_predicate_residuals(
                    operand,
                    clause,
                    &format!("{identity}:and:{index}"),
                    sources,
                    allow_join_equality,
                    residuals,
                );
            }
        }
        Predicate::Or(logical)
            if clause != ConditionClause::JoinOn
                && logical.operands().iter().any(|operand| {
                    !predicate_residual_reasons(operand, clause, sources, allow_join_equality)
                        .is_empty()
                }) =>
        {
            for (index, operand) in logical.operands().iter().enumerate() {
                append_predicate_residuals(
                    operand,
                    clause,
                    &format!("{identity}:or:{index}"),
                    sources,
                    allow_join_equality,
                    residuals,
                );
            }
        }
        _ => {
            for reason in
                predicate_residual_reasons(predicate, clause, sources, allow_join_equality)
            {
                residuals.push(ResidualCondition::new(reason, clause, identity));
            }
        }
    }
}

pub(super) fn predicate_residual_reasons(
    predicate: &Predicate,
    clause: ConditionClause,
    sources: &[SourceRelation],
    allow_join_equality: bool,
) -> Vec<ResidualConditionReason> {
    match predicate {
        Predicate::Comparison(comparison) => match (comparison.left(), comparison.right()) {
            (Expression::Column(_), Expression::Literal(_))
            | (Expression::Literal(_), Expression::Column(_)) => Vec::new(),
            (Expression::Column(left), Expression::Column(right))
                if allow_join_equality
                    && comparison.operator() == ComparisonOperator::Eq
                    && columns_reference_distinct_sources(left, right, sources) =>
            {
                Vec::new()
            }
            (Expression::Column(_), Expression::Column(_)) => {
                vec![ResidualConditionReason::ColumnComparison]
            }
            _ => vec![ResidualConditionReason::ComputedExpression],
        },
        Predicate::And(logical) => {
            collect_logical_residual_reasons(logical, clause, sources, allow_join_equality)
        }
        Predicate::Or(logical) => {
            if clause == ConditionClause::JoinOn {
                return vec![ResidualConditionReason::UnsupportedPredicate];
            }

            let child_reasons =
                collect_logical_residual_reasons(logical, clause, sources, allow_join_equality);
            if !child_reasons.is_empty() {
                return child_reasons;
            }

            let mut columns = BTreeSet::new();
            collect_predicate_column_refs(predicate, sources, &mut columns);
            if columns.len() > 1 {
                vec![ResidualConditionReason::CrossColumnDisjunction]
            } else if predicate_domains_contain_unknown(predicate, sources) {
                vec![ResidualConditionReason::UnsupportedPredicate]
            } else {
                Vec::new()
            }
        }
        // Whole-query scalar exactness is independent of an operator-local
        // binary-prefix witness and requires comparison-setting attestations.
        Predicate::LikePrefix(_) => vec![ResidualConditionReason::ComputedExpression],
        Predicate::Not(_) => vec![ResidualConditionReason::LogicalNot],
        Predicate::IsNull(predicate) => {
            if matches!(predicate.expression(), Expression::Column(_)) {
                Vec::new()
            } else {
                vec![ResidualConditionReason::ComputedExpression]
            }
        }
        Predicate::In(predicate) => {
            if matches!(predicate.expression(), Expression::Column(_))
                && predicate
                    .values()
                    .iter()
                    .all(|value| matches!(value, Expression::Literal(_)))
            {
                Vec::new()
            } else {
                vec![ResidualConditionReason::ComputedExpression]
            }
        }
        Predicate::Between(predicate) => {
            if matches!(predicate.expression(), Expression::Column(_))
                && matches!(predicate.lower(), Expression::Literal(_))
                && matches!(predicate.upper(), Expression::Literal(_))
            {
                Vec::new()
            } else {
                vec![ResidualConditionReason::ComputedExpression]
            }
        }
        Predicate::Exists(_) | Predicate::InSubquery(_) => {
            vec![ResidualConditionReason::SubqueryPredicate]
        }
        Predicate::BooleanExpression(Expression::Literal(literal)) => match literal.value() {
            LiteralValue::Boolean(true) => Vec::new(),
            LiteralValue::Boolean(false) | LiteralValue::Null => {
                vec![ResidualConditionReason::ConstantFalseOrNull]
            }
            LiteralValue::Number(_) | LiteralValue::Text(_) => {
                vec![ResidualConditionReason::UnsupportedPredicate]
            }
        },
        Predicate::BooleanExpression(_) => vec![ResidualConditionReason::ComputedExpression],
        Predicate::Unknown(_) | Predicate::Unsupported(_) => {
            vec![ResidualConditionReason::UnsupportedPredicate]
        }
    }
}

pub(super) fn columns_reference_distinct_sources(
    left: &ColumnExpression,
    right: &ColumnExpression,
    sources: &[SourceRelation],
) -> bool {
    let Some((left_index, _)) = source_for_column_expression(left, sources) else {
        return false;
    };
    let Some((right_index, _)) = source_for_column_expression(right, sources) else {
        return false;
    };
    left_index != right_index
}

pub(super) fn collect_logical_residual_reasons(
    logical: &LogicalPredicate,
    clause: ConditionClause,
    sources: &[SourceRelation],
    allow_join_equality: bool,
) -> Vec<ResidualConditionReason> {
    let mut reasons = Vec::new();
    for operand in logical.operands() {
        for reason in predicate_residual_reasons(operand, clause, sources, allow_join_equality) {
            if !reasons.contains(&reason) {
                reasons.push(reason);
            }
        }
    }
    reasons
}

pub(super) fn collect_predicate_column_refs(
    predicate: &Predicate,
    sources: &[SourceRelation],
    columns: &mut BTreeSet<ColumnRef>,
) {
    match predicate {
        Predicate::LikePrefix(predicate) => {
            collect_expression_column_refs(predicate.expression(), sources, columns);
        }
        Predicate::Comparison(comparison) => {
            collect_expression_column_refs(comparison.left(), sources, columns);
            collect_expression_column_refs(comparison.right(), sources, columns);
        }
        Predicate::And(logical) | Predicate::Or(logical) => {
            for operand in logical.operands() {
                collect_predicate_column_refs(operand, sources, columns);
            }
        }
        Predicate::Not(predicate) => {
            collect_predicate_column_refs(predicate.operand(), sources, columns)
        }
        Predicate::IsNull(predicate) => {
            collect_expression_column_refs(predicate.expression(), sources, columns)
        }
        Predicate::In(predicate) => {
            collect_expression_column_refs(predicate.expression(), sources, columns);
            for value in predicate.values() {
                collect_expression_column_refs(value, sources, columns);
            }
        }
        Predicate::InSubquery(predicate) => {
            for expression in predicate.operand().expressions() {
                collect_expression_column_refs(expression, sources, columns);
            }
        }
        Predicate::Between(predicate) => {
            collect_expression_column_refs(predicate.expression(), sources, columns);
            collect_expression_column_refs(predicate.lower(), sources, columns);
            collect_expression_column_refs(predicate.upper(), sources, columns);
        }
        Predicate::BooleanExpression(expression) => {
            collect_expression_column_refs(expression, sources, columns)
        }
        Predicate::Exists(_) | Predicate::Unknown(_) | Predicate::Unsupported(_) => {}
    }
}

pub(super) fn collect_expression_column_refs(
    expression: &Expression,
    sources: &[SourceRelation],
    columns: &mut BTreeSet<ColumnRef>,
) {
    match expression {
        Expression::Column(column) => {
            columns.insert(resolve_column(column, sources));
        }
        Expression::Function(function) => {
            for argument in function.arguments() {
                collect_expression_column_refs(argument, sources, columns);
            }
        }
        Expression::Case(case) => {
            if let Some(operand) = case.operand() {
                collect_expression_column_refs(operand, sources, columns);
            }
            for branch in case.branches() {
                collect_expression_column_refs(branch.condition(), sources, columns);
                collect_expression_column_refs(branch.result(), sources, columns);
            }
            if let Some(else_result) = case.else_result() {
                collect_expression_column_refs(else_result, sources, columns);
            }
        }
        Expression::BooleanPredicate(predicate) => {
            collect_predicate_column_refs(predicate, sources, columns)
        }
        Expression::SignedIntegerCast(cast) => {
            collect_expression_column_refs(cast.expression(), sources, columns)
        }
        Expression::Unary(unary) => {
            collect_expression_column_refs(unary.operand(), sources, columns)
        }
        Expression::Binary(binary) => {
            collect_expression_column_refs(binary.left(), sources, columns);
            collect_expression_column_refs(binary.right(), sources, columns);
        }
        Expression::AggregateFunction(aggregate) => {
            for argument in aggregate.arguments() {
                if let AggregateArgument::Expression(expression) = argument {
                    collect_expression_column_refs(expression, sources, columns);
                }
            }
            if let Some(filter) = aggregate.filter() {
                collect_predicate_column_refs(filter, sources, columns);
            }
        }
        Expression::WindowFunction(window) => {
            for argument in window.function().arguments() {
                collect_expression_column_refs(argument, sources, columns);
            }
            for partition in window.window().partition_by() {
                collect_expression_column_refs(partition, sources, columns);
            }
            for ordering in window.window().order_by() {
                collect_expression_column_refs(ordering.expression(), sources, columns);
            }
        }
        Expression::Literal(_)
        | Expression::ScalarSubquery(_)
        | Expression::Unknown(_)
        | Expression::Unsupported(_) => {}
    }
}
