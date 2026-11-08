//! TASK-177 bounded native DuckDB oracles for five cross-dialect canonical
//! document divergence cases. Physical outputs are first-party independent
//! goldens; dialect equivalence remains blocked until canonical explanations
//! are reviewed, even when one engine executes the SQL successfully.

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{analyze_sql, dialect_from_name, to_json, ProtocolStatement};

const SAMPLES: &str = include_str!("../docs/task-177-parser-samples.json");
const DRIFTS: &str = include_str!("../docs/task-177-canonical-drift-boundaries.json");

fn original_sql<'a>(samples: &'a Value, id: &str) -> &'a str {
    samples["cases"]
        .as_array()
        .expect("pinned SQL cases")
        .iter()
        .find(|case| case["id"] == id)
        .expect("stable TASK-177 original sample")["sql"]
        .as_str()
        .expect("sample SQL text")
}

fn source_bag(db: &Connection) -> Vec<(Option<i64>, i64)> {
    let mut stmt = db
        .prepare("SELECT id, amount FROM orders")
        .expect("source SQL");
    let result = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .expect("source query");
    let mut bag: Vec<_> = result.map(|row| row.expect("typed physical row")).collect();
    bag.sort();
    bag
}

fn grouped_bag(db: &Connection, sql: &str) -> Vec<(Option<i64>, i64)> {
    let mut stmt = db.prepare(sql).expect("pinned native grouped SQL");
    let result = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .expect("native grouped output");
    let mut bag: Vec<_> = result.map(|row| row.expect("grouped output row")).collect();
    bag.sort();
    bag
}

fn lateral_bag(db: &Connection, sql: &str) -> Vec<Option<i64>> {
    let mut stmt = db.prepare(sql).expect("pinned native LATERAL SQL");
    let result = stmt
        .query_map([], |row| row.get(0))
        .expect("native LATERAL output");
    let mut bag: Vec<_> = result.map(|row| row.expect("lateral identity")).collect();
    bag.sort();
    bag
}

#[test]
fn native_grouping_lateral_and_struct_cases_have_complete_source_terminal_bag_oracles() {
    let samples: Value = serde_json::from_str(SAMPLES).expect("TASK-66 samples");
    let drifts: Value = serde_json::from_str(DRIFTS).expect("drift owner boundaries");
    assert_eq!(drifts["drift_cell_count"], 23);
    assert_eq!(drifts["affected_case_count"], 5);

    let db = Connection::open_in_memory().expect("native DuckDB");
    db.execute_batch(
        "CREATE TABLE orders (id INTEGER, amount INTEGER);
         INSERT INTO orders VALUES (1,10),(1,20),(2,30),(NULL,40);",
    )
    .expect("complete input source bag");

    assert_eq!(
        source_bag(&db),
        vec![(None, 40), (Some(1), 10), (Some(1), 20), (Some(2), 30)],
        "fixed source has NULL, duplicate group key, and full integer column values"
    );

    let rollup = vec![(None, 1), (None, 4), (Some(1), 2), (Some(2), 1)];
    for case_id in ["88-C177-S100", "88-C177-S101"] {
        let sql = original_sql(&samples, case_id);
        assert_eq!(
            grouped_bag(&db, sql),
            rollup,
            "{case_id}: complete ROLLUP/CUBE bag"
        );
    }

    assert_eq!(
        grouped_bag(&db, original_sql(&samples, "88-C177-S104")),
        vec![(Some(0), 1), (Some(0), 1), (Some(0), 2), (Some(1), 4)],
        "GROUPING(id) distinguishes subtotal from a real NULL group"
    );

    assert_eq!(
        lateral_bag(&db, original_sql(&samples, "88-C177-S152")),
        vec![None, Some(1), Some(1), Some(2)],
        "LATERAL must preserve outer row identities and duplicate occurrences"
    );

    let struct_sql = original_sql(&samples, "88-C177-S179");
    let expanded = format!("SELECT v.a, v.b FROM ({struct_sql}) derived");
    let mut stmt = db.prepare(&expanded).expect("native STRUCT projection");
    let mut result = stmt.query([]).expect("native STRUCT row");
    let row = result
        .next()
        .expect("STRUCT fetch")
        .expect("one STRUCT value");
    assert_eq!(row.get::<_, i64>(0).expect("first STRUCT field"), 1);
    assert_eq!(row.get::<_, String>(1).expect("second STRUCT field"), "a");
    assert!(result.next().expect("exhausted STRUCT result").is_none());

    for id in [
        "88-C177-S100",
        "88-C177-S101",
        "88-C177-S104",
        "88-C177-S152",
        "88-C177-S179",
    ] {
        let dialect = dialect_from_name("duckdb").expect("public DuckDB dialect");
        let protocol = analyze_sql(original_sql(&samples, id), "duckdb", dialect.as_ref())
            .unwrap_or_else(|error| panic!("{id}: public protocol must parse: {error}"));
        assert!(
            protocol
                .statements()
                .iter()
                .all(|statement| matches!(statement, ProtocolStatement::Query(_))),
            "{id}: expected explicit query semantics, not a dropped statement"
        );
        let json: Value =
            serde_json::from_str(&to_json(&protocol)).expect("same canonical protocol contract");
        let columns = json["inputs"][0]["statements"][0]["output"]["columns"]
            .as_array()
            .expect("each query retains complete output columns");
        assert!(
            !columns.is_empty(),
            "{id}: query output must not be omitted"
        );
        assert!(
            columns.iter().all(|column| column["domain"].is_object()),
            "{id}: uncertain domains must be represented, not silently omitted"
        );
        assert!(
            drifts["cells"]
                .as_array()
                .expect("named drift owners")
                .iter()
                .any(|cell| cell["case_id"] == id),
            "{id}: native execution must not auto-promote cross-dialect differences"
        );
    }
}
