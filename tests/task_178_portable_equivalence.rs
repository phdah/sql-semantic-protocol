//! TASK-178: public protocol equivalence and independent DuckDB bag oracles.
//!
//! The frozen portable cases are a *subset* of TASK-178's mandatory matrix.
//! Comparing parser outputs alone cannot certify execution, adapter parity,
//! dialect-specific laws, or downstream physical construction.

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_sql, dialect_from_name, to_json, LiteralValue, ProtocolStatement, ValueDomain,
};

const FREEZE: &str = include_str!("../docs/task-178-acceptance-freeze.json");
const SAMPLES: &str = include_str!("../docs/task-177-parser-samples.json");
const EQUIVALENCE: &str = include_str!("../docs/task-177-equivalence-snapshot.json");

fn parse(source: &str) -> Value {
    serde_json::from_str(source).expect("versioned task evidence")
}

fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().expect("required string")
}

fn bag(value: &Value) -> Vec<Vec<Option<i64>>> {
    let mut rows: Vec<Vec<Option<i64>>> =
        serde_json::from_value(value.clone()).expect("literal typed nullable bag");
    rows.sort();
    rows
}

fn actual_bag(db: &Connection, sql: &str, columns: usize) -> Vec<Vec<Option<i64>>> {
    let mut stmt = db.prepare(sql).expect("native SQL statement");
    let result = stmt
        .query_map([], |row| {
            (0..columns)
                .map(|index| row.get::<usize, Option<i64>>(index))
                .collect::<Result<Vec<_>, _>>()
        })
        .expect("native SQL result");
    let mut rows: Vec<_> = result.map(|row| row.expect("physical row")).collect();
    rows.sort();
    rows
}

fn normalized_public_document(
    sql: &str,
    dialect_name: &str,
) -> (sql_semantic_protocol::Protocol, Value) {
    let dialect = dialect_from_name(dialect_name).expect("supported parser dialect");
    let protocol = analyze_sql(sql, dialect_name, dialect.as_ref())
        .unwrap_or_else(|error| panic!("{dialect_name}: public analysis failed: {error}"));
    let mut document: Value = parse(&to_json(&protocol));
    // Dialect identity is input provenance. All semantic outcomes remain in
    // the comparison: diagnostic reasons, physical plans, NULLs, domains,
    // statement effects, source identities, counts and proof strengths.
    for input in document["inputs"].as_array_mut().expect("public inputs") {
        input["dialect"] = Value::Null;
    }
    (protocol, document)
}

#[test]
fn frozen_seven_portable_families_preserve_complete_canonical_documents_in_all_dialects() {
    let freeze = parse(FREEZE);
    let source = parse(SAMPLES);
    let old = parse(EQUIVALENCE);
    let dialects = freeze["dialects"].as_array().expect("13 public dialects");
    assert_eq!(freeze["schema_version"], 1);
    assert_eq!(freeze["task_id"], "TASK-178");
    assert_eq!(freeze["positive_fixture_id"], "88-C178-P");
    assert_eq!(freeze["negative_fixture_id"], "88-C178-N");
    assert_eq!(dialects.len(), 13);
    assert_eq!(freeze["dialects"], source["dialects"]);
    assert_eq!(freeze["dialects"], old["dialects"]);

    let cases = freeze["cases"]
        .as_array()
        .expect("mandatory portable cases");
    assert_eq!(cases.len(), 7);
    for case in cases {
        let id = text(case, "id");
        let original_id = text(case, "source_sample_id");
        let sample = source["cases"]
            .as_array()
            .expect("original sample registry")
            .iter()
            .find(|sample| sample["id"] == original_id)
            .expect("independent source SQL");
        assert_eq!(case["sql"], sample["sql"], "{id}: preserve original SQL");
        assert_eq!(case["feature_id"], sample["feature_id"]);
        assert_eq!(case["variant"], sample["variant"]);
        assert_eq!(case["parser_dialects"], freeze["dialects"]);

        let mut reference: Option<Value> = None;
        let mut count = 0;
        for name in dialects {
            let name = name.as_str().expect("named dialect");
            let (protocol, canonical) = normalized_public_document(text(case, "sql"), name);
            assert!(
                !protocol
                    .statements()
                    .iter()
                    .any(|statement| matches!(statement, ProtocolStatement::Unsupported(_))),
                "{id}/{name}: explicit unsupported is not certified equivalence"
            );
            if let Some(expected) = &reference {
                assert_eq!(
                    &canonical, expected,
                    "{id}/{name}: complete canonical semantic document differs"
                );
            } else {
                reference = Some(canonical);
            }
            count += 1;
        }
        assert_eq!(count, 13, "{id}: no dialect can be skipped");
        assert_eq!(
            old["matrix"][original_id], "CCCCCCCCCCCCC",
            "{id}: existing parser-only evidence must not be relabeled"
        );
    }
}

#[test]
fn independent_duckdb_source_and_terminal_bags_preserve_duplicates_nulls_and_writes() {
    let freeze = parse(FREEZE);
    for case in freeze["cases"].as_array().expect("frozen cases") {
        let family = text(case, "family");
        let db = Connection::open_in_memory().expect("DuckDB memory database");
        let setup = if family == "ddl" {
            text(&freeze, "ddl_source_setup")
        } else {
            text(&freeze, "source_setup")
        };
        db.execute_batch(setup)
            .expect("independent complete source setup");
        for &name in if family == "ddl" {
            &["customers"][..]
        } else {
            &["orders", "customers"][..]
        } {
            assert_eq!(
                actual_bag(&db, &format!("SELECT id, amount FROM {name}"), 2),
                bag(&freeze["source_bags"][name]),
                "{family}: complete {name} physical source bag"
            );
        }
        let sql = text(case, "sql");
        let terminal = match family {
            "dml" => {
                db.execute_batch(sql).expect("original DML execution");
                actual_bag(&db, "SELECT id, amount FROM orders", 2)
            }
            "ddl" => {
                db.execute_batch(sql).expect("original CTAS execution");
                actual_bag(&db, "SELECT id FROM orders", 1)
            }
            _ => actual_bag(&db, sql, 1),
        };
        assert_eq!(
            terminal,
            bag(&case["expected_terminal_bag"]),
            "{family}: full terminal, post-write or produced-relation bag"
        );
    }
}

#[test]
fn nullable_predicate_has_both_exact_rejection_directions_and_inclusive_domain() {
    let freeze = parse(FREEZE);
    let case = freeze["cases"]
        .as_array()
        .expect("cases")
        .iter()
        .find(|case| case["family"] == "predicate")
        .expect("portable predicate case");
    let db = Connection::open_in_memory().expect("DuckDB");
    db.execute_batch(text(&freeze, "source_setup"))
        .expect("complete physical source");
    for (truth, field) in [
        ("IS TRUE", "expected_positive_bag"),
        ("IS FALSE", "expected_rejected_bag"),
        ("IS UNKNOWN", "expected_unknown_bag"),
    ] {
        assert_eq!(
            actual_bag(
                &db,
                &format!("SELECT amount FROM orders WHERE (amount >= 10) {truth}"),
                1
            ),
            bag(&case[field]),
            "{truth}: independent complete SQL three-valued predicate partition"
        );
    }
    let (protocol, _) = normalized_public_document(text(case, "sql"), "duckdb");
    let [ProtocolStatement::Query(query)] = protocol.statements() else {
        panic!("public predicate query is missing or unsupported");
    };
    let [column] = query.output().columns() else {
        panic!("predicate must produce exactly one output column");
    };
    let ValueDomain::Ranges(ranges) = column.domain() else {
        panic!("required precise output domain must not become unknown");
    };
    let [range] = ranges.ranges() else {
        panic!("expected one non-disjoint integer interval");
    };
    let lower = range.lower().expect("inclusive lower bound");
    assert_eq!(
        lower.value().value(),
        &LiteralValue::Number("10".to_string())
    );
    assert!(lower.inclusive());
    assert!(range.upper().is_none());
    assert_eq!(
        case["expected_projected_domain"]["lower"],
        parse("[10,true]")
    );
    assert!(case["expected_projected_domain"]["upper"].is_null());
}

#[test]
fn join_negative_membership_and_pagination_excluded_rows_use_same_closed_source() {
    let freeze = parse(FREEZE);
    let db = Connection::open_in_memory().expect("DuckDB");
    db.execute_batch(text(&freeze, "source_setup"))
        .expect("closed physical source bags");
    let cases = freeze["cases"].as_array().expect("cases");
    let join = cases
        .iter()
        .find(|case| case["family"] == "join")
        .expect("join case");
    assert_eq!(
        actual_bag(&db, text(join, "sql"), 1),
        bag(&join["expected_positive_bag"])
    );
    assert_eq!(
        actual_bag(
            &db,
            "SELECT l.id FROM orders l WHERE NOT EXISTS \
             (SELECT 1 FROM customers r WHERE l.id = r.id)",
            1
        ),
        bag(&join["expected_rejected_bag"]),
        "complete anti-membership includes unmatched physical NULL rows"
    );

    let pagination = cases
        .iter()
        .find(|case| case["family"] == "pagination")
        .expect("pagination case");
    let source = actual_bag(&db, "SELECT id FROM orders", 1);
    let selected = actual_bag(&db, text(pagination, "sql"), 1);
    let mut excluded = source;
    for row in &selected {
        let index = excluded
            .iter()
            .position(|candidate| candidate == row)
            .expect("each paged output occurrence comes from the source");
        excluded.remove(index);
    }
    excluded.sort();
    assert_eq!(
        excluded,
        bag(&pagination["expected_rejected_bag"]),
        "OFFSET/LIMIT cannot silently discard an extra duplicate occurrence"
    );
}

#[test]
fn unverified_variant_and_vendor_boundaries_block_task_done() {
    let freeze = parse(FREEZE);
    let boundaries = freeze["mandatory_variant_boundaries"]
        .as_array()
        .expect("named conditional boundaries");
    for id in [
        "88-C178-TOP",
        "88-C178-FETCH",
        "88-C178-MINUS",
        "88-C178-SUPERGROUP",
    ] {
        assert!(
            boundaries.iter().any(|cell| {
                cell["id"] == id
                    && cell["owner"] == "TASK-178"
                    && cell["released"] == false
                    && cell["required_evidence"]
                        .as_str()
                        .is_some_and(|text| !text.is_empty())
            }),
            "{id}: conditional evidence cannot be silently promoted"
        );
    }
    let completion = &freeze["completion"];
    for key in [
        "mandatory_exact_oracle_matrix_certified",
        "all_13_dialect_typed_semantics_certified",
        "direct_dbt_odcs_equivalence_certified",
        "all_23_drifts_resolved",
        "negative_and_impossible_oracles_complete",
        "native_vendor_certified",
        "final_ci_green",
        "independent_review_verified",
        "child_done",
        "parent_done",
        "release_approved",
    ] {
        assert_eq!(completion[key], false, "{key}: not signed off");
    }
    let duckdb = Connection::open_in_memory().expect("DuckDB");
    duckdb
        .execute_batch("CREATE TABLE orders(id INTEGER)")
        .expect("fixture relation exists before testing syntax");
    assert!(
        duckdb
            .prepare("SELECT id FROM orders ORDER BY id FETCH FIRST 2 ROWS WITH TIES")
            .is_err(),
        "the original FETCH WITH TIES SQL remains a native syntax boundary"
    );
}
