//! Closed-world temporal inequality witnesses with explicit session-independent
//! timestamp evidence. No timezone/session coercion is inferred for bare TIMESTAMP.

use super::*;
use crate::constructive::TemporalJoinLaw;
use crate::data_type::DataType;
use crate::protocol::{ColumnRef, ComparisonOperator, JoinKind};
use crate::relation::TimestampZone;

fn timestamp_key(
    source: &str,
    columns: &[crate::relation::SchemaColumn],
    name: &str,
) -> Option<ColumnRef> {
    let column = columns.iter().find(|col| col.name() == name)?;
    if !matches!(column.data_type(), DataType::Timestamp { .. })
        || column.timestamp_zone() != Some(TimestampZone::WithoutTimeZone)
    {
        return None;
    }
    Some(ColumnRef::new(Some(source.to_string()), name.to_string()))
}

fn timestamp(text: &str) -> Option<String> {
    TemporalJoinLaw::valid_timestamp(text).then(|| text.to_string())
}

type TemporalSourceBag = Vec<Option<String>>;
type TemporalCandidateBags = Vec<(TemporalSourceBag, TemporalSourceBag)>;

fn timestamp_candidates(wanted: u64) -> Option<TemporalCandidateBags> {
    let a = || timestamp("2026-10-17 00:05:00");
    let b = || timestamp("2026-10-17 00:10:00");
    let c = || timestamp("2026-10-17 00:04:00");
    let d = || timestamp("2026-10-17 00:07:00");
    let candidates = if wanted == 3 {
        vec![(vec![a(), b()], vec![c(), d()])]
    } else if wanted == 0 {
        vec![(vec![c()], vec![d()]), (Vec::new(), vec![c()])]
    } else if wanted <= 64 {
        let n = usize::try_from(wanted).ok()?;
        vec![(vec![a(); n], vec![c()])]
    } else {
        return None;
    };
    Some(candidates)
}

/// A complete source-backed, parser-normalized inner timestamp inequality law.
pub(super) fn physical_temporal_join_count_plan(
    bundle: &AnalysisBundle,
    layer_id: &str,
    wanted: u64,
    exact_sources: &BTreeMap<String, u64>,
) -> Option<WitnessDirection> {
    let mut walker = Walker::new(bundle);
    walker.visit(layer_id).ok()?;
    if walker
        .nodes
        .iter()
        .filter(|n| matches!(n.id(), PhysicalPlanRef::Layer(_)))
        .count()
        != 1
    {
        return None;
    }
    let layer = walker.layers.get(layer_id).copied()?;
    let query = query_for(bundle, layer)?;
    let [join] = query.joins() else { return None };
    let [left, right] = query.sources() else {
        return None;
    };
    if join.kind() != JoinKind::Inner
        || join.execution().is_some()
        || join.left().relation() != left.name()
        || join.right().relation() != right.name()
        || left.name() == right.name()
        || walker.sources.len() != 2
        || !walker.sources.contains(left.name())
        || !walker.sources.contains(right.name())
        || !query.plain_goal_output_shape()
        || !query.diagnostics().is_empty()
        || query.aggregation().is_some()
        || query.set_operation().is_some()
        || query.window_witness().is_some()
        || !query.subquery_witnesses().is_empty()
        || query.predicates().where_predicate().is_some()
        || query.predicates().having_predicate().is_some()
        || query.predicates().qualify_predicate().is_some()
        || query.output().columns().iter().any(|col| {
            !matches!(
                col.expression(),
                Expression::Column(_) | Expression::Literal(_)
            )
        })
        || layer
            .write_kind()
            .is_some_and(|kind| !kind.fully_defines_relation())
        || blocks_unconstrained_physical_plans(bundle, left.name())
        || blocks_unconstrained_physical_plans(bundle, right.name())
    {
        return None;
    }
    let Predicate::Comparison(cmp) = join.condition()? else {
        return None;
    };
    let (Expression::Column(a), Expression::Column(b)) = (cmp.left(), cmp.right()) else {
        return None;
    };
    let left_instance = left.alias().unwrap_or(left.name());
    let right_instance = right.alias().unwrap_or(right.name());
    if left_instance == right_instance {
        return None;
    }
    let (left_column, right_column, op) =
        if a.relation() == Some(left_instance) && b.relation() == Some(right_instance) {
            (a, b, cmp.operator())
        } else if a.relation() == Some(right_instance) && b.relation() == Some(left_instance) {
            (
                b,
                a,
                match cmp.operator() {
                    ComparisonOperator::Lte => ComparisonOperator::Gte,
                    _ => return None,
                },
            )
        } else {
            return None;
        };
    if op != ComparisonOperator::Gte {
        return None;
    }
    let left_schema = bundle
        .source_schemas()
        .iter()
        .find(|schema| schema.relation() == left.name())?;
    let right_schema = bundle
        .source_schemas()
        .iter()
        .find(|schema| schema.relation() == right.name())?;
    let left_key = timestamp_key(left.name(), left_schema.columns(), left_column.name())?;
    let right_key = timestamp_key(right.name(), right_schema.columns(), right_column.name())?;
    let left_row = RowVariable::new(left.name(), left_instance, "temporal_left")?;
    let right_row = RowVariable::new(right.name(), right_instance, "temporal_right")?;
    let law = TemporalJoinLaw::GreaterThanOrEqual;
    let mut cases = Vec::new();
    for (left_values, right_values) in timestamp_candidates(wanted)? {
        if exact_sources
            .get(left.name())
            .is_some_and(|count| usize::try_from(*count).ok() != Some(left_values.len()))
            || exact_sources
                .get(right.name())
                .is_some_and(|count| usize::try_from(*count).ok() != Some(right_values.len()))
            || law.selected_rows(&left_values, &right_values)?.len() as u64 != wanted
        {
            continue;
        }
        let mut obligations = Vec::new();
        for (relation, count) in [
            (left.name(), left_values.len()),
            (right.name(), right_values.len()),
        ] {
            let boundary = WitnessBoundary::new(relation, GroupBoundaryKind::Physical, layer_id)?;
            let rows = u64::try_from(count).ok()?;
            obligations.push(WitnessObligation::Rows {
                boundary: boundary.clone(),
                quantifier: RowQuantifier::ForAll,
                bounds: CountBounds::new(rows, Some(rows))?,
                predicate: count_tautology(),
                closed_world: true,
            });
            obligations.push(WitnessObligation::ClosedWorld {
                boundary,
                coverage: ClosedWorldCoverage::EntireRelation,
            });
        }
        obligations.push(WitnessObligation::TemporalJoinPopulation {
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
            layer_id: layer_id.to_string(),
            bounds: CountBounds::new(wanted, Some(wanted))?,
        });
        if let Some(case) = WitnessCase::new(obligations, ProofStrength::Sufficient) {
            cases.push(case);
        }
    }
    WitnessDirection::feasible(cases)
}
