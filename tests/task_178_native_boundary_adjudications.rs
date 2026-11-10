//! TASK-178: every original SQL without a fixed first-party DuckDB terminal
//! has a separately owned native rejection/conditional negative oracle.
//! This is not a positive row-bag proof or cross-vendor execution certificate.

use std::collections::BTreeSet;

use duckdb::Connection;
use serde_json::Value;

const INVENTORY: &str = include_str!("../docs/task-178-full-variant-inventory.json");
const LEDGER: &str = include_str!("../docs/task-177-physical-proof-ledger.json");
const SOURCES: &str = include_str!("../docs/task-177-parser-samples.json");
const REGISTRY: &str = include_str!("../docs/task-178-native-boundary-adjudications.json");
const NATIVE: &str = include_str!("../docs/task-177-native-boundary-probes.json");
const EXPANSION: &str = include_str!("../docs/task-177-native-expansion.json");
const PREDICATES: &str = include_str!("../docs/task-177-predicate-environment-oracles.json");
const RELATIONS: &str = include_str!("../docs/task-177-relational-bag-oracles.json");

fn parsed(json: &str) -> Value {
    serde_json::from_str(json).expect("versioned first-party semantic evidence")
}

#[test]
fn every_unproved_native_sql_variant_has_a_pinned_and_honest_negative_boundary() {
    let inventory = parsed(INVENTORY);
    let ledger = parsed(LEDGER);
    let source = parsed(SOURCES);
    let registry = parsed(REGISTRY);
    let native = parsed(NATIVE);
    let expansion = parsed(EXPANSION);
    let predicates = parsed(PREDICATES);
    assert_eq!(registry["case_count"], 14);
    assert_eq!(registry["original_native_backed_in_scope"], 149);
    assert_eq!(registry["vendor_engine_execution_verified"], false);
    assert_eq!(registry["independent_review_approved"], false);
    assert_eq!(registry["release_approved"], false);

    let named: BTreeSet<_> = inventory["cases"]
        .as_array()
        .expect("named mandatory variants")
        .iter()
        .map(|case| case["id"].as_str().expect("stable ID"))
        .collect();
    assert_eq!(named.len(), 163);
    let unresolved: BTreeSet<_> = ledger["cases"]
        .as_array()
        .expect("historical original-SQL execution ledger")
        .iter()
        .filter_map(|case| {
            let id = case["id"].as_str()?;
            (named.contains(id)
                && case["native_duckdb_original_sql"] != "full_source_and_terminal_bag_oracle")
                .then_some(id)
        })
        .collect();
    assert_eq!(unresolved.len(), 14);
    let registry_cases = registry["cases"]
        .as_array()
        .expect("per-case exception ledger");
    let declared: BTreeSet<_> = registry_cases
        .iter()
        .map(|case| case["id"].as_str().expect("original SQL ID"))
        .collect();
    assert_eq!(
        declared, unresolved,
        "missing native exceptions are never waived"
    );
    for case in registry_cases {
        let id = case["id"].as_str().expect("ID");
        let original = source["cases"]
            .as_array()
            .expect("original SQL cases")
            .iter()
            .find(|candidate| candidate["id"] == id)
            .expect("source case cannot be renamed");
        assert_eq!(
            case["sql"], original["sql"],
            "{id}: original SQL text was edited"
        );
        assert_eq!(case["independent_signoff"], false);
        assert_eq!(case["release_approved"], false);
        assert_eq!(case["proven_native_terminal"], false);
        assert_eq!(case["proven_13_vendor_native_semantics"], false);
        let expected = match case["case_status"].as_str().expect("negative control kind") {
            "native_rejection" => {
                native["expected_native_rejections"]
                    .as_array()
                    .expect("original rejected syntax")
                    .iter()
                    .any(|x| x["id"] == id)
                    || expansion["native_rejected_cases"]
                        .as_array()
                        .expect("native negative cases")
                        .iter()
                        .any(|x| x == id)
            }
            "native_evaluation_error" => predicates["native_rejected_cases"]
                .as_array()
                .expect("native overflow")
                .iter()
                .any(|x| x["id"] == id),
            "stochastic_no_fixed_terminal" => native["stochastic_sampling_cases"]
                .as_array()
                .expect("sampling original IDs")
                .iter()
                .any(|x| x == id),
            other => panic!("{id}: unsupported boundary category {other}"),
        };
        assert!(
            expected,
            "{id}: native limitation lacks original-SQL evidence"
        );
    }
}

fn input_bag(db: &Connection, name: &str) -> Vec<Vec<Option<i64>>> {
    let mut stmt = db
        .prepare(&format!("SELECT id, amount FROM {name}"))
        .expect("two-column named source");
    let mut rows: Vec<Vec<Option<i64>>> = stmt
        .query_map([], |row| {
            Ok(vec![
                row.get::<_, Option<i64>>(0)?,
                row.get::<_, Option<i64>>(1)?,
            ])
        })
        .expect("complete source")
        .map(|row| row.expect("physical row"))
        .collect();
    rows.sort();
    rows
}

#[test]
fn native_overflow_and_mismatched_set_arity_reject_without_mutating_complete_sources() {
    let sources = parsed(RELATIONS);
    let cases = parsed(SOURCES);
    for id in ["88-C177-S033", "88-C177-S086"] {
        let db = Connection::open_in_memory().expect("native DuckDB");
        db.execute_batch(sources["source_setup"].as_str().expect("full input setup"))
            .expect("source with duplicates and nullable fields");
        for name in ["orders", "customers"] {
            let mut expected: Vec<Vec<Option<i64>>> =
                serde_json::from_value(sources["source_bags"][name].clone())
                    .expect("independent fixed source bag");
            expected.sort();
            assert_eq!(input_bag(&db, name), expected, "{name} before {id}");
        }
        let sql = cases["cases"]
            .as_array()
            .expect("original SQL")
            .iter()
            .find(|case| case["id"] == id)
            .expect("stable named invalid SQL")["sql"]
            .as_str()
            .expect("text");
        let result: duckdb::Result<()> = db.prepare(sql).and_then(|mut stmt| {
            let mut rows = stmt.query([])?;
            while rows.next()?.is_some() {}
            Ok(())
        });
        assert!(
            result.is_err(),
            "{id}: original SQL must remain a rejected/invalid terminal, never a fabricated output bag"
        );
        for name in ["orders", "customers"] {
            let mut expected: Vec<Vec<Option<i64>>> =
                serde_json::from_value(sources["source_bags"][name].clone())
                    .expect("independent source");
            expected.sort();
            assert_eq!(input_bag(&db, name), expected, "{name} after rejected {id}");
        }
    }
}
