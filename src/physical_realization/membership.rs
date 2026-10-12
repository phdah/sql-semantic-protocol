//! Closed-world subquery membership.

use super::*;

/// A complete empty inner source makes a locally exact correlated or
/// uncorrelated EXISTS/IN membership predicate false for *every* outer row
/// (and NOT EXISTS/NOT IN true). This is valid through transparent producer
/// chains without pretending that a locally feasible candidate represents
/// every row. Nonempty correlated matches require key-population proofs and
/// retain typed residuals here.
pub(super) fn physical_empty_subquery_membership_plan(
    bundle: &AnalysisBundle,
    target_layer_id: &str,
    wanted: u64,
) -> Option<WitnessDirection> {
    use crate::subquery_witness::{
        SubqueryMembershipCase, SubqueryMembershipDirection, SubqueryMembershipKind,
    };
    let mut walker = Walker::new(bundle);
    walker.visit(target_layer_id).ok()?;
    let layer = walker.layers.get(target_layer_id).copied()?;
    let query = query_for(bundle, layer)?;
    let [outer_relation] = query.sources() else {
        return None;
    };
    let [local] = query.subquery_witnesses() else {
        return None;
    };
    if !matches!(
        query.predicates().where_predicate()?,
        Predicate::Exists(_) | Predicate::InSubquery(_)
    ) || !matches!(layer.write_kind(), None | Some(WriteKind::Definition))
        || !query.joins().is_empty()
        || query.aggregation().is_some()
        || query.set_operation().is_some()
        || query.window_witness().is_some()
        || !query.diagnostics().is_empty()
        || !query
            .output()
            .columns()
            .iter()
            .all(|column| column.plain_copy_source().is_some())
    {
        return None;
    }
    let empty_selects = matches!(
        local.kind(),
        SubqueryMembershipKind::NotExists | SubqueryMembershipKind::NotIn
    );
    if empty_selects != (wanted > 0) {
        return None;
    }
    let direction = if empty_selects {
        local.qualifying()
    } else {
        local.rejected()
    };
    if !matches!(direction, SubqueryMembershipDirection::Exact(cases)
        if cases.contains(&SubqueryMembershipCase::NoCandidates))
    {
        return None;
    }
    let outer_physical =
        physical_transparent_producer_leaf(bundle, &walker, outer_relation.name())?;
    let inner_relation = local.inner_relation()?;
    let inner_physical = physical_transparent_producer_leaf(bundle, &walker, inner_relation)?;
    if outer_physical == inner_physical
        || walker.sources.len() != 2
        || !walker.sources.contains(&outer_physical)
        || !walker.sources.contains(&inner_physical)
    {
        return None;
    }
    // Only independently controllable, actually attested relations can be
    // assigned source rows; constraints cannot be assumed away.
    for relation in [&outer_physical, &inner_physical] {
        bundle
            .source_schemas()
            .iter()
            .find(|s| s.relation() == relation)?;
        if bundle.relation_constraints().iter().any(|set| {
            set.relation() == relation
                && (!set.constraints().is_empty() || !set.diagnostics().is_empty())
        }) {
            return None;
        }
    }
    let outer_rows = wanted.max(1);
    let outer_boundary = WitnessBoundary::new(
        &outer_physical,
        GroupBoundaryKind::Physical,
        target_layer_id,
    )?;
    let inner_boundary = WitnessBoundary::new(
        &inner_physical,
        GroupBoundaryKind::Physical,
        target_layer_id,
    )?;
    let outer_row = RowVariable::new(&outer_physical, outer_relation.name(), "membership_outer")?;
    let mut obligations = vec![WitnessObligation::Membership {
        outer: outer_row,
        inner: inner_boundary.clone(),
        case: SubqueryMembershipCase::NoCandidates,
        // Inner is physically empty, so no correlated key/value can
        // be evaluated; all local domain tests are vacuous.
        correlations: Vec::new(),
        membership_key: None,
        inner_domains: Vec::new(),
        closed_world: true,
    }];
    for (boundary, count) in [(outer_boundary, outer_rows), (inner_boundary, 0)] {
        obligations.push(WitnessObligation::Rows {
            boundary: boundary.clone(),
            quantifier: RowQuantifier::ForAll,
            bounds: CountBounds::new(count, Some(count))?,
            predicate: count_tautology(),
            closed_world: true,
        });
        obligations.push(WitnessObligation::ClosedWorld {
            boundary,
            coverage: ClosedWorldCoverage::EntireRelation,
        });
    }
    obligations.push(WitnessObligation::OutputRows {
        layer_id: target_layer_id.to_string(),
        bounds: CountBounds::new(wanted, Some(wanted))?,
    });
    let case = WitnessCase::new(obligations, ProofStrength::Sufficient)?;
    WitnessDirection::feasible(vec![case])
}
