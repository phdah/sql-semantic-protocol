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
        WindowWitnessDirection::Exact(case)
            if limit > 0 && case.min_preceding() == 0 => {}
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
        let node = walker.nodes.iter().find(|node| {
            node.id() == &PhysicalPlanRef::Layer(current.to_string())
        })?;
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
    let partition = resolve_filter_column_with_evidence(
        bundle, &walker, operator_id, partition_key, 0, true
    )?;
    let order_key = resolve_filter_column_with_evidence(
        bundle, &walker, operator_id, order.column(), 0, true
    )?;
    if partition.relation() != Some(source.as_str())
        || order_key.relation() != Some(source.as_str())
        || partition == order_key
    {
        return None;
    }
    let schema = bundle.source_schemas().iter().find(|item| item.relation() == source)?;
    if bundle.relation_constraints().iter().any(|set| {
        set.relation() == source
            && (!set.constraints().is_empty() || !set.diagnostics().is_empty())
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
            || (partitions > 0
                && i128::from(partitions - 1) > partition_domain.maximum)
            || i128::from(rows_per_partition - 1) > order_domain.maximum
        {
            continue;
        }
        let Some(boundary) = WitnessBoundary::new(
            &source, GroupBoundaryKind::Physical, target_layer_id
        ) else {
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
            let intermediate = WitnessBoundary::new(
                rank_input, GroupBoundaryKind::Intermediate, target_layer_id
            )?;
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
