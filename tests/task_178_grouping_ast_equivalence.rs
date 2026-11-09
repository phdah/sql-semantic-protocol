//! Red/green TASK-178 grouping AST drift: GROUP BY ROLLUP and CUBE
//! must not vary the physical empty-input proof solely with parser dialect.
//! TASK-177 historical observed drift remains separately frozen.

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{analyze_sql, dialect_from_name, to_json};

fn canonical(sql: &str, name: &str) -> Value {
    let dialect = dialect_from_name(name).expect("named dialect");
    let analyzed = analyze_sql(sql, name, dialect.as_ref()).expect("SQL parse");
    let mut json: Value = serde_json::from_str(&to_json(&analyzed)).unwrap();
    for input in json["inputs"].as_array_mut().expect("inputs") {
        input["dialect"] = Value::Null;
    }
    json
}

#[test]
fn rollup_cube_and_grouping_have_one_portable_empty_source_proof() {
    for sql in [
        "SELECT id, COUNT(*) FROM orders GROUP BY ROLLUP(id)",
        "SELECT id, COUNT(*) FROM orders GROUP BY CUBE(id)",
        "SELECT GROUPING(id), COUNT(*) FROM orders GROUP BY ROLLUP(id)",
    ] {
        let ansi = canonical(sql, "ansi");
        let dialects = [
            "ansi", "bigquery", "clickhouse", "databricks", "duckdb",
            "generic", "hive", "mssql", "mysql", "postgresql",
            "redshift", "snowflake", "sqlite",
        ];
        for name in dialects {
            let actual = canonical(sql, name);
            assert_eq!(
                actual, ansi,
                "TASK-178: same ROLLUP/CUBE SQL must emit identical complete public semantic document: {name}"
            );
            assert_eq!(
                actual["graph"]["physical_source_plans"][0]["zero_output"]["status"],
                "residual",
                "{name}: a subtotal can exist from an empty physical source, so zero-output cannot be guaranteed"
            );
        }
    }
}


#[test]
fn duckdb_empty_rollup_and_cube_produce_grand_total_so_zero_source_cannot_prove_zero_output() {
    let db = Connection::open_in_memory().expect("DuckDB");
    db.execute_batch("CREATE TABLE orders (id INT, amount INT)")
        .expect("controlled empty physical source");
    let rows = |sql: &str| {
        let mut statement = db.prepare(sql).expect("native grouping SQL");
        let mut bag: Vec<(Option<i64>, i64)> = statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .expect("native grouped output")
            .map(|item| item.expect("typed physical row"))
            .collect();
        bag.sort();
        bag
    };
    assert_eq!(
        rows("SELECT id, COUNT(*) FROM orders GROUP BY ROLLUP(id)"),
        vec![(None, 0)],
        "ROLLUP still has the empty grouping set on zero physical rows"
    );
    assert_eq!(
        rows("SELECT id, COUNT(*) FROM orders GROUP BY CUBE(id)"),
        vec![(None, 0)],
        "CUBE still has the empty grouping set on zero physical rows"
    );
    assert_eq!(
        rows("SELECT GROUPING(id), COUNT(*) FROM orders GROUP BY ROLLUP(id)"),
        vec![(Some(1), 0)],
        "GROUPING distinguishes grand total from a real NULL key"
    );
    assert_eq!(
        rows("SELECT id, COUNT(*) FROM orders GROUP BY ROLLUP(id) HAVING COUNT(*) > 0"),
        Vec::<(Option<i64>, i64)>::new(),
        "HAVING rejects the grand total of the empty source"
    );
}
