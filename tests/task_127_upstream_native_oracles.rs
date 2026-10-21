//! TASK-127: independent executable DuckDB upstream JOIN/GROUP oracles.
//! These assertions certify the SQL truth table, not the unfinished canonical
//! physical-producer composition proof for these two mandatory cases.
use duckdb::Connection;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

fn freeze() -> Value {
    serde_json::from_str(include_str!("../docs/task-127-acceptance-freeze.json")).unwrap()
}

fn cell(case: &Value) -> BTreeMap<String, Value> {
    case.as_object()
        .unwrap()
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

fn seed(case: &Value) -> Connection {
    let db = Connection::open_in_memory().unwrap();
    for (relation, schema) in case["source_schema"].as_object().unwrap() {
        let columns = schema.as_object().unwrap();
        let definition = columns
            .iter()
            .map(|(name, kind)| format!("{name} {}", kind.as_str().unwrap()))
            .collect::<Vec<_>>()
            .join(", ");
        db.execute_batch(&format!("CREATE TABLE {relation}({definition});"))
            .unwrap();
        for row in case["physical_source_bags"][relation].as_array().unwrap() {
            let values = columns
                .keys()
                .map(|column| row[column].to_string())
                .collect::<Vec<_>>()
                .join(", ");
            db.execute_batch(&format!("INSERT INTO {relation} VALUES ({values});"))
                .unwrap();
        }
        let actual = rows(&db, &format!("SELECT * FROM {relation}"));
        let expected = case["physical_source_bags"][relation]
            .as_array()
            .unwrap()
            .iter()
            .map(cell)
            .collect::<Vec<_>>();
        assert_eq!(bag(&actual), bag(&expected), "{relation}: full source bag");
    }
    db
}

fn rows(db: &Connection, sql: &str) -> Vec<BTreeMap<String, Value>> {
    let mut stmt = db.prepare(sql).unwrap();
    stmt.query_map([], |row| {
        let mut values = BTreeMap::new();
        for index in 0..row.as_ref().column_count() {
            let key = row.as_ref().column_name(index)?.to_owned();
            let val: Option<i64> = row.get(index)?;
            values.insert(key, val.map_or(Value::Null, Value::from));
        }
        Ok(values)
    })
    .unwrap()
    .collect::<Result<Vec<_>, _>>()
    .unwrap()
}

fn bag(rows: &[BTreeMap<String, Value>]) -> Vec<String> {
    let mut values = rows.iter().map(|row| format!("{row:?}")).collect::<Vec<_>>();
    values.sort();
    values
}

fn check_full_terminal(db: &Connection, case: &Value) {
    let actual = rows(db, case["sql"].as_str().unwrap());
    let expected = case["expected_full_terminal_bag"]
        .as_array()
        .unwrap()
        .iter()
        .map(cell)
        .collect::<Vec<_>>();
    assert_eq!(bag(&actual), bag(&expected), "{}", case["id"]);
    for (name, domain) in case["typed_terminal_domains"].as_object().unwrap() {
        let values = actual
            .iter()
            .map(|row| row[name].as_i64().unwrap())
            .collect::<Vec<_>>();
        let bounds = domain["inclusive_bounds"].as_array().unwrap();
        assert_eq!(values.iter().min().copied(), bounds[0].as_i64(), "{name}");
        assert_eq!(values.iter().max().copied(), bounds[1].as_i64(), "{name}");
    }
}

#[test]
fn frozen_join_has_exact_source_pair_survivors_and_qualify_rejections() {
    let matrix = freeze();
    let case = matrix["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["id"] == "74-C127-P4")
        .unwrap();
    let db = seed(case);
    check_full_terminal(&db, case);
    let joined = db
        .prepare(
            "SELECT l.rowid, r.rowid, ROW_NUMBER() OVER (
                PARTITION BY l.k ORDER BY r.x DESC NULLS LAST,
                l.id ASC NULLS LAST) AS ordinal
             FROM l JOIN r ON l.k = r.k ORDER BY l.rowid, r.rowid",
        )
        .unwrap()
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(joined, vec![(0, 0, 1), (0, 1, 2), (1, 2, 1)]);
    let selected = joined
        .iter()
        .filter(|(_, _, ordinal)| *ordinal == 1)
        .map(|(left, right, _)| format!("l#{left}|r#{right}"))
        .collect::<Vec<_>>();
    let rejected_qualify = joined
        .iter()
        .filter(|(_, _, ordinal)| *ordinal != 1)
        .map(|(left, right, _)| format!("l#{left}|r#{right}"))
        .collect::<Vec<_>>();
    assert_eq!(selected, ["l#0|r#0", "l#1|r#2"]);
    assert_eq!(rejected_qualify, ["l#0|r#1"]);
    let joined_pairs = joined
        .iter()
        .map(|(left, right, _)| (*left, *right))
        .collect::<BTreeSet<_>>();
    let rejected_join = (0..2)
        .flat_map(|left| (0..3).map(move |right| (left, right)))
        .filter(|pair| !joined_pairs.contains(pair))
        .collect::<Vec<_>>();
    assert_eq!(rejected_join, [(0, 2), (1, 0), (1, 1)]);
}

#[test]
fn frozen_group_has_source_membership_and_ranked_group_rejection() {
    let matrix = freeze();
    let case = matrix["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["id"] == "74-C127-P5")
        .unwrap();
    let db = seed(case);
    check_full_terminal(&db, case);
    let grouped = rows(
        &db,
        "SELECT k, COUNT(*) AS total FROM s GROUP BY k ORDER BY k",
    );
    assert_eq!(
        bag(&grouped),
        bag(&[
            BTreeMap::from([
                ("k".to_string(), Value::from(0)),
                ("total".to_string(), Value::from(3)),
            ]),
            BTreeMap::from([
                ("k".to_string(), Value::from(1)),
                ("total".to_string(), Value::from(3)),
            ]),
        ]),
    );
    let membership = db
        .prepare("SELECT k, rowid FROM s ORDER BY k, rowid")
        .unwrap()
        .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(
        membership,
        [(0, 0), (0, 1), (0, 2), (1, 3), (1, 4), (1, 5)]
    );
    let ranked = rows(
        &db,
        "SELECT k, total, ROW_NUMBER() OVER (
            ORDER BY total DESC NULLS LAST, k ASC NULLS LAST
        ) AS ordinal
        FROM (SELECT k, COUNT(*) AS total FROM s GROUP BY k) g",
    );
    let by_key = ranked
        .iter()
        .map(|row| (row["k"].as_i64().unwrap(), row["ordinal"].as_i64().unwrap()))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(by_key, BTreeMap::from([(0, 1), (1, 2)]));
}
