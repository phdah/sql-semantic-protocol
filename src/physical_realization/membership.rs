//! Closed-world subquery membership.

use super::*;

/// A nested filter is safe for the controlled-key population only when every
/// conjunct is a correlated equality. Other inner restrictions require their
/// own jointly realized source assignment and remain residual.
fn correlated_equalities_only(predicate: &Predicate) -> bool {
    match predicate {
        Predicate::And(and) => and.operands().iter().all(correlated_equalities_only),
        Predicate::Comparison(comparison) => {
            comparison.operator() == crate::protocol::ComparisonOperator::Eq
                && matches!(comparison.left(), Expression::Column(_))
                && matches!(comparison.right(), Expression::Column(_))
        }
        _ => false,
    }
}

/// Assign one complete non-NULL integer key population. All source rows are
/// constrained, not merely an arbitrary membership candidate. Repeated keys
/// mapped onto the same physical column must agree on their chosen value.
fn assign_membership_key(
    bundle: &AnalysisBundle,
    walker: &Walker<'_>,
    target: &str,
    key: &crate::protocol::ColumnRef,
    value: i64,
    assignments: &mut BTreeMap<(String, String), i64>,
) -> Option<crate::protocol::ColumnRef> {
    let mapped = resolve_filter_column_with_evidence(bundle, walker, target, key, 0, true)?;
    let relation = mapped.relation()?;
    if !walker.sources.contains(relation) {
        return None;
    }
    let schema = bundle
        .source_schemas()
        .iter()
        .find(|s| s.relation() == relation)?;
    let domain = physical_integer_evidence(schema, &mapped)?;
    if domain.minimum > i128::from(value) || domain.maximum < i128::from(value) {
        return None;
    }
    let identity = (relation.to_string(), mapped.name().to_string());
    if assignments
        .insert(identity, value)
        .is_some_and(|prior| prior != value)
    {
        return None;
    }
    Some(mapped)
}

/// Construct one complete physical population for an EXISTS/IN/NOT predicate
/// through certified, identity-only producer chains. Correlated equality and
/// IN matching keys use a shared, typed integer value for *every* physical
/// outer/inner row. A distinct key realizes complete absence for a correlated
/// EXISTS or a non-NULL IN candidate set. No SQL NULL or arbitrary nested
/// filter is silently equated with FALSE.
pub(super) fn physical_subquery_membership_plan(
    bundle: &AnalysisBundle,
    target_layer_id: &str,
    wanted: u64,
) -> Option<WitnessDirection> {
    use crate::protocol::ComparisonOperator;
    use crate::subquery_witness::{
        SubqueryCorrelation, SubqueryMembershipCase, SubqueryMembershipDirection,
        SubqueryMembershipKind,
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
    let inner_filter = match query.predicates().where_predicate()? {
        Predicate::Exists(exists) => exists.subquery().predicates().where_predicate(),
        Predicate::InSubquery(membership) => membership.subquery().predicates().where_predicate(),
        _ => return None,
    };
    if !matches!(layer.write_kind(), None | Some(WriteKind::Definition))
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

    // Only nonempty constructions need the entire nested predicate to be
    // certified as correlated key equalities. Empty-inner source proofs remain
    // valid even when other certified inner filters are present.
    let safe_nonempty = match inner_filter {
        None => local.correlations().is_empty() && local.inner_column_domains().is_empty(),
        // The normal domain analyzer cannot attach a scalar bound to a
        // correlation, so its Unknown(column-vs-column) domains do not veto
        // an otherwise exact, fully enumerated typed correlation witness.
        // No additional inner filter is accepted on this path.
        Some(predicate) => {
            !local.correlations().is_empty() && correlated_equalities_only(predicate)
        }
    };

    let positive = wanted > 0;
    let direction = if positive {
        local.qualifying()
    } else {
        local.rejected()
    };
    let SubqueryMembershipDirection::Exact(allowed) = direction else {
        return None;
    };

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
    for relation in [&outer_physical, &inner_physical] {
        bundle
            .source_schemas()
            .iter()
            .find(|s| s.relation() == relation)?;
        if bundle
            .relation_constraints()
            .iter()
            .any(|set| set.relation() == relation && set.blocks_unconstrained_physical_plans())
        {
            return None;
        }
    }

    // Each entry is a complete sufficient construction, not an exhaustive
    // classification. Zero-inner is still valid for arbitrary correlations
    // and membership keys, since no candidate can survive.
    let empty_selects = matches!(
        local.kind(),
        SubqueryMembershipKind::NotExists | SubqueryMembershipKind::NotIn
    );
    let mut populations = Vec::new();
    if empty_selects == positive {
        populations.push((SubqueryMembershipCase::NoCandidates, 0_u64, false));
    }
    match (local.kind(), positive) {
        (SubqueryMembershipKind::Exists, true) | (SubqueryMembershipKind::NotExists, false) => {
            populations.push((SubqueryMembershipCase::MatchingRow, 1, false));
        }
        (SubqueryMembershipKind::In, true) | (SubqueryMembershipKind::NotIn, false) => {
            populations.push((SubqueryMembershipCase::MatchingNonNullKey, 1, false));
        }
        (SubqueryMembershipKind::Exists, false) | (SubqueryMembershipKind::NotExists, true)
            if !local.correlations().is_empty() =>
        {
            populations.push((SubqueryMembershipCase::NoCandidates, 1, true));
        }
        (SubqueryMembershipKind::In, false) | (SubqueryMembershipKind::NotIn, true) => {
            populations.push((SubqueryMembershipCase::NoMatchNoNull, 1, true));
        }
        _ => {}
    }

    let outer_rows = wanted.max(1);
    let mut cases = Vec::new();
    for (membership_case, inner_rows, nonmatching) in populations {
        if (inner_rows > 0 && !safe_nonempty) || !allowed.contains(&membership_case) {
            continue;
        }
        let mut assignments = BTreeMap::<(String, String), i64>::new();
        let mut correlations = Vec::new();
        let mut invalid = false;
        if inner_rows > 0 {
            for pair in local.correlations() {
                let outer_value = if nonmatching && local.membership_key().is_none() {
                    1
                } else {
                    0
                };
                let Some(outer) = assign_membership_key(
                    bundle,
                    &walker,
                    target_layer_id,
                    pair.outer(),
                    outer_value,
                    &mut assignments,
                ) else {
                    invalid = true;
                    break;
                };
                let Some(inner) = assign_membership_key(
                    bundle,
                    &walker,
                    target_layer_id,
                    pair.inner(),
                    0,
                    &mut assignments,
                ) else {
                    invalid = true;
                    break;
                };
                let Some(mapped) = SubqueryCorrelation::new(outer, inner) else {
                    invalid = true;
                    break;
                };
                correlations.push(mapped);
            }
        }
        if invalid {
            continue;
        }
        let membership_key = if inner_rows > 0 {
            if let Some(pair) = local.membership_key() {
                let Some(outer) = assign_membership_key(
                    bundle,
                    &walker,
                    target_layer_id,
                    pair.outer(),
                    if nonmatching { 1 } else { 0 },
                    &mut assignments,
                ) else {
                    continue;
                };
                let Some(inner) = assign_membership_key(
                    bundle,
                    &walker,
                    target_layer_id,
                    pair.inner(),
                    0,
                    &mut assignments,
                ) else {
                    continue;
                };
                SubqueryCorrelation::new(outer, inner)
            } else {
                None
            }
        } else {
            None
        };
        if inner_rows > 0 && local.membership_key().is_some() && membership_key.is_none() {
            continue;
        }
        let Some(outer_boundary) = WitnessBoundary::new(
            &outer_physical,
            GroupBoundaryKind::Physical,
            target_layer_id,
        ) else {
            continue;
        };
        let Some(inner_boundary) = WitnessBoundary::new(
            &inner_physical,
            GroupBoundaryKind::Physical,
            target_layer_id,
        ) else {
            continue;
        };
        let Some(outer_row) =
            RowVariable::new(&outer_physical, local.outer_relation(), "membership_outer")
        else {
            continue;
        };
        let Some(inner_row) = RowVariable::new(&inner_physical, inner_relation, "membership_inner")
        else {
            continue;
        };
        let mut obligations = vec![WitnessObligation::Membership {
            outer: outer_row.clone(),
            inner: inner_boundary.clone(),
            case: membership_case,
            correlations,
            membership_key,
            inner_domains: Vec::new(),
            closed_world: true,
        }];
        for (boundary, count, variable) in [
            (outer_boundary, outer_rows, outer_row),
            (inner_boundary, inner_rows, inner_row),
        ] {
            let predicates = assignments
                .iter()
                .filter(|((relation, _), _)| relation.as_str() == boundary.relation())
                .map(|((relation, column), value)| WitnessFormula::Comparison {
                    left: WitnessTerm::Column {
                        row: variable.clone(),
                        column: crate::protocol::ColumnRef::new(
                            Some(relation.clone()),
                            column.clone(),
                        ),
                    },
                    operator: ComparisonOperator::Eq,
                    right: WitnessTerm::Integer(*value),
                })
                .collect::<Vec<_>>();
            let predicate = if predicates.is_empty() {
                count_tautology()
            } else {
                WitnessFormula::All(predicates)
            };
            obligations.push(WitnessObligation::Rows {
                boundary: boundary.clone(),
                quantifier: RowQuantifier::ForAll,
                bounds: CountBounds::new(count, Some(count))?,
                predicate,
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
        if let Some(case) = WitnessCase::new(obligations, ProofStrength::Sufficient) {
            cases.push(case);
        }
    }
    WitnessDirection::feasible(cases)
}
