//! TASK-180 independent final-gate replay of all original TASK-196 DuckDB
//! lifecycle and DML source/terminal bags. Native syntax failure and metadata
//! authority limitations are not promoted into vendor-equivalent semantics.

use std::collections::BTreeSet;

use duckdb::Connection;
use serde_json::Value;

const WRITES: &str = include_str!("../docs/task-196-acceptance-freeze.json");

fn read() -> Value {
    serde_json::from_str(WRITES).expect("independent original SQL/native review freeze")
}

fn bag(db: &Connection, sql: &str, expected: &Value, id: &str) {
    let mut expected_rows: Vec<Vec<Option<i64>>> =
        serde_json::from_value(expected.clone()).expect("frozen full native integer/NULL bag");
    expected_rows.sort();
    let columns = expected_rows.first().map_or(1, Vec::len);
    assert!(expected_rows.iter().all(|row| row.len() == columns));
    let mut statement = db
        .prepare(sql)
        .unwrap_or_else(|error| panic!("{id}: original source/terminal SQL: {error}"));
    let actual = statement
        .query_map([], |row| {
            (0..columns)
                .map(|index| row.get::<usize, Option<i64>>(index))
                .collect::<Result<Vec<_>, _>>()
        })
        .unwrap_or_else(|error| panic!("{id}: native source/terminal rows: {error}"));
    let mut actual_rows: Vec<_> = actual
        .map(|row| row.expect("native complete physical row"))
        .collect();
    actual_rows.sort();
    assert_eq!(
        actual_rows, expected_rows,
        "{id}: whole native bag and NULL multiplicity"
    );
}

#[test]
fn original_schema_and_write_state_bags_replay_without_vendor_overclaim() {
    let frozen = read();
    let mut seen = BTreeSet::new();
    let mut dml = 0;
    let mut schema = 0;
    let mut constraint_negatives = 0;
    let mut returning = 0;
    let mut failed_vendor = 0;
    let mut metadata_only = 0;
    for case in frozen["cases"]
        .as_array()
        .expect("all 38 original SQL cases")
    {
        let id = case["id"].as_str().expect("stable original ID");
        assert!(seen.insert(id), "{id}: duplicated original DDL/DML variant");
        let sql = case["sql"].as_str().expect("unaltered original SQL");
        let oracle = &case["native_oracle"];
        match oracle["kind"]
            .as_str()
            .expect("independent native category")
        {
            "original_native_dml_complete_target_bag" => {
                let db = Connection::open_in_memory().expect("native isolated DML catalog");
                db.execute_batch(oracle["source_setup"].as_str().expect("source setup"))
                    .unwrap_or_else(|error| panic!("{id}: source population: {error}"));
                bag(
                    &db,
                    "SELECT id, amount FROM orders",
                    &oracle["complete_source_bags"]["orders"],
                    id,
                );
                let customers = &oracle["complete_source_bags"]["customers"];
                if !customers.is_null() {
                    bag(&db, "SELECT id, amount FROM customers", customers, id);
                }
                if oracle["expected_returning_bag"].is_null() {
                    db.execute_batch(sql)
                        .unwrap_or_else(|error| panic!("{id}: original DML execution: {error}"));
                } else {
                    bag(&db, sql, &oracle["expected_returning_bag"], id);
                    returning += 1;
                }
                bag(
                    &db,
                    "SELECT id, amount FROM orders",
                    &oracle["expected_poststate_bag"],
                    id,
                );
                if !customers.is_null() {
                    bag(&db, "SELECT id, amount FROM customers", customers, id);
                }
                dml += 1;
            }
            "original_native_schema_lifecycle_source_and_terminal" => {
                let db = Connection::open_in_memory().expect("native isolated schema catalog");
                let setup = oracle["source_setup"]
                    .as_str()
                    .expect("schema source setup");
                if !setup.is_empty() {
                    db.execute_batch(setup)
                        .unwrap_or_else(|error| panic!("{id}: native setup: {error}"));
                }
                bag(
                    &db,
                    oracle["before_query"].as_str().expect("prestate SQL"),
                    &oracle["before_bag"],
                    id,
                );
                db.execute_batch(sql)
                    .unwrap_or_else(|error| panic!("{id}: original schema DDL: {error}"));
                let followup = oracle["followup_sql"].as_str().expect("post DDL input");
                if !followup.is_empty() {
                    db.execute_batch(followup)
                        .unwrap_or_else(|error| panic!("{id}: reviewed followup: {error}"));
                }
                let terminal = oracle["terminal_query"].as_str().expect("terminal SQL");
                bag(&db, terminal, &oracle["terminal_bag"], id);
                if let Some(invalid) = oracle["must_reject_after"].as_str() {
                    assert!(
                        db.execute_batch(invalid).is_err(),
                        "{id}: nonconstructive constraint/lifecycle state was accepted"
                    );
                    bag(&db, terminal, &oracle["terminal_bag"], id);
                    constraint_negatives += 1;
                }
                schema += 1;
            }
            "unapproved_vendor_or_unsupported_dml_boundary" => {
                assert!(matches!(id, "88-C177-S199" | "88-C177-S200"));
                assert!(oracle["expected_poststate_bag"].is_null());
                let db = Connection::open_in_memory().expect("native rejection fixture");
                db.execute_batch(
                    "CREATE TABLE orders(id INTEGER, amount INTEGER);
                     INSERT INTO orders VALUES (1,10),(2,NULL);
                     CREATE TABLE customers(id INTEGER, amount INTEGER);
                     INSERT INTO customers VALUES (1,100);",
                )
                .expect("complete source states");
                let expected_orders = serde_json::json!([[1, 10], [2, null]]);
                let expected_customers = serde_json::json!([[1, 100]]);
                bag(&db, "SELECT id, amount FROM orders", &expected_orders, id);
                bag(
                    &db,
                    "SELECT id, amount FROM customers",
                    &expected_customers,
                    id,
                );
                assert!(
                    db.execute_batch(sql).is_err(),
                    "{id}: original vendor syntax is not certified by DuckDB"
                );
                bag(&db, "SELECT id, amount FROM orders", &expected_orders, id);
                bag(
                    &db,
                    "SELECT id, amount FROM customers",
                    &expected_customers,
                    id,
                );
                failed_vendor += 1;
            }
            "metadata_adapter_only_requires_authoritative_fixture" => {
                assert!(matches!(id, "88-C177-S191" | "88-C177-S193"));
                assert!(oracle["expected_poststate_bag"].is_null());
                assert_eq!(oracle["boundary_owner"], "TASK-197");
                metadata_only += 1;
            }
            other => panic!("{id}: unreviewed physical SQL category {other}"),
        }
    }
    assert_eq!(seen.len(), 38);
    assert_eq!((dml, schema, failed_vendor, metadata_only), (16, 18, 2, 2));
    assert_eq!(returning, 3);
    assert!(constraint_negatives >= 5);
}
