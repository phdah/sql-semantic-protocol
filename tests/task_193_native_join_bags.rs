//! TASK-193 whole-bag native oracles over the same closed physical source
//! instances. Original SQL/terminal expectations are separately frozen in
//! TASK-177; the TASK-193 occurrence-level assertions are independent.
//! These are DuckDB executions, not vendor-runtime certifications.

use std::collections::BTreeSet;

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{analyze_sql, dialect_from_name, to_json, ProtocolStatement};

const FREEZE: &str = include_str!("../docs/task-193-acceptance-freeze.json");
const SOURCE_PAIRS: &str = include_str!("../docs/task-193-source-instance-oracles.json");

fn parsed(text: &str) -> Value {
    serde_json::from_str(text).expect("committed TASK-193 finite acceptance fixture")
}

fn integer_bag(db: &Connection, sql: &str, columns: usize) -> Vec<Vec<Option<i64>>> {
    let mut statement = db.prepare(sql).expect("certified native DuckDB SQL");
    let rows = statement
        .query_map([], |row| {
            (0..columns)
                .map(|column| row.get::<usize, Option<i64>>(column))
                .collect::<Result<Vec<_>, _>>()
        })
        .expect("complete native rowset");
    let mut result: Vec<_> = rows.map(|row| row.expect("native integer/NULL row")).collect();
    result.sort();
    result
}

fn expected_bag(value: &Value) -> Vec<Vec<Option<i64>>> {
    let mut result: Vec<Vec<Option<i64>>> =
        serde_json::from_value(value.clone()).expect("frozen complete integer/NULL bag");
    result.sort();
    result
}

fn new_db(freeze: &Value) -> Connection {
    let db = Connection::open_in_memory().expect("DuckDB in-memory source");
    db.execute_batch(
        freeze["source_setup"].as_str().expect("exact physical source setup"),
    )
    .expect("closed source population");
    db
}

#[test]
fn all_24_original_join_and_lateral_terminal_bags_match_native_oracles() {
    let freeze = parsed(FREEZE);
    let db = new_db(&freeze);
    let dialect = dialect_from_name("duckdb").expect("project dialect");
    let cases = freeze["cases"].as_array().expect("24 fixed SQL cases");
    assert_eq!(cases.len(), 24);
    let mut seen = BTreeSet::new();
    for case in cases {
        let id = case["id"].as_str().expect("stable original case");
        assert!(seen.insert(id), "{id}: duplicated join SQL variant");
        let sql = case["sql"].as_str().expect("unaltered TASK-66 SQL");
        if case["original_duckdb_syntax_boundary"].as_str().is_some_and(|name| {
            name.contains("rejected_by_duckdb_parser")
        }) {
            assert!(
                db.prepare(sql).is_err(),
                "{id}: the original native LEFT SEMI/ANTI spelling is not executable"
            );
        }
        let native = case["native_duckdb_sql"]
            .as_str()
            .expect("original SQL or explicitly documented native equivalent");
        let columns = case["expected_columns"].as_u64().expect("projection arity") as usize;
        assert_eq!(
            integer_bag(&db, native, columns),
            expected_bag(&case["expected_terminal_bag"]),
            "{id}: complete native output, NULLs and duplicate multiplicities"
        );
        let protocol = analyze_sql(sql, "duckdb", dialect.as_ref())
            .unwrap_or_else(|error| panic!("{id}: failed to analyze original SQL: {error}"));
        assert!(
            protocol
                .statements()
                .iter()
                .all(|statement| !matches!(statement, ProtocolStatement::Unsupported(_))),
            "{id}: registered parser-accepted SQL cannot silently disappear"
        );
        let wire = parsed(&to_json(&protocol));
        let actual_columns = wire["inputs"][0]["statements"][0]["output"]["columns"]
            .as_array()
            .expect("explicit public output columns");
        assert_eq!(actual_columns.len(), columns, "{id}: projected arity");
        for column in actual_columns {
            assert!(
                column["domain"].is_object(),
                "{id}: every projected column requires an explicit domain, even if unbounded"
            );
        }
    }
    assert_eq!(seen.len(), 24);
}

#[test]
fn source_instances_prove_equal_matches_and_complete_false_or_unknown_complement() {
    let freeze = parsed(FREEZE);
    let oracles = parsed(SOURCE_PAIRS);
    let db = new_db(&freeze);
    assert_eq!(oracles["case_count"], 11);
    assert_eq!(oracles["all_paired_input_occurrences"], 30);
    assert_eq!(oracles["exact_equal_pairs"], 5);
    assert_eq!(oracles["not_true_pairs"], 25);
    assert_eq!(
        integer_bag(&db, "SELECT id, amount FROM orders", 2),
        expected_bag(&freeze["complete_source_bags"]["orders"]),
        "source multiplicities include two identical orders and an SQL NULL"
    );
    assert_eq!(
        integer_bag(&db, "SELECT id, amount FROM customers", 2),
        expected_bag(&freeze["complete_source_bags"]["customers"]),
        "partner source preserves duplicates and physical NULL"
    );
    let mut seen = BTreeSet::new();
    for case in oracles["cases"].as_array().expect("eleven independent rowid oracles") {
        let id = case["id"].as_str().expect("stable occurrence fixture ID");
        assert!(seen.insert(id), "{id}: repeated occurrence proof");
        let sql = case["sql"].as_str().expect("native SQL over original tables");
        let expected = expected_bag(&case["expected_rows"]);
        let arity = case["expected_arity"].as_u64().expect("expected arity") as usize;
        assert_eq!(
            expected.len(),
            case["expected_count"].as_u64().expect("literal row count") as usize,
            "{id}: inconsistent frozen count"
        );
        assert_eq!(
            integer_bag(&db, sql, arity),
            expected,
            "{id}: each original physical occurrence (not a deduplicated value) is preserved"
        );
    }
    assert_eq!(seen.len(), 11);
    let matched = oracles["cases"]
        .as_array()
        .expect("cases")
        .iter()
        .find(|case| case["id"] == "88-C193-P01")
        .expect("matched pairs");
    let nonmatched = oracles["cases"]
        .as_array()
        .expect("cases")
        .iter()
        .find(|case| case["id"] == "88-C193-N01")
        .expect("NOT TRUE complement");
    let true_pairs: BTreeSet<_> = expected_bag(&matched["expected_rows"]).into_iter().collect();
    let not_true_pairs: BTreeSet<_> =
        expected_bag(&nonmatched["expected_rows"]).into_iter().collect();
    assert!(true_pairs.is_disjoint(&not_true_pairs));
    assert_eq!(true_pairs.len() + not_true_pairs.len(), 30);
}

#[test]
fn physical_null_partner_and_preserved_sql_null_are_distinct() {
    let freeze = parsed(FREEZE);
    let oracles = parsed(SOURCE_PAIRS);
    let db = new_db(&freeze);
    let left = oracles["cases"]
        .as_array()
        .expect("cases")
        .iter()
        .find(|case| case["id"] == "88-C193-P02")
        .expect("LEFT source-instance pairs");
    let rows = integer_bag(
        &db,
        left["sql"].as_str().expect("original LEFT join SQL"),
        2,
    );
    assert_eq!(rows, expected_bag(&left["expected_rows"]));
    let unmatched: Vec<_> = rows.iter().filter(|row| row[1].is_none()).collect();
    assert_eq!(unmatched.len(), 3, "three preserved orders have no partner");
    assert_eq!(
        unmatched.iter().filter(|row| row[0] == Some(4)).count(),
        1,
        "physical source rowid 4 has SQL NULL key but an existing source row"
    );
}
