//! Bounded ordinary CTE analysis: SQL AST enters here, never the physical evaluator.
//! Unsupported row shaping, recursion or computed expressions never produce a program.

use super::*;
use crate::protocol::{CteColumnRef, CteInput, CteProgram, CteProjection, CteStage};

fn column(expression: &Expr) -> Option<CteColumnRef> {
    match expression {
        Expr::Identifier(name) => Some(CteColumnRef::new(None, name.value.clone())),
        Expr::CompoundIdentifier(names) if names.len() == 2 => Some(CteColumnRef::new(
            Some(names[0].value.clone()),
            names[1].value.clone(),
        )),
        _ => None,
    }
}

fn source(factor: &TableFactor, lexical: &BTreeMap<String, String>) -> Option<CteInput> {
    let TableFactor::Table {
        name,
        alias,
        args: None,
        sample: None,
        ..
    } = factor
    else {
        return None;
    };
    let name = name.to_string();
    let canonical = lexical.get(&name).cloned().unwrap_or_else(|| name.clone());
    let instance = alias
        .as_ref()
        .map_or(name, |alias| alias.name.value.clone());
    Some(CteInput::new(canonical, instance))
}

fn select_stage(
    query: &SqlQuery,
    id: String,
    lexical: &BTreeMap<String, String>,
) -> Option<CteStage> {
    if query.limit_clause.is_some()
        || query.fetch.is_some()
        || !query.locks.is_empty()
        || query.for_clause.is_some()
        || query.settings.is_some()
        || query.format_clause.is_some()
        || !query.pipe_operators.is_empty()
    {
        return None;
    }
    let SetExpr::Select(select) = query.body.as_ref() else {
        return None;
    };
    if select.distinct.is_some()
        || select.top.is_some()
        || select.selection.is_some()
        || select.having.is_some()
        || select.qualify.is_some()
        || select.prewhere.is_some()
        || select.into.is_some()
        || select.exclude.is_some()
        || select.connect_by.is_some()
        || !select.lateral_views.is_empty()
        || !select.named_window.is_empty()
        || !select.cluster_by.is_empty()
        || !select.distribute_by.is_empty()
        || !select.sort_by.is_empty()
        || select.value_table_mode.is_some()
        || !matches!(&select.group_by, GroupByExpr::Expressions(keys, mods)
            if keys.is_empty() && mods.is_empty())
    {
        return None;
    }
    let [from] = select.from.as_slice() else {
        return None;
    };
    let mut inputs = vec![source(&from.relation, lexical)?];
    let equality = match from.joins.as_slice() {
        [] => None,
        [join] => {
            let JoinOperator::Inner(JoinConstraint::On(Expr::BinaryOp {
                left,
                op: SqlBinaryOperator::Eq,
                right,
            })) = &join.join_operator
            else {
                return None;
            };
            inputs.push(source(&join.relation, lexical)?);
            Some((column(left)?, column(right)?))
        }
        _ => return None,
    };
    if inputs.len() == 2 && inputs[0].alias() == inputs[1].alias() {
        return None;
    }
    let projections = select
        .projection
        .iter()
        .map(|projection| match projection {
            SelectItem::UnnamedExpr(expression) => {
                let ref_column = column(expression)?;
                Some(CteProjection::new(
                    ref_column.column().to_string(),
                    ref_column,
                ))
            }
            SelectItem::ExprWithAlias { expr, alias } => {
                Some(CteProjection::new(alias.value.clone(), column(expr)?))
            }
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;
    if projections.is_empty()
        || projections
            .iter()
            .map(CteProjection::name)
            .collect::<BTreeSet<_>>()
            .len()
            != projections.len()
    {
        return None;
    }
    Some(CteStage::new(id, inputs, projections, equality))
}

fn lower(
    query: &SqlQuery,
    id: String,
    inherited: &BTreeMap<String, String>,
    stages: &mut Vec<CteStage>,
) -> Option<()> {
    let mut lexical = inherited.clone();
    if let Some(with) = &query.with {
        if with.recursive {
            return None;
        }
        for (index, cte) in with.cte_tables.iter().enumerate() {
            // Do not bind this CTE before lowering its definition. A forward
            // reference resolves as external evidence, not a later sibling.
            let name = cte.alias.name.value.clone();
            let local_id = format!("{id}/cte:{index}:{name}");
            // Positional column aliases are not a proven direct projection.
            if !cte.alias.columns.is_empty() {
                return None;
            }
            lower(&cte.query, local_id.clone(), &lexical, stages)?;
            lexical.insert(name, local_id);
        }
    }
    stages.push(select_stage(query, id, &lexical)?);
    Some(())
}

/// Normalize one limited, nonrecursive WITH program into lexical producer facts.
///
/// Retain only direct column projections and equality joins. A missing program
/// denotes an unproven operation, never an empty or automatically exact bag.
pub(super) fn analyze_cte_program(query: &SqlQuery) -> Option<CteProgram> {
    query.with.as_ref()?;
    let terminal = "terminal".to_string();
    let mut stages = Vec::new();
    lower(query, terminal.clone(), &BTreeMap::new(), &mut stages)?;
    Some(CteProgram::new(stages, terminal))
}
