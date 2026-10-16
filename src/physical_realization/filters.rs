//! Row truth and filtered source proofs.

use super::*;

/// Construct canonical producer references, and lift an individual physical-row
/// predicate witness only when the entire path is transparently reversible.
///
/// Unknown and multi-operator cases deliberately return independent residual
/// directions. The returned topology is still useful when proof is residual:
/// no consumer should fabricate direct writes into derived tables.
pub fn physical_source_plan(bundle: &AnalysisBundle, target_layer_id: &str) -> PhysicalSourcePlan {
    let mut walker = Walker::new(bundle);
    let walk = if walker.layers.contains_key(target_layer_id) {
        walker.visit(target_layer_id)
    } else {
        Err(PhysicalProofGap::UnknownTarget)
    };
    let mut gap = walk.err();
    let zero_output = if let Some(reason) = gap {
        residual(reason)
    } else {
        prove_zero_rows(bundle, &walker, target_layer_id)
    };
    if gap.is_none() {
        let target = walker.layers.get(target_layer_id).copied();
        match target.and_then(|layer| match layer.composed_semantics() {
            ComposedSemantics::Resolved(semantics) => Some(semantics.as_ref()),
            ComposedSemantics::Unresolved(_) => None,
        }) {
            Some(semantics) => {
                let proofs = local_constructive_witnesses(semantics);
                let conflicting_domains = semantics
                    .column_domains()
                    .iter()
                    .any(|domain| matches!(domain.domain(), crate::protocol::ValueDomain::Empty));
                if semantics.boolean_witnesses().len() == proofs.len() {
                    if let Some((qualifying, rejected)) =
                        joint_physical_filters(bundle, &walker, semantics)
                    {
                        // An empty composed domain must never yield a feasible
                        // qualifying row. A separately certified conjunction
                        // may nevertheless prove TRUE impossible, rather than
                        // hiding that necessary contradiction behind a gap.
                        if !conflicting_domains
                            || matches!(qualifying, WitnessDirection::Impossible)
                        {
                            let partial = matches!(qualifying, WitnessDirection::Residual { .. })
                                || matches!(rejected, WitnessDirection::Residual { .. });
                            return PhysicalSourcePlan {
                                target_layer_id: target_layer_id.to_string(),
                                nodes: walker.nodes,
                                sources: walker.sources.into_iter().collect(),
                                qualifying,
                                rejected,
                                zero_output,
                                gap: partial.then_some(PhysicalProofGap::LocalWitnessUnproven),
                            };
                        }
                    }
                }

                gap = if conflicting_domains {
                    Some(PhysicalProofGap::ConflictingDomains)
                } else if proofs.is_empty() {
                    Some(PhysicalProofGap::NoWitness)
                } else if proofs.len() != 1 {
                    Some(PhysicalProofGap::MultipleWitnesses)
                } else if proofs[0].operator() != WitnessOperator::Boolean {
                    Some(PhysicalProofGap::UnsupportedOperator)
                } else if !matches!(
                    (proofs[0].qualifying(), proofs[0].rejected()),
                    (WitnessDirection::Feasible(_), _) | (_, WitnessDirection::Feasible(_))
                ) {
                    Some(PhysicalProofGap::LocalWitnessUnproven)
                } else if walker.sources.len() != 1 {
                    Some(PhysicalProofGap::UnboundPhysicalSource)
                } else if !walker
                    .nodes
                    .iter()
                    .filter_map(|node| match &node.id {
                        PhysicalPlanRef::Layer(id) => walker.layers.get(id.as_str()).copied(),
                        PhysicalPlanRef::Source(_) => None,
                    })
                    .all(|layer| {
                        let Some(query) = query_for(bundle, layer) else {
                            return false;
                        };
                        if layer.id() == proofs[0].origin_layer_id() {
                            query.filter_only_row_shape()
                                && query.sources().len() == 1
                                && query.joins().is_empty()
                                && query.aggregation().is_none()
                                && query.set_operation().is_none()
                                && query.window_witness().is_none()
                                && query.subquery_witnesses().is_empty()
                                && query.predicates().where_predicate().is_some()
                                && query.predicates().having_predicate().is_none()
                                && query.predicates().qualify_predicate().is_none()
                                && query.diagnostics().is_empty()
                                && query
                                    .output()
                                    .columns()
                                    .iter()
                                    .all(|c| c.plain_copy_source().is_some())
                        } else {
                            transparent_projection(query)
                        }
                    })
                {
                    Some(PhysicalProofGap::NonInvertibleTransformation)
                } else {
                    let only_source = walker.sources.iter().next().map(String::as_str);
                    let physical = |direction: &WitnessDirection| match direction {
                        WitnessDirection::Feasible(cases) => cases.iter().all(|case| {
                            case.obligations()
                                .iter()
                                .all(|obligation| match obligation {
                                    WitnessObligation::Predicate(
                                        crate::constructive::WitnessFormula::RowTruth {
                                            row, ..
                                        },
                                    ) => Some(row.relation()) == only_source,
                                    _ => false,
                                })
                        }),
                        WitnessDirection::Impossible | WitnessDirection::Residual { .. } => true,
                    };
                    if physical(proofs[0].qualifying()) && physical(proofs[0].rejected()) {
                        let still_residual =
                            matches!(proofs[0].qualifying(), WitnessDirection::Residual { .. })
                                || matches!(
                                    proofs[0].rejected(),
                                    WitnessDirection::Residual { .. }
                                );
                        return PhysicalSourcePlan {
                            target_layer_id: target_layer_id.to_string(),
                            nodes: walker.nodes,
                            sources: walker.sources.into_iter().collect(),
                            qualifying: proofs[0].qualifying().clone(),
                            rejected: proofs[0].rejected().clone(),
                            zero_output,
                            gap: still_residual.then_some(PhysicalProofGap::IntermediateBoundary),
                        };
                    }
                    Some(PhysicalProofGap::UnboundPhysicalSource)
                };
            }
            None => gap = Some(PhysicalProofGap::UnresolvedSemantics),
        }
    }
    let gap = gap.unwrap_or(PhysicalProofGap::NoWitness);
    PhysicalSourcePlan {
        target_layer_id: target_layer_id.to_string(),
        nodes: walker.nodes,
        sources: walker.sources.into_iter().collect(),
        qualifying: residual(gap),
        rejected: residual(gap),
        zero_output,
        gap: Some(gap),
    }
}

/// Promote a physical-row TRUE or NOT TRUE classification to a
/// repeatable, schema-backed predicate only when the complete source is
/// controllable. An individually feasible candidate cannot be duplicated
/// arbitrarily when uniqueness or other source constraints are unknown.
pub(super) fn repeatable_row_truth(
    bundle: &AnalysisBundle,
    physical: &PhysicalSourcePlan,
    direction: &WitnessDirection,
    required_truth: crate::boolean_witness::BooleanTruthCase,
) -> Option<WitnessFormula> {
    let [source] = physical.sources() else {
        return None;
    };
    if !bundle
        .source_schemas()
        .iter()
        .any(|schema| schema.relation() == source)
        || bundle
            .relation_constraints()
            .iter()
            .any(|set| set.relation() == source && set.blocks_unconstrained_physical_plans())
    {
        return None;
    }
    let WitnessDirection::Feasible(cases) = direction else {
        return None;
    };
    let [case] = cases.as_slice() else {
        return None;
    };
    let [WitnessObligation::Predicate(
        formula @ WitnessFormula::RowTruth {
            row,
            predicate,
            truth,
        },
    )] = case.obligations()
    else {
        return None;
    };
    (row.relation() == source && predicate.is_exact() && *truth == required_truth)
        .then(|| formula.clone())
}

pub(super) fn repeatable_filter_predicate(
    bundle: &AnalysisBundle,
    physical: &PhysicalSourcePlan,
) -> Option<WitnessFormula> {
    repeatable_row_truth(
        bundle,
        physical,
        physical.qualifying(),
        crate::boolean_witness::BooleanTruthCase::True,
    )
}

/// Supplement a physical count with a standalone scalar WHERE predicate
/// without modifying the emitted operator-local Boolean witness contract.
/// Only one direct, typed physical-source filter and subsequent transparent
/// producers are supported; competing filters require joint truth solving.
/// Only completely row-preserving, single-parent materialization chains
/// can transport physical predicates. Each node retains its producer kind,
/// and typed columns are separately checked at every reference boundary.
pub(super) fn transparent_materialized_source(
    bundle: &AnalysisBundle,
    walker: &Walker<'_>,
    node: &PhysicalPlanNode,
    source: &str,
) -> bool {
    let [PhysicalPlanRef::Layer(initial_id)] = node.inputs() else {
        return false;
    };
    let mut next_id = initial_id.as_str();
    for _ in 0..walker.layers.len() {
        let Some(producer) = walker
            .nodes
            .iter()
            .find(|node| node.id() == &PhysicalPlanRef::Layer(next_id.to_string()))
        else {
            return false;
        };
        let valid = walker
            .layers
            .get(next_id)
            .and_then(|layer| query_for(bundle, layer))
            .is_some_and(transparent_projection);
        if !valid || !matches!(producer.write_kind(), None | Some(WriteKind::Definition)) {
            return false;
        }
        match producer.inputs() {
            [PhysicalPlanRef::Source(actual)] => return actual == source,
            [PhysicalPlanRef::Layer(upstream)] => next_id = upstream,
            _ => return false,
        }
    }
    false
}

/// Type-verified scalar WHERE witness through either a direct source or one
/// complete materialization. Every row still refers to the canonical
/// physical source; the intermediate producer is never treated as writable.
pub(super) fn scalar_physical_row_truth(
    bundle: &AnalysisBundle,
    physical: &PhysicalSourcePlan,
    target_layer_id: &str,
    required_truth: crate::boolean_witness::BooleanTruthCase,
) -> Option<WitnessFormula> {
    let [source] = physical.sources() else {
        return None;
    };
    let schema = bundle
        .source_schemas()
        .iter()
        .find(|schema| schema.relation() == source)?;
    if bundle
        .relation_constraints()
        .iter()
        .any(|set| set.relation() == source && set.blocks_unconstrained_physical_plans())
    {
        return None;
    }

    let mut walker = Walker::new(bundle);
    walker.visit(target_layer_id).ok()?;
    let mut filter = None;
    for node in &walker.nodes {
        let PhysicalPlanRef::Layer(id) = node.id() else {
            continue;
        };
        let layer = walker.layers.get(id.as_str()).copied()?;
        let query = query_for(bundle, layer)?;
        if query.filter_only_row_shape() {
            let [input_source] = query.sources() else {
                return None;
            };
            let direct = node.inputs() == [PhysicalPlanRef::Source(source.clone())]
                && input_source.name() == source;
            let through_producer = transparent_materialized_source(bundle, &walker, node, source)
                && input_source.name() != source;
            if filter.is_some()
                || (!direct && !through_producer)
                || query.aggregation().is_some()
                || query.set_operation().is_some()
                || query.window_witness().is_some()
                || !query.subquery_witnesses().is_empty()
                || !query.condition_exactness().is_exact()
                || !query.diagnostics().is_empty()
                || query
                    .output()
                    .columns()
                    .iter()
                    .any(|column| !total_scalar_projection(column.expression()))
            {
                return None;
            }
            filter = Some((query, layer.id(), input_source.name()));
        } else if !transparent_projection(query) {
            return None;
        }
    }
    let (filter, origin_layer, filter_source) = filter?;
    let input_schema = bundle
        .source_schemas()
        .iter()
        .find(|candidate| candidate.relation() == filter_source)?;
    if bundle
        .relation_constraints()
        .iter()
        .any(|set| set.relation() == filter_source && set.blocks_unconstrained_physical_plans())
    {
        return None;
    }
    let witness = crate::boolean_witness::analyze_physical_scalar(
        filter.predicates().where_predicate(),
        filter.sources(),
        |column| {
            (column.relation() == Some(filter_source))
                .then(|| physical_integer_evidence(input_schema, column))
                .flatten()
        },
        |_column| None,
        |column| {
            (column.relation() == Some(filter_source))
                .then(|| physical_column_datatype(input_schema, column))
                .flatten()
        },
    )?;
    let witness = if filter_source == source {
        witness
    } else {
        witness.mapped_to_physical_certified(
            |column| {
                resolve_filter_column_with_evidence(bundle, &walker, origin_layer, column, 0, true)
            },
            |original, mapped| {
                let original_type = input_schema
                    .columns()
                    .iter()
                    .find(|known| known.name() == original.name())
                    .map(|known| known.data_type());
                let mapped_type = schema
                    .columns()
                    .iter()
                    .find(|known| known.name() == mapped.name())
                    .map(|known| known.data_type());
                original_type.is_some() && original_type == mapped_type
            },
        )?
    };
    let direction = match required_truth {
        crate::boolean_witness::BooleanTruthCase::True => witness.qualifying(),
        crate::boolean_witness::BooleanTruthCase::NotTrue => witness.rejected(),
    };
    if !matches!(
        direction,
        crate::boolean_witness::BooleanWitnessDirection::Exact(truth)
            if *truth == required_truth
    ) {
        return None;
    }
    let row = crate::constructive::RowVariable::new(source, source, "candidate")?;
    Some(WitnessFormula::RowTruth {
        row,
        predicate: witness.condition().clone(),
        truth: required_truth,
    })
}

/// Resolve an independent source consumed through an exclusively
/// identity-preserving producer chain. This does not assert the producer can
/// be written independently; it identifies the sole complete physical leaf.
pub(super) fn physical_transparent_producer_leaf(
    bundle: &AnalysisBundle,
    walker: &Walker<'_>,
    relation: &str,
) -> Option<String> {
    if walker.sources.contains(relation) {
        return Some(relation.to_string());
    }
    let mut current = walker.nodes.iter().find(|node| {
        node.produced_relations()
            .iter()
            .any(|name| name == relation)
    })?;
    for _ in 0..walker.layers.len() {
        let PhysicalPlanRef::Layer(id) = current.id() else {
            return None;
        };
        let layer = walker.layers.get(id.as_str()).copied()?;
        if !matches!(current.write_kind(), Some(WriteKind::Definition))
            || !transparent_projection(query_for(bundle, layer)?)
        {
            return None;
        }
        match current.inputs() {
            [PhysicalPlanRef::Source(name)] if walker.sources.contains(name) => {
                return Some(name.clone());
            }
            [PhysicalPlanRef::Layer(parent)] => {
                current = walker
                    .nodes
                    .iter()
                    .find(|node| node.id() == &PhysicalPlanRef::Layer(parent.clone()))?;
            }
            _ => return None,
        }
    }
    None
}
