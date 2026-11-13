//! TASK-192: independent physical instance truth partitions for the remaining
//! executable original predicate cases, plus evaluated-error and shape controls.
//! A complete pinned DuckDB source/terminal bag never attests external vendors.

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{analyze_sql, dialect_from_name, to_json};

const FREEZE: &str = include_str!("../docs/task-192-native-partitions.json");
const SAMPLES: &str = include_str!("../docs/task-177-parser-samples.json");
const ENV: &str = include_str!("../docs/task-177-predicate-environment-oracles.json");
const FINAL: &str = include_str!("../docs/task-177-final-native-deterministic.json");
const ORDERING: &str = include_str!("../docs/task-177-native-expansion.json");

fn read(source: &str) -> Value {
    serde_json::from_str(source).expect("versioned independent evidence")
}

fn original<'a>(samples: &'a Value, id: &str) -> &'a str {
    samples["cases"]
        .as_array()
        .expect("original sample inventory")
        .iter()
        .find(|case| case["id"] == id)
        .expect("original SQL case")["sql"]
        .as_str()
        .expect("original SQL")
}

fn integers(db: &Connection, sql: &str) -> Vec<i64> {
    let mut stmt = db.prepare(sql).expect("physical row identity query");
    let iter = stmt
        .query_map([], |row| row.get::<_, i64>(0))
        .expect("source identity rows");
    let mut values: Vec<_> = iter.map(|row| row.expect("rowid")).collect();
    values.sort();
    values
}

fn bag(db: &Connection, sql: &str) -> Vec<Option<i64>> {
    let mut stmt = db.prepare(sql).expect("original native read");
    let rows = stmt
        .query_map([], |row| row.get::<_, Option<i64>>(0))
        .expect("native terminal");
    let mut values: Vec<_> = rows.map(|row| row.expect("terminal row")).collect();
    values.sort();
    values
}

fn frozen_ids(case: &Value, key: &str) -> Vec<i64> {
    serde_json::from_value(case[key].clone()).expect("frozen rowid set")
}

fn frozen_nullable_bag(expected: &Value) -> Vec<Option<i64>> {
    // The source contains extra string/timestamp/float columns; this test
    // compares the complete source-instance ID bag, not those heterogeneous values.
    let mut values: Vec<_> = expected
        .as_array()
        .expect("complete source or terminal bag")
        .iter()
        .map(|row| row[0].as_i64())
        .collect();
    values.sort();
    values
}

fn prior_expected_terminal(id: &str, env: &Value, final_native: &Value) -> Value {
    let candidates = [
        env["customer_cases"]
            .as_array()
            .expect("customer terminals"),
        env["temporal_cases"]
            .as_array()
            .expect("temporal terminals"),
        env["floating_cases"].as_array().expect("float terminals"),
        final_native["original_integer_selects"]
            .as_array()
            .expect("extra terminals"),
    ];
    candidates
        .into_iter()
        .flatten()
        .find(|case| case["id"] == id)
        .expect("original independent DuckDB terminal")["expected_terminal_bag"]
        .clone()
}

#[test]
fn all_eleven_additional_predicates_have_a_complete_rowid_partition_and_native_terminal() {
    let freeze = read(FREEZE);
    let samples = read(SAMPLES);
    let env = read(ENV);
    let final_native = read(FINAL);
    assert_eq!(freeze["schema_version"], 1);
    assert_eq!(freeze["task_id"], "TASK-192");
    assert_eq!(freeze["case_count"], 14);
    assert_eq!(freeze["physical_partition_case_count"], 11);

    let groups = &freeze["source_groups"];
    let mut visited = std::collections::BTreeSet::new();
    for case in freeze["predicates"].as_array().expect("eleven predicates") {
        let id = case["id"].as_str().expect("stable original case");
        assert!(visited.insert(id), "{id}: repeated native claim");
        let group_name = case["group"].as_str().expect("physical source family");
        let group = &groups[group_name];
        let data = if group_name == "nan" {
            &final_native
        } else {
            &env
        };
        let setup = data[group["setup"].as_str().expect("source setup key")]
            .as_str()
            .expect("complete pinned native input");
        let relation = group["relation"].as_str().expect("source identity");
        let db = Connection::open_in_memory().expect("DuckDB");
        db.execute_batch(setup).expect("fully populated input");

        let expected_count = group["row_count"].as_u64().expect("row count") as i64;
        let expected_all: Vec<_> = (0..expected_count).collect();
        assert_eq!(
            integers(&db, &format!("SELECT rowid FROM {relation}")),
            expected_all,
            "{id}: physical source instance identities"
        );
        let source_bag = &data[group["bag"].as_str().expect("bag key")];
        let want = frozen_nullable_bag(source_bag);
        assert_eq!(
            bag(&db, &format!("SELECT id FROM {relation}")),
            want,
            "{id}: complete source IDs, including repeated/nullable values"
        );

        let sql = original(&samples, id);
        let predicate = case["predicate"]
            .as_str()
            .expect("pinned original predicate");
        assert!(
            sql.ends_with(&format!(" WHERE {predicate}")),
            "{id}: predicate must come from the original source, never a substituted SQL form"
        );

        let mut complete = Vec::new();
        for (suffix, key) in [
            ("IS TRUE", "expected_true_rowids"),
            ("IS FALSE", "expected_false_rowids"),
            ("IS UNKNOWN", "expected_unknown_rowids"),
        ] {
            let expected = frozen_ids(case, key);
            assert_eq!(
                integers(
                    &db,
                    &format!("SELECT rowid FROM {relation} WHERE ({predicate}) {suffix}")
                ),
                expected,
                "{id}: {suffix} must hold for whole source instance bag"
            );
            complete.extend(expected);
        }
        complete.sort();
        assert_eq!(
            complete, expected_all,
            "{id}: no omitted or repeated physical occurrence"
        );
        assert_eq!(
            integers(
                &db,
                &format!("SELECT rowid FROM {relation} WHERE ({predicate}) IS NOT TRUE")
            ),
            frozen_ids(case, "expected_not_true_rowids"),
            "{id}: closed-world NOT TRUE requires FALSE plus UNKNOWN"
        );
        assert_eq!(
            bag(&db, sql),
            frozen_nullable_bag(&prior_expected_terminal(id, &env, &final_native)),
            "{id}: independent full original DuckDB output bag"
        );

        let parser = dialect_from_name("duckdb").expect("public DuckDB dialect");
        let protocol = analyze_sql(sql, "duckdb", parser.as_ref())
            .unwrap_or_else(|error| panic!("{id}: public analyzer failed: {error}"));
        let wire = read(&to_json(&protocol));
        assert_eq!(wire["inputs"][0]["statements"][0]["kind"], "query", "{id}");
        assert!(
            wire["inputs"][0]["statements"][0]["output"]["columns"][0]["domain"].is_object(),
            "{id}: output domain must be explicit, even when not provably constrained"
        );
    }
    assert_eq!(visited.len(), 11);
}

#[test]
fn overflow_ordering_and_struct_have_explicit_non_predicate_outcome_evidence() {
    let freeze = read(FREEZE);
    let samples = read(SAMPLES);
    let env = read(ENV);
    let ordering = read(ORDERING);
    let cases = freeze["other"]
        .as_array()
        .expect("three non-predicate controls");
    assert_eq!(cases.len(), 3);

    let overflow = cases
        .iter()
        .find(|case| case["id"] == "88-C177-S033")
        .expect("overflow");
    assert_eq!(overflow["expected"], "evaluation_error");
    let db = Connection::open_in_memory().expect("DuckDB");
    let original_overflow = original(&samples, "88-C177-S033");
    let result = db
        .prepare(original_overflow)
        .and_then(|mut stmt| stmt.query([]).and_then(|mut rows| rows.next().map(|_| ())));
    assert!(
        result.is_err(),
        "overflow cannot produce a successful row bag"
    );

    let page = cases
        .iter()
        .find(|case| case["id"] == "88-C177-S042")
        .expect("null ordering");
    let db = Connection::open_in_memory().expect("DuckDB");
    db.execute_batch(
        ordering["ordering_source_setup"]
            .as_str()
            .expect("pinned order source"),
    )
    .expect("source insert");
    let original_order = original(&samples, "88-C177-S042");
    let mut stmt = db.prepare(original_order).expect("original ordering");
    let actual: Vec<Option<i64>> = stmt
        .query_map([], |row| row.get(0))
        .expect("ordered rows")
        .map(|row| row.expect("id"))
        .collect();
    let expected: Vec<Option<i64>> = page["expected_ordered_terminal"]
        .as_array()
        .expect("full fixed ordering")
        .iter()
        .map(|v| v.as_i64())
        .collect();
    assert_eq!(
        actual, expected,
        "NULLS LAST, duplicate ties and physical source bag"
    );
    assert_eq!(
        ordering["ordering_source_bag"]
            .as_array()
            .expect("source rows")
            .len(),
        6
    );

    let structure = cases
        .iter()
        .find(|case| case["id"] == "88-C177-S179")
        .expect("struct");
    let sql = original(&samples, "88-C177-S179");
    let nested = format!("SELECT v.a, v.b FROM ({sql}) derived");
    let db = Connection::open_in_memory().expect("DuckDB");
    let mut stmt = db.prepare(&nested).expect("original STRUCT scalar");
    let mut rows = stmt.query([]).expect("one native struct");
    let row = rows.next().expect("native row").expect("one struct");
    let a: i64 = row.get(0).expect("struct field a");
    let b: String = row.get(1).expect("struct field b");
    assert_eq!(
        a,
        structure["expected_fields"]["a"]
            .as_i64()
            .expect("int field")
    );
    assert_eq!(
        b,
        structure["expected_fields"]["b"]
            .as_str()
            .expect("string field")
    );
    assert!(rows.next().expect("one row").is_none());

    // These are independent DuckDB boundaries, not native vendor evidence.
    assert_eq!(env["native_rejected_cases"][0]["id"], "88-C177-S033");
    assert_eq!(structure["vendor_session_approved"], false);
}
