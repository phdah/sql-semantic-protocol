//! Complete physical histograms shared by multiple independently proven terminals.
//!
//! Transparent terminals preserve the whole source bag; one directly filtered
//! terminal may instead select a proved subset of the same complete typed bag.
//! Independent feasible examples are never assumed jointly satisfiable.

use super::*;
use crate::boolean_witness::{BooleanRowConstraint, BooleanTruthCase};
use crate::constraints::ConstraintValue;
use crate::outcome_goals::{equivalent_frequencies, OutcomeGoal, OutputValueCount};
use crate::outcome_proofs::OutcomeWitness;
use crate::protocol::ComparisonOperator;

/// Prove a complete physical-source assignment for a set of terminal goals.
/// Transparent, schema-certified terminals share full frequency bags; a single
/// direct filter may instead select SQL TRUE occurrences from the same closed
/// physical bag. All requested terminal counts are reconciled against that
/// one source. Source constraints and uncertain producers fail closed.
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
    if let Some(proof) = shared_filtered_histogram_plan(bundle, goals) {
        return proof;
    }
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

/// Compose an attested *complete* source histogram with a second, direct
/// row-preserving filter over that same physical source. The filter must be
/// proved against the very column whose complete bag is known; knowing the
/// size of a filtered example is not a source-complete cardinality proof.
///
/// This deliberately excludes joins, grouping, computed output columns and
/// conditions over any unassigned physical column. Missing facts are Residual,
/// not an inferred empty result or an independently invented source bag.
fn shared_filtered_histogram_plan(
    bundle: &AnalysisBundle,
    goals: &[OutcomeGoal],
) -> Option<WitnessDirection> {
    let [first, second] = goals else {
        return None;
    };
    for (whole, filtered) in [(first, second), (second, first)] {
        if whole.groups().is_some() || filtered.groups().is_some() {
            continue;
        }
        let Some(OutcomeWitness::SourceRows {
            relation,
            rows,
            columns,
        }) = physical_distribution_plan(bundle, whole.layer_id(), whole)
        else {
            continue;
        };
        // This finite law requires a transparent terminal, not a broader
        // reversible scalar inversion or materialized producer.
        if exact_transparent_source(bundle, whole.layer_id()).as_deref()
            != Some(relation.as_str())
        {
            continue;
        }
        let [source_column] = columns.as_slice() else {
            continue;
        };
        if source_column
            .values()
            .iter()
            .try_fold(0_u64, |sum, value| sum.checked_add(value.rows()))
            != Some(rows)
        {
            continue;
        }

        let Some(layer) = bundle
            .layers()
            .iter()
            .find(|layer| layer.id() == filtered.layer_id())
        else {
            continue;
        };
        let Some(query) = query_for(bundle, layer) else {
            continue;
        };
        let [input] = query.sources() else {
            continue;
        };
        if input.name() != relation
            || !query.filter_only_row_shape()
            || !query.joins().is_empty()
            || !query.diagnostics().is_empty()
            || query.predicates().where_predicate().is_none()
            || query
                .output()
                .columns()
                .iter()
                .any(|column| column.plain_copy_source().is_none())
        {
            continue;
        }

        // A projected histogram may be compared with the complete source bag
        // only when it is a proven literal copy of the assigned source column.
        let mut projections_verified = true;
        for distribution in filtered.distributions() {
            let mut matches = query
                .output()
                .columns()
                .iter()
                .filter(|column| column.name() == distribution.column());
            let Some(projected) = matches.next() else {
                projections_verified = false;
                break;
            };
            if matches.next().is_some()
                || !projected.plain_copy_source().is_some_and(|source| {
                    source.column() == source_column.column() && source.relation() == relation
                })
            {
                projections_verified = false;
                break;
            }
        }
        if !projections_verified {
            continue;
        }

        let physical = physical_source_plan(bundle, filtered.layer_id());
        if physical.sources() != [relation.clone()] {
            continue;
        }
        let requested = filtered.rows()?;
        // A pure WHERE cannot manufacture additional physical row occurrences.
        // This is a necessary bound, regardless of which rows satisfy WHERE.
        if requested > rows {
            return Some(WitnessDirection::Impossible);
        }

        let Some(WitnessFormula::RowTruth {
            predicate,
            truth: BooleanTruthCase::True,
            ..
        }) = scalar_physical_row_truth(
            bundle,
            &physical,
            filtered.layer_id(),
            BooleanTruthCase::True,
        )
        else {
            continue;
        };
        let mut selected = Vec::new();
        let mut selected_rows = 0_u64;
        let mut completely_evaluated = true;
        for bucket in source_column.values() {
            let Some(survives) =
                known_histogram_filter_truth(&predicate, source_column.column(), bucket.value())
            else {
                completely_evaluated = false;
                break;
            };
            if survives {
                selected_rows = selected_rows.checked_add(bucket.rows())?;
                selected.push(bucket.clone());
            }
        }
        if !completely_evaluated {
            continue;
        }
        if selected_rows != requested
            || filtered
                .distributions()
                .iter()
                .any(|distribution| !equivalent_frequencies(distribution.values(), &selected))
        {
            // Every source occurrence is fixed by the complete witness.
            // No alternative assignment can change the filtered count/bag.
            return Some(WitnessDirection::Impossible);
        }

        let boundary =
            WitnessBoundary::new(&relation, GroupBoundaryKind::Physical, whole.layer_id())?;
        let bounds = CountBounds::new(rows, Some(rows))?;
        let mut obligations = vec![
            WitnessObligation::Rows {
                boundary: boundary.clone(),
                quantifier: RowQuantifier::ForAll,
                bounds,
                predicate: count_tautology(),
                closed_world: true,
            },
            WitnessObligation::ClosedWorld {
                boundary,
                coverage: ClosedWorldCoverage::EntireRelation,
            },
            WitnessObligation::OperatorPopulation {
                witness: OutcomeWitness::SourceRows {
                    relation,
                    rows,
                    columns,
                },
            },
        ];
        let mut outputs = [(whole.layer_id(), rows), (filtered.layer_id(), requested)];
        outputs.sort();
        for (layer_id, rows) in outputs {
            obligations.push(WitnessObligation::OutputRows {
                layer_id: layer_id.to_string(),
                bounds: CountBounds::new(rows, Some(rows))?,
            });
        }
        return Some(
            WitnessCase::new(obligations, ProofStrength::Sufficient)
                .and_then(|case| WitnessDirection::feasible(vec![case]))
                .unwrap_or_else(|| residual(PhysicalProofGap::UnprovedCrossRowCorrelation)),
        );
    }
    None
}

/// SQL WHERE retains only TRUE. FALSE and UNKNOWN both reject that
/// *specific* source occurrence. All possible values for the predicate must
/// come from the same complete typed histogram; otherwise the proof is absent.
fn known_histogram_filter_truth(
    predicate: &BooleanRowConstraint,
    column_name: &str,
    value: &ConstraintValue,
) -> Option<bool> {
    match predicate {
        BooleanRowConstraint::All(children) => {
            let mut matches = true;
            for child in children.iter() {
                matches &= known_histogram_filter_truth(child, column_name, value)?;
            }
            Some(matches)
        }
        BooleanRowConstraint::Any(children) => {
            let mut matches = false;
            for child in children.iter() {
                matches |= known_histogram_filter_truth(child, column_name, value)?;
            }
            Some(matches)
        }
        BooleanRowConstraint::NullTest { column, negated } if column.name() == column_name => {
            Some(matches!(value, ConstraintValue::Null) != *negated)
        }
        BooleanRowConstraint::UnknownNullComparison { column } if column.name() == column_name => {
            Some(false)
        }
        BooleanRowConstraint::IntegerComparison {
            column,
            operator,
            literal,
        } if column.name() == column_name => match value {
            ConstraintValue::Null => Some(matches!(operator, ComparisonOperator::IsDistinctFrom)),
            ConstraintValue::Integer(actual) => Some(match operator {
                ComparisonOperator::Eq | ComparisonOperator::IsNotDistinctFrom => actual == literal,
                ComparisonOperator::Neq | ComparisonOperator::IsDistinctFrom => actual != literal,
                ComparisonOperator::Lt => actual < literal,
                ComparisonOperator::Lte => actual <= literal,
                ComparisonOperator::Gt => actual > literal,
                ComparisonOperator::Gte => actual >= literal,
            }),
            _ => None,
        },
        BooleanRowConstraint::NullTest { .. }
        | BooleanRowConstraint::UnknownNullComparison { .. }
        | BooleanRowConstraint::IntegerComparison { .. }
        | BooleanRowConstraint::IntegerColumnComparison { .. }
        | BooleanRowConstraint::StringEquality { .. }
        | BooleanRowConstraint::StringColumnComparison { .. }
        | BooleanRowConstraint::StringPrefix { .. }
        | BooleanRowConstraint::Residual { .. } => None,
    }
}
