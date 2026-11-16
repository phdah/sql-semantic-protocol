//! TASK-196 source-anchored native state transitions for original SQL.
//!
//! Replays immutable TASK-178 statements, complete source bags, poststates,
//! RETURNING, constraint-rejection and DDL lifetime evidence in DuckDB.
//! This certifies a DuckDB fixture, not an external vendor or a full typed
//! protocol-effect proof.

use std::collections::BTreeSet;

use duckdb::Connection;
use serde_json::Value;

const FREEZE: &str = include_str!("../docs/task-196-acceptance-freeze.json");

fn fixture() -> Value {
    serde_json::from_str(FREEZE).expect("versioned original TASK-196 SQL and native bags")
}

fn rows(db: &Connection, sql: &str, width: usize) -> Vec<Vec<Option<i64>>> {
    let mut statement = db.prepare(sql).expect("native original-state query");
    let mapped = statement
        .query_map([], |row| {
            (0..width)
                .map(|index| row.get::<usize, Option<i64>>(index))
                .collect::<Result<Vec<_>, _>>()
        })
        .expect("native integer/NULL source or terminal");
    let mut result: Vec<_> = mapped
        .map(|row| row.expect("physical integer or NULL occurrence"))
        .collect();
    result.sort();
    result
}

fn literal_bag(value: &Value) -> Vec<Vec<Option<i64>>> {
    let mut bag: Vec<Vec<Option<i64>>> =
        serde_json::from_value(value.clone()).expect("pinned independent literal bag");
    bag.sort();
    bag
}

fn check_bag(db: &Connection, sql: &str, expected: &Value, context: &str) {
    let bag = literal_bag(expected);
    let width = bag.first().map_or(1, Vec::len);
    assert!(bag.iter().all(|row| row.len() == width));
    assert_eq!(rows(db, sql, width), bag, "{context}: complete physical bag");
}

#[test]
fn all_16_original_dml_writes_preserve_complete_physical_before_after_and_returning_bags() {
    let freeze = fixture();
    let mut checked = BTreeSet::new();
    let mut returning = 0;
    let mut pk = 0;
    for case in freeze["cases"].as_array().expect("38 original SQL cases") {
        let oracle = &case["native_oracle"];
        if oracle["kind"] != "original_native_dml_complete_target_bag" {
            continue;
        }
        let id = case["id"].as_str().expect("original ID");
        assert!(checked.insert(id));
        let db = Connection::open_in_memory().expect("isolated native catalog");
        db.execute_batch(oracle["source_setup"].as_str().expect("original source SQL"))
            .unwrap_or_else(|error| panic!("{id}: initial source fixture: {error}"));

        check_bag(
            &db,
            "SELECT id, amount FROM orders",
            &oracle["complete_source_bags"]["orders"],
            &format!("{id}: prestate orders"),
        );
        let customers = &oracle["complete_source_bags"]["customers"];
        if customers.is_null() {
            pk += 1;
        } else {
            check_bag(
                &db,
                "SELECT id, amount FROM customers",
                customers,
                &format!("{id}: prestate customers"),
            );
        }

        let sql = case["sql"].as_str().expect("unaltered original DML SQL");
        if oracle["expected_returning_bag"].is_null() {
            db.execute_batch(sql)
                .unwrap_or_else(|error| panic!("{id}: original native DML: {error}"));
        } else {
            check_bag(
                &db,
                sql,
                &oracle["expected_returning_bag"],
                &format!("{id}: all RETURNING row occurrences"),
            );
            returning += 1;
        }
        check_bag(
            &db,
            "SELECT id, amount FROM orders",
            &oracle["expected_poststate_bag"],
            &format!("{id}: complete target poststate"),
        );
        if !customers.is_null() {
            check_bag(
                &db,
                "SELECT id, amount FROM customers",
                customers,
                &format!("{id}: unrelated source remains unmodified"),
            );
        }
    }
    assert_eq!(checked.len(), 16);
    assert_eq!(returning, 3, "INSERT/UPDATE/DELETE original RETURNING cases");
    assert_eq!(pk, 1, "original ON CONFLICT has its own PK source catalog");
}

#[test]
fn all_18_original_schema_and_ddl_programs_preserve_catalog_prestate_poststate_and_rejections() {
    let freeze = fixture();
    let mut checked = BTreeSet::new();
    let mut rejected = 0;
    for case in freeze["cases"].as_array().expect("original cases") {
        let oracle = &case["native_oracle"];
        if oracle["kind"] != "original_native_schema_lifecycle_source_and_terminal" {
            continue;
        }
        let id = case["id"].as_str().expect("original fixture ID");
        assert!(checked.insert(id));
        let db = Connection::open_in_memory().expect("independent schema catalog");
        let setup = oracle["source_setup"].as_str().expect("original source setup");
        if !setup.is_empty() {
            db.execute_batch(setup)
                .unwrap_or_else(|error| panic!("{id}: source setup failed: {error}"));
        }
        check_bag(
            &db,
            oracle["before_query"].as_str().expect("catalog/source prestate query"),
            &oracle["before_bag"],
            &format!("{id}: native catalog prestate"),
        );

        db.execute_batch(case["sql"].as_str().expect("original DDL"))
            .unwrap_or_else(|error| panic!("{id}: original DDL must run: {error}"));
        let followup = oracle["followup_sql"].as_str().expect("explicit follow-up SQL");
        if !followup.is_empty() {
            db.execute_batch(followup)
                .unwrap_or_else(|error| panic!("{id}: follow-up state mutation: {error}"));
        }

        let query = oracle["terminal_query"]
            .as_str()
            .expect("complete state or catalog observation");
        check_bag(
            &db,
            query,
            &oracle["terminal_bag"],
            &format!("{id}: native catalog poststate"),
        );
        if let Some(invalid) = oracle["must_reject_after"].as_str() {
            assert!(
                db.execute_batch(invalid).is_err(),
                "{id}: schema/constraint violation must not become a successful terminal"
            );
            check_bag(
                &db,
                query,
                &oracle["terminal_bag"],
                &format!("{id}: rejected SQL must leave proven poststate unchanged"),
            );
            rejected += 1;
        }
    }
    assert_eq!(checked.len(), 18);
    assert!(rejected >= 5, "constraint, relation-drop and rename negatives");
}

#[test]
fn vendor_specific_original_dml_rejections_cannot_forge_a_successful_duckdb_poststate() {
    let freeze = fixture();
    let original_setup =
        "CREATE TABLE orders(id INTEGER, amount INTEGER);          INSERT INTO orders VALUES (1,10),(2,NULL);          CREATE TABLE customers(id INTEGER, amount INTEGER);          INSERT INTO customers VALUES (1,100)";
    let expected_orders = serde_json::json!([[1, 10], [2, null]]);
    let expected_customers = serde_json::json!([[1, 100]]);
    let mut checked = BTreeSet::new();
    for case in freeze["cases"].as_array().expect("original variants") {
        let id = case["id"].as_str().expect("stable ID");
        if !matches!(id, "88-C177-S199" | "88-C177-S200") {
            continue;
        }
        assert!(checked.insert(id));
        let oracle = &case["native_oracle"];
        assert_eq!(oracle["kind"], "unapproved_vendor_or_unsupported_dml_boundary");
        assert!(oracle["expected_poststate_bag"].is_null());
        let db = Connection::open_in_memory().expect("native negative catalog");
        db.execute_batch(original_setup).expect("independent source setup");
        check_bag(&db, "SELECT id, amount FROM orders", &expected_orders, id);
        check_bag(
            &db,
            "SELECT id, amount FROM customers",
            &expected_customers,
            id,
        );
        assert!(
            db.execute_batch(case["sql"].as_str().expect("original vendor DML")).is_err(),
            "{id}: a DuckDB syntax rejection cannot become a successful output"
        );
        check_bag(
            &db,
            "SELECT id, amount FROM orders",
            &expected_orders,
            &format!("{id}: rejected statement cannot mutate target"),
        );
        check_bag(
            &db,
            "SELECT id, amount FROM customers",
            &expected_customers,
            &format!("{id}: rejected statement cannot mutate source"),
        );
    }
    assert_eq!(checked.len(), 2);
}
