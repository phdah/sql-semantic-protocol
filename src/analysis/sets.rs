//! Set operator normalization and complete branch evidence.

use super::*;

// Annotate each operation node with the leaves belonging to its own subtree.
// Otherwise nested operations serialize an empty branch list and falsely report
// set-level modifiers even when only the outer operation owns its LIMIT/FETCH.
pub(super) fn populate_set_operation_evidence(
    expression: &SetExpr,
    operation: SetOperation,
    branches: &[SetBranch],
    set_level_safe: bool,
) -> SetOperation {
    match expression {
        SetExpr::Query(query) => populate_set_operation_evidence(
            query.body.as_ref(),
            operation,
            branches,
            set_level_safe && query.limit_clause.is_none() && query.fetch.is_none(),
        ),
        SetExpr::SetOperation { left, right, .. } => {
            let left_count = count_set_select_leaves(left).min(branches.len());
            let (left_branches, right_branches) = branches.split_at(left_count);
            let left_operand = populate_set_operand_evidence(left, operation.left(), left_branches);
            let right_operand =
                populate_set_operand_evidence(right, operation.right(), right_branches);
            SetOperation::new(
                operation.operator(),
                operation.quantifier(),
                left_operand,
                right_operand,
            )
            .with_branches(branches.to_vec())
            .with_set_level_safety(set_level_safe)
        }
        _ => operation
            .with_branches(branches.to_vec())
            .with_set_level_safety(set_level_safe),
    }
}

pub(super) fn populate_set_operand_evidence(
    expression: &SetExpr,
    operand: &SetOperand,
    branches: &[SetBranch],
) -> SetOperand {
    match operand {
        SetOperand::Query => SetOperand::Query,
        SetOperand::Operation(nested) => SetOperand::Operation(Box::new(
            populate_set_operation_evidence(expression, (**nested).clone(), branches, true),
        )),
    }
}

pub(super) fn count_set_select_leaves(expression: &SetExpr) -> usize {
    match expression {
        SetExpr::Select(_) => 1,
        SetExpr::Query(query) => count_set_select_leaves(query.body.as_ref()),
        SetExpr::SetOperation { left, right, .. } => {
            count_set_select_leaves(left) + count_set_select_leaves(right)
        }
        SetExpr::Values(_)
        | SetExpr::Insert(_)
        | SetExpr::Update(_)
        | SetExpr::Delete(_)
        | SetExpr::Table(_) => 0,
    }
}

// Evaluate each leaf in its own query scope. In particular, do not collapse WHERE a > 10
// on one side of UNION ALL with WHERE a < 0 on the other side.
pub(super) fn collect_set_branch_evidence(
    expression: &SetExpr,
    context: &SqlQuery,
    identity: &str,
    metadata: &AnalysisMetadata<'_>,
    strip_outer_set_modifiers: bool,
    branches: &mut Vec<SetBranch>,
) {
    match expression {
        SetExpr::SetOperation { left, right, .. } => {
            collect_set_branch_evidence(
                left,
                context,
                &format!("{identity}:left"),
                metadata,
                strip_outer_set_modifiers,
                branches,
            );
            collect_set_branch_evidence(
                right,
                context,
                &format!("{identity}:right"),
                metadata,
                strip_outer_set_modifiers,
                branches,
            );
        }
        SetExpr::Query(query) => {
            // A parenthesized query's own LIMIT/FETCH and CTE declarations belong to
            // this operand and must not be discarded with the outer set query's clauses.
            collect_set_branch_evidence(
                query.body.as_ref(),
                query,
                &format!("{identity}:query"),
                metadata,
                false,
                branches,
            );
        }
        SetExpr::Select(_) => {
            let mut branch_query = context.clone();
            *branch_query.body = expression.clone();
            // Set-level shaping is not a filter on an individual branch. Nested
            // query-level shaping remains residual at the enclosing set scope.
            if strip_outer_set_modifiers {
                branch_query.limit_clause = None;
                branch_query.fetch = None;
            }
            let analyzed = analyze_query(&branch_query, None, metadata);
            // Inner CTE rows are not independent physical bags. Only an
            // identity-preserving single-source CTE, certified below, can be
            // rebound to the complete underlying physical source assignment.
            let (boundary, certified_local) = if branch_query.limit_clause.is_none()
                && branch_query.fetch.is_none()
            {
                match branch_query.with.as_ref() {
                    None => (analyze_set_leaf_boundary(expression, &analyzed), false),
                    Some(with) => {
                        if let Some(cte_boundary) =
                            certify_identity_set_cte(expression, &branch_query, metadata)
                        {
                            (Some(cte_boundary), true)
                        } else {
                            // An unrelated CTE does not invalidate a direct
                            // physical branch, provided its relation is not
                            // shadowed by a CTE name in this scope.
                            let source_is_local = match expression {
                                SetExpr::Select(select) => {
                                    select.from.iter().any(|table| match &table.relation {
                                        TableFactor::Table { name, .. } => with
                                            .cte_tables
                                            .iter()
                                            .any(|cte| cte.alias.name.value == name.to_string()),
                                        _ => true,
                                    })
                                }
                                _ => true,
                            };
                            if source_is_local {
                                (None, false)
                            } else {
                                let mut direct = branch_query.clone();
                                direct.with = None;
                                let independent = analyze_query(&direct, None, metadata);
                                (
                                    analyze_set_leaf_boundary(expression, &independent),
                                    independent.row_preserving_projection()
                                        || independent.filter_only_row_shape(),
                                )
                            }
                        }
                    }
                }
            } else {
                (None, false)
            };
            branches.push(SetBranch::new(
                identity.to_string(),
                &analyzed,
                boundary,
                certified_local,
            ));
        }
        SetExpr::Values(_)
        | SetExpr::Insert(_)
        | SetExpr::Update(_)
        | SetExpr::Delete(_)
        | SetExpr::Table(_) => {
            // These operands still have a set-operation residual on the outer query.
        }
    }
}

/// Certify the finite physical leaf of an exactly lossless local CTE.
///
/// This is intentionally more restrictive than lineage: the CTE must have
/// one direct physical table, an unfiltered row-preserving SELECT of plain
/// copies and the consuming leaf must select its named output columns
/// positionally. A filtered, aggregated, recursive, renamed or shaped CTE
/// cannot be substituted for a physical snapshot.
pub(super) fn certify_identity_set_cte(
    expression: &SetExpr,
    context: &SqlQuery,
    metadata: &AnalysisMetadata<'_>,
) -> Option<SetWitnessBoundary> {
    let with = context.with.as_ref()?;
    if with.recursive {
        return None;
    }
    let [cte] = with.cte_tables.as_slice() else {
        return None;
    };
    if !cte.alias.columns.is_empty()
        || cte.query.with.is_some()
        || cte.query.limit_clause.is_some()
        || cte.query.fetch.is_some()
    {
        return None;
    }
    let SetExpr::Select(outer) = expression else {
        return None;
    };
    let [from] = outer.from.as_slice() else {
        return None;
    };
    if !from.joins.is_empty()
        || outer.distinct.is_some()
        || outer.top.is_some()
        || outer.having.is_some()
        || outer.qualify.is_some()
        || outer.prewhere.is_some()
        || !outer.lateral_views.is_empty()
        || outer.connect_by.is_some()
        || !matches!(&outer.group_by, GroupByExpr::Expressions(items, modifiers)
            if items.is_empty() && modifiers.is_empty())
    {
        return None;
    }
    let TableFactor::Table {
        name,
        args: None,
        sample: None,
        ..
    } = &from.relation
    else {
        return None;
    };
    if name.to_string() != cte.alias.name.value {
        return None;
    }
    let inner = analyze_query(&cte.query, None, metadata);
    if !inner.row_preserving_projection()
        || !inner.diagnostics().is_empty()
        || inner.sources().len() != 1
        || inner.dependencies().len() != 1
        || inner.aggregation().is_some()
        || inner.set_operation().is_some()
        || inner.output().columns().is_empty()
    {
        return None;
    }
    let physical = inner.sources()[0].name();
    if inner.dependencies()[0] != physical {
        return None;
    }
    let mapped = inner
        .output()
        .columns()
        .iter()
        .map(|column| {
            let copied = column.plain_copy_source()?;
            if copied.relation() != physical || copied.column() != column.name() {
                return None;
            }
            Some(copied.column().to_string())
        })
        .collect::<Option<Vec<_>>>()?;
    let outer_columns = match &outer.projection[..] {
        columns if columns.len() == mapped.len() => columns,
        _ => return None,
    };
    for (item, name) in outer_columns.iter().zip(&mapped) {
        match item {
            SelectItem::UnnamedExpr(Expr::Identifier(column)) if &column.value == name => {}
            SelectItem::UnnamedExpr(Expr::CompoundIdentifier(parts))
                if parts.len() == 2
                    && parts[0].value == cte.alias.name.value
                    && &parts[1].value == name => {}
            _ => return None,
        }
    }
    Some(SetWitnessBoundary::new(physical.to_string(), mapped, false))
}

/// A row-preserving, single-relation SELECT has an exact positional tuple-count
/// correspondence at its immediate relation boundary. This is intentionally not
/// a claim that an intermediate/CTE producer can already be materialized.
pub(super) fn analyze_set_leaf_boundary(
    expression: &SetExpr,
    query: &QueryStatement,
) -> Option<SetWitnessBoundary> {
    let SetExpr::Select(select) = expression else {
        return None;
    };
    let [table] = select.from.as_slice() else {
        return None;
    };
    if !table.joins.is_empty() || !matches!(table.relation, TableFactor::Table { args: None, .. }) {
        return None;
    }
    if query
        .aggregation()
        .is_some_and(|agg| agg.distinct() || agg.group_by().is_some())
    {
        return None;
    }
    let [source] = query.sources() else {
        return None;
    };
    if !query.joins().is_empty() {
        return None;
    }
    let columns = query
        .output()
        .columns()
        .iter()
        .map(|column| {
            // The immediate input column, not the transitive physical lineage, is
            // the correct coordinate system for a CTE/intermediate boundary.
            let Expression::Column(expression) = column.expression() else {
                return None;
            };
            Some(expression.name().to_string())
        })
        .collect::<Option<Vec<_>>>()?;
    if columns.is_empty() {
        return None;
    }
    let intermediate = !query.dependencies().iter().any(|dep| dep == source.name());
    Some(SetWitnessBoundary::new(
        source.name().to_string(),
        columns,
        intermediate,
    ))
}

pub(super) fn analyze_set_operation(expression: &SetExpr) -> Option<SetOperation> {
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
        | SetExpr::Table(_) => None,
    }
}

pub(super) fn analyze_set_operand(expression: &SetExpr) -> SetOperand {
    match analyze_set_operation(expression) {
        Some(operation) => SetOperand::Operation(Box::new(operation)),
        None => SetOperand::Query,
    }
}

/// Only attach row-shaping evidence that is independent of dialect defaults.
/// An absent plan for a shaped query intentionally leaves set-level exactness false.
pub(super) fn analyze_set_row_shaping(
    query: &SqlQuery,
    branches: &[SetBranch],
) -> Option<crate::protocol::SetRowShaping> {
    use crate::protocol::{SetRowShaping, SetSortKey};
    use sqlparser::ast::{LimitClause, OrderByKind};

    fn nonnegative_literal(expression: &Expr) -> Option<usize> {
        match expression {
            Expr::Value(value) => match &value.value {
                Value::Number(number, _) => number.parse::<usize>().ok(),
                _ => None,
            },
            _ => None,
        }
    }

    let columns = branches.first()?.output().columns();
    let order = query.order_by.as_ref()?;
    if order.interpolate.is_some() {
        return None;
    }
    let OrderByKind::Expressions(expressions) = &order.kind else {
        return None;
    };
    if expressions.is_empty() {
        return None;
    }
    let mut keys = Vec::new();
    for expression in expressions {
        if expression.with_fill.is_some() {
            return None;
        }
        let Expr::Identifier(name) = &expression.expr else {
            return None;
        };
        let index = columns
            .iter()
            .position(|column| column.name() == name.value)?;
        if keys.iter().any(|key: &SetSortKey| key.index == index) {
            return None;
        }
        keys.push(SetSortKey {
            index,
            descending: expression.options.asc == Some(false),
            nulls_first: expression.options.nulls_first,
        });
    }

    let (limit, offset) = match (&query.limit_clause, &query.fetch) {
        (
            Some(LimitClause::LimitOffset {
                limit: Some(limit),
                offset,
                limit_by,
            }),
            None,
        ) if limit_by.is_empty() => (
            nonnegative_literal(limit)?,
            match offset {
                Some(offset) => nonnegative_literal(&offset.value)?,
                None => 0,
            },
        ),
        (Some(LimitClause::OffsetCommaLimit { offset, limit }), None) => {
            (nonnegative_literal(limit)?, nonnegative_literal(offset)?)
        }
        (None, Some(fetch)) if !fetch.with_ties && !fetch.percent => (
            match &fetch.quantity {
                Some(quantity) => nonnegative_literal(quantity)?,
                None => 1,
            },
            0,
        ),
        _ => return None,
    };

    Some(SetRowShaping {
        keys,
        offset,
        limit,
    })
}

pub(super) fn analyze_set_operator(operator: SqlSetOperator) -> SetOperator {
    match operator {
        SqlSetOperator::Union => SetOperator::Union,
        SqlSetOperator::Except | SqlSetOperator::Minus => SetOperator::Except,
        SqlSetOperator::Intersect => SetOperator::Intersect,
    }
}

pub(super) fn analyze_set_quantifier(quantifier: SqlSetQuantifier) -> SetQuantifier {
    match quantifier {
        SqlSetQuantifier::All => SetQuantifier::All,
        SqlSetQuantifier::Distinct | SqlSetQuantifier::None => SetQuantifier::Distinct,
        SqlSetQuantifier::ByName => SetQuantifier::ByName,
        SqlSetQuantifier::AllByName => SetQuantifier::AllByName,
        SqlSetQuantifier::DistinctByName => SetQuantifier::DistinctByName,
    }
}
