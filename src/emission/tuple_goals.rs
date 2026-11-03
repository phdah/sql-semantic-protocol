//! Opt-in typed correlated-tuple proof sidecar. The active protocol root is unchanged.
use super::joint_terminals::{domains_to_value, source_to_value, terminal_to_value};
use super::*;
use crate::constraints::ConstraintValue;
use crate::physical_realization::{
    physical_single_tuple_histogram_plan, PhysicalSingleTupleExact, PhysicalSingleTupleProof,
    PhysicalTupleHistogramGoal, PhysicalWriteState,
};

fn requested_scalar(value: &ConstraintValue) -> Value {
    match value {
        ConstraintValue::Null => json!({"kind":"null"}),
        ConstraintValue::Integer(number) => json!({"kind":"integer","value":number.to_string()}),
        ConstraintValue::String(text) => json!({"kind":"string","value":text}),
        ConstraintValue::UnsignedInteger(number) =>
            json!({"kind":"unsigned_integer","value":number.to_string()}),
        ConstraintValue::Number(number) => json!({"kind":"decimal","value":number}),
        ConstraintValue::Boolean(value) => json!({"kind":"boolean","value":value}),
    }
}

/// Deterministically emit an independently verified terminal and its requested
/// multi-column distribution. Only Exact contains complete source/result bags.
///
/// This function evaluates the canonical proof itself so callers cannot pair a
/// proof with an unrelated histogram or physical assignment. No sqlparser AST
/// is exposed and no second active protocol version is introduced.
pub fn to_physical_single_tuple_json(
    bundle: &AnalysisBundle,
    snapshots: &[PhysicalWriteState],
    request: &PhysicalTupleHistogramGoal,
) -> String {
    let result = physical_single_tuple_histogram_plan(bundle, snapshots, request);
    let evidence = match result {
        PhysicalSingleTupleProof::Exact(exact) => {
            let (kind, relation, selected, rejected, rows, domains, extra) = match exact {
                PhysicalSingleTupleExact::Page(page) => {
                    let selected = page
                        .rows
                        .iter()
                        .filter(|row| row.survives)
                        .map(|row| row.source_row_id.clone())
                        .collect::<Vec<_>>();
                    let rejected = page
                        .rows
                        .iter()
                        .filter(|row| !row.survives)
                        .map(|row| row.source_row_id.clone())
                        .collect::<Vec<_>>();
                    (
                        "page",
                        page.source.clone(),
                        selected,
                        rejected,
                        terminal_to_value(&page.terminal),
                        domains_to_value(&page.terminal_domains),
                        json!({"contributors":page.terminal_contributors}),
                    )
                }
                PhysicalSingleTupleExact::Window(window) => (
                    "window",
                    window.source.clone(),
                    window.terminal_row_ids.clone(),
                    window.rejected_row_ids.clone(),
                    terminal_to_value(&window.terminal),
                    domains_to_value(&window.terminal_domains),
                    json!({
                        "source_row_ids":window.source_row_ids,
                        "rejected_upstream_row_ids":window.rejected_upstream_row_ids
                    }),
                ),
                PhysicalSingleTupleExact::Rank(rank) => (
                    "rank",
                    rank.source.clone(),
                    rank.ranked_rows
                        .iter()
                        .filter(|row| row.survives)
                        .map(|row| row.source_row_id.clone())
                        .collect(),
                    rank.ranked_rows
                        .iter()
                        .filter(|row| !row.survives)
                        .map(|row| row.source_row_id.clone())
                        .collect(),
                    terminal_to_value(&rank.terminal),
                    domains_to_value(&rank.terminal_domains),
                    json!({"ranked_rows":rank.ranked_rows.iter().map(|row|json!({
                        "id":row.source_row_id,
                        "row_number":row.row_number,
                        "rank":row.rank,
                        "dense_rank":row.dense_rank,
                        "survives":row.survives
                    })).collect::<Vec<_>>()}),
                ),
            };
            let source = snapshots
                .iter()
                .find(|state| state.relation() == relation)
                .map(source_to_value);
            json!({
                "status":"exact","operator":kind,"source":source,
                "selected_row_ids":selected,"not_true_row_ids":rejected,
                "terminal":rows,"domains":domains,"details":extra
            })
        }
        PhysicalSingleTupleProof::Impossible { reason } => {
            json!({"status":"impossible","reason":reason})
        }
        PhysicalSingleTupleProof::Residual { reason } => {
            json!({"status":"residual","reason":reason})
        }
    };
    json!({
        "protocol_version":bundle.protocol_version(),
        "kind":"physical_single_tuple_evidence",
        "layer_id":request.layer_id(),
        "requested": {
            "columns":request.columns(),
            "buckets":request.buckets().iter().map(|bucket|json!({
                "values":bucket.values().iter().map(requested_scalar).collect::<Vec<_>>(),
                "rows":bucket.rows()
            })).collect::<Vec<_>>()
        },
        "evidence":evidence
    })
    .to_string()
}
