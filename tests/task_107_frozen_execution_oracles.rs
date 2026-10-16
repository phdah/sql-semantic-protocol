//! TASK-107 closed-world DuckDB source and terminal execution witnesses.
//!
//! Unlike a coverage registry, these tests actually execute each frozen SQL
//! case against exactly the committed physical input bag. Both TRUE and
//! NOT TRUE include distinct row IDs, so SQL UNKNOWN cannot be discarded.
//! Fixture failures retain the minimal SQL and physical source in the panic.

use std::collections::BTreeSet;

use duckdb::Connection;
use serde_json::Value;

const FREEZE: &str = include_str!("../docs/task-107-acceptance-freeze.json");

fn ids_for(db: &Connection, sql: &str) -> Vec<i64> {
    let mut statement = db
        .prepare(sql)
        .unwrap_or_else(|error| panic!("{sql}: prepare failed: {error}"));
    let rows = statement
        .query_map([], |row| row.get::<_, i64>(0))
        .unwrap_or_else(|error| panic!("{sql}: execution failed: {error}"));
    rows.map(|row| row.expect("source row id"))
        .collect::<Vec<_>>()
}

fn expected_ids(fixture: &Value, key: &str) -> Vec<i64> {
    fixture[key]
        .as_array()
        .unwrap_or_else(|| panic!("{} missing {key}", fixture["id"]))
        .iter()
        .map(|id| id.as_i64().expect("stable integer physical row id"))
        .collect()
}

fn literal(value: &Value) -> String {
    if value.is_null() {
        "NULL".to_string()
    } else if let Some(s) = value.as_str() {
        format!("'{}'", s.replace('\'', "''"))
    } else {
        value.to_string()
    }
}

#[test]
fn frozen_golden_sources_match_duckdb_true_not_true_and_unknown_membership() {
    let freeze: Value = serde_json::from_str(FREEZE).expect("freeze JSON");
    let goldens = freeze["golden_cases"].as_array().expect("golden fixtures");
    assert_eq!(goldens.len(), 9);

    for fixture in goldens {
        let id = fixture["id"].as_str().expect("fixture ID");
        let db = Connection::open_in_memory().expect("in-memory DuckDB");
        db.execute_batch(
            "CREATE TABLE t (id BIGINT PRIMARY KEY, a INTEGER, b INTEGER, s VARCHAR(8))",
        )
        .unwrap();
        let string_case = fixture["source_type"].as_str() == Some("VARCHAR(8)");
        let source = fixture["source_rows"]
            .as_array()
            .expect("closed source rows");
        let mut inserts = Vec::new();
        for row in source {
            let values = row.as_array().expect("source row values");
            let insert = if string_case {
                assert_eq!(values.len(), 2, "{id}: text source shape");
                format!(
                    "INSERT INTO t (id, s) VALUES ({}, {})",
                    values[0],
                    literal(&values[1])
                )
            } else if values.len() == 3 {
                format!(
                    "INSERT INTO t (id, a, b) VALUES ({}, {}, {})",
                    values[0],
                    literal(&values[1]),
                    literal(&values[2])
                )
            } else {
                assert_eq!(values.len(), 2, "{id}: integer source shape");
                format!(
                    "INSERT INTO t (id, a) VALUES ({}, {})",
                    values[0],
                    literal(&values[1])
                )
            };
            db.execute_batch(&insert)
                .unwrap_or_else(|error| panic!("{id}: {insert}: {error}"));
            inserts.push(insert);
        }
        let original = fixture["sql"].as_str().expect("frozen SQL");
        let predicate = original
            .split_once(" WHERE ")
            .unwrap_or_else(|| panic!("{id}: expected one terminal WHERE: {original}"))
            .1;
        let true_sql = format!("SELECT id FROM t WHERE {predicate} ORDER BY id");
        let rejected_sql = format!("SELECT id FROM t WHERE ({predicate}) IS NOT TRUE ORDER BY id");
        let unknown_sql = format!("SELECT id FROM t WHERE ({predicate}) IS UNKNOWN ORDER BY id");
        let true_ids = ids_for(&db, &true_sql);
        let rejected_ids = ids_for(&db, &rejected_sql);
        let unknown_ids = ids_for(&db, &unknown_sql);

        assert_eq!(
            true_ids,
            expected_ids(fixture, "true_row_ids"),
            "{id}: TRUE mismatch; SQL={true_sql}; source={inserts:?}"
        );
        assert_eq!(
            rejected_ids,
            expected_ids(fixture, "not_true_row_ids"),
            "{id}: NOT TRUE must include UNKNOWN; SQL={rejected_sql}; source={inserts:?}"
        );
        let true_set: BTreeSet<_> = true_ids.iter().copied().collect();
        let rejected_set: BTreeSet<_> = rejected_ids.iter().copied().collect();
        let input_set: BTreeSet<_> = ids_for(&db, "SELECT id FROM t ORDER BY id")
            .into_iter()
            .collect();
        assert!(
            true_set.is_disjoint(&rejected_set),
            "{id}: cannot qualify and reject"
        );
        assert_eq!(
            true_set
                .union(&rejected_set)
                .copied()
                .collect::<BTreeSet<_>>(),
            input_set,
            "{id}: incomplete physical source obligation"
        );
        assert!(
            unknown_ids
                .iter()
                .all(|row_id| rejected_set.contains(row_id)),
            "{id}: UNKNOWN must be rejected under SQL WHERE"
        );
        let domain = &fixture["expected_output_domain"];
        match domain["type"].as_str().expect("typed output expectation") {
            "empty" => assert!(true_ids.is_empty(), "{id}: false impossibility"),
            "integer" => {
                let bounds = (domain["lower"].as_str(), domain["upper"].as_str());
                if let (Some(lower), Some(upper)) = bounds {
                    let lower: i64 = lower.parse().unwrap();
                    let upper: i64 = upper.parse().unwrap();
                    let selected = format!("SELECT a FROM t WHERE {predicate}");
                    let mut stmt = db.prepare(&selected).unwrap();
                    let rows = stmt
                        .query_map([], |row| row.get::<_, Option<i64>>(0))
                        .unwrap();
                    for value in rows {
                        let value = value.unwrap().expect("bounded exact integer not NULL");
                        assert!(
                            value >= lower && value <= upper,
                            "{id}: {value} outside inclusive domain"
                        );
                    }
                }
            }
            "text" => {
                let prefix = domain["prefix"].as_str().expect("binary LIKE prefix");
                let selected = format!("SELECT s FROM t WHERE {predicate}");
                let mut stmt = db.prepare(&selected).unwrap();
                let rows = stmt
                    .query_map([], |row| row.get::<_, Option<String>>(0))
                    .unwrap();
                for value in rows {
                    assert!(value.unwrap().expect("selected text").starts_with(prefix));
                }
            }
            other => panic!("{id}: unexpected output domain kind {other}"),
        }
    }
}
