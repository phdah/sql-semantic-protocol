//! Joint complete operator populations.

use super::*;

/// Reconcile one certified mixed-operator population with independent
/// transparent terminal goals that read its *same* physical source rows.
/// Each mixed case already contains complete closed-world physical counts
/// and operator requirements. We select only compatible cases; a missing
/// sufficient example is residual, not a necessary cardinality contradiction.
pub(super) fn joint_mixed_operator_and_source_goals(
    bundle: &AnalysisBundle,
    targets: &[(&str, u64)],
) -> Option<WitnessDirection> {
    if targets.len() < 2 {
        return None;
    }

    // Each already-proven mixed operator case contains a complete source
    // population. Separate cases may be conjoined only when no physical
    // relation is assigned twice; overlapping operators still need a
    // correlated population proof and remain residual.
    let mut mixed_plans = Vec::new();
    let mut transparent = Vec::new();
    for &(id, rows) in targets {
        if let Some(plan) = physical_join_group_count_plan(bundle, id, rows)
            .or_else(|| physical_group_rank_count_plan(bundle, id, rows))
            .or_else(|| physical_rank_partition_count_plan(bundle, id, rows))
            .or_else(|| physical_join_rank_count_plan(bundle, id, rows))
            .or_else(|| physical_group_aggregate_filter_count_plan(bundle, id, rows))
            .or_else(|| physical_subquery_membership_plan(bundle, id, rows))
        {
            let WitnessDirection::Feasible(cases) = plan else {
                return Some(residual(PhysicalProofGap::UnprovedCrossRowCorrelation));
            };
            mixed_plans.push(cases);
        } else {
            let relation = exact_transparent_source(bundle, id)?;
            transparent.push((id, relation, rows));
        }
    }
    if mixed_plans.is_empty() {
        return None;
    }

    let mut combinations = vec![(
        Vec::<WitnessObligation>::new(),
        BTreeMap::<String, u64>::new(),
    )];
    for plan_cases in mixed_plans {
        let mut next = Vec::new();
        for case in &plan_cases {
            let mut counts = BTreeMap::new();
            for obligation in case.obligations() {
                if let WitnessObligation::Rows {
                    boundary,
                    quantifier: RowQuantifier::ForAll,
                    bounds,
                    closed_world: true,
                    ..
                } = obligation
                {
                    if boundary.kind() == GroupBoundaryKind::Physical
                        && bounds.maximum() == Some(bounds.minimum())
                        && counts
                            .insert(boundary.relation().to_string(), bounds.minimum())
                            .is_some()
                    {
                        // Repeated row obligations must be reconciled by
                        // the operator-local proof, not counted twice.
                        return Some(residual(PhysicalProofGap::UnprovedCrossRowCorrelation));
                    }
                }
            }
            if counts.is_empty()
                || counts.keys().any(|relation| {
                    !case.obligations().iter().any(|obligation| {
                        matches!(
                            obligation,
                            WitnessObligation::ClosedWorld { boundary, coverage: ClosedWorldCoverage::EntireRelation }
                                if boundary.kind() == GroupBoundaryKind::Physical
                                    && boundary.relation() == relation
                        )
                    })
                })
            {
                return Some(residual(PhysicalProofGap::UnprovedCrossRowCorrelation));
            }
            for (obligations, existing) in &combinations {
                if counts
                    .keys()
                    .any(|relation| existing.contains_key(relation))
                {
                    continue;
                }
                let mut joined = obligations.clone();
                joined.extend(case.obligations().iter().cloned());
                let mut assigned = existing.clone();
                assigned.extend(counts.iter().map(|(key, value)| (key.clone(), *value)));
                next.push((joined, assigned));
            }
        }
        if next.is_empty() {
            return Some(residual(PhysicalProofGap::UnprovedCrossRowCorrelation));
        }
        combinations = next;
    }

    let cases = combinations
        .into_iter()
        .filter_map(|(mut obligations, counts)| {
            for &(id, ref relation, rows) in &transparent {
                if counts.get(relation) != Some(&rows) {
                    return None;
                }
                obligations.push(WitnessObligation::OutputRows {
                    layer_id: id.to_string(),
                    bounds: CountBounds::new(rows, Some(rows))?,
                });
            }
            WitnessCase::new(obligations, ProofStrength::Sufficient)
        })
        .collect::<Vec<_>>();
    Some(
        WitnessDirection::feasible(cases)
            .unwrap_or_else(|| residual(PhysicalProofGap::MultipleWitnesses)),
    )
}

/// A certified operator population controls the *complete* physical leaf
/// relation, not an arbitrary matching row. The local typed witness owns SQL
/// grouping, ranking, bag multiplicity and NULL equality; only the graph
/// composer assigns those existing facts to physical boundaries.
pub(super) fn operator_population_witness(
    bundle: &AnalysisBundle,
    target_layer_id: &str,
    rows: u64,
) -> Option<crate::outcome_proofs::OutcomeWitness> {
    use crate::outcome_proofs::OutcomeWitness;
    if rows == 0 {
        return None;
    }
    let target = bundle
        .layers()
        .iter()
        .find(|layer| layer.id() == target_layer_id)?;
    let query = query_for(bundle, target)?;
    let goal =
        crate::outcome_goals::OutcomeGoal::new(target_layer_id, Some(rows), None, Vec::new())
            .ok()?;
    let result = crate::outcome_proofs::construct(bundle, target, query, &goal)
        .or_else(|| physical_operator_count_witness(bundle, target_layer_id, rows))
        .or_else(|| {
            physical_materialized_group_or_rank_witness(bundle, target_layer_id, rows, None)
        })
        .or_else(|| physical_materialized_set_witness(bundle, target_layer_id, &goal))?;
    matches!(
        result,
        OutcomeWitness::Groups { .. }
            | OutcomeWitness::Ranked { .. }
            | OutcomeWitness::SetTuples { .. }
    )
    .then_some(result)
}

/// Translate a full-source, operator-local construction into exact physical
/// relation counts. These are complete assignments, not independently
/// feasible candidate rows. A set case is one tuple class repeated across
/// distinct values, with the proven ALL/DISTINCT scaling rules.
pub(super) fn operator_population_counts(
    witness: &crate::outcome_proofs::OutcomeWitness,
    wanted: u64,
) -> Option<BTreeMap<String, u64>> {
    use crate::outcome_proofs::OutcomeWitness;
    let mut counts = BTreeMap::new();
    match witness {
        OutcomeWitness::Groups {
            relation,
            groups,
            rows_per_group,
            ..
        } if *groups == wanted => {
            counts.insert(relation.clone(), groups.checked_mul(*rows_per_group)?);
        }
        OutcomeWitness::Ranked { relation, rows, .. } if *rows == wanted => {
            counts.insert(relation.clone(), *rows);
        }
        OutcomeWitness::SetTuples {
            tuples,
            case,
            values,
            scale_by_value_rows,
        } => {
            let copies = if values.is_empty() {
                *tuples
            } else if *scale_by_value_rows {
                values
                    .iter()
                    .try_fold(0_u64, |total, entry| total.checked_add(entry.rows()))?
            } else {
                u64::try_from(values.iter().filter(|entry| entry.rows() > 0).count()).ok()?
            };
            if (values.is_empty() && case.output_tuple_count().checked_mul(copies) != Some(wanted))
                || (!values.is_empty() && copies != wanted)
                || case.obligations().is_empty()
            {
                return None;
            }
            let mut assigned = BTreeMap::<String, (Vec<String>, u64)>::new();
            for obligation in case.obligations() {
                let boundary = obligation.boundary();
                if boundary.is_intermediate() || boundary.tuple_columns().is_empty() {
                    return None;
                }
                let relation = boundary.relation().to_string();
                let population = (
                    boundary.tuple_columns().to_vec(),
                    obligation.matching_tuple_count().checked_mul(copies)?,
                );
                if let Some(previous) = assigned.get(&relation) {
                    if previous != &population {
                        // The same physical rows cannot satisfy different
                        // column/frequency obligations in two copied arms.
                        return None;
                    }
                } else {
                    assigned.insert(relation, population);
                }
            }
            for (relation, (_, count)) in assigned {
                counts.insert(relation, count);
            }
        }
        _ => return None,
    }
    Some(counts)
}

pub(super) fn physical_operator_population_case(
    bundle: &AnalysisBundle,
    layer_id: &str,
    rows: u64,
) -> Option<(
    crate::outcome_proofs::OutcomeWitness,
    BTreeMap<String, u64>,
    WitnessCase,
)> {
    let witness = operator_population_witness(bundle, layer_id, rows)?;
    let counts = operator_population_counts(&witness, rows)?;
    let mut walker = Walker::new(bundle);
    walker.visit(layer_id).ok()?;
    if counts.keys().cloned().collect::<BTreeSet<_>>() != walker.sources {
        return None;
    }
    let mut obligations = vec![WitnessObligation::OperatorPopulation {
        witness: witness.clone(),
    }];
    for (relation, count) in &counts {
        let boundary = WitnessBoundary::new(relation, GroupBoundaryKind::Physical, layer_id)?;
        obligations.push(WitnessObligation::Rows {
            boundary: boundary.clone(),
            quantifier: RowQuantifier::ForAll,
            bounds: CountBounds::new(*count, Some(*count))?,
            predicate: count_tautology(),
            closed_world: true,
        });
        obligations.push(WitnessObligation::ClosedWorld {
            boundary,
            coverage: ClosedWorldCoverage::EntireRelation,
        });
    }
    obligations.push(WitnessObligation::OutputRows {
        layer_id: layer_id.to_string(),
        bounds: CountBounds::new(rows, Some(rows))?,
    });
    let case = WitnessCase::new(obligations, ProofStrength::Sufficient)?;
    Some((witness, counts, case))
}

/// Safely share fully controlled physical relations across several terminal
/// operators. Identical constructions can reuse the same rows; independent
/// sources remain disjoint. A different sufficient population over one source
/// is *residual*, even when their example counts conflict: other complete
/// populations might satisfy both terminal goals.
pub(super) fn joint_operator_population_goals(
    bundle: &AnalysisBundle,
    targets: &[(&str, u64)],
) -> Option<WitnessDirection> {
    if !targets
        .iter()
        .any(|(id, rows)| operator_population_witness(bundle, id, *rows).is_some())
    {
        return None;
    }
    let mut populations = BTreeMap::<String, crate::outcome_proofs::OutcomeWitness>::new();
    let mut sources = BTreeMap::<String, u64>::new();
    let mut obligations = Vec::new();
    for &(id, rows) in targets {
        if let Some((witness, counts, case)) = physical_operator_population_case(bundle, id, rows) {
            for (relation, count) in counts {
                if let Some(existing) = sources.get(&relation) {
                    if *existing != count || populations.get(&relation) != Some(&witness) {
                        return Some(residual(PhysicalProofGap::UnprovedCrossRowCorrelation));
                    }
                } else {
                    sources.insert(relation.clone(), count);
                    populations.insert(relation, witness.clone());
                }
            }
            obligations.extend(case.obligations().iter().cloned());
        } else if let Some(relation) = exact_transparent_source(bundle, id) {
            let Some(&existing) = sources.get(&relation) else {
                // Process operator and transparent goals in either order
                // without inventing independent source populations.
                continue;
            };
            if rows != existing {
                return Some(residual(PhysicalProofGap::MultipleWitnesses));
            }
            obligations.push(WitnessObligation::OutputRows {
                layer_id: id.to_string(),
                bounds: CountBounds::new(rows, Some(rows))?,
            });
        } else {
            return Some(residual(PhysicalProofGap::UnprovedCrossRowCorrelation));
        }
    }
    // The first pass may have deferred transparent goals encountered before
    // their source's operator. Re-check every goal against the same assignment.
    for &(id, rows) in targets {
        if let Some(relation) = exact_transparent_source(bundle, id) {
            if sources.get(&relation) != Some(&rows) {
                return Some(residual(PhysicalProofGap::MultipleWitnesses));
            }
            obligations.push(WitnessObligation::OutputRows {
                layer_id: id.to_string(),
                bounds: CountBounds::new(rows, Some(rows))?,
            });
        }
    }
    WitnessCase::new(obligations, ProofStrength::Sufficient)
        .and_then(|case| WitnessDirection::feasible(vec![case]))
        .or_else(|| Some(residual(PhysicalProofGap::UnprovedCrossRowCorrelation)))
}
