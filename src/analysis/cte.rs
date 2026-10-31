//! Bounded ordinary CTE analysis: SQL AST enters here, never the physical evaluator.
//! Unsupported row shaping, recursion or computed expressions never produce a program.

use super::*;
use crate::protocol::{CteColumnRef, CteInput, CteProgram, CteProjection, CteStage};

fn column(expression: &Expr) -> Option<CteColumnRef> {
    match expression {
        Expr::Identifier(name) if name.quote_style.is_none() => {
            Some(CteColumnRef::new(None, name.value.clone()))
        }
        Expr::CompoundIdentifier(names)
            if names.len() == 2 && names.iter().all(|name| name.quote_style.is_none()) =>
        {
            Some(CteColumnRef::new(
                Some(names[0].value.to_ascii_lowercase()),
                names[1].value.clone(),
            ))
        }
        _ => None,
    }
}

fn source(factor: &TableFactor, lexical: &BTreeMap<String, String>) -> Option<CteInput> {
    let TableFactor::Table {
        name,
        alias,
        args: None,
        with_hints,
        version: None,
        with_ordinality: false,
        partitions,
        json_path: None,
        sample: None,
        index_hints,
    } = factor
    else {
        return None;
    };
    // A physical snapshot of the whole table cannot prove a restricted
    // partition, a hinted scan, or another engine-specific table source.
    if !with_hints.is_empty()
        || !partitions.is_empty()
        || !index_hints.is_empty()
        || alias
            .as_ref()
            .is_some_and(|alias| !alias.columns.is_empty() || alias.name.quote_style.is_some())
        || name.0.iter().any(|part| {
            part.as_ident()
                .is_none_or(|ident| ident.quote_style.is_some())
        })
    {
        return None;
    }
    // Local WITH aliases are single unqualified identifiers. Preserve the
    // external physical relation name, but resolve unquoted lexical names
    // case-insensitively. Quoted names require dialect-specific binding rules.
    let source_name = name.to_string();
    let canonical = if name.0.len() == 1 {
        lexical
            .get(&source_name.to_ascii_lowercase())
            .cloned()
            .unwrap_or_else(|| source_name.clone())
    } else {
        source_name.clone()
    };
    let instance = alias.as_ref().map_or_else(
        || source_name.to_ascii_lowercase(),
        |alias| alias.name.value.to_ascii_lowercase(),
    );
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
            // ClickHouse GLOBAL JOIN has unproved distributed execution laws.
            // A complete local source bag cannot certify that operator.
            if join.global {
                return None;
            }
            let (JoinOperator::Join(JoinConstraint::On(Expr::BinaryOp {
                left,
                op: SqlBinaryOperator::Eq,
                right,
            }))
            | JoinOperator::Inner(JoinConstraint::On(Expr::BinaryOp {
                left,
                op: SqlBinaryOperator::Eq,
                right,
            }))) = &join.join_operator
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
            SelectItem::ExprWithAlias { expr, alias } if alias.quote_style.is_none() => {
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
    // Plain sorting does not alter a bag, but WITH FILL synthesizes rows.
    // Sorting keys must also resolve to executable output labels.
    // Restrict this bounded proof to resolved output labels. A bare source
    // column not projected by SELECT (or a misspelled label) may be legal or
    // invalid depending on query shape; neither can be certified here.
    if query.order_by.as_ref().is_some_and(|order| {
        order.interpolate.is_some()
            || !matches!(
                &order.kind,
                sqlparser::ast::OrderByKind::Expressions(keys)
                    if keys.iter().all(|key| {
                        key.with_fill.is_none()
                            && matches!(
                                &key.expr,
                                Expr::Identifier(name)
                                    if projections.iter().any(|projection| projection.name() == name.value)
                            )
                    })
            )
    }) {
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
        let mut defined_here = BTreeSet::new();
        for (index, cte) in with.cte_tables.iter().enumerate() {
            // Quoted names have dialect-specific case and shadowing rules.
            // Duplicate unquoted CTE names cannot establish a unique binding.
            if cte.alias.name.quote_style.is_some() {
                return None;
            }
            let name = cte.alias.name.value.clone();
            let scope_name = name.to_ascii_lowercase();
            if !defined_here.insert(scope_name.clone()) {
                return None;
            }
            // Do not bind this CTE before lowering its definition. A forward
            // reference resolves as external evidence, not a later sibling.
            let local_id = format!("{id}/cte:{index}:{name}");
            // Positional renames and dialect-specific CTE FROM extensions
            // are not covered by this direct-projection physical proof.
            if !cte.alias.columns.is_empty() || cte.from.is_some() {
                return None;
            }
            let later_siblings = with
                .cte_tables
                .iter()
                .skip(index + 1)
                .map(|future| future.alias.name.value.to_ascii_lowercase())
                .collect::<BTreeSet<_>>();
            let start = stages.len();
            lower(&cte.query, local_id.clone(), &lexical, stages)?;
            // Whether a forward name means an external table or a later
            // CTE is an engine/binder rule. No complete source snapshot
            // can certify the unproven binding.
            if stages[start..].iter().any(|stage| {
                stage.inputs().iter().any(|input| {
                    later_siblings.contains(&input.relation().to_ascii_lowercase())
                        && !lexical.contains_key(&input.relation().to_ascii_lowercase())
                })
            }) {
                return None;
            }
            lexical.insert(scope_name, local_id);
        }
    }
    stages.push(select_stage(query, id, &lexical)?);
    Some(())
}

fn recursive_integer(expr: &Expr) -> Option<i128> {
    let Expr::Value(value) = expr else {
        return None;
    };
    let sqlparser::ast::Value::Number(number, _) = &value.value else {
        return None;
    };
    number.parse::<i128>().ok()
}

fn recursive_select_shape(select: &Select, sources: usize, filtered: bool) -> bool {
    select.from.len() == sources
        && select.selection.is_some() == filtered
        && select.projection.len() == 1
        && select.distinct.is_none()
        && select.top.is_none()
        && select.having.is_none()
        && select.qualify.is_none()
        && select.prewhere.is_none()
        && select.into.is_none()
        && select.exclude.is_none()
        && select.connect_by.is_none()
        && select.lateral_views.is_empty()
        && select.named_window.is_empty()
        && select.cluster_by.is_empty()
        && select.distribute_by.is_empty()
        && select.sort_by.is_empty()
        && select.value_table_mode.is_none()
        && matches!(&select.group_by, GroupByExpr::Expressions(keys, modifiers)
            if keys.is_empty() && modifiers.is_empty())
        && select.from.iter().all(|source| source.joins.is_empty())
}

fn bounded_recursive_program(query: &SqlQuery) -> Option<CteProgram> {
    use crate::protocol::{CteRecursion, CteRecursionSetMode};
    let with = query.with.as_ref()?;
    let [cte] = with.cte_tables.as_slice() else {
        return None;
    };
    if !with.recursive
        || cte.alias.name.quote_style.is_some()
        || cte.alias.columns.len() != 1
        || cte.alias.columns[0].name.quote_style.is_some()
        || cte.from.is_some()
        || cte.query.with.is_some()
        || cte.query.order_by.is_some()
        || cte.query.limit_clause.is_some()
        || cte.query.fetch.is_some()
        || !cte.query.locks.is_empty()
        || cte.query.for_clause.is_some()
        || cte.query.settings.is_some()
        || cte.query.format_clause.is_some()
        || !cte.query.pipe_operators.is_empty()
    {
        return None;
    }
    let name = &cte.alias.name.value;
    let output = &cte.alias.columns[0].name.value;
    let SetExpr::SetOperation {
        op: SqlSetOperator::Union,
        set_quantifier,
        left,
        right,
    } = cte.query.body.as_ref()
    else {
        return None;
    };
    let mode = match set_quantifier {
        SqlSetQuantifier::All => CteRecursionSetMode::All,
        SqlSetQuantifier::None | SqlSetQuantifier::Distinct => CteRecursionSetMode::Distinct,
        _ => return None,
    };
    let (SetExpr::Select(base), SetExpr::Select(step)) = (left.as_ref(), right.as_ref()) else {
        return None;
    };
    if !recursive_select_shape(base, 0, false) || !recursive_select_shape(step, 1, true) {
        return None;
    }
    let seed = match &base.projection[0] {
        SelectItem::UnnamedExpr(expr) | SelectItem::ExprWithAlias { expr, .. } => {
            recursive_integer(expr)?
        }
        _ => return None,
    };
    let input = source(&step.from[0].relation, &BTreeMap::new())?;
    if !input.relation().eq_ignore_ascii_case(name) || !input.alias().eq_ignore_ascii_case(name) {
        return None;
    }
    let expr = match &step.projection[0] {
        SelectItem::UnnamedExpr(expr) | SelectItem::ExprWithAlias { expr, .. } => expr,
        _ => return None,
    };
    let Expr::BinaryOp {
        left,
        op: SqlBinaryOperator::Plus,
        right,
    } = expr
    else {
        return None;
    };
    if column(left)?.column() != output || column(left)?.relation().is_some() {
        return None;
    }
    let increment = recursive_integer(right)?;
    let Expr::BinaryOp { left, op, right } = step.selection.as_ref()? else {
        return None;
    };
    if column(left)?.column() != output || column(left)?.relation().is_some() {
        return None;
    }
    let inclusive = match op {
        SqlBinaryOperator::Lt => false,
        SqlBinaryOperator::LtEq => true,
        _ => return None,
    };
    let bound = recursive_integer(right)?;
    let recursion = CteRecursion::new(output.clone(), seed, increment, bound, inclusive, mode)?;
    let producer = format!("terminal/cte:0:{name}");
    let mut lexical = BTreeMap::new();
    lexical.insert(name.to_ascii_lowercase(), producer.clone());
    let terminal = "terminal".to_string();
    let final_stage = select_stage(query, terminal.clone(), &lexical)?;
    if final_stage.inputs().len() != 1
        || final_stage.inputs()[0].relation() != producer
        || final_stage.projections().iter().any(|projection| {
            projection.source().column() != output
                || projection
                    .source()
                    .relation()
                    .is_some_and(|qualifier| qualifier != final_stage.inputs()[0].alias())
        })
    {
        return None;
    }
    Some(CteProgram::new(
        vec![
            CteStage::recursive(producer, name.to_ascii_lowercase(), recursion),
            final_stage,
        ],
        terminal,
    ))
}

/// Do not reuse source-derived bounds for an outer recursive projection
/// whose explicit SQL qualifier is not bound to the actual FROM instance.
/// This guard also applies when the bounded physical program is rejected:
/// ordinary output analysis may still have seen the recursive seed domain.
pub(super) fn guard_recursive_projection_qualifiers(query: &SqlQuery, output: Output) -> Output {
    if !query.with.as_ref().is_some_and(|with| with.recursive) {
        return output;
    }
    let SetExpr::Select(select) = query.body.as_ref() else {
        return output;
    };
    let [from] = select.from.as_slice() else {
        return output;
    };
    let TableFactor::Table { name, alias, .. } = &from.relation else {
        return output;
    };
    if select.projection.len() != output.columns().len() {
        return output;
    }
    let bound_name = alias
        .as_ref()
        .map_or_else(|| name.to_string(), |alias| alias.name.value.clone());
    Output::new(
        output
            .columns()
            .iter()
            .zip(&select.projection)
            .map(|(column, projection)| {
                let expression = match projection {
                    SelectItem::UnnamedExpr(expr) | SelectItem::ExprWithAlias { expr, .. } => expr,
                    _ => return column.clone(),
                };
                let Expr::CompoundIdentifier(parts) = expression else {
                    return column.clone();
                };
                let [qualifier, _] = parts.as_slice() else {
                    return column.clone();
                };
                if qualifier.quote_style.is_none()
                    && qualifier.value.eq_ignore_ascii_case(&bound_name)
                {
                    return column.clone();
                }
                column.clone().with_domain(ValueDomain::unknown(
                    "recursive output qualifier does not match the source instance",
                ))
            })
            .collect(),
    )
}

pub(super) fn refine_bounded_recursive_output(
    output: Output,
    program: Option<&CteProgram>,
) -> Output {
    let Some(program) = program else {
        return output;
    };
    let [recursive_stage, terminal] = program.stages() else {
        return output;
    };
    let Some(recursion) = recursive_stage.recursion() else {
        return output;
    };
    let Some(values) = recursion.finite_values() else {
        return output;
    };
    if output.columns().len() != terminal.projections().len()
        || !output
            .columns()
            .iter()
            .zip(terminal.projections())
            .all(|(column, projection)| {
                column.name() == projection.name()
                    && projection.source().column() == recursion.column()
            })
    {
        return output;
    }
    let ranges = if recursion.increment() == 1 {
        let Some(first) = values.first() else {
            return output;
        };
        let Some(last) = values.last() else {
            return output;
        };
        vec![ValueRange::new(
            Some(Bound::new(integer_literal(*first), true)),
            Some(Bound::new(integer_literal(*last), true)),
        )]
    } else {
        values
            .iter()
            .map(|value| {
                ValueRange::new(
                    Some(Bound::new(integer_literal(*value), true)),
                    Some(Bound::new(integer_literal(*value), true)),
                )
            })
            .collect()
    };
    let domain = ValueDomain::ranges(ranges);
    Output::new(
        output
            .columns()
            .iter()
            .cloned()
            .map(|column| column.with_domain(domain.clone()))
            .collect(),
    )
}

/// Normalize one limited, nonrecursive WITH program into lexical producer facts.
///
/// Retain only direct column projections and equality joins. A missing program
/// denotes an unproven operation, never an empty or automatically exact bag.
pub(super) fn analyze_cte_program(query: &SqlQuery) -> Option<CteProgram> {
    let with = query.with.as_ref()?;
    if with.recursive {
        return bounded_recursive_program(query);
    }
    let terminal = "terminal".to_string();
    let mut stages = Vec::new();
    lower(query, terminal.clone(), &BTreeMap::new(), &mut stages)?;
    Some(CteProgram::new(stages, terminal))
}
