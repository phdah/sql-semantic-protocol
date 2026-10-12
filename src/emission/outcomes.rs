//! Goal and outcome witness serialization.

use super::*;

pub(super) fn outcome_witness_to_value(witness: &crate::outcome_proofs::OutcomeWitness) -> Value {
    use crate::outcome_proofs::OutcomeWitness;
    match witness {
        OutcomeWitness::Singleton => json!({ "kind": "singleton" }),
        OutcomeWitness::EmptySources { relations } => {
            json!({ "kind": "empty_sources", "relations": relations })
        }
        OutcomeWitness::SourceRows {
            relation,
            rows,
            columns,
        } => json!({
            "kind": "source_rows",
            "relation": relation,
            "rows": rows,
            "columns": columns.iter().map(|column| json!({
                "column": column.column(),
                "values": column.values().iter().map(|entry| json!({
                    "value": constraint_value_to_value(entry.value()), "rows": entry.rows()
                })).collect::<Vec<_>>()
            })).collect::<Vec<_>>()
        }),
        OutcomeWitness::JoinPairs { left, right, pairs } => json!({
            "kind": "join_pairs",
            "left": { "relation": left.relation(), "column": left.column(), "relation_instance": left.relation_instance() },
            "right": { "relation": right.relation(), "column": right.column(), "relation_instance": right.relation_instance() },
            "pairs": pairs
        }),
        OutcomeWitness::Groups {
            relation,
            key,
            groups,
            rows_per_group,
        } => json!({
            "kind": "groups", "relation": relation,
            "key": column_ref_to_value(key), "groups": groups,
            "rows_per_group": rows_per_group
        }),
        OutcomeWitness::Ranked {
            relation,
            partition_key,
            order_by,
            rows,
        } => json!({
            "kind": "ranked", "relation": relation,
            "partition_key": partition_key.as_ref().map(column_ref_to_value),
            "order_by": order_by.iter().map(|key| json!({
                "column": column_ref_to_value(key.column()),
                "ascending": key.ascending(),
                "nulls_first": key.nulls_first()
            })).collect::<Vec<_>>(),
            "rows": rows
        }),
        OutcomeWitness::SetTuples {
            tuples,
            case,
            values,
            scale_by_value_rows,
        } => json!({
            "kind": "set_tuples", "tuples": tuples,
            "scale_by_value_rows": scale_by_value_rows,
            "case": set_witness_case_to_value(case),
            "values": values.iter().map(|item| json!({
                "value": constraint_value_to_value(item.value()),
                "rows": item.rows()
            })).collect::<Vec<_>>()
        }),
    }
}
