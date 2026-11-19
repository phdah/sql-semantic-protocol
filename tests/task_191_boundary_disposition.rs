//! TASK-191 independently owns the exact parser, analyzer and native exception sets.
//!
//! These tests intentionally do not convert an alternate spelling, a parser
//! success or a DuckDB control into a claim about the original native SQL.

use std::collections::{BTreeMap, BTreeSet};

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{analyze_sql, dialect_from_name, Error, ProtocolStatement};

const FREEZE: &str = include_str!("../docs/task-191-acceptance-freeze.json");
const SAMPLES: &str = include_str!("../docs/task-177-parser-samples.json");
const SNAPSHOT: &str = include_str!("../docs/task-177-parser-snapshot.json");
const PARSER: &str = include_str!("../docs/task-177-parser-boundaries.json");
const UNSUPPORTED: &str = include_str!("../docs/task-177-analysis-boundaries.json");
const NATIVE: &str = include_str!("../docs/task-177-remaining-original-cases.json");
const PHYSICAL: &str = include_str!("../docs/task-177-physical-proof-ledger.json");
const COVERAGE: &str = include_str!("../docs/coverage-manifest.json");

fn parse(source: &str) -> Value {
    serde_json::from_str(source).expect("committed evidence must be valid JSON")
}

fn array<'a>(value: &'a Value, field: &str) -> &'a [Value] {
    value[field].as_array().expect("versioned evidence array")
}

fn string<'a>(value: &'a Value, field: &str) -> &'a str {
    value[field].as_str().expect("versioned evidence text")
}

fn sample_by_id<'a>(samples: &'a Value, id: &str) -> &'a Value {
    array(samples, "cases")
        .iter()
        .find(|sample| sample["id"] == id)
        .unwrap_or_else(|| panic!("{id}: missing immutable original SQL"))
}

fn exact_ids(rows: &[Value], field: &str) -> BTreeSet<String> {
    let all: Vec<_> = rows
        .iter()
        .map(|row| string(row, field).to_owned())
        .collect();
    let ids: BTreeSet<_> = all.iter().cloned().collect();
    assert_eq!(ids.len(), all.len(), "{field}: duplicate frozen row");
    ids
}

#[test]
fn freeze_reconciles_every_exception_against_the_independent_original_ledgers() {
    let freeze = parse(FREEZE);
    let sources = parse(SAMPLES);
    let snapshot = parse(SNAPSHOT);
    let parser = parse(PARSER);
    let unsupported = parse(UNSUPPORTED);
    let native = parse(NATIVE);
    let physical = parse(PHYSICAL);
    let coverage = parse(COVERAGE);

    assert_eq!(freeze["task_id"], "TASK-191");
    assert_eq!(freeze["version"], 1);
    assert_eq!(freeze["fixture_ids"]["positive"], "88-C191-P");
    assert_eq!(freeze["fixture_ids"]["negative"], "88-C191-N");
    assert_eq!(freeze["native_vendor_attestation"], "not_certified");
    assert_eq!(freeze["task_88_release_approved"], false);
    assert_eq!(freeze["independent_review"], "not_done");

    let dialects = array(&sources, "dialects");
    assert_eq!(dialects, array(&snapshot, "dialects"));
    assert_eq!(dialects.len(), 13);
    let source_map: BTreeMap<_, _> = array(&sources, "cases")
        .iter()
        .map(|row| (string(row, "id"), row))
        .collect();
    assert_eq!(source_map.len(), 251);

    let expected_parser: BTreeSet<_> = snapshot["matrix"]
        .as_object()
        .expect("historical parser masks")
        .iter()
        .flat_map(|(id, mask)| {
            mask.as_str()
                .expect("parser mask")
                .chars()
                .enumerate()
                .filter_map(move |(index, marker)| {
                    if marker == 'R' {
                        let dialect = dialects[index].as_str().expect("dialect");
                        Some(format!("{id}-{dialect}"))
                    } else {
                        None
                    }
                })
        })
        .collect();
    assert_eq!(expected_parser.len(), 55);

    let expected_unsupported: BTreeSet<_> = snapshot["canonical_analysis_matrix"]
        .as_object()
        .expect("historical analyzer masks")
        .iter()
        .flat_map(|(id, mask)| {
            mask.as_str()
                .expect("analyzer mask")
                .chars()
                .enumerate()
                .filter_map(move |(index, marker)| {
                    if marker == 'U' {
                        let dialect = dialects[index].as_str().expect("dialect");
                        Some(format!("{id}-{dialect}"))
                    } else {
                        None
                    }
                })
        })
        .collect();
    assert_eq!(expected_unsupported.len(), 116);
    assert!(
        expected_parser.is_disjoint(&expected_unsupported),
        "one SQL/dialect cell cannot both fail parsing and return Unsupported"
    );

    let parser_rows = array(&freeze, "parser_rejected_cells");
    let unsupported_rows = array(&freeze, "analyzer_unsupported_cells");
    let native_rows = array(&freeze, "native_unproven_original_cases");
    assert_eq!(exact_ids(parser_rows, "cell_id"), expected_parser);
    assert_eq!(exact_ids(unsupported_rows, "cell_id"), expected_unsupported);
    assert_eq!(
        exact_ids(parser_rows, "cell_id"),
        exact_ids(array(&parser, "cells"), "id")
    );
    assert_eq!(
        exact_ids(unsupported_rows, "cell_id"),
        exact_ids(array(&unsupported, "cells"), "id")
    );

    for (rows, class, key) in [
        (parser_rows, "R", "expected_public_parser"),
        (unsupported_rows, "U", "expected_public_analyzer"),
    ] {
        for row in rows {
            let id = string(row, "original_id");
            let dialect = string(row, "dialect");
            let original = source_map.get(id).expect("original fixture");
            assert_eq!(row["feature_id"], original["feature_id"]);
            assert_eq!(row["variant"], original["variant"]);
            assert_eq!(row["semantic_owner"], original["semantic_owner"]);
            assert_eq!(row[key], class);
            assert!(!string(row, "reason").is_empty());
            assert_eq!(row["reviewer_status"], "not_reviewed");
            assert_eq!(row["external_engine_session"], "not_certified");
            let i = dialects
                .iter()
                .position(|item| item == dialect)
                .expect("known dialect");
            let mask = if class == "R" {
                string(&snapshot["matrix"], id)
            } else {
                string(&snapshot["canonical_analysis_matrix"], id)
            };
            assert_eq!(mask.as_bytes()[i], class.as_bytes()[0]);
        }
    }

    let expected_native: BTreeSet<_> = array(&physical, "cases")
        .iter()
        .filter(|row| row["native_duckdb_original_sql"] == "not_proven")
        .map(|row| string(row, "id").to_owned())
        .collect();
    assert_eq!(expected_native.len(), 23);
    assert_eq!(exact_ids(native_rows, "original_id"), expected_native);
    assert_eq!(
        exact_ids(native_rows, "original_id"),
        exact_ids(array(&native, "cases"), "id")
    );
    for row in native_rows {
        let id = string(row, "original_id");
        let original = source_map.get(id).expect("original fixture");
        assert_eq!(row["feature_id"], original["feature_id"]);
        assert_eq!(row["variant"], original["variant"]);
        assert_eq!(row["semantic_owner"], original["semantic_owner"]);
        assert_eq!(row["exact_original_sql_native_terminal"], "not_proven");
        assert_eq!(
            row["source_and_terminal_oracle"],
            "no_fixed_successful_original_terminal"
        );
        assert_eq!(row["external_vendor_session"], "not_certified");
        assert_eq!(
            row["release_decision"],
            "blocked_pending_independent_review"
        );
    }

    let coverage = &coverage["task_191_boundary_disposition_acceptance"];
    assert_eq!(coverage["freeze"], "docs/task-191-acceptance-freeze.json");
    assert_eq!(coverage["parser_rejected_cells"], 55);
    assert_eq!(coverage["analyzer_unsupported_cells"], 116);
    assert_eq!(coverage["unproven_original_native_cases"], 23);
    assert_eq!(coverage["full_matrix_release_approved"], false);
}

#[test]
fn every_task_191_original_parser_and_analyzer_boundary_stays_fail_closed() {
    let freeze = parse(FREEZE);
    let sources = parse(SAMPLES);
    for row in array(&freeze, "parser_rejected_cells") {
        let id = string(row, "original_id");
        let name = string(row, "dialect");
        let dialect = dialect_from_name(name).expect("exposed parser dialect");
        let sql = string(sample_by_id(&sources, id), "sql");
        assert!(
            matches!(
                analyze_sql(sql, name, dialect.as_ref()),
                Err(Error::Parse(_))
            ),
            "{id}/{name}: rejected original syntax unexpectedly accepted"
        );
        assert_eq!(row["parser_disposition"], "blocked_original_syntax");
        assert!(row["alternate_sql"].is_null());
    }

    for row in array(&freeze, "analyzer_unsupported_cells") {
        let id = string(row, "original_id");
        let name = string(row, "dialect");
        let dialect = dialect_from_name(name).expect("exposed parser dialect");
        let sql = string(sample_by_id(&sources, id), "sql");
        let protocol = analyze_sql(sql, name, dialect.as_ref())
            .unwrap_or_else(|error| panic!("{id}/{name}: expected explicit Unsupported: {error}"));
        assert!(
            protocol
                .statements()
                .iter()
                .any(|statement| matches!(statement, ProtocolStatement::Unsupported(_))),
            "{id}/{name}: unsupported original lost its explicit diagnostic boundary"
        );
        assert_eq!(
            row["safe_protocol_outcome"],
            "explicit_unsupported_not_exact"
        );
        assert_eq!(
            row["review_disposition"],
            "blocking_unimplemented_or_unproved"
        );
    }
}

fn order_rows(db: &Connection) -> Vec<(i64, Option<i64>, Option<i64>)> {
    let mut statement = db
        .prepare("SELECT row_id, id, amount FROM orders ORDER BY row_id")
        .expect("orders source");
    statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .expect("complete orders bag")
        .map(|row| row.expect("source occurrence"))
        .collect()
}

fn customer_rows(db: &Connection) -> Vec<(i64, Option<i64>)> {
    let mut statement = db
        .prepare("SELECT row_id, id FROM customers ORDER BY row_id")
        .expect("customers source");
    statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .expect("complete customers bag")
        .map(|row| row.expect("source occurrence"))
        .collect()
}

fn terminal_rows(db: &Connection, sql: &str) -> Vec<(i64, Option<i64>)> {
    let mut statement = db.prepare(sql).expect("companion SQL");
    statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .expect("complete terminal bag")
        .map(|row| row.expect("output occurrence"))
        .collect()
}

#[test]
fn positive_and_rejected_partner_controls_use_one_complete_nullable_source_bag() {
    let freeze = parse(FREEZE);
    let samples = parse(SAMPLES);
    let oracle = &freeze["positive_negative_oracles"];
    let db = Connection::open_in_memory().expect("DuckDB in-memory");
    db.execute_batch(string(oracle, "source_setup"))
        .expect("fixed physical source bags");
    let before_orders = order_rows(&db);
    let before_customers = customer_rows(&db);
    let expected_orders: Vec<(i64, Option<i64>, Option<i64>)> =
        serde_json::from_value(oracle["expected_ordered_orders"].clone())
            .expect("independent expected source rows");
    let expected_customers: Vec<(i64, Option<i64>)> =
        serde_json::from_value(oracle["expected_ordered_customers"].clone())
            .expect("independent expected customer rows");
    assert_eq!(before_orders, expected_orders);
    assert_eq!(before_customers, expected_customers);

    for key in ["original_positive_id", "original_negative_id"] {
        let sql = string(sample_by_id(&samples, string(oracle, key)), "sql");
        let result: duckdb::Result<()> = db.prepare(sql).and_then(|mut statement| {
            let mut rows = statement.query([])?;
            while rows.next()?.is_some() {}
            Ok(())
        });
        assert!(
            result.is_err(),
            "{key}: a working companion cannot certify the rejected original SQL"
        );
    }

    let observed_positive = terminal_rows(&db, string(oracle, "positive_sql"));
    let observed_rejected = terminal_rows(&db, string(oracle, "rejected_partner_sql"));
    let expected_positive: Vec<(i64, Option<i64>)> =
        serde_json::from_value(oracle["positive_terminal"].clone())
            .expect("independent positive bag");
    let expected_rejected: Vec<(i64, Option<i64>)> =
        serde_json::from_value(oracle["rejected_partner_terminal"].clone())
            .expect("independent negative bag");
    assert_eq!(observed_positive, expected_positive);
    assert_eq!(observed_rejected, expected_rejected);
    assert_eq!(
        observed_positive.len() + observed_rejected.len(),
        before_orders.len(),
        "EXISTS and NOT EXISTS must partition every physical outer occurrence"
    );
    assert_eq!(
        order_rows(&db),
        before_orders,
        "read-only control mutated orders"
    );
    assert_eq!(
        customer_rows(&db),
        before_customers,
        "read-only control mutated customers"
    );
    assert_eq!(
        oracle["native_status"],
        "companions_only_no_original_sql_vendor_claim"
    );
}
