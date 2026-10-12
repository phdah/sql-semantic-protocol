//! Cardinality, rejected rows, state and distributions.

use super::*;

/// Produce typed, whole-physical-source obligations for an exact terminal
/// row-count request. This is an independent constructive plan; it never
/// upgrades the single-candidate row classification returned by
/// [`physical_source_plan`].
///
/// Nonzero cardinality is constructive for source-free singletons,
/// single-source unfiltered row-preserving producer chains, and transparent
/// filter chains with a jointly proven, repeatable TRUE source-row predicate.
/// Physical leaves must have a compatible schema without unproved uniqueness
/// or source constraints.
/// A filter, join, aggregate, set, partial write or ambiguous source remains
/// residual rather than silently assuming independently sampled rows.
pub fn physical_row_count_plan(
    bundle: &AnalysisBundle,
    target_layer_id: &str,
    rows: u64,
) -> WitnessDirection {
    let physical = physical_source_plan(bundle, target_layer_id);
    if let Some(plan) = physical_subquery_membership_plan(bundle, target_layer_id, rows) {
        return plan;
    }
    if let Some(plan) = physical_group_aggregate_filter_count_plan(bundle, target_layer_id, rows) {
        return plan;
    }
    if rows == 0 {
        if let WitnessDirection::Feasible(cases) = physical.zero_output() {
            let Some(bounds) = CountBounds::new(0, Some(0)) else {
                return residual(PhysicalProofGap::NoWitness);
            };
            let cases = cases
                .iter()
                .filter_map(|case| {
                    let mut obligations = case.obligations().to_vec();
                    obligations.push(WitnessObligation::OutputRows {
                        layer_id: target_layer_id.to_string(),
                        bounds,
                    });
                    WitnessCase::new(obligations, ProofStrength::Sufficient)
                })
                .collect::<Vec<_>>();
            return WitnessDirection::feasible(cases)
                .unwrap_or_else(|| residual(PhysicalProofGap::NoWitness));
        }
    }
    if physical.nodes().is_empty() {
        return residual(physical.gap().unwrap_or(PhysicalProofGap::NoWitness));
    }
    let Some(target) = bundle.layers().iter().find(|l| l.id() == target_layer_id) else {
        return residual(PhysicalProofGap::UnknownTarget);
    };
    let Some(query) = query_for(bundle, target) else {
        return residual(PhysicalProofGap::UnresolvedSemantics);
    };
    if query.proven_single_row_output() && physical.sources().is_empty() {
        if rows != 1 {
            return WitnessDirection::Impossible;
        }
        let Some(bounds) = CountBounds::new(1, Some(1)) else {
            return residual(PhysicalProofGap::NoWitness);
        };
        let case = WitnessCase::new(
            vec![WitnessObligation::OutputRows {
                layer_id: target_layer_id.to_string(),
                bounds,
            }],
            ProofStrength::Sufficient,
        );
        return case
            .and_then(|case| WitnessDirection::feasible(vec![case]))
            .unwrap_or_else(|| residual(PhysicalProofGap::NoWitness));
    }
    // The complete join population is already a physical closed-world
    // construction. Do not let the earlier one-source filter specialization
    // hide multi-parent or repeated-source join proofs.
    if let Some(join_plan) = physical_join_population_count_plan(bundle, target_layer_id, rows) {
        return join_plan;
    }
    if let Some(plan) = physical_join_group_count_plan(bundle, target_layer_id, rows) {
        return plan;
    }
    if let Some(plan) = physical_group_rank_count_plan(bundle, target_layer_id, rows) {
        return plan;
    }
    if let Some((_, _, case)) = physical_operator_population_case(bundle, target_layer_id, rows) {
        return WitnessDirection::feasible(vec![case])
            .unwrap_or_else(|| residual(PhysicalProofGap::UnprovedCrossRowCorrelation));
    }
    if physical.sources().len() != 1 {
        return residual(PhysicalProofGap::UnboundPhysicalSource);
    }
    // A complete physical input containing exactly 'rows' identical
    // qualifying rows produces exactly 'rows' terminal rows through the
    // already-proved identity-preserving filter/projection chain. This is
    // constructive only for unconstrained, typed source schemas; it does
    // not infer cardinalities from an isolated membership example.
    if rows > 0 {
        if let Some(predicate) = repeatable_filter_predicate(bundle, &physical).or_else(|| {
            scalar_physical_row_truth(
                bundle,
                &physical,
                target_layer_id,
                crate::boolean_witness::BooleanTruthCase::True,
            )
        }) {
            let source = &physical.sources()[0];
            let Some(boundary) =
                WitnessBoundary::new(source, GroupBoundaryKind::Physical, target_layer_id)
            else {
                return residual(PhysicalProofGap::UnboundPhysicalSource);
            };
            let Some(bounds) = CountBounds::new(rows, Some(rows)) else {
                return residual(PhysicalProofGap::LocalWitnessUnproven);
            };
            let Some(case) = WitnessCase::new(
                vec![
                    WitnessObligation::Rows {
                        boundary: boundary.clone(),
                        quantifier: RowQuantifier::ForAll,
                        bounds,
                        predicate,
                        closed_world: true,
                    },
                    WitnessObligation::ClosedWorld {
                        boundary,
                        coverage: ClosedWorldCoverage::EntireRelation,
                    },
                    WitnessObligation::OutputRows {
                        layer_id: target_layer_id.to_string(),
                        bounds,
                    },
                ],
                ProofStrength::Sufficient,
            ) else {
                return residual(PhysicalProofGap::LocalWitnessUnproven);
            };
            return WitnessDirection::feasible(vec![case])
                .unwrap_or_else(|| residual(PhysicalProofGap::LocalWitnessUnproven));
        }
    }
    if physical.nodes().iter().any(|node| {
        let PhysicalPlanRef::Layer(id) = node.id() else {
            return false;
        };
        let Some(layer) = bundle.layers().iter().find(|layer| layer.id() == id) else {
            return true;
        };
        let Some(query) = query_for(bundle, layer) else {
            return true;
        };
        !query.row_preserving_projection()
            || query.sources().len() != 1
            || !query.joins().is_empty()
            || query.aggregation().is_some()
            || query.set_operation().is_some()
            || query.window_witness().is_some()
            || !query.subquery_witnesses().is_empty()
            || !query.diagnostics().is_empty()
    }) {
        return residual(PhysicalProofGap::NonInvertibleTransformation);
    }
    let Some(first_layer) = physical.nodes().iter().find_map(|node| {
        let PhysicalPlanRef::Layer(id) = node.id() else {
            return None;
        };
        bundle.layers().iter().find(|layer| layer.id() == id)
    }) else {
        return residual(PhysicalProofGap::MissingProducer);
    };
    let Some(first_query) = query_for(bundle, first_layer) else {
        return residual(PhysicalProofGap::UnresolvedSemantics);
    };
    let goal = match crate::outcome_goals::OutcomeGoal::new(
        first_layer.id(),
        Some(rows),
        None,
        Vec::new(),
    ) {
        Ok(goal) => goal,
        Err(_) => return residual(PhysicalProofGap::NoWitness),
    };
    let Some(crate::outcome_proofs::OutcomeWitness::SourceRows {
        relation,
        rows: witness_rows,
        ..
    }) = crate::outcome_proofs::construct(bundle, first_layer, first_query, &goal)
    else {
        return residual(PhysicalProofGap::LocalWitnessUnproven);
    };
    if physical.sources().first() != Some(&relation) || witness_rows != rows {
        return residual(PhysicalProofGap::UnboundPhysicalSource);
    }
    let Some(boundary) =
        WitnessBoundary::new(&relation, GroupBoundaryKind::Physical, target_layer_id)
    else {
        return residual(PhysicalProofGap::UnboundPhysicalSource);
    };
    let Some(bounds) = CountBounds::new(rows, Some(rows)) else {
        return residual(PhysicalProofGap::NoWitness);
    };
    let Some(case) = WitnessCase::new(
        vec![
            WitnessObligation::Rows {
                boundary: boundary.clone(),
                quantifier: RowQuantifier::ForAll,
                bounds,
                predicate: WitnessFormula::IsNull {
                    term: WitnessTerm::Integer(1),
                    negated: true,
                },
                closed_world: true,
            },
            WitnessObligation::ClosedWorld {
                boundary,
                coverage: ClosedWorldCoverage::EntireRelation,
            },
            WitnessObligation::OutputRows {
                layer_id: target_layer_id.to_string(),
                bounds,
            },
        ],
        ProofStrength::Sufficient,
    ) else {
        return residual(PhysicalProofGap::LocalWitnessUnproven);
    };
    WitnessDirection::feasible(vec![case])
        .unwrap_or_else(|| residual(PhysicalProofGap::LocalWitnessUnproven))
}
/// Construct a deliberately rejected, nonempty physical source for a
/// terminal with exactly zero output rows. Every physical row must fail the
/// entire jointly proved filter path with SQL FALSE or UNKNOWN, and the
/// physical relation is closed to exclude otherwise qualifying rows.
///
/// This is stronger than a single rejected candidate witness: the caller
/// requests a complete physical input of `source_rows` rejected rows.
/// It is deliberately limited to unconstrained one-source Boolean filter
/// chains and never assumes row absence from an open-world sample.
pub fn physical_rejected_row_count_plan(
    bundle: &AnalysisBundle,
    target_layer_id: &str,
    source_rows: u64,
) -> WitnessDirection {
    if source_rows == 0 {
        return physical_row_count_plan(bundle, target_layer_id, 0);
    }
    let physical = physical_source_plan(bundle, target_layer_id);
    if !matches!(physical.zero_output(), WitnessDirection::Feasible(_)) {
        return residual(
            physical
                .gap()
                .unwrap_or(PhysicalProofGap::NonInvertibleTransformation),
        );
    }
    let Some(predicate) = repeatable_row_truth(
        bundle,
        &physical,
        physical.rejected(),
        crate::boolean_witness::BooleanTruthCase::NotTrue,
    )
    .or_else(|| {
        scalar_physical_row_truth(
            bundle,
            &physical,
            target_layer_id,
            crate::boolean_witness::BooleanTruthCase::NotTrue,
        )
    }) else {
        return residual(
            physical
                .gap()
                .unwrap_or(PhysicalProofGap::LocalWitnessUnproven),
        );
    };
    let [source] = physical.sources() else {
        return residual(PhysicalProofGap::UnboundPhysicalSource);
    };
    let Some(boundary) = WitnessBoundary::new(source, GroupBoundaryKind::Physical, target_layer_id)
    else {
        return residual(PhysicalProofGap::UnboundPhysicalSource);
    };
    let (Some(source_bounds), Some(output_bounds)) = (
        CountBounds::new(source_rows, Some(source_rows)),
        CountBounds::new(0, Some(0)),
    ) else {
        return residual(PhysicalProofGap::LocalWitnessUnproven);
    };
    let Some(case) = WitnessCase::new(
        vec![
            WitnessObligation::Rows {
                boundary: boundary.clone(),
                quantifier: RowQuantifier::ForAll,
                bounds: source_bounds,
                predicate,
                closed_world: true,
            },
            WitnessObligation::ClosedWorld {
                boundary,
                coverage: ClosedWorldCoverage::EntireRelation,
            },
            WitnessObligation::OutputRows {
                layer_id: target_layer_id.to_string(),
                bounds: output_bounds,
            },
        ],
        ProofStrength::Sufficient,
    ) else {
        return residual(PhysicalProofGap::LocalWitnessUnproven);
    };
    WitnessDirection::feasible(vec![case])
        .unwrap_or_else(|| residual(PhysicalProofGap::LocalWitnessUnproven))
}

/// Construct an exact before/after physical state for an unconditional
/// DELETE with a caller-controlled initial target. This is a *complete*
/// relation-state construction, not the affected-row cardinality estimate
/// from the standalone write-effect contract.
///
/// Other mutations require source/target collision, predicate and branch
/// proofs and deliberately remain residual. An in-bundle earlier producer
/// of the target invalidates an independent physical initial assignment.
pub fn physical_unconditional_delete_plan(
    bundle: &AnalysisBundle,
    layer_id: &str,
    initial_rows: u64,
) -> WitnessDirection {
    let Some(write) = bundle
        .write_state_effects()
        .into_iter()
        .find(|write| write.layer_id() == layer_id)
    else {
        return residual(PhysicalProofGap::UnknownTarget);
    };
    let effect = write.effect();
    if effect.post_state() != crate::protocol::WritePostState::Empty
        || effect.cardinality_rule() != crate::protocol::WriteCardinalityRule::SubtractDeletes
        || !write.sources().is_empty()
        || bundle
            .write_state_effects()
            .iter()
            .any(|other| other.layer_id() != layer_id && other.target() == write.target())
        || bundle.layers().iter().any(|layer| {
            layer.id() != layer_id
                && layer
                    .produces()
                    .iter()
                    .any(|relation| relation.relation_name() == Some(write.target()))
        })
        || !bundle
            .source_schemas()
            .iter()
            .any(|schema| schema.relation() == write.target())
        || bundle.relation_constraints().iter().any(|set| {
            set.relation() == write.target()
                && (!set.constraints().is_empty() || !set.diagnostics().is_empty())
        })
        || effect.resulting_rows(
            initial_rows,
            crate::protocol::WriteRowCounts::new(0, 0, initial_rows),
        ) != Ok(0)
    {
        return residual(PhysicalProofGap::PartialProducer);
    }
    let Some(boundary) =
        WitnessBoundary::new(write.target(), GroupBoundaryKind::Physical, layer_id)
    else {
        return residual(PhysicalProofGap::UnboundPhysicalSource);
    };
    let (Some(initial), Some(empty)) = (
        CountBounds::new(initial_rows, Some(initial_rows)),
        CountBounds::new(0, Some(0)),
    ) else {
        return residual(PhysicalProofGap::LocalWitnessUnproven);
    };
    let Some(case) = WitnessCase::new(
        vec![
            WitnessObligation::Rows {
                boundary: boundary.clone(),
                quantifier: RowQuantifier::ForAll,
                bounds: initial,
                predicate: count_tautology(),
                closed_world: true,
            },
            WitnessObligation::ClosedWorld {
                boundary,
                coverage: ClosedWorldCoverage::EntireRelation,
            },
            WitnessObligation::StateRows {
                relation: write.target().to_string(),
                before: initial,
                after: empty,
            },
        ],
        ProofStrength::Sufficient,
    ) else {
        return residual(PhysicalProofGap::LocalWitnessUnproven);
    };
    WitnessDirection::feasible(vec![case])
        .unwrap_or_else(|| residual(PhysicalProofGap::LocalWitnessUnproven))
}

