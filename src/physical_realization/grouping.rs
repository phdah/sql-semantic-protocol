//! Grouping, aggregates and ranked compositions.

use super::*;

/// Evaluate an aggregate-derived Boolean filter against a complete,
/// homogeneous COUNT(*) group population. No coercion, unknown column,
/// aggregate expression or SQL UNKNOWN is silently treated as FALSE.
fn exact_group_count_filter_truth(
    bundle: &AnalysisBundle,
    grouped: &QueryStatement,
    relation: &str,
    predicate: &Predicate,
    count: u64,
) -> Option<bool> {
    use crate::protocol::{AggregateArgument, ComparisonOperator, LiteralType, LiteralValue};
    match predicate {
        Predicate::And(and) => {
            let truths = and
                .operands()
                .iter()
                .map(|item| exact_group_count_filter_truth(bundle, grouped, relation, item, count))
                .collect::<Option<Vec<_>>>()?;
            Some(truths.into_iter().all(|truth| truth))
        }
        Predicate::Or(or) => {
            let truths = or
                .operands()
                .iter()
                .map(|item| exact_group_count_filter_truth(bundle, grouped, relation, item, count))
                .collect::<Option<Vec<_>>>()?;
            Some(truths.into_iter().any(|truth| truth))
        }
        Predicate::Not(not) => Some(!exact_group_count_filter_truth(
            bundle,
            grouped,
            relation,
            not.operand(),
            count,
        )?),
        Predicate::Comparison(comparison) => {
            let (column, literal, reversed) = match (comparison.left(), comparison.right()) {
                (Expression::Column(column), Expression::Literal(literal)) => {
                    (column, literal, false)
                }
                (Expression::Literal(literal), Expression::Column(column)) => {
                    (column, literal, true)
                }
                _ => return None,
            };
            if column.relation().is_some_and(|source| source != relation)
                || literal.literal_type() != LiteralType::Integer
            {
                return None;
            }
            let LiteralValue::Number(number) = literal.value() else {
                return None;
            };
            let threshold = number.parse::<i128>().ok()?;
            let mut projected = grouped
                .output()
                .columns()
                .iter()
                .filter(|item| item.name() == column.name());
            let output = projected.next()?;
            if projected.next().is_some() {
                return None;
            }
            let Expression::AggregateFunction(aggregate) = output.expression() else {
                return None;
            };
            if !aggregate.name().eq_ignore_ascii_case("count")
                || !matches!(aggregate.arguments(), [AggregateArgument::Wildcard])
            {
                return None;
            }
            let reference = crate::protocol::ColumnRef::new(
                Some(relation.to_string()),
                column.name().to_string(),
            );
            let schema = bundle
                .source_schemas()
                .iter()
                .find(|s| s.relation() == relation)?;
            let bounds = physical_integer_evidence(schema, &reference)?;
            let value = i128::from(count);
            if bounds.minimum > value || bounds.maximum < value {
                return None;
            }
            let (left, right) = if reversed {
                (threshold, value)
            } else {
                (value, threshold)
            };
            Some(match comparison.operator() {
                ComparisonOperator::Eq => left == right,
                ComparisonOperator::Neq | ComparisonOperator::IsDistinctFrom => left != right,
                ComparisonOperator::Lt => left < right,
                ComparisonOperator::Lte => left <= right,
                ComparisonOperator::Gt => left > right,
                ComparisonOperator::Gte => left >= right,
                ComparisonOperator::IsNotDistinctFrom => left == right,
            })
        }
        _ => None,
    }
}

/// Compose a proven complete GROUP BY COUNT(*) population with a later
/// materialized aggregate-output filter. Every constructed group has exactly
/// the same contributor count, so a scalar comparison of that COUNT(*) output
/// is decidable for *all* groups, including deliberate zero-output rejection.
/// No intermediate grouped relation is independently writable.
pub(super) fn physical_group_aggregate_filter_count_plan(
    bundle: &AnalysisBundle,
    target_layer_id: &str,
    wanted: u64,
) -> Option<WitnessDirection> {
    use crate::outcome_proofs::OutcomeWitness;
    let mut walker = Walker::new(bundle);
    walker.visit(target_layer_id).ok()?;
    let target_node = walker
        .nodes
        .iter()
        .find(|node| node.id() == &PhysicalPlanRef::Layer(target_layer_id.to_string()))?;
    let [PhysicalPlanRef::Layer(group_id)] = target_node.inputs() else {
        return None;
    };
    let target_layer = walker.layers.get(target_layer_id).copied()?;
    let query = query_for(bundle, target_layer)?;
    let group_layer = walker.layers.get(group_id.as_str()).copied()?;
    let grouped = query_for(bundle, group_layer)?;
    if !query.filter_only_row_shape()
        || !query.joins().is_empty()
        || query.sources().len() != 1
        || !query
            .output()
            .columns()
            .iter()
            .all(|col| col.plain_copy_source().is_some())
        || !matches!(target_node.write_kind(), None | Some(WriteKind::Definition))
        || !grouped.group_rows_match_surviving_groups()
        || grouped.aggregation().is_none()
    {
        return None;
    }
    let relation = query.sources()[0].name();
    let producer_node = walker
        .nodes
        .iter()
        .find(|node| node.id() == &PhysicalPlanRef::Layer(group_id.clone()))?;
    if !matches!(producer_node.write_kind(), Some(WriteKind::Definition))
        || !producer_node
            .produced_relations()
            .contains(&relation.to_string())
    {
        return None;
    }

    let predicate = query.predicates().where_predicate()?;
    let groups = wanted.max(1);
    let (population, counts, group_case) =
        physical_operator_population_case(bundle, group_id, groups)?;
    let OutcomeWitness::Groups {
        rows_per_group,
        groups: witnessed_groups,
        ..
    } = population
    else {
        return None;
    };
    if witnessed_groups != groups
        || counts.is_empty()
        || exact_group_count_filter_truth(bundle, grouped, relation, predicate, rows_per_group)?
            != (wanted > 0)
    {
        return None;
    }

    let boundary =
        WitnessBoundary::new(relation, GroupBoundaryKind::Intermediate, target_layer_id)?;
    let mut obligations = group_case.obligations().to_vec();
    obligations.push(WitnessObligation::Producer {
        boundary,
        physical_sources: counts.keys().cloned().collect(),
    });
    obligations.push(WitnessObligation::OutputRows {
        layer_id: target_layer_id.to_string(),
        bounds: CountBounds::new(wanted, Some(wanted))?,
    });
    let case = WitnessCase::new(obligations, ProofStrength::Sufficient)?;
    WitnessDirection::feasible(vec![case])
}

/// One group is emitted per distinct key, so ROW_NUMBER() partitioned by
/// that key has exactly one row in every partition. This is a complete
/// source-level group assignment, not an independent sample of ranked rows.
pub(super) fn physical_group_rank_count_plan(
    bundle: &AnalysisBundle,
    target_layer_id: &str,
    rows: u64,
) -> Option<WitnessDirection> {
    use crate::outcome_proofs::OutcomeWitness;
    use crate::window_witness::WindowWitnessDirection;
    if rows == 0 {
        return None;
    }
    let mut walker = Walker::new(bundle);
    walker.visit(target_layer_id).ok()?;
    let target_node = walker
        .nodes
        .iter()
        .find(|node| node.id() == &PhysicalPlanRef::Layer(target_layer_id.to_string()))?;
    let [PhysicalPlanRef::Layer(group_id)] = target_node.inputs() else {
        return None;
    };
    let rank_layer = walker.layers.get(target_layer_id).copied()?;
    let rank_query = query_for(bundle, rank_layer)?;
    let group_layer = walker.layers.get(group_id.as_str()).copied()?;
    let group_query = query_for(bundle, group_layer)?;
    if !matches!(target_node.write_kind(), None | Some(WriteKind::Definition))
        || !rank_query.ranked_goal_output_shape()
        || !rank_query.joins().is_empty()
        || rank_query.aggregation().is_some()
        || rank_query.set_operation().is_some()
        || !rank_query.subquery_witnesses().is_empty()
        || rank_query.sources().len() != 1
        || !group_query.group_rows_match_surviving_groups()
        || group_query.aggregation().is_none()
    {
        return None;
    }
    let group_relation = rank_query.sources()[0].name();
    let group_node = walker
        .nodes
        .iter()
        .find(|node| node.id() == &PhysicalPlanRef::Layer(group_id.clone()))?;
    if !matches!(group_node.write_kind(), Some(WriteKind::Definition))
        || !group_node
            .produced_relations()
            .contains(&group_relation.to_string())
    {
        return None;
    }
    let local = rank_query.window_witness()?;
    let [partition] = local.partition_by() else {
        return None;
    };
    let [order] = local.order_by() else {
        return None;
    };
    if local.boundary() != Some(group_relation)
        || partition.relation() != Some(group_relation)
        || order.column().relation() != Some(group_relation)
        || partition.name() != order.column().name()
        || local.limit().is_none_or(|limit| limit == 0)
        || !matches!(local.qualifying(), WindowWitnessDirection::Exact(case)
            if case.min_preceding() == 0)
    {
        return None;
    }

    let [group_input] = group_query.sources() else {
        return None;
    };
    let group_key = match group_query.aggregation()?.group_by()? {
        GroupBy::Expressions(keys) if keys.len() == 1 => match &keys[0] {
            GroupingExpression::Expression(Expression::Column(key)) => key,
            _ => return None,
        },
        _ => return None,
    };
    let source_key = group_query
        .output()
        .columns()
        .iter()
        .filter(|col| col.name() == partition.name())
        .collect::<Vec<_>>();
    let [source_key] = source_key.as_slice() else {
        return None;
    };
    let copied = source_key.plain_copy_source()?;
    if copied.relation() != group_input.name() || copied.column() != group_key.name() {
        return None;
    }
    let grouped_type = bundle
        .source_schemas()
        .iter()
        .find(|schema| schema.relation() == group_relation)?
        .columns()
        .iter()
        .find(|column| column.name() == partition.name())?
        .data_type();
    let input_type = bundle
        .source_schemas()
        .iter()
        .find(|schema| schema.relation() == group_input.name())?
        .columns()
        .iter()
        .find(|column| column.name() == group_key.name())?
        .data_type();
    if grouped_type != input_type {
        return None;
    }

    let cases = if let Some((population, counts, case)) =
        physical_operator_population_case(bundle, group_id, rows)
    {
        let OutcomeWitness::Groups {
            relation, groups, ..
        } = population
        else {
            return None;
        };
        if groups != rows || !counts.contains_key(&relation) {
            return None;
        }
        vec![case]
    } else {
        let WitnessDirection::Feasible(cases) =
            physical_join_group_count_plan(bundle, group_id, rows)?
        else {
            return None;
        };
        cases
    };
    let mut composed = Vec::new();
    for group_case in cases {
        let boundary = WitnessBoundary::new(
            group_relation,
            GroupBoundaryKind::Intermediate,
            target_layer_id,
        )?;
        let candidate = RowVariable::new(group_relation, group_relation, "group_rank")?;
        let mut obligations = group_case.obligations().to_vec();
        obligations.push(WitnessObligation::Producer {
            boundary: boundary.clone(),
            physical_sources: walker.sources.iter().cloned().collect(),
        });
        obligations.push(WitnessObligation::Ranked {
            boundary,
            candidate,
            partition_by: vec![partition.clone()],
            order_by: vec![order.clone()],
            preceding: CountBounds::new(0, Some(0))?,
            strict_unique: true,
            closed_world: true,
        });
        obligations.push(WitnessObligation::OutputRows {
            layer_id: target_layer_id.to_string(),
            bounds: CountBounds::new(rows, Some(rows))?,
        });
        if let Some(case) = WitnessCase::new(obligations, ProofStrength::Sufficient) {
            composed.push(case);
        }
    }
    WitnessDirection::feasible(composed)
}

/// Prove the smallest nonempty GROUP/HAVING construction over a filtered
/// two-parent join. One *complete* matching join pair produces exactly one
/// group for any total scalar group key. A downstream WHERE is admitted only
/// when its equality was already enforced on every row by the matching
/// upstream source producer. No intermediate relation is independently seeded.
fn physical_filtered_join_single_group(
    bundle: &AnalysisBundle,
    target_layer_id: &str,
    wanted: u64,
) -> Option<WitnessDirection> {
    use crate::protocol::{AggregateArgument, ComparisonOperator, LiteralType, LiteralValue};

    if wanted != 1 {
        return None;
    }
    eprintln!("PFG TRACE start target={target_layer_id} wanted={wanted}");
    let mut walker = Walker::new(bundle);
    walker.visit(target_layer_id).ok()?;
    let target = walker.layers.get(target_layer_id).copied()?;
    let group_query = query_for(bundle, target)?;
    let group_node = walker
        .nodes
        .iter()
        .find(|node| node.id() == &PhysicalPlanRef::Layer(target_layer_id.to_string()))?;
    let [PhysicalPlanRef::Layer(join_id)] = group_node.inputs() else {
        return None;
    };
    let join_layer = walker.layers.get(join_id.as_str()).copied()?;
    let join_query = query_for(bundle, join_layer)?;
    eprintln!("PFG TRACE found join id={join_id} group inputs={:?}", group_node.inputs());
    let [source] = group_query.sources() else {
        return None;
    };
    if !group_node
        .inputs()
        .iter()
        .all(|input| matches!(input, PhysicalPlanRef::Layer(_)))
        || !matches!(group_node.write_kind(), None | Some(WriteKind::Definition))
        || !group_query.group_rows_match_surviving_groups()
        || !group_query.joins().is_empty()
        || group_query.window_witness().is_some()
        || group_query.set_operation().is_some()
        || !group_query.subquery_witnesses().is_empty()
        || !group_query
            .diagnostics()
            .iter()
            .all(|item| item.code() == "unsupported_order_by")
        || join_query.joins().len() != 1
        || !walker
            .nodes
            .iter()
            .find(|node| node.id() == &PhysicalPlanRef::Layer(join_id.clone()))?
            .produced_relations()
            .contains(&source.name().to_string())
    {
        return None;
    }
    eprintln!("PFG TRACE group gate passed");
    let key = match group_query.aggregation()?.group_by()? {
        GroupBy::Expressions(keys) if keys.len() == 1 => match &keys[0] {
            GroupingExpression::Expression(Expression::Column(key)) => key,
            _ => return None,
        },
        _ => return None,
    };
    if key.relation().is_some_and(|relation| {
        relation != source.name() && source.alias() != Some(relation)
    }) {
        return None;
    }
    // The group key is a deterministic per-row value projected by the join.
    // Its upstream projection may be CASE, but its expression must be total:
    // one joined row then has exactly one (possibly NULL) grouping key.
    let key_output = join_query
        .output()
        .columns()
        .iter()
        .find(|column| column.name() == key.name())?;
    let key_origin = key_output.plain_copy_source()?;
    eprintln!("PFG TRACE key_origin={key_origin:?}");
    // Query source aliases are scoped to this SELECT; a layer-level producer
    // identity alone does not necessarily contain its local SQL alias.
    let unique_join_source = |name: &str| {
        let mut matches = join_query.sources().iter().filter(|input| {
            input.name() == name || input.alias() == Some(name)
        });
        let first = matches.next()?;
        matches.next().is_none().then_some(first.name())
    };
    let key_stage_name = unique_join_source(key_origin.relation())?.to_string();
    let stage_node = walker.nodes.iter().find(|node| {
        node.produced_relations()
            .contains(&key_stage_name.to_string())
    })?;
    let PhysicalPlanRef::Layer(stage_id) = stage_node.id() else {
        return None;
    };
    let stage_query = query_for(bundle, walker.layers.get(stage_id.as_str()).copied()?)?;
    let key_stage = stage_query
        .output()
        .columns()
        .iter()
        .find(|column| column.name() == key_origin.column())?;
    eprintln!("PFG TRACE stage key total={} expr={:?}", total_scalar_projection(key_stage.expression()), key_stage.expression());
    if !total_scalar_projection(key_stage.expression()) {
        return None;
    }

    // WHERE on the grouped input may never be assumed TRUE merely because
    // a concrete DuckDB example passed. It must be the exact same equality
    // as a complete upstream source filter, transported through plain copies.
    eprintln!("PFG TRACE reached grouped WHERE");
    if let Some(predicate) = group_query.predicates().where_predicate() {
        let Predicate::Comparison(comparison) = predicate else {
            return None;
        };
        if comparison.operator() != ComparisonOperator::Eq {
            return None;
        }
        let (Expression::Column(column), Expression::Literal(value)) =
            (comparison.left(), comparison.right())
        else {
            return None;
        };
        if column.relation().is_some_and(|name| {
            name != source.name() && source.alias() != Some(name)
        }) {
            return None;
        }
        let origin = join_query
            .output()
            .columns()
            .iter()
            .find(|output| output.name() == column.name())?
            .plain_copy_source()?;
        let stage_name = unique_join_source(origin.relation())?.to_string();
        let stage_node = walker
            .nodes
            .iter()
            .find(|node| node.produced_relations().contains(&stage_name.to_string()))?;
        let PhysicalPlanRef::Layer(stage_id) = stage_node.id() else {
            return None;
        };
        let stage_layer = walker.layers.get(stage_id.as_str()).copied()?;
        let stage_query = query_for(bundle, stage_layer)?;
        let copy = stage_query
            .output()
            .columns()
            .iter()
            .find(|output| output.name() == origin.column())?
            .plain_copy_source()?;
        let Predicate::Comparison(filter) = stage_query.predicates().where_predicate()? else {
            return None;
        };
        let (Expression::Column(filter_column), Expression::Literal(filter_value)) =
            (filter.left(), filter.right())
        else {
            return None;
        };
        if filter.operator() != ComparisonOperator::Eq
            || filter_value != value
            || filter_column.name() != copy.column()
            || filter_column.relation().is_some_and(|name| {
                stage_query.sources()[0].name() != name
                    && stage_query.sources()[0].alias() != Some(name)
            })
            || stage_query.sources()[0].name() != copy.relation()
            || !matches!(
                value.value(),
                LiteralValue::Number(_) | LiteralValue::Boolean(_)
            )
        {
            return None;
        }
    }

    // Count(*) from a one-row complete join bag is exactly one, including
    // when the grouping key is NULL. The existing local HAVING witness is
    // conservatively residual for a preceding WHERE; independently prove
    // this precise typed law only after the WHERE has been linked to the
    // complete upstream source filter above.
    eprintln!("PFG TRACE WHERE passed");
    let Predicate::Comparison(having) = group_query.predicates().having_predicate()? else {
        return None;
    };
    let (function, operator, literal) = match (having.left(), having.right()) {
        (Expression::AggregateFunction(function), Expression::Literal(literal)) => {
            (function, having.operator(), literal)
        }
        (Expression::Literal(literal), Expression::AggregateFunction(function)) => {
            (function, having.operator().reversed(), literal)
        }
        _ => return None,
    };
    if !function.name().eq_ignore_ascii_case("count")
        || !matches!(function.arguments(), [AggregateArgument::Wildcard])
        || function.distinct()
        || function.filter().is_some()
        || literal.literal_type() != LiteralType::Integer
    {
        return None;
    }
    let LiteralValue::Number(threshold) = literal.value() else {
        return None;
    };
    let value = threshold.parse::<i128>().ok()?;
    let survives = match operator {
        ComparisonOperator::Eq | ComparisonOperator::IsNotDistinctFrom => 1 == value,
        ComparisonOperator::Neq | ComparisonOperator::IsDistinctFrom => 1 != value,
        ComparisonOperator::Lt => 1 < value,
        ComparisonOperator::Lte => 1 <= value,
        ComparisonOperator::Gt => 1 > value,
        ComparisonOperator::Gte => 1 >= value,
    };
    eprintln!("PFG TRACE HAVING evaluated survives={survives}");
    if !survives {
        return None;
    }
    eprintln!("PFG TRACE seeking join case");
    let WitnessDirection::Feasible(join_cases) =
        physical_join_population_count_plan(bundle, join_id, 1)?
    else {
        return None;
    };

    let group_boundary = WitnessBoundary::new(
        source.name(),
        GroupBoundaryKind::Intermediate,
        target_layer_id,
    )?;
    let bounds = CountBounds::new(1, Some(1))?;
    let mut proven = Vec::new();
    for join_case in join_cases {
        let mut obligations = join_case.obligations().to_vec();
        obligations.push(WitnessObligation::Producer {
            boundary: group_boundary.clone(),
            physical_sources: walker.sources.iter().cloned().collect(),
        });
        obligations.push(WitnessObligation::Group {
            boundary: group_boundary.clone(),
            key: vec![crate::protocol::ColumnRef::new(
                Some(source.name().to_string()),
                key.name().to_string(),
            )],
            aggregate: crate::group_witness::GroupAggregate::CountRows,
            distinct: false,
            argument: None,
            rows: bounds,
            non_null: bounds,
            tests: Vec::new(),
        });
        obligations.push(WitnessObligation::OutputRows {
            layer_id: target_layer_id.to_string(),
            bounds,
        });
        if let Some(case) = WitnessCase::new(obligations, ProofStrength::Sufficient) {
            proven.push(case);
        }
    }
    WitnessDirection::feasible(proven)
}

/// Lift a complete equijoin population into a grouped output using the same
/// physical key distribution. The distinct, common and homogeneous grouped
/// patterns certify one, many-in-one and multiple multi-contributor groups.
/// Aggregate/HAVING obligations are taken from the exact local group witness;
/// unsupported key mappings and heterogeneous bags remain residual.
pub(super) fn physical_join_group_count_plan(
    bundle: &AnalysisBundle,
    target_layer_id: &str,
    rows: u64,
) -> Option<WitnessDirection> {
    use crate::outcome_proofs::OutcomeWitness;
    if rows == 0 {
        return None;
    }
    if let Some(plan) = physical_filtered_join_single_group(bundle, target_layer_id, rows) {
        return Some(plan);
    }
    let mut walker = Walker::new(bundle);
    walker.visit(target_layer_id).ok()?;
    let group_node = walker
        .nodes
        .iter()
        .find(|node| node.id() == &PhysicalPlanRef::Layer(target_layer_id.to_string()))?;
    let [PhysicalPlanRef::Layer(join_id)] = group_node.inputs() else {
        return None;
    };
    let group_layer = walker.layers.get(target_layer_id).copied()?;
    let join_layer = walker.layers.get(join_id.as_str()).copied()?;
    let group_query = query_for(bundle, group_layer)?;
    let join_query = query_for(bundle, join_layer)?;
    if !matches!(group_node.write_kind(), None | Some(WriteKind::Definition))
        || !group_query.group_rows_match_surviving_groups()
        || group_query.sources().len() != 1
        || !group_query.joins().is_empty()
        || group_query.set_operation().is_some()
        || group_query.window_witness().is_some()
        || group_query.predicates().where_predicate().is_some()
        || !group_query.subquery_witnesses().is_empty()
        || join_query.joins().len() != 1
    {
        return None;
    }
    let [source] = group_query.sources() else {
        return None;
    };
    // A grouped consumer must read the join's actual named materialization,
    // not a detached query output or another alias with the same column.
    if !walker
        .nodes
        .iter()
        .find(|node| node.id() == &PhysicalPlanRef::Layer(join_id.clone()))?
        .produced_relations()
        .contains(&source.name().to_string())
    {
        return None;
    }
    let group_key = match group_query.aggregation()?.group_by()? {
        GroupBy::Expressions(keys) if keys.len() == 1 => match &keys[0] {
            GroupingExpression::Expression(Expression::Column(key)) => key,
            _ => return None,
        },
        _ => return None,
    };
    // Named join output is a plain identity copy of one join input. Source
    // datatypes must match across the join output and the physical endpoint.
    let projected = join_query
        .output()
        .columns()
        .iter()
        .filter(|col| col.name() == group_key.name())
        .collect::<Vec<_>>();
    let [projected] = projected.as_slice() else {
        return None;
    };
    let origin = projected.plain_copy_source()?;
    let original = crate::protocol::ColumnRef::new(
        Some(origin.relation().to_string()),
        origin.column().to_string(),
    );
    let physical_key =
        resolve_filter_column_with_evidence(bundle, &walker, join_id, &original, 0, true)?;
    let grouped_type = bundle
        .source_schemas()
        .iter()
        .find(|schema| schema.relation() == source.name())?
        .columns()
        .iter()
        .find(|column| column.name() == group_key.name())?
        .data_type();
    let physical_relation = physical_key.relation()?;
    let physical_type = bundle
        .source_schemas()
        .iter()
        .find(|schema| schema.relation() == physical_relation)?
        .columns()
        .iter()
        .find(|column| column.name() == physical_key.name())?
        .data_type();
    if grouped_type != physical_type {
        return None;
    }

    let group_goal =
        crate::outcome_goals::OutcomeGoal::new(target_layer_id, Some(rows), Some(rows), Vec::new())
            .ok()?;
    let mapper = |key: &crate::protocol::ColumnRef| {
        (key.relation() == Some(source.name()) && key.name() == group_key.name())
            .then(|| physical_key.clone())
    };
    let OutcomeWitness::Groups {
        relation,
        key,
        groups,
        rows_per_group,
    } = crate::outcome_proofs::construct_mapped_group(
        bundle,
        group_query,
        &group_goal,
        rows,
        &mapper,
    )?
    else {
        return None;
    };
    // Every distinct matched key creates exactly one contributor. A
    // common matched key creates one group with the complete join bag.
    let join_rows = rows.checked_mul(rows_per_group)?;
    let WitnessDirection::Feasible(join_cases) =
        physical_join_population_count_plan(bundle, join_id, join_rows)?
    else {
        return None;
    };
    let local = group_query.group_witness()?;
    let group_tests = match local.qualifying() {
        crate::group_witness::GroupWitnessDirection::Exact(cases) => cases
            .iter()
            .find(|case| {
                case.min_rows() <= rows_per_group
                    && case.max_rows().is_none_or(|max| rows_per_group <= max)
            })?
            .tests()
            .to_vec(),
        crate::group_witness::GroupWitnessDirection::Residual { .. } => return None,
    };
    let mut cases = Vec::new();
    for join_case in join_cases {
        let population = join_case
            .obligations()
            .iter()
            .find_map(|obligation| match obligation {
                WitnessObligation::JoinPopulation {
                    pattern,
                    left_key,
                    right_key,
                    output_rows,
                    ..
                } if *output_rows == join_rows && (&key == left_key || &key == right_key) => {
                    Some(*pattern)
                }
                _ => None,
            });
        let proven = matches!(
            population,
            Some(JoinPopulationPattern::DistinctMatched) if rows_per_group == 1
        ) || matches!(
            population,
            Some(JoinPopulationPattern::CommonMatched) if groups == 1
        ) || matches!(
            population,
            Some(JoinPopulationPattern::GroupedMatched {
                groups: key_groups
            }) if key_groups == groups && groups > 1
        );
        if !proven {
            continue;
        }
        let group_boundary = WitnessBoundary::new(
            source.name(),
            GroupBoundaryKind::Intermediate,
            target_layer_id,
        )?;
        let bounds = CountBounds::new(rows_per_group, Some(rows_per_group))?;
        let mut obligations = join_case.obligations().to_vec();
        obligations.push(WitnessObligation::Producer {
            boundary: group_boundary.clone(),
            physical_sources: walker.sources.iter().cloned().collect(),
        });
        obligations.push(WitnessObligation::Group {
            boundary: group_boundary,
            key: vec![crate::protocol::ColumnRef::new(
                Some(source.name().to_string()),
                group_key.name().to_string(),
            )],
            aggregate: local.aggregate()?,
            distinct: false,
            argument: local.argument().map(|_| {
                crate::protocol::ColumnRef::new(
                    Some(source.name().to_string()),
                    group_key.name().to_string(),
                )
            }),
            rows: bounds,
            // All matched equijoin population keys are non-NULL, so
            // COUNT(group_key) and COUNT(*) have equal contributors.
            non_null: bounds,
            tests: group_tests.clone(),
        });
        if rows_per_group == 1 {
            // For a distinct bijection, the grouping key's controlled leaf
            // really does have one physical row per output group.
            obligations.push(WitnessObligation::OperatorPopulation {
                witness: OutcomeWitness::Groups {
                    relation: relation.clone(),
                    key: key.clone(),
                    groups,
                    rows_per_group,
                },
            });
        }
        obligations.push(WitnessObligation::OutputRows {
            layer_id: target_layer_id.to_string(),
            bounds: CountBounds::new(rows, Some(rows))?,
        });
        if let Some(case) = WitnessCase::new(obligations, ProofStrength::Sufficient) {
            cases.push(case);
        }
    }
    WitnessDirection::feasible(cases)
}
