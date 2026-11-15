//! TASK-177 first-party complete relational bag oracles for original TASK-66 SQL.
//!
//! Expected bags are literal, independent fixtures. DuckDB executes the exact
//! registered SQL over full NULL/duplicate physical sources. These checks do
//! not certify strongest typed domains, 13 vendor runtimes, or source adapters.

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{analyze_sql, dialect_from_name, ProtocolStatement};

const FIXTURE: &str = include_str!("../docs/task-177-relational-bag-oracles.json");
const SAMPLES: &str = include_str!("../docs/task-177-parser-samples.json");

fn parse(source: &str) -> Value {
    serde_json::from_str(source).expect("versioned native oracle fixture")
}

fn expected_bag(value: &Value) -> Vec<Vec<Option<i64>>> {
    let mut result: Vec<_> = value
        .as_array()
        .expect("literal complete physical bag")
        .iter()
        .map(|row| {
            row.as_array()
                .expect("physical row")
                .iter()
                .map(|item| {
                    if item.is_null() {
                        None
                    } else {
                        Some(item.as_i64().expect("integer"))
                    }
                })
                .collect()
        })
        .collect();
    result.sort();
    result
}

fn actual_bag(db: &Connection, sql: &str, columns: usize) -> Vec<Vec<Option<i64>>> {
    let mut stmt = db.prepare(sql).expect("original native SQL");
    let rows = stmt
        .query_map([], |row| {
            (0..columns)
                .map(|index| row.get::<usize, Option<i64>>(index))
                .collect::<Result<Vec<_>, _>>()
        })
        .expect("native output rows");
    let mut result: Vec<_> = rows.map(|row| row.expect("typed physical value")).collect();
    result.sort();
    result
}

fn verify_group(group: &str) {
    let fixture = parse(FIXTURE);
    let samples = parse(SAMPLES);
    assert_eq!(fixture["schema_version"], 1);
    assert_eq!(fixture["owner"], "TASK-177");
    assert_eq!(fixture["case_count"], 98);
    let groups = fixture["groups"].as_object().expect("nine finite families");
    assert_eq!(groups.len(), 9);
    let counted: usize = groups
        .values()
        .map(|ids| ids.as_array().expect("stable sample IDs").len())
        .sum();
    assert_eq!(counted, 98, "a named native oracle was lost");

    let db = Connection::open_in_memory().expect("native DuckDB");
    db.execute_batch(
        fixture["source_setup"]
            .as_str()
            .expect("physical source DDL and rows"),
    )
    .expect("complete native physical input");
    for (relation, columns) in [("orders", 2), ("customers", 2), ("products", 1)] {
        assert_eq!(
            actual_bag(&db, &format!("SELECT * FROM {relation}"), columns),
            expected_bag(&fixture["source_bags"][relation]),
            "{relation}: complete input bag (including duplicate occurrences)"
        );
    }

    let dialect = dialect_from_name("duckdb").expect("named parser dialect");
    let cases = fixture["cases"].as_array().expect("fixed oracles");
    let ids = fixture["groups"][group]
        .as_array()
        .expect("named native family");
    for id in ids {
        let id = id.as_str().expect("stable case ID");
        let case = cases
            .iter()
            .find(|candidate| candidate["id"] == id)
            .expect("every group case has pinned physical proof");
        let sample = samples["cases"]
            .as_array()
            .expect("TASK-66 sample registry")
            .iter()
            .find(|sample| sample["id"] == id)
            .expect("source case exists");
        assert_eq!(
            case["sql"], sample["sql"],
            "{id}: original SQL may not drift"
        );
        assert_eq!(
            case["feature_id"], sample["feature_id"],
            "{id}: feature identity"
        );
        assert_eq!(case["variant"], sample["variant"], "{id}: variant identity");
        let sql = case["sql"].as_str().expect("original SQL");
        let columns = case["columns"].as_u64().expect("projection arity") as usize;
        assert_eq!(
            actual_bag(&db, sql, columns),
            expected_bag(&case["expected_terminal_bag"]),
            "{id}: complete independently pinned native terminal bag"
        );

        let protocol = analyze_sql(sql, "duckdb", dialect.as_ref())
            .unwrap_or_else(|error| panic!("{id}: public API must analyze: {error}"));
        let [ProtocolStatement::Query(query)] = protocol.statements() else {
            panic!("{id}: query must have explicit canonical query semantics");
        };
        if case["canonical_output_boundary"] == "name_alignment_output_unsupported" {
            // TASK-177 records the historical unsupported boundary; TASK-194
            // adds sound output typing for the original bijective name mapping
            // without retroactively changing the earlier evidence ledger.
            assert_eq!(id, "88-C177-S083");
            assert_eq!(
                query
                    .output()
                    .columns()
                    .iter()
                    .map(|column| column.name())
                    .collect::<Vec<_>>(),
                vec!["id", "amount"]
            );
            assert!(
                query
                    .diagnostics()
                    .iter()
                    .all(|diagnostic| diagnostic.code() != "unsupported_set_operation_alignment"),
                "{id}: exactly matching names have resolved typed alignment"
            );
        } else {
            assert_eq!(
                query.output().columns().len(),
                columns,
                "{id}: native projection arity and canonical output must agree"
            );
        }
    }
}

#[test]
fn native_joins_preserve_identity_and_duplicate_multiplicity() {
    verify_group("joins");
}

#[test]
fn native_set_operators_preserve_null_and_bag_multiplicity() {
    verify_group("sets");
}

#[test]
fn native_grouped_outputs_preserve_null_and_aggregate_multiplicity() {
    verify_group("grouping");
}

#[test]
fn rejected_left_semi_and_anti_syntax_has_a_portable_native_equivalence_oracle() {
    let samples = parse(SAMPLES);
    let fixture = parse(FIXTURE);
    let db = Connection::open_in_memory().expect("native DuckDB");
    db.execute_batch(
        fixture["source_setup"]
            .as_str()
            .expect("complete original physical sources"),
    )
    .expect("complete source fixture");
    for (id, native, expected) in [
        (
            "88-C177-S054",
            "SELECT l.id FROM orders l SEMI JOIN customers r ON l.id = r.id",
            vec![vec![Some(1)], vec![Some(1)], vec![Some(2)]],
        ),
        (
            "88-C177-S055",
            "SELECT l.id FROM orders l ANTI JOIN customers r ON l.id = r.id",
            vec![vec![None], vec![Some(3)], vec![Some(4)]],
        ),
    ] {
        let original = samples["cases"]
            .as_array()
            .expect("TASK-66 registry")
            .iter()
            .find(|sample| sample["id"] == id)
            .expect("stable original SQL")["sql"]
            .as_str()
            .expect("sample SQL");
        assert!(
            db.prepare(original).is_err(),
            "{id}: LEFT SEMI/ANTI is an exact-syntax DuckDB rejection"
        );
        assert_eq!(
            actual_bag(&db, native, 1),
            expected,
            "{id}: independently pinned equivalent native SEMI/ANTI output"
        );
    }
}

#[test]
fn native_supergroups_have_exact_null_and_subtotal_bags() {
    verify_group("supergroups");
}

#[test]
fn native_window_and_qualify_outputs_have_exact_bags() {
    verify_group("windows");
}

#[test]
fn native_subqueries_preserve_3vl_and_correlated_membership() {
    verify_group("subqueries");
}

#[test]
fn native_cte_and_derived_scopes_have_exact_terminal_bags() {
    verify_group("scopes");
}

#[test]
fn native_limit_and_fetch_rows_match_complete_sources() {
    verify_group("pagination");
}

#[test]
fn native_unnest_values_and_generators_have_exact_bags() {
    verify_group("table_sources");
}

#[test]
fn fetch_with_ties_is_a_named_native_syntax_boundary() {
    let fixture = parse(FIXTURE);
    let samples = parse(SAMPLES);
    let db = Connection::open_in_memory().expect("DuckDB");
    db.execute_batch(
        fixture["source_setup"]
            .as_str()
            .expect("complete physical source"),
    )
    .expect("source rows");
    let sample = samples["cases"]
        .as_array()
        .expect("source examples")
        .iter()
        .find(|sample| sample["id"] == "88-C177-S160")
        .expect("original FETCH WITH TIES case");
    let original = sample["sql"].as_str().expect("fixed SQL");
    assert!(
        db.prepare(original).is_err(),
        "DuckDB does not accept the frozen FETCH FIRST WITH TIES spelling"
    );
    assert_eq!(
        actual_bag(&db, "SELECT id FROM orders ORDER BY id LIMIT 2", 1),
        vec![vec![Some(1)], vec![Some(1)]],
        "the complete source fixture has exactly two tied first rows"
    );
}
