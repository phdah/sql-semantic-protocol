//! Complete physical rank partitions through identity-only producers.

use super::*;

/// Construct a closed physical source with distinct integer ordering tuples
/// inside each complete ROW_NUMBER partition. The sole local QUALIFY witness
/// owns the rank law; the graph composer proves a shared, materialized input
/// and carries a complete per-partition key population to physical leaves.
///
/// Every partition key is one of 0..partitions and every order key is one of
/// 0..rows_per_partition within that partition. Therefore exactly
/// min(rows_per_partition, limit) rows survive in each partition, independent
/// of execution order, duplicate source rows or an unspecified tie breaker.
pub(super) fn physical_rank_partition_count_plan(
    bundle: &AnalysisBundle,
    target_layer_id: &str,
    wanted: u64,
) -> Option<WitnessDirection> {
    use crate::window_witness::WindowWitnessDirection;

    let mut walker = Walker::new(bundle);
    walker.visit(target_layer_id).ok()?;
    if walker.sources.len() != 1 {
        return None;
    }
    let mut operator_id = None;
    for node in &walker.nodes {
        let PhysicalPlanRef::Layer(id) = node.id() else {
            continue;
        };
        if !matches!(node.write_kind(), None | Some(WriteKind::Definition)) {
            return None;
        }
        let layer = walker.layers.get(id.as_str()).copied()?;
        let query = query_for(bundle, layer)?;
        if query.window_witness().is_some() {
            if operator_id.is_some()
                || !query.ranked_goal_output_shape()
                || query.sources().len() != 1
                || query.aggregation().is_some()
                || query.set_operation().is_some()
                || !query.joins().is_empty()
                || !query.subquery_witnesses().is_empty()
                || !query.diagnostics().is_empty()
            {
                return None;
            }
            operator_id = Some(id.as_str());
        } else if !transparent_projection(query) {
            return None;
        }
    }
    let operator_id = operator_id?;
    let operator_layer = walker.layers.get(operator_id).copied()?;
    let query = query_for(bundle, operator_layer)?;
    let local = query.window_witness()?;
    let [partition_key] = local.partition_by() else {
        return None;
    };
    let [order] = local.order_by() else {
        return None;
    };
    if local.boundary() != Some(query.sources()[0].name())
        || local.limit().is_none()
        || query.predicates().where_predicate().is_some()
        || query.predicates().having_predicate().is_some()
    {
        return None;
    }
    let limit = local.limit()?;
    match local.qualifying() {
        WindowWitnessDirection::Exact(case) if limit > 0 && case.min_preceding() == 0 => {}
        WindowWitnessDirection::Impossible if limit == 0 && wanted == 0 => {}
        _ => return None,
    }

    // The output must be the rank operator or a linear, row-preserving
    // materialization of its output. No other operator may shape its rows.
    let mut current = target_layer_id;
    for _ in 0..walker.layers.len() {
        if current == operator_id {
            break;
        }
        let node = walker
            .nodes
            .iter()
            .find(|node| node.id() == &PhysicalPlanRef::Layer(current.to_string()))?;
        let [PhysicalPlanRef::Layer(parent)] = node.inputs() else {
            return None;
        };
        current = parent;
    }
    if current != operator_id {
        return None;
    }
    let rank_input = query.sources()[0].name();
    let source = physical_transparent_producer_leaf(bundle, &walker, rank_input)?;
    if !walker.sources.contains(&source) {
        return None;
    }
    let partition =
        resolve_filter_column_with_evidence(bundle, &walker, operator_id, partition_key, 0, true)?;
    let order_key =
        resolve_filter_column_with_evidence(bundle, &walker, operator_id, order.column(), 0, true)?;
    if partition.relation() != Some(source.as_str())
        || order_key.relation() != Some(source.as_str())
        || partition == order_key
    {
        return None;
    }
    let schema = bundle
        .source_schemas()
        .iter()
        .find(|item| item.relation() == source)?;
    if bundle.relation_constraints().iter().any(|set| {
        set.relation() == source && (!set.constraints().is_empty() || !set.diagnostics().is_empty())
    }) {
        return None;
    }
    let partition_domain = physical_integer_evidence(schema, &partition)?;
    let order_domain = physical_integer_evidence(schema, &order_key)?;
    if partition_domain.minimum > 0 || order_domain.minimum > 0 {
        return None;
    }

    let mut shapes = Vec::new();
    if wanted == 0 {
        // A completely empty input proves zero output for a positive rank
        // bound. Rank <= 0 rejects even one nonempty physical row.
        shapes.push((if limit == 0 { 1 } else { 0 }, 1));
    } else {
        shapes.push((wanted, 1));
        if wanted <= limit {
            shapes.push((1, wanted));
        }
        if limit > 0 && wanted.is_multiple_of(limit) {
            if let Some(size) = limit.checked_add(1) {
                shapes.push((wanted / limit, size));
            }
        }
        for selected in 2..=limit.min(wanted).min(64) {
            if wanted.is_multiple_of(selected) {
                shapes.push((wanted / selected, selected));
            }
        }
    }
    shapes.sort();
    shapes.dedup();

    let mut cases = Vec::new();
    for (partitions, rows_per_partition) in shapes {
        let Some(source_rows) = partitions.checked_mul(rows_per_partition) else {
            continue;
        };
        let selected = rows_per_partition.min(limit);
        if partitions.checked_mul(selected) != Some(wanted)
            || (partitions > 0 && i128::from(partitions - 1) > partition_domain.maximum)
            || i128::from(rows_per_partition - 1) > order_domain.maximum
        {
            continue;
        }
        let Some(boundary) =
            WitnessBoundary::new(&source, GroupBoundaryKind::Physical, target_layer_id)
        else {
            continue;
        };
        let mapped_order = order.with_column(order_key.clone());
        let mut obligations = vec![
            WitnessObligation::Rows {
                boundary: boundary.clone(),
                quantifier: RowQuantifier::ForAll,
                bounds: CountBounds::new(source_rows, Some(source_rows))?,
                predicate: count_tautology(),
                closed_world: true,
            },
            WitnessObligation::ClosedWorld {
                boundary: boundary.clone(),
                coverage: ClosedWorldCoverage::EntireRelation,
            },
        ];
        if source_rows > 0 {
            obligations.push(WitnessObligation::RankPopulation {
                boundary: boundary.clone(),
                partition_key: partition.clone(),
                order: mapped_order.clone(),
                partitions,
                rows_per_partition,
                rank_limit: limit,
                closed_world: true,
            });
        }
        if rank_input != source {
            let intermediate =
                WitnessBoundary::new(rank_input, GroupBoundaryKind::Intermediate, target_layer_id)?;
            obligations.push(WitnessObligation::Producer {
                boundary: intermediate,
                physical_sources: vec![source.clone()],
            });
        }
        obligations.push(WitnessObligation::OutputRows {
            layer_id: target_layer_id.to_string(),
            bounds: CountBounds::new(wanted, Some(wanted))?,
        });
        if let Some(case) = WitnessCase::new(obligations, ProofStrength::Sufficient) {
            cases.push(case);
        }
    }
    WitnessDirection::feasible(cases)
}

/// Lift a matched equijoin's complete keyed bag through one materialized
/// ROW_NUMBER/QUALIFY. When the window partition and order columns both come
/// from the join's repeated side, the other side contributes exactly one
/// matching partner per key. The ranked side can therefore be assigned a
/// strictly ordered complete partition without changing the join's bag law.
pub(super) fn physical_join_rank_count_plan(
    bundle: &AnalysisBundle,
    target_layer_id: &str,
    wanted: u64,
) -> Option<WitnessDirection> {
    use crate::window_witness::WindowWitnessDirection;
    if wanted == 0 {
        return None;
    }
    let mut walker = Walker::new(bundle);
    walker.visit(target_layer_id).ok()?;
    let rank_node = walker
        .nodes
        .iter()
        .find(|node| node.id() == &PhysicalPlanRef::Layer(target_layer_id.to_string()))?;
    let [PhysicalPlanRef::Layer(join_id)] = rank_node.inputs() else {
        return None;
    };
    let rank_layer = walker.layers.get(target_layer_id).copied()?;
    let rank_query = query_for(bundle, rank_layer)?;
    let join_layer = walker.layers.get(join_id.as_str()).copied()?;
    let join_query = query_for(bundle, join_layer)?;
    let join_node = walker
        .nodes
        .iter()
        .find(|node| node.id() == &PhysicalPlanRef::Layer(join_id.clone()))?;
    if !rank_query.ranked_goal_output_shape()
        || rank_query.sources().len() != 1
        || rank_query.aggregation().is_some()
        || rank_query.set_operation().is_some()
        || !rank_query.joins().is_empty()
        || !rank_query.subquery_witnesses().is_empty()
        || !rank_query.diagnostics().is_empty()
        || !matches!(rank_node.write_kind(), None | Some(WriteKind::Definition))
        || !matches!(join_node.write_kind(), Some(WriteKind::Definition))
        || join_query.joins().len() != 1
    {
        return None;
    }
    let input = rank_query.sources()[0].name();
    if !join_node.produced_relations().contains(&input.to_string()) {
        return None;
    }
    let local = rank_query.window_witness()?;
    let [partition] = local.partition_by() else {
        return None;
    };
    let [order] = local.order_by() else {
        return None;
    };
    let limit = local.limit()?;
    if limit == 0
        || local.boundary() != Some(input)
        || !matches!(local.qualifying(), WindowWitnessDirection::Exact(case)
            if case.min_preceding() == 0)
    {
        return None;
    }
    // Both observed window columns must originate from the same side of the
    // materialized join and preserve declared datatypes at the boundary.
    let map_output = |column: &crate::protocol::ColumnRef| {
        if column.relation() != Some(input) {
            return None;
        }
        let mut projected = join_query
            .output()
            .columns()
            .iter()
            .filter(|item| item.name() == column.name());
        let source = projected.next()?.plain_copy_source()?;
        if projected.next().is_some() {
            return None;
        }
        let origin = crate::protocol::ColumnRef::new(
            Some(source.relation().to_string()),
            source.column().to_string(),
        );
        let mapped = resolve_filter_column_with_evidence(
            bundle, &walker, join_id, &origin, 0, true
        )?;
        let produced_type = bundle.source_schemas().iter()
            .find(|schema| schema.relation() == input)?
            .columns().iter().find(|item| item.name() == column.name())?
            .data_type();
        let mapped_relation = mapped.relation()?;
        let source_type = bundle.source_schemas().iter()
            .find(|schema| schema.relation() == mapped_relation)?
            .columns().iter().find(|item| item.name() == mapped.name())?
            .data_type();
        (produced_type == source_type).then_some(mapped)
    };
    let key = map_output(partition)?;
    let ordered = map_output(order.column())?;
    if key.relation() != ordered.relation()
        || key == ordered
        || !walker.sources.contains(key.relation()?)
    {
        return None;
    }
    let physical_relation = key.relation()?;
    let schema = bundle.source_schemas().iter()
        .find(|item| item.relation() == physical_relation)?;
    let key_domain = physical_integer_evidence(schema, &key)?;
    let order_domain = physical_integer_evidence(schema, &ordered)?;
    if key_domain.minimum > 0 || order_domain.minimum > 0 {
        return None;
    }

    let mut shapes = Vec::new();
    for selected in 1..=wanted.min(limit).min(64) {
        if wanted.is_multiple_of(selected) {
            let groups = wanted / selected;
            shapes.push((groups, selected));
            if selected == limit {
                if let Some(rows_per_partition) = selected.checked_add(1) {
                    shapes.push((groups, rows_per_partition));
                }
            }
        }
    }
    let mut cases = Vec::new();
    for (groups, per_partition) in shapes {
        let Some(join_rows) = groups.checked_mul(per_partition) else {
            continue;
        };
        if i128::from(groups - 1) > key_domain.maximum
            || i128::from(per_partition - 1) > order_domain.maximum
        {
            continue;
        }
        let WitnessDirection::Feasible(join_cases) =
            physical_join_population_count_plan(bundle, join_id, join_rows)?
        else {
            continue;
        };
        for case in join_cases {
            let compatible = case.obligations().iter().any(|obligation| {
                let WitnessObligation::JoinPopulation {
                    left_key,
                    right_key,
                    left_rows,
                    right_rows,
                    pattern,
                    output_rows,
                    ..
                } = obligation
                else {
                    return false;
                };
                if *output_rows != join_rows {
                    return false;
                }
                let matched_side = if *left_key == key {
                    (*left_rows, *right_rows)
                } else if *right_key == key {
                    (*right_rows, *left_rows)
                } else {
                    return false;
                };
                let (ranked_rows, partner_rows) = matched_side;
                (ranked_rows == join_rows && partner_rows == groups)
                    && (matches!(pattern,
                        JoinPopulationPattern::GroupedMatched { groups: n } if *n == groups)
                        || (groups == 1 && *pattern == JoinPopulationPattern::CommonMatched)
                        || (per_partition == 1
                            && *pattern == JoinPopulationPattern::DistinctMatched))
            });
            if !compatible {
                continue;
            }
            let physical = WitnessBoundary::new(
                key.relation()?,
                GroupBoundaryKind::Physical,
                target_layer_id,
            )?;
            let computed =
                WitnessBoundary::new(input, GroupBoundaryKind::Intermediate, target_layer_id)?;
            let mut obligations = case.obligations().to_vec();
            obligations.push(WitnessObligation::Producer {
                boundary: computed,
                physical_sources: walker.sources.iter().cloned().collect(),
            });
            obligations.push(WitnessObligation::RankPopulation {
                boundary: physical,
                partition_key: key.clone(),
                order: order.with_column(ordered.clone()),
                partitions: groups,
                rows_per_partition: per_partition,
                rank_limit: limit,
                closed_world: true,
            });
            obligations.push(WitnessObligation::OutputRows {
                layer_id: target_layer_id.to_string(),
                bounds: CountBounds::new(wanted, Some(wanted))?,
            });
            if let Some(realization) = WitnessCase::new(obligations, ProofStrength::Sufficient) {
                cases.push(realization);
            }
        }
    }
    WitnessDirection::feasible(cases)
}
