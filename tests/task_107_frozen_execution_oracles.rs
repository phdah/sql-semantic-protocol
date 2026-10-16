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
                let selected = original;
                let mut stmt = db.prepare(selected).unwrap();
                let rows = stmt
                    .query_map([], |row| row.get::<_, Option<i64>>(0))
                    .unwrap();
                let expected_set: Option<BTreeSet<i64>> = domain["set"].as_array().map(|values| {
                    values
                        .iter()
                        .map(|value| value.as_str().unwrap().parse::<i64>().unwrap())
                        .collect()
                });
                let lower = domain["lower"].as_str().map(|s| s.parse::<i64>().unwrap());
                let upper = domain["upper"].as_str().map(|s| s.parse::<i64>().unwrap());
                let non_null = domain["null"] == false;
                for value in rows {
                    match value.unwrap() {
                        Some(value) => {
                            if let Some(set) = &expected_set {
                                assert!(
                                    set.contains(&value),
                                    "{id}: {value} outside frozen finite output domain"
                                );
                            }
                            if let Some(lower) = lower {
                                assert!(
                                    value > lower
                                        || (value == lower && domain["lower_inclusive"] == true),
                                    "{id}: {value} below frozen interval"
                                );
                            }
                            if let Some(upper) = upper {
                                assert!(
                                    value < upper
                                        || (value == upper && domain["upper_inclusive"] == true),
                                    "{id}: {value} above frozen interval"
                                );
                            }
                        }
                        None => {
                            assert!(
                                !non_null && domain["nullable"] == true,
                                "{id}: NULL outside certified non-nullable output domain"
                            );
                        }
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

fn complete_mandatory_source() -> Connection {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE t(id BIGINT PRIMARY KEY, a INTEGER, b INTEGER, s VARCHAR(8));
         INSERT INTO t VALUES
         (1, NULL, NULL, NULL),
         (2, 0, 1, 'aa'),
         (3, 1, 2, 'ab'),
         (4, 2, 2, 'ab_cd'),
         (5, 3, 0, 'abc'),
         (6, 4, 4, 'ab_%'),
         (7, -1, 1, 'zz'),
         (8, -2147483648, 2147483647, 'ab'),
         (9, 2147483647, -2147483648, 'abX')",
    )
    .unwrap();
    db
}

#[test]
fn all_mandatory_variants_match_independent_duckdb_closed_source_membership() {
    let freeze: Value = serde_json::from_str(FREEZE).unwrap();
    let db = complete_mandatory_source();
    // Hand-worked expectations, not computed from the protocol analyzer.
    // Every mandatory variant ID is listed exactly once, including output-only
    // shapes which must preserve the complete physical bag.
    let expected: [(&str, &[i64]); 28] = [
        ("E107-V001", &[3]),
        ("E107-V002", &[2, 4, 5, 6, 7, 8, 9]),
        ("E107-V003", &[2, 3, 7, 8]),
        ("E107-V004", &[2, 3, 4, 7, 8]),
        ("E107-V005", &[5, 6, 9]),
        ("E107-V006", &[4, 5, 6, 9]),
        ("E107-V007", &[3, 4, 5]),
        ("E107-V008", &[2, 6, 7, 8, 9]),
        ("E107-V009", &[4]),
        ("E107-V010", &[3, 5]),
        ("E107-V011", &[2, 4, 5, 6, 7, 8, 9]),
        ("E107-V012", &[3]),
        ("E107-V013", &[]),
        ("E107-V014", &[1]),
        ("E107-V015", &[2, 3, 4, 5, 6, 7, 8, 9]),
        ("E107-V016", &[3]),
        ("E107-V017", &[2, 4, 5, 6, 7, 8, 9]),
        ("E107-V018", &[1, 2, 3, 4, 5, 6, 7, 8, 9]),
        ("E107-V019", &[3]),
        ("E107-V020", &[3, 4, 5, 6, 8, 9]),
        ("E107-V022", &[4, 6]),
        ("E107-V025", &[3, 4, 5, 6, 8, 9]),
        ("E107-V027", &[3, 4, 5]),
        ("E107-V028", &[1, 2, 3, 4, 5, 6, 7, 8, 9]),
        ("E107-V029", &[1, 2, 3, 4, 5, 6, 7, 8, 9]),
        ("E107-V030", &[1, 2, 3, 4, 5, 6, 7, 8, 9]),
        ("E107-V031", &[1, 2, 3, 4, 5, 6, 7, 8, 9]),
        ("E107-V033", &[1, 2, 3, 4, 5, 6, 7, 8, 9]),
    ];
    let exact: Vec<_> = freeze["inventory_cells"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|cell| cell["classification"] == "exact_required")
        .collect();
    assert_eq!(exact.len(), expected.len());

    for (cell, (name, truth)) in exact.iter().zip(expected) {
        let id = cell["id"].as_str().unwrap();
        assert_eq!(
            id, name,
            "the reviewed independent oracle inventory changed"
        );
        let original = cell["sql_shape"].as_str().unwrap();
        let predicate = original.split_once(" WHERE ").map(|(_, p)| p);
        let sql = predicate.map_or_else(
            || "SELECT id FROM t ORDER BY id".to_owned(),
            |p| format!("SELECT id FROM t WHERE {p} ORDER BY id"),
        );
        let observed = ids_for(&db, &sql);
        assert_eq!(
            observed, truth,
            "{id}: DuckDB physical TRUE membership; SQL={sql}"
        );
        if let Some(predicate) = predicate {
            let rejected_sql =
                format!("SELECT id FROM t WHERE ({predicate}) IS NOT TRUE ORDER BY id");
            let rejected = ids_for(&db, &rejected_sql);
            let true_set: BTreeSet<_> = truth.iter().copied().collect();
            let not_true: Vec<_> = (1..=9).filter(|row| !true_set.contains(row)).collect();
            assert_eq!(
                rejected, not_true,
                "{id}: SQL NOT TRUE must retain FALSE and UNKNOWN; SQL={rejected_sql}"
            );
        }
    }
}

#[test]
fn frozen_output_projections_have_independent_null_failure_and_exact_value_oracles() {
    let freeze: Value = serde_json::from_str(FREEZE).unwrap();
    let db = complete_mandatory_source();
    // Complete ordered output bags, hand-calculated from the nine committed
    // physical source instances. A NULL output is distinct from an absent row.
    let cases: [(&str, &[Option<i64>]); 6] = [
        ("E107-V027", &[Some(3), Some(4), Some(5)]),
        ("E107-V028", &[None, Some(0), Some(1), Some(2), Some(3), Some(4), Some(-1), None, None]),
        ("E107-V029", &[Some(0), Some(0), Some(0), Some(2), Some(3), Some(4), Some(0), Some(0), Some(2147483647)]),
        ("E107-V030", &[Some(0), Some(0), Some(1), Some(2), Some(3), Some(4), Some(-1), Some(-2147483648), Some(2147483647)]),
        ("E107-V031", &[None, Some(0), None, Some(2), Some(3), Some(4), Some(-1), Some(-2147483648), Some(2147483647)]),
        ("E107-V033", &[None, Some(0), Some(3), Some(6), Some(9), Some(12), Some(-3), Some(-6442450944), Some(6442450941)]),
    ];
    for (id, expected) in cases {
        let fixture = freeze["inventory_cells"].as_array().unwrap()
            .iter().find(|cell| cell["id"] == id).expect("frozen output variant");
        assert_eq!(fixture["classification"], "exact_required");
        let sql = format!("{} ORDER BY id", fixture["sql_shape"].as_str().unwrap());
        let mut stmt = db.prepare(&sql).unwrap_or_else(|error| panic!("{id}: {sql}: {error}"));
        let results = stmt
            .query_map([], |row| row.get::<_, Option<i64>>(0))
            .unwrap_or_else(|error| panic!("{id}: {sql}: {error}"))
            .map(|value| value.expect("typed output row"))
            .collect::<Vec<_>>();
        assert_eq!(
            results, expected,
            "{id}: executed output bag does not match independently worked type/null/overflow boundaries; SQL={sql}"
        );
    }
}
