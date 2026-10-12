//! Complete physical histograms shared by multiple independently proven terminals.
//!
//! Each supported terminal preserves the entire same physical bag. Reconcile
//! columns by canonical physical identity before constructing a joint witness;
//! independent feasible examples are never assumed jointly satisfiable.

use super::*;
use crate::outcome_goals::{equivalent_frequencies, OutcomeGoal, OutputValueCount};
use crate::outcome_proofs::OutcomeWitness;

/// Prove a complete physical-source assignment for a set of terminal
/// histograms. Each terminal must be a cardinality-preserving, schema-certified
/// single-source DAG. Shared physical columns must have equal full frequency
/// bags, while disjoint columns may be independently assigned over the same
/// fixed row count. Source constraints and uncertain producer paths fail closed.
///
/// A feasible case includes the canonical physical `source_rows` histogram
/// witness, exact source-row cardinalities, closed-world coverage and every
/// requested terminal count. An impossible result is returned only for a
/// necessary contradiction between complete source bags.
pub fn physical_joint_distribution_plan(
    bundle: &AnalysisBundle,
    goals: &[OutcomeGoal],
) -> WitnessDirection {
    if goals.is_empty() {
        return residual(PhysicalProofGap::NoWitness);
    }
    // A complete empty result has one uniquely determined histogram: every
    // bucket has frequency zero. Reuse the canonical joint closed-world count
    // proof rather than pretending an arbitrary intermediate table is writable.
    if goals.iter().any(|goal| goal.rows().is_none()) {
        return residual(PhysicalProofGap::LocalWitnessUnproven);
    }
    if goals.iter().any(|goal| goal.groups().is_some()) {
        return residual(PhysicalProofGap::UnprovedCrossRowCorrelation);
    }
    let exact = goals
        .iter()
        .map(|goal| {
            let rows = goal.rows()?;
            let valid = goal.distributions().iter().all(|distribution| {
                distribution
                    .values()
                    .iter()
                    .try_fold(0_u64, |total, entry| total.checked_add(entry.rows()))
                    == Some(rows)
            });
            valid.then_some((goal.layer_id(), rows))
        })
        .collect::<Option<Vec<_>>>();
    let Some(exact) = exact else {
        return WitnessDirection::Impossible;
    };
    if exact.iter().all(|(_, rows)| *rows == 0)
        || goals.iter().all(|goal| goal.distributions().is_empty())
    {
        return physical_joint_row_count_plan(bundle, &exact);
    }

    let mut ordered = goals.to_vec();
    ordered.sort_by(|left, right| left.layer_id().cmp(right.layer_id()));
    let mut targets = BTreeMap::<String, u64>::new();
    let mut source_rows = BTreeMap::<String, u64>::new();
    let mut columns = BTreeMap::<String, BTreeMap<String, Vec<OutputValueCount>>>::new();

    for goal in &ordered {
        let Some(rows) = goal.rows() else {
            return residual(PhysicalProofGap::LocalWitnessUnproven);
        };
        if rows == 0 || goal.groups().is_some() {
            // Mixing positive histogram populations with a zero terminal may
            // require correlating SQL TRUE / NOT TRUE and source frequencies.
            return residual(PhysicalProofGap::UnprovedCrossRowCorrelation);
        }
        if targets
            .insert(goal.layer_id().to_string(), rows)
            .is_some_and(|existing| existing != rows)
        {
            return WitnessDirection::Impossible;
        }
        let assigned = if goal.distributions().is_empty() {
            // No histogram is requested for this terminal, but the source
            // count must still reconcile with every complete shared bag.
            let Some(relation) = exact_transparent_source(bundle, goal.layer_id()) else {
                return residual(PhysicalProofGap::NonInvertibleTransformation);
            };
            if !matches!(
                physical_row_count_plan(bundle, goal.layer_id(), rows),
                WitnessDirection::Feasible(_)
            ) {
                return residual(PhysicalProofGap::LocalWitnessUnproven);
            }
            Some(OutcomeWitness::SourceRows {
                relation,
                rows,
                columns: Vec::new(),
            })
        } else {
            physical_distribution_plan(bundle, goal.layer_id(), goal)
        };
        let Some(OutcomeWitness::SourceRows {
            relation,
            rows: actual,
            columns: mapped,
        }) = assigned
        else {
            return residual(PhysicalProofGap::NonInvertibleTransformation);
        };
        if actual != rows {
            return residual(PhysicalProofGap::LocalWitnessUnproven);
        }
        if source_rows
            .insert(relation.clone(), rows)
            .is_some_and(|previous| previous != rows)
        {
            // A transparent path is count-preserving, not just an example
            // sufficient population. Two complete different source row counts
            // are therefore necessarily contradictory.
            return WitnessDirection::Impossible;
        }
        let named = columns.entry(relation).or_default();
        for item in mapped {
            let frequency = named
                .entry(item.column().to_string())
                .or_insert_with(|| item.values().to_vec());
            if !equivalent_frequencies(frequency, item.values()) {
                return WitnessDirection::Impossible;
            }
        }
    }

    let mut obligations = Vec::new();
    let Some(origin) = targets.keys().next() else {
        return residual(PhysicalProofGap::NoWitness);
    };
    for (relation, rows) in source_rows {
        let Some(mapped) = columns.remove(&relation) else {
            return residual(PhysicalProofGap::UnboundPhysicalSource);
        };
        let witness = if mapped.is_empty() {
            None
        } else {
            let Some(witness) = crate::outcome_proofs::construct_mapped_source(
                bundle,
                &relation,
                rows,
                mapped.into_iter().collect(),
            ) else {
                return residual(PhysicalProofGap::ConflictingDomains);
            };
            Some(witness)
        };
        let Some(boundary) = WitnessBoundary::new(&relation, GroupBoundaryKind::Physical, origin)
        else {
            return residual(PhysicalProofGap::UnboundPhysicalSource);
        };
        let Some(bounds) = CountBounds::new(rows, Some(rows)) else {
            return residual(PhysicalProofGap::LocalWitnessUnproven);
        };
        obligations.push(WitnessObligation::Rows {
            boundary: boundary.clone(),
            quantifier: RowQuantifier::ForAll,
            bounds,
            predicate: count_tautology(),
            closed_world: true,
        });
        obligations.push(WitnessObligation::ClosedWorld {
            boundary,
            coverage: ClosedWorldCoverage::EntireRelation,
        });
        if let Some(witness) = witness {
            obligations.push(WitnessObligation::OperatorPopulation { witness });
        }
    }
    for (layer_id, rows) in targets {
        let Some(bounds) = CountBounds::new(rows, Some(rows)) else {
            return residual(PhysicalProofGap::LocalWitnessUnproven);
        };
        obligations.push(WitnessObligation::OutputRows { layer_id, bounds });
    }
    WitnessCase::new(obligations, ProofStrength::Sufficient)
        .and_then(|case| WitnessDirection::feasible(vec![case]))
        .unwrap_or_else(|| residual(PhysicalProofGap::UnprovedCrossRowCorrelation))
}
