//! TASK-196 independently frozen physical occurrence and three-valued DML laws.
//!
//! Native DuckDB witnesses use the original source insertion-order rowids.
//! They never reconstruct expected TRUE/FALSE/UNKNOWN from protocol output.

use std::collections::BTreeSet;

use duckdb::Connection;
use serde_json::Value;

const OCCURRENCES: &str = include_str!("../docs/task-196-original-write-occurrences.json");
const FREEZE: &str = include_str!("../docs/task-196-acceptance-freeze.json");

fn read(source: &str) -> Value {
    serde_json::from_str(source).expect("versioned independent task evidence")
}

fn ordinals(db: &Connection, sql: &str) -> Vec<usize> {
    let mut query = db.prepare(sql).expect("independent native physical identity query");
    query
        .query_map([], |row| row.get::<_, i64>(0))
        .expect("physical rowid result")
        .map(|row| usize::try_from(row.expect("nonnegative DuckDB rowid")).expect("ordinal"))
        .collect()
}

fn expected(value: &Value) -> Vec<usize> {
    value
        .as_array()
        .expect("explicit physical occurrence ids")
        .iter()
        .map(|entry| usize::try_from(entry.as_u64().expect("nonnegative ordinal")).expect("ordinal"))
        .collect()
}

#[test]
fn original_dml_true_false_unknown_and_source_partner_occurrences_match_independent_literals() {
    let ledger = read(OCCURRENCES);
    let freeze = read(FREEZE);
    assert_eq!(ledger["schema_version"], 1);
    assert_eq!(ledger["owner"], "TASK-196");
    assert_eq!(ledger["counts"]["original_dml_cases"], 16);
    assert_eq!(ledger["counts"]["selected_mutation_cases"], 11);
    assert_eq!(ledger["external_vendor_execution_attested"], false);

    let mut seen = BTreeSet::new();
    let mut selections = 0;
    for case in ledger["cases"].as_array().expect("16 explicit DML laws") {
        let id = case["id"].as_str().expect("stable original id");
        assert!(seen.insert(id));
        let frozen = freeze["cases"]
            .as_array()
            .expect("original 38 source cases")
            .iter()
            .find(|original| original["id"] == id)
            .expect("source SQL cannot be replaced");
        let oracle = &frozen["native_oracle"];
        assert_eq!(oracle["kind"], "original_native_dml_complete_target_bag");
        let db = Connection::open_in_memory().expect("native read-only source witness");
        db.execute_batch(oracle["source_setup"].as_str().expect("complete original source"))
            .unwrap_or_else(|error| panic!("{id}: native source fixture {error}"));
        let source = &case["independent_law"];
        let count = if source["all_customer_occurrences_before"] == 0 {
            0
        } else {
            ordinals(&db, "SELECT rowid FROM customers ORDER BY rowid").len()
        };
        assert_eq!(count as u64, source["all_customer_occurrences_before"].as_u64().expect("customer count"), "{id}");
        assert_eq!(
            ordinals(&db, "SELECT rowid FROM orders ORDER BY rowid").len(),
            source["all_target_occurrences_before"].as_u64().expect("source bag count") as usize
        );
        if let Some(predicate) = source["target_predicate"].as_str() {
            selections += 1;
            let selected = ordinals(
                &db,
                &format!(
                    "SELECT rowid FROM orders WHERE ({predicate}) IS TRUE ORDER BY rowid"
                ),
            );
            let rejected = ordinals(
                &db,
                &format!(
                    "SELECT rowid FROM orders WHERE ({predicate}) IS FALSE ORDER BY rowid"
                ),
            );
            let unknown = ordinals(
                &db,
                &format!(
                    "SELECT rowid FROM orders WHERE ({predicate}) IS NULL ORDER BY rowid"
                ),
            );
            assert_eq!(selected, expected(&source["target_true_ordinals"]), "{id}: TRUE");
            assert_eq!(rejected, expected(&source["target_false_ordinals"]), "{id}: FALSE");
            assert_eq!(unknown, expected(&source["target_unknown_ordinals"]), "{id}: UNKNOWN");
            let mut partition = [selected, rejected, unknown].concat();
            partition.sort_unstable();
            assert_eq!(
                partition,
                (0..source["all_target_occurrences_before"].as_u64().expect("row count") as usize)
                    .collect::<Vec<_>>(),
                "{id}: every physical row must be TRUE, FALSE or UNKNOWN exactly once"
            );
        } else {
            assert!(source["target_true_ordinals"].as_array().expect("no target selection").is_empty());
            assert!(source["target_false_ordinals"].as_array().expect("no target selection").is_empty());
            assert!(source["target_unknown_ordinals"].as_array().expect("no target selection").is_empty());
        }
        if source["all_customer_occurrences_before"] != 0 {
            let matched = ordinals(
                &db,
                "SELECT c.rowid FROM customers c                  WHERE c.id IN (SELECT o.id FROM orders o)                  ORDER BY c.rowid",
            );
            let expected_matched = expected(&source["matched_customer_ordinals"]);
            if !expected_matched.is_empty() {
                assert_eq!(matched, expected_matched, "{id}: source partner rowids");
            }
            if !source["inserted_customer_ordinals"]
                .as_array()
                .expect("inserted customer occurrences")
                .is_empty()
            {
                let unmatched = if case["kind"] == "insert_select" {
                    ordinals(&db, "SELECT rowid FROM customers ORDER BY rowid")
                } else {
                    ordinals(
                        &db,
                        "SELECT c.rowid FROM customers c WHERE c.id NOT IN \
                         (SELECT o.id FROM orders o WHERE o.id IS NOT NULL) ORDER BY c.rowid",
                    )
                };
                assert_eq!(
                    unmatched,
                    expected(&source["inserted_customer_ordinals"]),
                    "{id}: original source-only rows remain distinct even when duplicate"
                );
            }
        }
        let before_count = source["all_target_occurrences_before"].as_i64().expect("before count");
        let after_count = oracle["expected_poststate_bag"]
            .as_array()
            .expect("complete native poststate")
            .len() as i64;
        let inserted = source["inserted"].as_i64().expect("inserted");
        let deleted = source["deleted"].as_i64().expect("deleted");
        assert_eq!(
            after_count,
            before_count + inserted - deleted,
            "{id}: source-derived cardinality must match complete native poststate"
        );
        assert!(
            source["updated"].as_u64().expect("updated count")
                <= source["target_true_ordinals"].as_array().expect("matched rows").len() as u64,
            "{id}: updated occurrences cannot exceed original TRUE target rows"
        );
    }
    assert_eq!(seen.len(), 16);
    assert_eq!(selections, 11);
}
