//! Shared-source set cancellation at complete physical boundaries.

use super::*;

/// Two independently materialized, lossless copies of the same source-column
/// bag are identical even with SQL duplicates and NULL. Their EXCEPT/EXCEPT
/// ALL difference is empty. The local set law supplies the typed exact 1/1
/// negative case; the physical composer closes one actual source population.
pub(super) fn physical_shared_set_cancellation_plan(
    bundle: &AnalysisBundle,
    target_layer_id: &str,
    wanted: u64,
) -> Option<WitnessDirection> {
    physical_shared_set_cancellation_with_rows(bundle, target_layer_id, wanted, 1)
}

/// Scale the complete, lossless shared-source EXCEPT cancellation to a
/// caller-proven physical source count. Both operands read the same bag of
/// rows, including duplicate and NULL tuples, so subtracting its entire
/// multiplicity from itself yields exactly zero. No intermediate relation
/// is assigned independently.
pub(super) fn physical_shared_set_cancellation_with_rows(
    bundle: &AnalysisBundle,
    target_layer_id: &str,
    wanted: u64,
    source_rows: u64,
) -> Option<WitnessDirection> {
    use crate::protocol::{SetMultiplicityRule, SetWitnessDirection};

    if wanted != 0 || source_rows == 0 {
        return None;
    }
    let mut walker = Walker::new(bundle);
    walker.visit(target_layer_id).ok()?;
    let layer = walker.layers.get(target_layer_id).copied()?;
    let query = query_for(bundle, layer)?;
    let set = query.set_operation()?;
    if !matches!(
        set.multiplicity_rule()?,
        SetMultiplicityRule::SaturatingDifference | SetMultiplicityRule::ExceptDistinct
    ) || query.output().columns().len() != 1
        || !query.diagnostics().is_empty()
        || !matches!(layer.write_kind(), None | Some(WriteKind::Definition))
        || walker.sources.len() != 1
    {
        return None;
    }
    let [left, right] = set.branches() else {
        return None;
    };
    let map = |branch: &crate::protocol::SetBranch| {
        if branch.dependencies().len() != 1
            || branch.predicates().where_predicate().is_some()
            || branch.predicates().having_predicate().is_some()
            || branch.predicates().qualify_predicate().is_some()
            || !branch.condition_exactness().is_exact()
        {
            return None;
        }
        let boundary = branch.witness_boundary()?;
        // Column lineage alone is insufficient: a filtered named producer
        // can drop only one arm's rows while preserving its column identity.
        // Both full bags must read the identical lossless source population.
        let physical_leaf =
            physical_transparent_producer_leaf(bundle, &walker, boundary.relation())?;
        if !walker.sources.contains(&physical_leaf) {
            return None;
        }
        let [column] = boundary.tuple_columns() else {
            return None;
        };
        let input = crate::protocol::ColumnRef::new(
            Some(boundary.relation().to_string()),
            column.to_string(),
        );
        let resolved =
            resolve_filter_column_with_evidence(bundle, &walker, target_layer_id, &input, 0, true)?;
        (resolved.relation() == Some(physical_leaf.as_str())
            && resolved.relation() == walker.sources.iter().next().map(String::as_str))
        .then_some(resolved)
    };
    let left_key = map(left)?;
    let right_key = map(right)?;
    if left_key != right_key {
        return None;
    }
    let relation = left_key.relation()?;
    let schema = bundle
        .source_schemas()
        .iter()
        .find(|item| item.relation() == relation)?;
    let evidence = physical_integer_evidence(schema, &left_key)?;
    if evidence.minimum > 0
        || evidence.maximum < 0
        || bundle.relation_constraints().iter().any(|set| {
            set.relation() == relation
                && (!set.constraints().is_empty() || !set.diagnostics().is_empty())
        })
    {
        return None;
    }

    // The local typed operator proof must actually admit cancellation of
    // one candidate from each branch. Never infer EXCEPT semantics from a
    // bare textual operator name or from a sample tuple.
    let (_, SetWitnessDirection::Exact(negative)) = set.witness_directions() else {
        return None;
    };
    let witness = negative.iter().find(|candidate| {
        candidate.output_tuple_count() == 0
            && candidate.obligations().len() == 2
            && candidate
                .obligations()
                .iter()
                .all(|part| part.matching_tuple_count() == 1)
    })?;
    let boundary = WitnessBoundary::new(relation, GroupBoundaryKind::Physical, target_layer_id)?;
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
    for part in witness.obligations() {
        obligations.push(WitnessObligation::SetTuple {
            branch_identity: part.branch_identity().to_string(),
            boundary: boundary.clone(),
            tuple_columns: vec![left_key.name().to_string()],
            matching_rows: source_rows,
            column_domains: Vec::new(),
            closed_world: true,
        });
    }
    obligations.push(WitnessObligation::SetResultTuple {
        matching_rows: 0,
        nulls_equal: true,
    });
    obligations.push(WitnessObligation::OutputRows {
        layer_id: target_layer_id.to_string(),
        bounds: CountBounds::new(0, Some(0))?,
    });
    let case = WitnessCase::new(obligations, ProofStrength::Sufficient)?;
    WitnessDirection::feasible(vec![case])
}
