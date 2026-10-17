//! Finite, typed closed-world ASOF and per-left-row correlated population proofs.
//!
//! Joins carry normalized AST evidence; this solver never reparses SQL text.
//! Unsupported projections, source restrictions, tie-breaking and untransported
//! constraints fail closed before any physical output cardinality is certified.

use super::*;
use crate::constructive::FiniteJoinLaw;
use crate::protocol::{ColumnRef, ComparisonOperator, JoinExecution};

fn correlated_pair<'a>(
    left_relation: &str,
    right_relation: &str,
    predicate: &'a Predicate,
) -> Option<(
    &'a crate::protocol::ColumnExpression,
    &'a crate::protocol::ColumnExpression,
    ComparisonOperator,
)> {
    let Predicate::Comparison(cmp) = predicate else {
        return None;
    };
    let (Expression::Column(first), Expression::Column(second)) = (cmp.left(), cmp.right()) else {
        return None;
    };
    if first.relation() == Some(left_relation) && second.relation() == Some(right_relation) {
        Some((first, second, cmp.operator()))
    } else if second.relation() == Some(left_relation) && first.relation() == Some(right_relation) {
        let reversed = match cmp.operator() {
            ComparisonOperator::Lt => ComparisonOperator::Gt,
            ComparisonOperator::Lte => ComparisonOperator::Gte,
            ComparisonOperator::Gt => ComparisonOperator::Lt,
            ComparisonOperator::Gte => ComparisonOperator::Lte,
            ComparisonOperator::Eq => ComparisonOperator::Eq,
            _ => return None,
        };
        Some((second, first, reversed))
    } else {
        None
    }
}

type FiniteSourceBag = Vec<Option<i64>>;
type FiniteCandidateBags = Vec<(FiniteSourceBag, FiniteSourceBag)>;

/// Choose complete source rows for a bounded cardinality goal.
/// The ASOF two-row construction deliberately matches the independent
/// [5,10] and [4,7] predecessor fixture, including the nonwinning true pair.
fn candidate_bags(law: FiniteJoinLaw, wanted: u64) -> Option<FiniteCandidateBags> {
    if wanted > 64 {
        return None;
    }
    let n = usize::try_from(wanted).ok()?;
    if wanted == 0 {
        return Some(vec![(Vec::new(), Vec::new())]);
    }
    let integer_bag = |size: usize| {
        (0..size)
            .map(|index| i64::try_from(index).ok().and_then(|i| i.checked_add(1)))
            .collect::<Vec<_>>()
    };
    let mut candidates = match law {
        FiniteJoinLaw::AsOfNearestPriorInclusive => {
            let (left, right) = if wanted == 2 {
                (vec![Some(5), Some(10)], vec![Some(4), Some(7)])
            } else {
                let left = (0..n)
                    .map(|index| {
                        i64::try_from(index)
                            .ok()
                            .and_then(|i| i.checked_mul(5))
                            .and_then(|v| v.checked_add(5))
                    })
                    .collect::<Vec<_>>();
                let right = (0..n)
                    .map(|index| {
                        i64::try_from(index)
                            .ok()
                            .and_then(|i| i.checked_mul(5))
                            .and_then(|v| v.checked_add(4))
                    })
                    .collect::<Vec<_>>();
                (left, right)
            };
            vec![(left, right)]
        }
        FiniteJoinLaw::CorrelatedInnerEquality => {
            let keys = integer_bag(n);
            let mut cases = vec![(keys.clone(), keys)];
            if wanted > 1 {
                cases.push((vec![Some(1)], vec![Some(1); n]));
            }
            cases
        }
        FiniteJoinLaw::CorrelatedOuterEquality => {
            vec![(integer_bag(n), vec![Some(1)])]
        }
    };
    if matches!(
        law,
        FiniteJoinLaw::AsOfNearestPriorInclusive | FiniteJoinLaw::CorrelatedOuterEquality
    ) {
        // Null-extension is a result-row law; the physical right source is
        // genuinely empty, never one source row containing NULL.
        candidates.push((integer_bag(n), Vec::new()));
    }
    Some(candidates)
}

/// Certified finite row-population proof for a single simple source-level
/// ASOF / LATERAL / APPLY operator and an exactly bounded goal.
pub(super) fn physical_finite_join_count_plan(
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
    let execution = join.execution()?;
    let [left_source, _] = query.sources() else {
        return None;
    };
    let right_source = match execution {
        JoinExecution::AsOfNearest => query.sources().get(1)?.name(),
        JoinExecution::LateralInner | JoinExecution::CrossApply | JoinExecution::OuterApply => {
            join.right_physical_source()?
        }
    };
    if left_source.name() == right_source
        || walker.sources.len() != 2
        || !walker.sources.contains(left_source.name())
        || !walker.sources.contains(right_source)
        || !matches!(join.left().relation(), name if name == left_source.name())
        || !matches!(query.output().columns(), columns if columns.iter().all(|column| matches!(column.expression(), Expression::Column(_) | Expression::Literal(_))))
        || !(query.plain_goal_output_shape() || query.filtered_join_goal_output_shape())
        || query.aggregation().is_some()
        || query.set_operation().is_some()
        || query.window_witness().is_some()
        || !query.subquery_witnesses().is_empty()
        || query.predicates().where_predicate().is_some()
        || query.predicates().having_predicate().is_some()
        || query.predicates().qualify_predicate().is_some()
        || layer
            .write_kind()
            .is_some_and(|kind| !kind.fully_defines_relation())
        || walker.nodes.iter().any(|node| {
            node.produced_relations()
                .iter()
                .any(|relation| untransported_materialization_laws(bundle, relation))
        })
        || blocks_unconstrained_physical_plans(bundle, left_source.name())
        || blocks_unconstrained_physical_plans(bundle, right_source)
    {
        return None;
    }
    // No general-purpose diagnostics may be discarded to manufacture a proof.
    // Only the previously recorded canonical semantic "needs physical proof"
    // and local correlated scope diagnostics can be discharged by this solver.
    if query.diagnostics().iter().any(|diag| {
        !matches!(
            diag.code(),
            "unsupported_asof_nearest_prior"
                | "unsupported_cross_apply"
                | "unsupported_outer_apply"
                | "unsupported_lateral_row_scope"
        )
    }) {
        return None;
    }
    let (law, right_qualifier) = match execution {
        JoinExecution::AsOfNearest => (
            FiniteJoinLaw::AsOfNearestPriorInclusive,
            join.right().alias().unwrap_or(join.right().relation()),
        ),
        JoinExecution::LateralInner | JoinExecution::CrossApply => {
            (FiniteJoinLaw::CorrelatedInnerEquality, right_source)
        }
        JoinExecution::OuterApply => (FiniteJoinLaw::CorrelatedOuterEquality, right_source),
    };
    let left_qualifier = left_source.alias().unwrap_or(left_source.name());
    let (left_expr, right_expr, operator) =
        correlated_pair(left_qualifier, right_qualifier, join.condition()?)?;
    if !match execution {
        JoinExecution::AsOfNearest => operator == ComparisonOperator::Gte,
        JoinExecution::LateralInner | JoinExecution::CrossApply | JoinExecution::OuterApply => {
            operator == ComparisonOperator::Eq
        }
    } {
        return None;
    }
    let left_key = ColumnRef::new(
        Some(left_source.name().to_string()),
        left_expr.name().to_string(),
    );
    let right_key = ColumnRef::new(
        Some(right_source.to_string()),
        right_expr.name().to_string(),
    );
    let left_schema = bundle
        .source_schemas()
        .iter()
        .find(|schema| schema.relation() == left_source.name())?;
    let right_schema = bundle
        .source_schemas()
        .iter()
        .find(|schema| schema.relation() == right_source)?;
    let left_bounds = physical_integer_evidence(left_schema, &left_key)?;
    let right_bounds = physical_integer_evidence(right_schema, &right_key)?;
    let left_row = RowVariable::new(left_source.name(), left_qualifier, "finite_left")?;
    let right_row = RowVariable::new(right_source, right_qualifier, "finite_right")?;
    let mut cases = Vec::new();
    for (left_values, right_values) in candidate_bags(law, wanted)? {
        if left_values
            .iter()
            .flatten()
            .any(|v| i128::from(*v) < left_bounds.minimum || i128::from(*v) > left_bounds.maximum)
            || right_values.iter().flatten().any(|v| {
                i128::from(*v) < right_bounds.minimum || i128::from(*v) > right_bounds.maximum
            })
            || exact_sources
                .get(left_source.name())
                .is_some_and(|count| usize::try_from(*count).ok() != Some(left_values.len()))
            || exact_sources
                .get(right_source)
                .is_some_and(|count| usize::try_from(*count).ok() != Some(right_values.len()))
        {
            continue;
        }
        let selected = law.selected_rows(&left_values, &right_values)?;
        if u64::try_from(selected.len()).ok() != Some(wanted) {
            continue;
        }
        let mut obligations = Vec::new();
        for (relation, count) in [
            (left_source.name(), left_values.len()),
            (right_source, right_values.len()),
        ] {
            let boundary =
                WitnessBoundary::new(relation, GroupBoundaryKind::Physical, target_layer_id)?;
            let count = u64::try_from(count).ok()?;
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
        obligations.push(WitnessObligation::FiniteJoinPopulation {
            left_row: left_row.clone(),
            right_row: right_row.clone(),
            left_key: left_key.clone(),
            right_key: right_key.clone(),
            law,
            left_values,
            right_values,
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
