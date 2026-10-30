//! Independent complete DuckDB bag and physical identity oracles for TASK-141.
//! These do not certify the canonical protocol physical evaluator, which is a
//! separate acceptance requirement.

use duckdb::{params, Connection};
use serde_json::Value;

fn source_rows(db: &Connection, table: &str) -> Vec<(String, Option<i64>)> {
    let sql = format!("SELECT id, k FROM {table} ORDER BY id");
    let mut statement = db.prepare(&sql).unwrap();
    statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<duckdb::Result<Vec<_>>>()
        .unwrap()
}

fn contributor_ids(db: &Connection, case_id: &str) -> Vec<Vec<String>> {
    let sql = match case_id {
        "78-C141-P-REUSE" | "78-C141-N-NULL-DUP" => {
            "WITH shared AS (SELECT id, k FROM s) \
             SELECT a.id, b.id FROM shared a \
             JOIN shared b ON a.k = b.k ORDER BY a.id, b.id"
        }
        "78-C141-P-SHADOW" => {
            "WITH shared AS (SELECT id, k FROM outer_src), \
             chosen AS (WITH shared AS (SELECT id, k FROM inner_src) \
             SELECT id, k FROM shared) SELECT id FROM chosen ORDER BY id"
        }
        "78-C141-P-CHAIN" => {
            "WITH first AS (SELECT id, k FROM s), \
             second AS (SELECT id, k FROM first) \
             SELECT id FROM second ORDER BY id"
        }
        _ => panic!("unknown mandatory TASK-141 case: {case_id}"),
    };
    let arity = if case_id.ends_with("REUSE") || case_id.ends_with("NULL-DUP") {
        2
    } else {
        1
    };
    let mut stmt = db.prepare(sql).unwrap();
    stmt.query_map([], |row| {
        (0..arity)
            .map(|index| row.get::<_, String>(index))
            .collect::<duckdb::Result<Vec<_>>>()
    })
    .unwrap()
    .collect::<duckdb::Result<Vec<_>>>()
    .unwrap()
}

#[test]
fn every_mandatory_cte_freeze_case_matches_independent_complete_duckdb_bags() {
    let freeze: Value =
        serde_json::from_str(include_str!("../docs/task-141-acceptance-freeze.json")).unwrap();
    let cases = freeze["cases"].as_array().unwrap();
    let mut verified = Vec::new();
    for case in cases {
        if case["classification"] != "mandatory_exact" {
            continue;
        }
        let id = case["id"].as_str().unwrap();
        let db = Connection::open_in_memory().unwrap();
        for (table, rows) in case["initial_bags"].as_object().unwrap() {
            assert!(
                table.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'),
                "{id}: untrusted SQL identifier in freeze"
            );
            db.execute_batch(&format!("CREATE TABLE {table} (id VARCHAR, k BIGINT)"))
                .unwrap();
            let mut expected = Vec::new();
            for row in rows.as_array().unwrap() {
                let row_id = row["id"].as_str().unwrap();
                let key = row["k"].as_i64();
                db.execute(
                    &format!("INSERT INTO {table} VALUES (?1, ?2)"),
                    params![row_id, key],
                )
                .unwrap();
                expected.push((row_id.to_string(), key));
            }
            expected.sort();
            assert_eq!(source_rows(&db, table), expected, "{id}: source {table}");
        }

        let columns = case["terminal_domains"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>();
        let mut stmt = db.prepare(case["sql"].as_str().unwrap()).unwrap();
        let mut actual = stmt
            .query_map([], |row| {
                (0..columns.len())
                    .map(|index| row.get::<_, Option<i64>>(index))
                    .collect::<duckdb::Result<Vec<_>>>()
            })
            .unwrap()
            .collect::<duckdb::Result<Vec<_>>>()
            .unwrap();
        let mut expected = case["terminal_bag"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| columns.iter().map(|col| entry[*col].as_i64()).collect::<Vec<_>>())
            .collect::<Vec<_>>();
        actual.sort();
        expected.sort();
        assert_eq!(actual, expected, "{id}: full terminal bag, including multiplicity");

        for (index, name) in columns.iter().enumerate() {
            let domain = &case["terminal_domains"][*name];
            let non_null = actual
                .iter()
                .filter_map(|row| row[index])
                .collect::<Vec<_>>();
            assert_eq!(
                actual.iter().any(|row| row[index].is_none()),
                domain["nullable"].as_bool().unwrap(),
                "{id}: {name} NULL domain"
            );
            assert_eq!(
                non_null.iter().min().copied(),
                domain["lower"]["value"].as_i64(),
                "{id}: {name} inclusive lower bound"
            );
            assert_eq!(
                non_null.iter().max().copied(),
                domain["upper"]["value"].as_i64(),
                "{id}: {name} inclusive upper bound"
            );
            assert_eq!(domain["lower"]["inclusive"], true, "{id}: {name}");
            assert_eq!(domain["upper"]["inclusive"], true, "{id}: {name}");
        }

        let mut observed_ids = contributor_ids(&db, id);
        observed_ids.sort();
        let mut expected_ids = case["terminal_bag"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| {
                row["source_ids"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|value| value.as_str().unwrap().to_string())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        expected_ids.sort();
        assert_eq!(observed_ids, expected_ids, "{id}: complete contributor identities");
        verified.push(id);
    }
    verified.sort();
    assert_eq!(
        verified,
        [
            "78-C141-N-NULL-DUP",
            "78-C141-P-CHAIN",
            "78-C141-P-REUSE",
            "78-C141-P-SHADOW",
        ]
    );
}
