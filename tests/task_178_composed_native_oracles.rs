//! TASK-178 independent cross-operator source/terminal and mutation-bag oracles.
//!
//! These are concrete DuckDB execution witnesses, not an assertion that a
//! vendor dialect executes equivalent SQL. Parser-level normalization is
//! checked separately on all exposed dialects.

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{analyze_sql, dialect_from_name, to_json, ProtocolStatement};

const SOURCE_SETUP: &str = "
    CREATE TABLE orders(id INTEGER, amount INTEGER);
    INSERT INTO orders VALUES (1,10),(1,10),(2,5),(NULL,NULL);
    CREATE TABLE customers(id INTEGER);
    INSERT INTO customers VALUES (1),(1),(3),(NULL);
    CREATE TABLE archive(id INTEGER, amount INTEGER);
    INSERT INTO archive VALUES (-1,0);
";
const JOIN_SET: &str = "
    SELECT o.id FROM orders o
    JOIN customers c ON o.id = c.id WHERE o.amount >= 10
    UNION ALL
    SELECT id FROM orders WHERE amount IS NULL
";
const WINDOW_PAGE: &str = "
    SELECT rn FROM (
      SELECT ROW_NUMBER() OVER (ORDER BY id NULLS LAST) AS rn FROM orders
    ) ranked WHERE rn <= 2
";
const WRITE: &str = "
    INSERT INTO archive(id, amount)
    SELECT o.id, o.amount FROM orders o
    JOIN customers c ON o.id = c.id WHERE o.amount >= 10
";
const DIALECTS: &[&str] = &[
    "ansi", "bigquery", "clickhouse", "databricks", "duckdb", "generic",
    "hive", "mssql", "mysql", "postgresql", "redshift", "snowflake", "sqlite",
];

fn bag(db: &Connection, sql: &str, columns: usize) -> Vec<Vec<Option<i64>>> {
    let mut statement = db.prepare(sql).expect("independently executable DuckDB SQL");
    let rows = statement.query_map([], |row| {
        (0..columns).map(|i| row.get::<_, Option<i64>>(i)).collect::<Result<Vec<_>, _>>()
    }).expect("native terminal");
    let mut values: Vec<_> = rows.map(|row| row.expect("full physical row")).collect();
    values.sort();
    values
}

#[test]
fn join_union_preserves_duplicate_null_and_rejected_membership() {
    let db = Connection::open_in_memory().expect("DuckDB");
    db.execute_batch(SOURCE_SETUP).expect("physical source");
    assert_eq!(bag(&db, "SELECT id,amount FROM orders", 2),
        vec![vec![None,None], vec![Some(1),Some(10)], vec![Some(1),Some(10)], vec![Some(2),Some(5)]]);
    assert_eq!(bag(&db, "SELECT id FROM customers", 1),
        vec![vec![None], vec![Some(1)], vec![Some(1)], vec![Some(3)]]);
    assert_eq!(bag(&db, JOIN_SET, 1),
        vec![vec![None], vec![Some(1)], vec![Some(1)], vec![Some(1)], vec![Some(1)]],
        "two equal orders crossed with two equal customers plus a NULL branch");
    assert_eq!(
        bag(&db, "SELECT id FROM orders WHERE (amount >= 10) IS NOT TRUE", 1),
        vec![vec![None], vec![Some(2)]],
        "FALSE and UNKNOWN are both rejected without collapsing NULL");
    assert_eq!(
        bag(&db, "SELECT o.id FROM orders o JOIN customers c ON o.id = c.id WHERE o.amount < 10", 1),
        Vec::<Vec<Option<i64>>>::new(),
        "no independent witness may be fabricated from an absent joined key");
    assert_eq!(
        bag(&db, "SELECT id FROM orders EXCEPT SELECT id FROM customers", 1),
        vec![vec![Some(2)]],
        "set subtraction treats duplicate NULL tuples as equal");
}

#[test]
fn window_page_partitions_positive_and_rejected_physical_occurrences() {
    let db = Connection::open_in_memory().expect("DuckDB");
    db.execute_batch(SOURCE_SETUP).expect("physical source");
    assert_eq!(bag(&db, WINDOW_PAGE, 1), vec![vec![Some(1)], vec![Some(2)]]);
    assert_eq!(bag(&db, "
        SELECT rn FROM (
          SELECT ROW_NUMBER() OVER (ORDER BY id NULLS LAST) AS rn FROM orders
        ) ranked WHERE rn > 2
    ", 1), vec![vec![Some(3)], vec![Some(4)]]);
}

#[test]
fn joined_insert_mutates_only_target_and_retains_complete_source_bags() {
    let db = Connection::open_in_memory().expect("DuckDB");
    db.execute_batch(SOURCE_SETUP).expect("physical source");
    let original_orders = bag(&db, "SELECT id,amount FROM orders", 2);
    let original_customers = bag(&db, "SELECT id FROM customers", 1);
    assert_eq!(bag(&db, "SELECT id,amount FROM archive", 2), vec![vec![Some(-1),Some(0)]]);
    db.execute_batch(WRITE).expect("native joined insert");
    assert_eq!(bag(&db, "SELECT id,amount FROM archive", 2),
        vec![vec![Some(-1),Some(0)], vec![Some(1),Some(10)], vec![Some(1),Some(10)],
             vec![Some(1),Some(10)], vec![Some(1),Some(10)]],
        "the complete final state includes the existing row and four multiplicative inserts");
    assert_eq!(bag(&db, "SELECT id,amount FROM orders", 2), original_orders);
    assert_eq!(bag(&db, "SELECT id FROM customers", 1), original_customers);
}

#[test]
fn composed_portable_sql_keeps_full_public_protocol_across_13_parser_dialects() {
    for sql in [JOIN_SET, WINDOW_PAGE, WRITE] {
        let mut reference: Option<Value> = None;
        for name in DIALECTS {
            let dialect = dialect_from_name(name).expect("exposed parser");
            let protocol = analyze_sql(sql, name, dialect.as_ref())
                .unwrap_or_else(|err| panic!("{name}: portable composed SQL parsing failed: {err}"));
            assert!(
                protocol.statements().iter().all(|stmt| !matches!(stmt, ProtocolStatement::Unsupported(_))),
                "{name}: portable SQL cannot silently become an unsupported statement"
            );
            let mut next: Value = serde_json::from_str(&to_json(&protocol)).expect("complete protocol");
            for input in next["inputs"].as_array_mut().expect("input") {
                input["dialect"] = Value::Null;
            }
            match &reference {
                Some(expected) => assert_eq!(&next, expected, "{name}: composition changed the public semantic contract"),
                None => reference = Some(next),
            }
        }
    }
}
