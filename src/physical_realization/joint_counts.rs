//! Shared source counts and joint truth constraints.

use super::*;

/// One jointly controlled physical input with an optional qualifying
/// restriction. Equality of physical counts is necessary only for a genuinely
/// row-preserving (unfiltered) terminal, not for a sufficient filter plan.
struct SourceCountRequirement {
    rows: u64,
    necessary: bool,
    predicate: WitnessFormula,
}

pub(super) fn count_tautology() -> WitnessFormula {
    WitnessFormula::IsNull {
        term: WitnessTerm::Integer(1),
        negated: true,
    }
}

pub(super) fn count_predicate_for_source(
    direction: &WitnessDirection,
    source: &str,
    rows: u64,
) -> Option<WitnessFormula> {
    let WitnessDirection::Feasible(cases) = direction else {
        return None;
    };
    let [case] = cases.as_slice() else {
        return None;
    };
    case.obligations()
        .iter()
        .find_map(|obligation| match obligation {
            WitnessObligation::Rows {
                boundary,
                quantifier: RowQuantifier::ForAll,
                bounds,
                predicate,
                closed_world: true,
            } if boundary.kind() == GroupBoundaryKind::Physical
                && boundary.relation() == source
                && bounds.minimum() == rows
                && bounds.maximum() == Some(rows) =>
            {
                Some(predicate.clone())
            }
            _ => None,
        })
}

/// Reconcile positive and deliberately empty terminals over one schema-backed
/// source. All-positive constructions use N rows selected by every positive
/// filter; a zero terminal must reject every one of those same rows. This is
/// one sufficient closed-world assignment, not a claim that independent
/// sufficient witnesses can be concatenated or freely multiplied.
pub(super) fn joint_positive_and_rejected_pair(
    bundle: &AnalysisBundle,
    targets: &[(&str, u64)],
) -> Option<WitnessDirection> {
    let mut outputs = BTreeMap::<String, u64>::new();
    let mut positive = Vec::new();
    let mut zero = Vec::new();
    for &(layer_id, rows) in targets {
        if outputs
            .insert(layer_id.to_string(), rows)
            .is_some_and(|existing| existing != rows)
        {
            return Some(WitnessDirection::Impossible);
        }
        if rows == 0 {
            zero.push(layer_id);
        } else {
            positive.push((layer_id, rows));
        }
    }
    positive.sort_by_key(|(layer_id, _)| *layer_id);
    zero.sort();
    let &(first_positive, rows) = positive.first()?;
    if zero.is_empty() || positive.iter().any(|(_, count)| *count != rows) {
        return None;
    }
    let first_physical = physical_source_plan(bundle, first_positive);
    let [source] = first_physical.sources() else {
        return None;
    };

    let mut qualifying = Vec::new();
    for &(layer_id, _) in &positive {
        let physical = physical_source_plan(bundle, layer_id);
        if physical.sources() != [source.clone()] {
            return None;
        }
        let predicate = count_predicate_for_source(
            &physical_row_count_plan(bundle, layer_id, rows),
            source,
            rows,
        )?;
        if predicate == count_tautology() {
            continue;
        }
        if !matches!(
            predicate,
            WitnessFormula::RowTruth {
                truth: crate::boolean_witness::BooleanTruthCase::True,
                ..
            }
        ) {
            return None;
        }
        qualifying.push(predicate);
    }

    let mut rejecting = Vec::new();
    for layer_id in zero {
        let physical = physical_source_plan(bundle, layer_id);
        if physical.sources() != [source.clone()] {
            return None;
        }
        let predicate = count_predicate_for_source(
            &physical_rejected_row_count_plan(bundle, layer_id, rows),
            source,
            rows,
        )?;
        if !matches!(
            predicate,
            WitnessFormula::RowTruth {
                truth: crate::boolean_witness::BooleanTruthCase::NotTrue,
                ..
            }
        ) {
            return None;
        }
        if !rejecting.contains(&predicate) {
            rejecting.push(predicate);
        }
    }

    let mut requirements = qualifying.clone();
    requirements.extend(rejecting.iter().cloned());
    let satisfiable = joint_source_truths_satisfiable(bundle, source, &requirements)?;
    if !satisfiable {
        // Two mutually exclusive positive filters can use disjoint source
        // subsets when no exact whole-source count is demanded. By contrast,
        // no positive row can survive a negative filter that every row must
        // reject, independent of the total physical source cardinality.
        if qualifying.is_empty()
            || qualifying.iter().any(|predicate| {
                let mut necessary = vec![predicate.clone()];
                necessary.extend(rejecting.iter().cloned());
                joint_source_truths_satisfiable(bundle, source, &necessary) == Some(false)
            })
        {
            return Some(WitnessDirection::Impossible);
        }
        return None;
    }

    let row_predicate = match requirements.as_slice() {
        [predicate] => predicate.clone(),
        predicates => WitnessFormula::All(predicates.to_vec()),
    };
    let boundary = WitnessBoundary::new(source, GroupBoundaryKind::Physical, first_positive)?;
    let rows_bounds = CountBounds::new(rows, Some(rows))?;
    let mut obligations = vec![
        WitnessObligation::Rows {
            boundary: boundary.clone(),
            quantifier: RowQuantifier::ForAll,
            bounds: rows_bounds,
            predicate: row_predicate,
            closed_world: true,
        },
        WitnessObligation::ClosedWorld {
            boundary,
            coverage: ClosedWorldCoverage::EntireRelation,
        },
    ];
    for (layer_id, count) in outputs {
        obligations.push(WitnessObligation::OutputRows {
            layer_id,
            bounds: CountBounds::new(count, Some(count))?,
        });
    }
    let case = WitnessCase::new(obligations, ProofStrength::Sufficient)?;
    WitnessDirection::feasible(vec![case])
}

/// Evidence for direct physical signed integer comparisons. Kept identical
/// across the positive-only and mixed-truth physical row solvers.
pub(super) fn physical_integer_evidence(
    schema: &crate::relation::RelationSchema,
    column: &crate::protocol::ColumnRef,
) -> Option<crate::boolean_witness::SignedIntegerEvidence> {
    let known = schema
        .columns()
        .iter()
        .find(|known| known.name() == column.name())?;
    let data_type = match known.data_type() {
        crate::data_type::DataType::Nullable(inner) => inner.as_ref(),
        other => other,
    };
    match data_type {
        crate::data_type::DataType::SignedInteger { bits: Some(bits) }
            if *bits > 0 && *bits <= 64 =>
        {
            let magnitude = 1_i128 << (u32::from(*bits) - 1);
            Some(crate::boolean_witness::SignedIntegerEvidence {
                minimum: -magnitude,
                maximum: magnitude - 1,
            })
        }
        _ => None,
    }
}

/// Validate mixed SQL TRUE and NOT TRUE conditions on one *identical* row.
/// The witnessed RowVariable must have the same source, instance and name,
/// rather than merely the same relation string.
pub(super) fn joint_source_truths_satisfiable(
    bundle: &AnalysisBundle,
    source: &str,
    predicates: &[WitnessFormula],
) -> Option<bool> {
    let schema = bundle
        .source_schemas()
        .iter()
        .find(|schema| schema.relation() == source)?;
    let mut row_identity = None;
    let mut requirements = Vec::new();
    for predicate in predicates {
        let WitnessFormula::RowTruth {
            row,
            predicate,
            truth,
        } = predicate
        else {
            return None;
        };
        if row.relation() != source || row_identity.is_some_and(|identity| identity != row) {
            return None;
        }
        row_identity = Some(row);
        requirements.push((predicate, *truth));
    }
    crate::boolean_witness::jointly_satisfiable_physical_truths(
        source,
        &requirements,
        |column| {
            schema
                .columns()
                .iter()
                .any(|known| known.name() == column.name())
        },
        |column| physical_integer_evidence(schema, column),
    )
}

/// Conjoin independently proven SQL truth directions on one physical row.
/// Independent terminal examples cannot prove a common source assignment.
pub(super) fn conjoin_source_row_truths(
    bundle: &AnalysisBundle,
    source: &str,
    left: &WitnessFormula,
    right: &WitnessFormula,
) -> Option<(WitnessFormula, bool)> {
    let WitnessFormula::RowTruth {
        row: left_row,
        predicate: left_condition,
        truth: left_truth,
    } = left
    else {
        return None;
    };
    let WitnessFormula::RowTruth {
        row: right_row,
        predicate: right_condition,
        truth: right_truth,
    } = right
    else {
        return None;
    };
    if left_row != right_row || left_row.relation() != source || left_truth != right_truth {
        return None;
    }
    let schema = bundle
        .source_schemas()
        .iter()
        .find(|schema| schema.relation() == source)?;
    let (predicate, satisfiable) = crate::boolean_witness::conjoin_physical_row_truths(
        source,
        *left_truth,
        &[left_condition, right_condition],
        |column| {
            schema
                .columns()
                .iter()
                .any(|known| known.name() == column.name())
        },
        |column| physical_integer_evidence(schema, column),
    )?;
    Some((
        WitnessFormula::RowTruth {
            row: left_row.clone(),
            predicate,
            truth: *left_truth,
        },
        satisfiable,
    ))
}

/// Construct a single complete physical source assignment for several
/// terminal row-count goals. Independent sufficient cases are composed only
/// after reconciling their *shared physical source identities*.
///
/// Transparent paths require exactly the terminal count at their one source.
/// Empty-input proofs for filters, joins and ordinary grouping are sufficient
/// but not necessary, so conflicts with nonzero source requirements are
/// residual rather than (incorrectly) proved impossible.
pub fn physical_joint_row_count_plan(
    bundle: &AnalysisBundle,
    targets: &[(&str, u64)],
) -> WitnessDirection {
    if targets.is_empty() {
        return residual(PhysicalProofGap::NoWitness);
    }
    if let [(layer_id, rows)] = targets {
        if let Some(proof) = physical_join_population_count_plan(bundle, layer_id, *rows) {
            return proof;
        }
    }
    let mut ordered = targets.to_vec();
    ordered.sort();
    if ordered
        .windows(2)
        .any(|pair| pair[0].0 == pair[1].0 && pair[0].1 != pair[1].1)
    {
        // A layer cannot simultaneously emit two distinct exact row counts,
        // irrespective of which sufficient join cases happen to be sampled.
        return WitnessDirection::Impossible;
    }
    ordered.dedup();
    if necessarily_missing_positive_join_input(bundle, &ordered) {
        return WitnessDirection::Impossible;
    }
    if let Some(witness) = joint_mixed_operator_and_source_goals(bundle, &ordered) {
        return witness;
    }
    if let Some(witness) = joint_operator_population_goals(bundle, &ordered) {
        return witness;
    }
    if let [(layer_id, rows)] = ordered.as_slice() {
        if let Some(witness) = physical_subquery_membership_plan(bundle, layer_id, *rows) {
            return witness;
        }
    }
    if let [(layer_id, rows)] = ordered.as_slice() {
        if let Some(witness) = physical_group_aggregate_filter_count_plan(bundle, layer_id, *rows) {
            return witness;
        }
    }
    if let [(layer_id, rows)] = ordered.as_slice() {
        if let Some(witness) = physical_rank_partition_count_plan(bundle, layer_id, *rows) {
            return witness;
        }
    }
    if let [(layer_id, rows)] = ordered.as_slice() {
        if let Some(witness) = physical_group_rank_count_plan(bundle, layer_id, *rows) {
            return witness;
        }
    }
    if let [(layer_id, rows)] = ordered.as_slice() {
        if let Some(witness) = physical_join_group_count_plan(bundle, layer_id, *rows) {
            return witness;
        }
    }
    if let Some(witness) = jointly_realized_join_goals(bundle, &ordered) {
        return witness;
    }
    if let Some(witness) = joint_positive_and_rejected_pair(bundle, &ordered) {
        return witness;
    }

    let mut outputs = BTreeMap::<String, u64>::new();
    let mut sources = BTreeMap::<String, SourceCountRequirement>::new();
    for &(layer_id, rows) in &ordered {
        if outputs
            .insert(layer_id.to_string(), rows)
            .is_some_and(|existing| existing != rows)
        {
            return WitnessDirection::Impossible;
        }

        let row_plan = physical_row_count_plan(bundle, layer_id, rows);
        match &row_plan {
            WitnessDirection::Impossible => return WitnessDirection::Impossible,
            WitnessDirection::Residual { .. } => {
                return residual(PhysicalProofGap::LocalWitnessUnproven)
            }
            WitnessDirection::Feasible(_) => {}
        }

        let physical = physical_source_plan(bundle, layer_id);
        if physical.sources().is_empty() {
            // Source-free singletons are handled by the individual proof.
            continue;
        }

        // An all-TRUE filter construction uses exactly 'rows' physical rows,
        // but that count is sufficient, not necessary: an alternative physical
        // assignment could contain rejected rows as well.
        let one_row_plan = physical_row_count_plan(bundle, layer_id, 1);
        let has_filter = physical.sources().iter().any(|source| {
            matches!(
                count_predicate_for_source(&one_row_plan, source, 1),
                Some(WitnessFormula::RowTruth { .. })
            )
        });
        let requires_exact_count =
            !has_filter && matches!(one_row_plan, WitnessDirection::Feasible(_));
        if requires_exact_count && physical.sources().len() != 1 {
            return residual(PhysicalProofGap::UnboundPhysicalSource);
        }

        for relation in physical.sources() {
            let Some(predicate) = count_predicate_for_source(&row_plan, relation, rows) else {
                return residual(PhysicalProofGap::LocalWitnessUnproven);
            };
            match sources.get(relation) {
                Some(existing) if existing.rows != rows => {
                    if existing.necessary && requires_exact_count {
                        return WitnessDirection::Impossible;
                    }
                    return residual(PhysicalProofGap::MultipleWitnesses);
                }
                Some(existing) => {
                    let joined_predicate = if existing.predicate == count_tautology() {
                        predicate
                    } else if predicate == count_tautology() || predicate == existing.predicate {
                        existing.predicate.clone()
                    } else {
                        let Some((joint, satisfiable)) = conjoin_source_row_truths(
                            bundle,
                            relation,
                            &existing.predicate,
                            &predicate,
                        ) else {
                            return residual(PhysicalProofGap::MultipleWitnesses);
                        };
                        if !satisfiable {
                            // Only a necessary exact source count makes a
                            // disjoint pair impossible. Otherwise extra
                            // physical rows could satisfy each output separately.
                            if existing.necessary || requires_exact_count {
                                return WitnessDirection::Impossible;
                            }
                            return residual(PhysicalProofGap::MultipleWitnesses);
                        }
                        joint
                    };
                    sources.insert(
                        relation.clone(),
                        SourceCountRequirement {
                            rows,
                            necessary: existing.necessary || requires_exact_count,
                            predicate: joined_predicate,
                        },
                    );
                }
                None => {
                    sources.insert(
                        relation.clone(),
                        SourceCountRequirement {
                            rows,
                            necessary: requires_exact_count,
                            predicate,
                        },
                    );
                }
            }
        }
    }

    let origin = outputs.keys().next().map(String::as_str).unwrap_or("");
    let mut obligations = Vec::new();
    for (relation, requirement) in sources {
        let Some(boundary) = WitnessBoundary::new(&relation, GroupBoundaryKind::Physical, origin)
        else {
            return residual(PhysicalProofGap::UnboundPhysicalSource);
        };
        let Some(bounds) = CountBounds::new(requirement.rows, Some(requirement.rows)) else {
            return residual(PhysicalProofGap::LocalWitnessUnproven);
        };
        obligations.push(WitnessObligation::Rows {
            boundary: boundary.clone(),
            quantifier: RowQuantifier::ForAll,
            bounds,
            predicate: requirement.predicate,
            closed_world: true,
        });
        obligations.push(WitnessObligation::ClosedWorld {
            boundary,
            coverage: ClosedWorldCoverage::EntireRelation,
        });
    }
    for (layer_id, rows) in outputs {
        let Some(bounds) = CountBounds::new(rows, Some(rows)) else {
            return residual(PhysicalProofGap::LocalWitnessUnproven);
        };
        obligations.push(WitnessObligation::OutputRows { layer_id, bounds });
    }
    let Some(case) = WitnessCase::new(obligations, ProofStrength::Sufficient) else {
        return residual(PhysicalProofGap::LocalWitnessUnproven);
    };
    WitnessDirection::feasible(vec![case])
        .unwrap_or_else(|| residual(PhysicalProofGap::LocalWitnessUnproven))
}
