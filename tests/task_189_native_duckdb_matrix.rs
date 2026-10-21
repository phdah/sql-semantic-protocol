//! TASK-189: executable DuckDB conditional SQL/session inventory.
//! A native engine's result does not establish cross-vendor portability;
//! the canonical physical planner must remain Residual for these cases.

use duckdb::Connection;
use serde_json::Value;
use std::collections::BTreeMap;

fn normalize(text: &str) -> String {
    match text.split_once('.') {
        Some((whole, fraction)) => {
            let fraction = fraction.trim_end_matches('0');
            if fraction.is_empty() {
                whole.to_string()
            } else {
                format!("{whole}.{fraction}")
            }
        }
        None => text.to_string(),
    }
}

#[test]
fn all_six_conditional_native_duckdb_oracles_have_complete_source_and_terminal_bags() {
    let freeze: Value =
        serde_json::from_str(include_str!("../docs/task-189-acceptance-freeze.json")).unwrap();
    let mut count = 0;
    for case in freeze["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|case| case["proof"] == "Residual_expected")
    {
        count += 1;
        let id = case["id"].as_str().unwrap();
        let db = Connection::open_in_memory().unwrap();
        let decimal = id == "74-C189-C6";
        let datatype = if decimal { "DECIMAL(10,2)" } else { "INTEGER" };
        db.execute_batch(&format!(
            "CREATE TABLE s(id INTEGER, k INTEGER, x {datatype});"
        ))
        .unwrap();
        for row in case["source_bag"].as_array().unwrap() {
            let value = if row["x"].is_null() {
                "NULL".to_string()
            } else {
                row["x"].to_string()
            };
            db.execute_batch(&format!(
                "INSERT INTO s VALUES ({}, {}, {value});",
                row["id"], row["k"],
            ))
            .unwrap();
        }
        let mut source = db
            .prepare("SELECT id,k,CAST(x AS VARCHAR) FROM s ORDER BY id")
            .unwrap();
        let full = source
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let expected_source = case["source_bag"].as_array().unwrap();
        assert_eq!(
            full.len(),
            expected_source.len(),
            "{id}: complete native source"
        );
        for (actual, expected) in full.iter().zip(expected_source) {
            assert_eq!(
                actual.0,
                expected["id"].as_i64().unwrap(),
                "{id}: source id"
            );
            assert_eq!(
                actual.1,
                expected["k"].as_i64().unwrap(),
                "{id}: source partition"
            );
            let target = expected["x"].as_f64().map(|v| normalize(&v.to_string()));
            let actual = actual.2.as_deref().map(normalize);
            assert_eq!(actual, target, "{id}: source numeric value");
        }

        let column = match id {
            "74-C189-C1" | "74-C189-C2" => "n",
            "74-C189-C5" => "mean",
            _ => "total",
        };
        let statement = format!(
            "SELECT id, CAST({column} AS VARCHAR) FROM ({}) AS evaluated ORDER BY id",
            case["sql"].as_str().unwrap()
        );
        let mut query = db.prepare(&statement).unwrap();
        let actual = query
            .query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?))
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(
            actual.len(),
            expected_source.len(),
            "{id}: complete native terminal"
        );
        for (index, pair) in actual.iter().enumerate() {
            assert_eq!(pair.0, expected_source[index]["id"].as_i64().unwrap());
        }
        if let Some(expected) = case["native_duckdb_terminal_values_in_id_order"].as_array() {
            let got = actual
                .iter()
                .map(|(_, value)| value.as_deref().map(normalize))
                .collect::<Vec<_>>();
            let wanted = expected
                .iter()
                .map(|value| value.as_str().map(normalize))
                .collect::<Vec<_>>();
            assert_eq!(got, wanted, "{id}: numeric native oracle");
        } else if id == "74-C189-C4" {
            let mut values = BTreeMap::new();
            for (id, value) in &actual {
                values.insert(*id, value.as_deref().map(normalize));
            }
            for (id, value) in [(1, Some("10")), (2, Some("30")), (3, Some("40")), (6, None)] {
                assert_eq!(values.get(&id).and_then(|v| v.as_deref()), value);
            }
            let mut peers = [4, 5]
                .iter()
                .map(|id| values.get(id).unwrap().clone())
                .collect::<Vec<_>>();
            peers.sort();
            assert_eq!(peers, [Some("10".to_string()), Some("5".to_string())]);
        } else if id == "74-C189-C5" {
            for (id, value) in &actual {
                if *id <= 3 {
                    let number = value.as_deref().unwrap().parse::<f64>().unwrap();
                    assert!(
                        (number - (5.0 / 3.0)).abs() < 1.0e-12,
                        "{id}: native floating AVG must be approximate"
                    );
                } else if *id <= 5 {
                    assert_eq!(value.as_deref().map(normalize), Some("5".into()));
                } else {
                    assert!(value.is_none(), "all-NULL partition AVG is NULL");
                }
            }
        } else {
            panic!("{id}: missing native oracle disposition");
        }
    }
    assert_eq!(count, 6);
}
