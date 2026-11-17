//! TASK-179 initial vendor/session-law oracles.
//!
//! These are independent, closed DuckDB examples, not attestations that
//! another warehouse uses DuckDB's collation, coercion, name or transaction law.
//! TASK-179 remains open until every mandatory named family has been reviewed.

use std::collections::BTreeSet;

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{analyze_sql, dialect_from_name, to_json};

const FREEZE: &str = include_str!("../docs/task-179-acceptance-freeze.json");
const ORIGINALS: &str = include_str!("../docs/task-177-parser-samples.json");
const PARSERS: &str = include_str!("../docs/task-177-parser-snapshot.json");

fn load(text: &str) -> Value {
    serde_json::from_str(text).expect("versioned original evidence")
}

fn ordered_ids(db: &Connection, sql: &str) -> Vec<i64> {
    let mut statement = db.prepare(sql).expect("named native query");
    statement
        .query_map([], |row| row.get::<_, i64>(0))
        .expect("complete native output")
        .map(|row| row.expect("source occurrence identity"))
        .collect()
}

fn state_rows(db: &Connection) -> Vec<Vec<Option<i64>>> {
    let mut statement = db
        .prepare("SELECT row_id, amount FROM state_rows ORDER BY row_id")
        .expect("typed final state");
    statement
        .query_map([], |row| {
            Ok(vec![
                row.get::<_, Option<i64>>(0)?,
                row.get::<_, Option<i64>>(1)?,
            ])
        })
        .expect("complete final state")
        .map(|row| row.expect("native state row"))
        .collect()
}

#[test]
fn initial_vendor_law_freeze_tracks_six_families_all_dialects_and_open_boundaries() {
    let freeze = load(FREEZE);
    let original = load(ORIGINALS);
    let parser_snapshot = load(PARSERS);
    assert_eq!(freeze["schema_version"], 1);
    assert_eq!(freeze["task_id"], "TASK-179");
    assert_eq!(freeze["parent"], "TASK-88");
    assert_eq!(freeze["positive_fixture_id"], "88-C179-P");
    assert_eq!(freeze["negative_fixture_id"], "88-C179-N");
    assert_eq!(freeze["dialects"], parser_snapshot["dialects"]);

    let cases = freeze["cases"].as_array().expect("frozen oracle matrix");
    let dialects = freeze["dialects"].as_array().expect("13 parser identities");
    assert_eq!(cases.len(), 14);
    assert_eq!(dialects.len(), 13);
    assert_eq!(freeze["case_count"], cases.len());
    assert_eq!(freeze["dialect_cell_count"], cases.len() * dialects.len());

    let expected_families = BTreeSet::from([
        "null_ordering",
        "collation",
        "implicit_coercion",
        "float_timezone",
        "dml_transaction",
        "identifier_case",
    ]);
    let families = cases
        .iter()
        .map(|case| case["family"].as_str().expect("named family"))
        .collect::<BTreeSet<_>>();
    assert_eq!(families, expected_families);

    let source_cases = original["cases"]
        .as_array()
        .expect("original source inventory");
    let mut ids = BTreeSet::new();
    for case in cases {
        let id = case["id"].as_str().expect("stable task ID");
        assert!(ids.insert(id), "duplicate {id}");
        assert!(case["setup"].as_str().is_some_and(|sql| !sql.is_empty()));
        assert!(case["sql"].as_str().is_some_and(|sql| !sql.is_empty()));
        assert!(case["expected_duckdb_terminal_or_poststate"].is_array());
        assert_eq!(case["review"], "not_signed_off");
        assert!(case["boundary"].as_str().is_some_and(|why| !why.is_empty()));
        assert!(case["typed_source"].as_str().is_some());
        assert!(case["source_instance_identity"].as_str().is_some());
        if let Some(original_id) = case["source_sample"].as_str() {
            assert!(
                source_cases.iter().any(|entry| entry["id"] == original_id),
                "{id} references missing original {original_id}"
            );
        }
        let cells = case["dialect_cells"]
            .as_array()
            .expect("per-dialect outcomes");
        assert_eq!(cells.len(), dialects.len(), "{id}: no silent omissions");
        for (cell, dialect) in cells.iter().zip(dialects) {
            assert_eq!(cell["dialect"], *dialect, "{id}: wrong dialect order");
            assert_eq!(cell["external_native"], "unattested");
            assert_eq!(cell["release_disposition"], "unapproved_pending_review");
            assert_eq!(cell["parser"], "unverified_until_executable_check");
        }
    }
    assert_eq!(ids.len(), 14);
    assert!(freeze["acceptance_criteria"]["ac1"].as_str().is_some());
    assert!(freeze["acceptance_criteria"]["ac5"].as_str().is_some());
}

#[test]
fn fixed_native_sources_produce_both_complete_positive_and_negative_outcomes() {
    let freeze = load(FREEZE);
    for case in freeze["cases"].as_array().expect("fixed case inventory") {
        let id = case["id"].as_str().expect("stable fixture ID");
        let family = case["family"].as_str().expect("named boundary family");
        let db = Connection::open_in_memory().expect("DuckDB");
        db.execute_batch(case["setup"].as_str().expect("fixed physical inputs"))
            .unwrap_or_else(|error| panic!("{id}: source setup: {error}"));

        // Verify every source row identity exists before making a terminal claim.
        // Multiplicity, NULL values and types are fixed by the explicit INSERT.
        let (source_sql, source_ids): (&str, &[i64]) = match family {
            "null_ordering" => (
                "SELECT row_id FROM sort_rows ORDER BY row_id",
                &[1, 2, 3, 4],
            ),
            "collation" => (
                "SELECT row_id FROM string_rows ORDER BY row_id",
                &[1, 2, 3, 4],
            ),
            "implicit_coercion" => ("SELECT row_id FROM cast_rows ORDER BY row_id", &[1, 2, 3]),
            "float_timezone" if case["variant"].as_str().unwrap_or("").contains("offset") => {
                ("SELECT row_id FROM zoned_rows ORDER BY row_id", &[1, 2, 3])
            }
            "float_timezone" => (
                "SELECT row_id FROM float_rows ORDER BY row_id",
                &[1, 2, 3, 4],
            ),
            "dml_transaction" => ("SELECT row_id FROM state_rows ORDER BY row_id", &[1, 2]),
            "identifier_case" => (
                "SELECT \"ResultValue\" FROM \"MixedCase\" ORDER BY 1",
                &[1, 2],
            ),
            other => panic!("{id}: unknown family {other}"),
        };
        assert_eq!(
            ordered_ids(&db, source_sql),
            source_ids,
            "{id}: source identities"
        );

        let sql = case["sql"].as_str().expect("exact candidate SQL");
        if family == "dml_transaction" {
            db.execute_batch(sql)
                .unwrap_or_else(|error| panic!("{id}: native transaction: {error}"));
            let expected: Vec<Vec<Option<i64>>> =
                serde_json::from_value(case["expected_duckdb_terminal_or_poststate"].clone())
                    .expect("independently frozen full mutation poststate");
            assert_eq!(
                state_rows(&db),
                expected,
                "{id}: complete final relation state"
            );
        } else {
            let actual = ordered_ids(&db, sql);
            let expected: Vec<i64> =
                serde_json::from_value(case["expected_duckdb_terminal_or_poststate"].clone())
                    .expect("independently frozen complete ordered terminal");
            assert_eq!(actual, expected, "{id}: ordered DuckDB terminal");
        }
    }
}

#[test]
fn portable_null_truth_remains_canonical_across_all_thirteen_parser_dialects() {
    // This shared portable control is deliberately narrower than any
    // vendor-specific NULL ordering, collation or implicit-cast promise.
    let freeze = load(FREEZE);
    let mut reference: Option<Value> = None;
    for dialect in freeze["dialects"].as_array().expect("13 dialects") {
        let name = dialect.as_str().expect("parser identity");
        let parser = dialect_from_name(name).expect("exposed public parser");
        let protocol = analyze_sql(
            "SELECT row_id FROM sort_rows WHERE amount IS NULL",
            name,
            parser.as_ref(),
        )
        .unwrap_or_else(|error| panic!("{name}: portable SQL analysis: {error}"));
        let mut emitted = load(&to_json(&protocol));
        for input in emitted["inputs"]
            .as_array_mut()
            .expect("public input metadata")
        {
            input["dialect"] = Value::Null;
        }
        if let Some(expected) = reference.as_ref() {
            assert_eq!(&emitted, expected, "{name}: shared canonical facts changed");
        } else {
            reference = Some(emitted);
        }
    }
}
