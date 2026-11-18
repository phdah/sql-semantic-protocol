//! TASK-179: original SQL WHERE truth is a whole-physical-source
//! partition, with SQL UNKNOWN inside NOT TRUE, not simple FALSE.

use std::collections::BTreeSet;

use duckdb::Connection;
use serde_json::Value;

const FROZEN: &str = include_str!("../docs/task-179-source-and-outcome-evidence-v4.json");
const PRIOR: &str = include_str!("../docs/task-177-predicate-oracles.json");
const ENV: &str = include_str!("../docs/task-177-predicate-environment-oracles.json");

fn read(text: &str) -> Value {
    serde_json::from_str(text).expect("committed oracle")
}

fn source_ordinals(db: &Connection, sql: &str) -> BTreeSet<i64> {
    let mut stmt = db.prepare(sql).expect("original native predicate");
    stmt.query_map([], |row| row.get::<_, i64>(0))
        .expect("DuckDB 3VL source rows")
        .map(|result| result.expect("physical rowid"))
        .collect()
}

#[test]
fn every_original_executable_predicate_replays_true_and_not_true_on_same_source() {
    let matrix = read(FROZEN);
    let prior = read(PRIOR);
    let environment = read(ENV);
    let mut tested = 0;
    let mut pinned = 0;
    for case in matrix["cases"]
        .as_array()
        .expect("37 SQL boundary variants")
    {
        let sql = case["sql"].as_str().expect("original SQL");
        let Some((_, predicate)) = sql.split_once(" WHERE ") else {
            continue;
        };
        let id = case["id"].as_str().expect("stable identity");
        let relation = case["source_relation"]
            .as_str()
            .expect("original input relation");
        let db = Connection::open_in_memory().expect("DuckDB");
        db.execute_batch(case["source_setup"].as_str().expect("physical source"))
            .unwrap_or_else(|error| panic!("{id}: native setup: {error}"));
        let positive = source_ordinals(
            &db,
            &format!("SELECT rowid FROM {relation} WHERE {predicate}"),
        );
        let not_true = source_ordinals(
            &db,
            &format!("SELECT rowid FROM {relation} WHERE ({predicate}) IS NOT TRUE"),
        );
        let false_rows = source_ordinals(
            &db,
            &format!("SELECT rowid FROM {relation} WHERE ({predicate}) IS FALSE"),
        );
        let unknown_rows = source_ordinals(
            &db,
            &format!("SELECT rowid FROM {relation} WHERE ({predicate}) IS NULL"),
        );
        assert!(
            positive.is_disjoint(&not_true),
            "{id}: TRUE and NOT TRUE cannot share a physical row"
        );
        assert!(
            false_rows.is_disjoint(&unknown_rows),
            "{id}: FALSE and UNKNOWN are distinct physical source occurrences"
        );
        assert_eq!(
            false_rows
                .union(&unknown_rows)
                .copied()
                .collect::<BTreeSet<_>>(),
            not_true,
            "{id}: NOT TRUE must include both FALSE and UNKNOWN"
        );
        let all = source_ordinals(&db, &format!("SELECT rowid FROM {relation}"));
        assert_eq!(
            positive.union(&not_true).copied().collect::<BTreeSet<_>>(),
            all,
            "{id}: 3VL partitions must preserve the entire NULL and duplicate source bag"
        );
        assert_eq!(
            all.len(),
            case["source_bag"]
                .as_array()
                .expect("all native input rows")
                .len(),
            "{id}: source row count drift"
        );
        let truth = &case["negative_truth_evidence"];
        for (key, actual) in [
            ("rejected_false_ordinals", &false_rows),
            ("rejected_unknown_ordinals", &unknown_rows),
        ] {
            if let Some(ordinals) = truth[key].as_array() {
                let expected = ordinals
                    .iter()
                    .map(|ordinal| ordinal.as_i64().expect("frozen physical ordinal"))
                    .collect::<BTreeSet<_>>();
                assert_eq!(*actual, expected, "{id}: independently pinned {key}");
            }
        }
        if let Some(ordinals) = truth["selected_source_ordinals"].as_array() {
            let expected = ordinals
                .iter()
                .map(|id| id.as_i64().expect("frozen source ordinal"))
                .collect::<BTreeSet<_>>();
            assert_eq!(positive, expected, "{id}: native TRUE direction");
            let expected = truth["not_true_source_ordinals"]
                .as_array()
                .expect("frozen native 3VL complement")
                .iter()
                .map(|id| id.as_i64().expect("native source ordinal"))
                .collect::<BTreeSet<_>>();
            assert_eq!(not_true, expected, "{id}: native NOT TRUE direction");
            pinned += 1;
        }
        if let Some(ids) = truth["selected_ids"].as_array() {
            let reference = prior["cases"]
                .as_array()
                .expect("independent numeric truth")
                .iter()
                .find(|entry| entry["id"] == case["source_id"])
                .expect("frozen numeric original");
            assert_eq!(
                ids,
                reference["expected_selected_ids"]
                    .as_array()
                    .expect("native selected IDs")
            );
            let expected = ids
                .iter()
                .map(|value| value.as_i64().expect("integer key"))
                .collect::<BTreeSet<_>>();
            let actual = positive
                .iter()
                .map(|ordinal| {
                    case["source_bag"][*ordinal as usize][0]
                        .as_i64()
                        .expect("source key")
                })
                .collect::<BTreeSet<_>>();
            assert_eq!(actual, expected, "{id}: pinned positive physical rows");
            pinned += 1;
        }
        if let Some(env_case) = environment["customer_cases"]
            .as_array()
            .expect("string fixtures")
            .iter()
            .chain(
                environment["temporal_cases"]
                    .as_array()
                    .expect("timestamp fixtures"),
            )
            .find(|entry| entry["id"] == case["source_id"])
        {
            let emitted = positive
                .iter()
                .map(|ordinal| vec![case["source_bag"][*ordinal as usize][0].clone()])
                .collect::<Vec<_>>();
            let mut reference = env_case["expected_terminal_bag"]
                .as_array()
                .expect("independent native terminal")
                .clone();
            let mut emitted = emitted.into_iter().map(Value::Array).collect::<Vec<_>>();
            emitted.sort_by_key(Value::to_string);
            reference.sort_by_key(Value::to_string);
            assert_eq!(
                emitted, reference,
                "{id}: native selected source rows must match frozen full terminal bag"
            );
        }
        tested += 1;
    }
    assert_eq!(
        tested, 20,
        "must replay every conditional original WHERE variant, including DML"
    );
    assert!(
        pinned >= 10,
        "3VL fixture must pin exact positive and rejected source identities"
    );
}
