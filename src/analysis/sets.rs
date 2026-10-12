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
            // A nested branch's own LIMIT/FETCH/CTE can change the exact
            // number of tuples at its physical boundary. Those modifiers
            // cannot be discharged by a source-row witness for the branch.
            let boundary = if branch_query.with.is_none()
                && branch_query.limit_clause.is_none()
                && branch_query.fetch.is_none()
            {
                analyze_set_leaf_boundary(expression, &analyzed)
            } else {
                None
            };
            branches.push(SetBranch::new(identity.to_string(), &analyzed, boundary));
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
