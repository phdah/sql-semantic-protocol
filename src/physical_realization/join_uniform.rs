//! TASK-113: exact uniform source-bag witnesses for portable Boolean joins.
//!
//! This solver only admits direct, independently writable, unconstrained
//! integer-key inputs. It proves the entire pair truth table on a single
//! complete physical population, never isolated sample matches.

use super::*;
use crate::constructive::UniformJoinLaw;
use crate::protocol::{ColumnExpression, ColumnRef, ComparisonOperator, Join, JoinKind};

fn oriented_pair<'a>(
    join: &Join,
    left: &'a Expression,
    right: &'a Expression,
) -> Option<(&'a ColumnExpression, &'a ColumnExpression, bool)> {
    let (Expression::Column(a), Expression::Column(b)) = (left, right) else {
        return None;
    };
    let left_instance = join.left().alias().unwrap_or(join.left().relation());
    let right_instance = join.right().alias().unwrap_or(join.right().relation());
    if a.relation() == Some(left_instance) && b.relation() == Some(right_instance) {
        Some((a, b, false))
    } else if a.relation() == Some(right_instance) && b.relation() == Some(left_instance) {
        Some((b, a, true))
    } else {
        None
    }
}

/// Recognize only normalized, fully witnessed two-column Boolean SQL forms.
/// The parser and analyzer already own syntax; this consumes canonical joins.
fn uniform_join_law(
    join: &Join,
) -> Option<(
    UniformJoinLaw,
    Option<(&ColumnExpression, &ColumnExpression)>,
)> {
    match (join.kind(), join.condition()) {
        (JoinKind::Cross, None) => Some((UniformJoinLaw::Cartesian, None)),
        (JoinKind::Inner, Some(Predicate::Comparison(cmp))) => {
            let (left, right, reversed) = oriented_pair(join, cmp.left(), cmp.right())?;
            let law = match (cmp.operator(), reversed) {
                (ComparisonOperator::Lt, false) | (ComparisonOperator::Gt, true) => {
                    UniformJoinLaw::LessThan
                }
                (ComparisonOperator::IsNotDistinctFrom, _) => UniformJoinLaw::NullSafeEquality,
                _ => return None,
            };
            Some((law, Some((left, right))))
        }
        (JoinKind::Inner, Some(Predicate::Or(or))) if or.operands().len() == 2 => {
            let predicates = or.operands();
            let (comparison, is_null) = match (&predicates[0], &predicates[1]) {
                (Predicate::Comparison(cmp), Predicate::IsNull(test)) => (cmp, test),
                (Predicate::IsNull(test), Predicate::Comparison(cmp)) => (cmp, test),
                _ => return None,
            };
            if comparison.operator() != ComparisonOperator::Eq || is_null.negated() {
                return None;
            }
            let (left, right, _) = oriented_pair(join, comparison.left(), comparison.right())?;
            let Expression::Column(null_test) = is_null.expression() else {
                return None;
            };
            if null_test.relation() != left.relation() || null_test.name() != left.name() {
                return None;
            }
            Some((UniformJoinLaw::EqualityOrLeftNull, Some((left, right))))
        }
        _ => None,
    }
}

/// Prove a complete pair bag with one uniform value per input key.
///
/// The chosen non-NULL integer values are checked against the authoritative
/// physical schemas. All other input columns remain unconstrained, so this is
/// disabled for any source with untransported schema/constraint laws.
pub(super) fn physical_uniform_join_count_plan(
    bundle: &AnalysisBundle,
    target_layer_id: &str,
    wanted: u64,
    exact_sources: &BTreeMap<String, u64>,
) -> Option<WitnessDirection> {
    let mut walker = Walker::new(bundle);
    walker.visit(target_layer_id).ok()?;
    if walker
        .nodes
        .iter()
        .filter(|node| matches!(node.id(), PhysicalPlanRef::Layer(_)))
        .count()
        != 1
    {
        return None;
    }
    let layer = walker.layers.get(target_layer_id).copied()?;
    let query = query_for(bundle, layer)?;
    let [join] = query.joins() else { return None };
    let [left, right] = query.sources() else {
        return None;
    };
    if !(query.plain_goal_output_shape() || query.filtered_join_goal_output_shape())
        || query.aggregation().is_some()
        || query.set_operation().is_some()
        || query.window_witness().is_some()
        || !query.subquery_witnesses().is_empty()
        || query.predicates().where_predicate().is_some()
        || query.predicates().having_predicate().is_some()
        || query.predicates().qualify_predicate().is_some()
        || !query.diagnostics().is_empty()
        || !walker
            .nodes
            .iter()
            .all(|node| node.produced_relations().is_empty())
        || query.output().columns().iter().any(|column| {
            !matches!(
                column.expression(),
                Expression::Column(_) | Expression::Literal(_)
            )
        })
        || left.name() == right.name()
        || join.left().relation() != left.name()
        || join.right().relation() != right.name()
        || walker.sources.len() != 2
        || !walker.sources.contains(left.name())
        || !walker.sources.contains(right.name())
        || blocks_unconstrained_physical_plans(bundle, left.name())
        || blocks_unconstrained_physical_plans(bundle, right.name())
    {
        return None;
    }
    let left_schema = bundle
        .source_schemas()
        .iter()
        .find(|s| s.relation() == left.name())?;
    let right_schema = bundle
        .source_schemas()
        .iter()
        .find(|s| s.relation() == right.name())?;
    let (law, keys) = uniform_join_law(join)?;
    let (left_key, right_key, bounds) = if let Some((l, r)) = keys {
        let l = ColumnRef::new(Some(left.name().to_string()), l.name().to_string());
        let r = ColumnRef::new(Some(right.name().to_string()), r.name().to_string());
        let bounds = (
            physical_integer_evidence(left_schema, &l)?,
            physical_integer_evidence(right_schema, &r)?,
        );
        (Some(l), Some(r), Some(bounds))
    } else {
        (None, None, None)
    };
    let (positive, negative) = match law {
        UniformJoinLaw::Cartesian => ((None, None), (None, None)),
        UniformJoinLaw::LessThan => ((Some(0), Some(1)), (Some(1), Some(0))),
        UniformJoinLaw::NullSafeEquality => ((Some(0), Some(0)), (Some(0), Some(1))),
        UniformJoinLaw::EqualityOrLeftNull => ((Some(0), Some(0)), (Some(1), Some(0))),
    };
    let mut candidates = Vec::new();
    if wanted == 0 {
        // The empty Cartesian input and the rejected nonempty Boolean pair
        // have different NOT TRUE obligations; do not conflate them.
        if law == UniformJoinLaw::Cartesian {
            candidates.push((0, 1, positive));
            candidates.push((1, 0, positive));
        } else {
            candidates.push((1, 1, negative));
            candidates.push((0, 1, positive));
            candidates.push((1, 0, positive));
        }
    } else {
        candidates.push((wanted, 1, positive));
        candidates.push((1, wanted, positive));
        for divisor in 2..=wanted.isqrt().min(64) {
            if wanted.is_multiple_of(divisor) {
                candidates.push((divisor, wanted / divisor, positive));
                candidates.push((wanted / divisor, divisor, positive));
            }
        }
    }
    let left_instance = left.alias().unwrap_or(left.name());
    let right_instance = right.alias().unwrap_or(right.name());
    if left_instance == right_instance {
        return None;
    }
    let left_row = RowVariable::new(left.name(), left_instance, "uniform_join_left")?;
    let right_row = RowVariable::new(right.name(), right_instance, "uniform_join_right")?;
    let mut cases = Vec::new();
    for (left_rows, right_rows, (left_value, right_value)) in candidates {
        if exact_sources
            .get(left.name())
            .is_some_and(|v| *v != left_rows)
            || exact_sources
                .get(right.name())
                .is_some_and(|v| *v != right_rows)
        {
            continue;
        }
        if let Some((left_bounds, right_bounds)) = &bounds {
            if left_value.is_some_and(|value| {
                i128::from(value) < left_bounds.minimum || i128::from(value) > left_bounds.maximum
            }) || right_value.is_some_and(|value| {
                i128::from(value) < right_bounds.minimum || i128::from(value) > right_bounds.maximum
            }) {
                continue;
            }
        }
        let matches = law.pair_matches(left_value, right_value)?;
        let result = if matches {
            left_rows.checked_mul(right_rows)?
        } else {
            0
        };
        if result != wanted {
            continue;
        }
        let mut obligations = Vec::new();
        for (relation, count) in [(left.name(), left_rows), (right.name(), right_rows)] {
            let boundary =
                WitnessBoundary::new(relation, GroupBoundaryKind::Physical, target_layer_id)?;
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
        obligations.push(WitnessObligation::UniformJoinPopulation {
            left_row: left_row.clone(),
            right_row: right_row.clone(),
            left_key: left_key.clone(),
            right_key: right_key.clone(),
            law,
            left_value,
            right_value,
            left_rows,
            right_rows,
            output_rows: wanted,
            closed_world: true,
        });
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
