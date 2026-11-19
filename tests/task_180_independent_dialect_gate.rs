//! TASK-180 final dialect gate inputs. Reconcile the independent owner reviews,
//! original SQL, adapter evidence, and native exceptions before claiming release
//! certification. Parser acceptance alone is never an Exact physical proof.

use std::collections::BTreeSet;

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_sql, dialect_from_name, LiteralValue, ProtocolStatement, SetMode, ValueDomain,
};

const FREEZE: &str = include_str!("../docs/task-180-acceptance-freeze.json");
const ORIGINAL: &str = include_str!("../docs/task-178-full-variant-inventory.json");
const ADAPTER: &str = include_str!("../docs/task-197-acceptance-ledger-v2.json");
const PREDICATES: &str = include_str!("../docs/task-177-predicate-oracles.json");
const EXCEPTIONS: &str = include_str!("../docs/task-191-independent-review-v4.json");
const MANIFEST: &str = include_str!("../docs/coverage-manifest.json");
const TASK: &str = include_str!(
    "../.backlog/tasks/task-180 - independently-certify-complete-dialect-matrix-and-gate-ci.md"
);

fn read(raw: &str) -> Value {
    serde_json::from_str(raw).expect("committed and versioned evidence")
}

#[test]
fn all_2119_original_dialect_obligations_have_an_independent_review_and_explicit_gate() {
    let freeze = read(FREEZE);
    let original = read(ORIGINAL);
    let adapter = read(ADAPTER);
    let exceptions = read(EXCEPTIONS);
    let manifest = read(MANIFEST);

    assert_eq!(freeze["schema_version"], 1);
    assert_eq!(freeze["task_id"], "TASK-180");
    assert_eq!(freeze["parent_task"], "TASK-88");
    assert_eq!(freeze["case_count"], 163);
    assert_eq!(freeze["cell_count"], 2119);
    assert_eq!(freeze["positive_fixture_id"], "88-C180-P");
    assert_eq!(freeze["negative_fixture_id"], "88-C180-N");
    assert_eq!(freeze["dialects"], original["dialects"]);
    assert_eq!(freeze["dialects"], adapter["dialects"]);
    assert_eq!(freeze["release_policy"]["all_mandatory_cells_finally_verified"], false);
    assert_eq!(freeze["release_policy"]["parent_88_signed_off"], false);
    assert_eq!(freeze["release_policy"]["task_91_release_approved"], false);

    let cases = freeze["cases"].as_array().expect("task-owned 163-case freeze");
    let source_cases = original["cases"].as_array().expect("original 163 SQLs");
    let adapter_cases = adapter["cases"].as_array().expect("reviewed adapter matrix");
    assert_eq!((cases.len(), source_cases.len(), adapter_cases.len()), (163, 163, 163));
    let mut ids = BTreeSet::new();
    let mut cells_seen = BTreeSet::new();
    let mut categories = [0_usize; 4];

    for (index, case) in cases.iter().enumerate() {
        let id = case["id"].as_str().expect("stable original fixture");
        assert!(ids.insert(id.to_string()), "{id}: duplicate original SQL");
        assert_eq!(case["id"], source_cases[index]["id"]);
        assert_eq!(case["id"], adapter_cases[index]["id"]);
        assert_eq!(case["sql"], source_cases[index]["sql"]);
        assert_eq!(case["sql"], adapter_cases[index]["sql"]);
        assert_eq!(case["feature_id"], source_cases[index]["feature_id"]);
        assert_eq!(case["owner"], adapter_cases[index]["owner"]);
        assert_eq!(case["source_relations"], adapter_cases[index]["source_relations_mentioned"]);
        assert_eq!(case["positive_fixture"], "88-C180-P");
        assert_eq!(case["negative_fixture"], "88-C180-N");
        assert!(case["typed_oracle"].is_object(), "{id}: missing independent typed oracle");
        assert!(case["owner_review_ref"]
            .as_str()
            .is_some_and(|reference| reference.starts_with("docs/task-")));
        let source_cells = source_cases[index]["dialect_cells"]
            .as_array()
            .expect("13 original parser outcomes");
        let reviewed_cells = adapter_cases[index]["cells"]
            .as_array()
            .expect("13 independent adapter decisions");
        let frozen_cells = case["cells"].as_array().expect("13 task-owned cells");
        assert_eq!(frozen_cells.len(), 13, "{id}: a dialect was skipped");
        assert_eq!(source_cells.len(), 13);
        assert_eq!(reviewed_cells.len(), 13);

        for (dialect_index, frozen) in frozen_cells.iter().enumerate() {
            let name = freeze["dialects"][dialect_index]
                .as_str()
                .expect("dialect");
            let key = format!("{id}/{name}");
            assert!(cells_seen.insert(key.clone()), "{key}: duplicated frozen cell");
            assert_eq!(frozen["id"], key);
            assert_eq!(frozen["dialect"], name);
            assert_eq!(frozen["dialect"], source_cells[dialect_index]["dialect"]);
            assert_eq!(frozen["dialect"], reviewed_cells[dialect_index]["dialect"]);
            assert_eq!(frozen["original_canonical_class"], source_cells[dialect_index]["observed_status"]);
            assert_eq!(frozen["original_canonical_class"], reviewed_cells[dialect_index]["original_observation"]);
            assert_eq!(frozen["independent_bounded_review"], reviewed_cells[dialect_index]["reviewed_semantic_disposition"]);
            assert_eq!(frozen["external_native_vendor"], "unattested");
            assert_eq!(reviewed_cells[dialect_index]["external_native_execution"], "unattested");

            let class = frozen["original_canonical_class"]
                .as_str()
                .expect("historical canonical observation");
            let (slot, required) = match class {
                "C" => (0, "requires_full_independent_canonical_and_physical_signoff"),
                "D" => (1, "divergent_canonical_requires_adjudication"),
                "U" => (2, "mandatory_unsupported_requires_implementation"),
                "R" => (3, "parser_rejection_requires_reviewed_disposition"),
                other => panic!("{key}: unknown canonical observation {other}"),
            };
            assert_eq!(frozen["release_gate"], required, "{key}: release claim upgraded");
            if matches!(class, "D" | "U" | "R") {
                assert_ne!(
                    reviewed_cells[dialect_index]["status"], "certified",
                    "{key}: an exceptional cell cannot be declared portable certified"
                );
            }
            categories[slot] += 1;
        }
    }
    assert_eq!(ids.len(), 163);
    assert_eq!(cells_seen.len(), 2119);
    assert_eq!(categories, [2014, 23, 55, 27]);
    for (class, n) in [("C", 2014), ("D", 23), ("U", 55), ("R", 27)] {
        assert_eq!(freeze["observed_class_counts"][class], n);
    }

    // The independent exception review is complete, not a waiver of 116
    // historical unsupported analyzer cells or external vendor execution.
    assert_eq!(exceptions["reviewed_public_unsupported_cells"], 116);
    assert_eq!(exceptions["native_non_duckdb_attested"], false);
    assert_eq!(exceptions["parent_task88_signed_off"], false);
    assert_eq!(manifest["task_191_boundary_disposition_acceptance"]["full_matrix_release_approved"], false);
    assert_eq!(manifest["task_180_complete_dialect_gate"]["complete_matrix_release_approved"], false);
    assert!(TASK.contains("status: In Progress"));
    assert!(!TASK.contains("status: Done"));
}

fn rows(db: &Connection, sql: &str, width: usize) -> Vec<Vec<Option<i64>>> {
    let mut statement = db.prepare(sql).expect("native DuckDB query");
    let records = statement
        .query_map([], |row| {
            (0..width)
                .map(|position| row.get::<usize, Option<i64>>(position))
                .collect::<Result<Vec<_>, _>>()
        })
        .expect("complete native bag");
    let mut bag: Vec<_> = records.map(|row| row.expect("native row")).collect();
    bag.sort();
    bag
}

fn expected_bag(case: &Value, field: &str) -> Vec<Vec<Option<i64>>> {
    let mut bag: Vec<Vec<Option<i64>>> =
        serde_json::from_value(case[field].clone()).expect("independent complete bag");
    bag.sort();
    bag
}

#[test]
fn native_duplicate_null_truth_and_typed_output_are_not_parser_only_evidence() {
    let matrix = read(FREEZE);
    let goldens = read(PREDICATES);
    let db = Connection::open_in_memory().expect("native DuckDB");
    db.execute_batch(goldens["source_setup"].as_str().expect("source SQL"))
        .expect("complete fixture setup");
    assert_eq!(
        rows(&db, "SELECT id, amount FROM orders", 2),
        expected_bag(&goldens, "orders_source_rows")
    );
    let mut tested = 0;
    for id in ["88-C177-S001", "88-C177-S002", "88-C177-S003"] {
        let oracle = goldens["cases"]
            .as_array()
            .expect("pinned true/false/unknown oracles")
            .iter()
            .find(|case| case["id"] == id)
            .expect("original SQL case");
        let frozen = matrix["cases"]
            .as_array()
            .expect("163 SQL cases")
            .iter()
            .find(|case| case["id"] == id)
            .expect("case owned by final gate");
        let sql = oracle["sql"].as_str().expect("original SQL");
        assert_eq!(sql, frozen["sql"].as_str().expect("frozen source SQL"));
        assert_eq!(
            rows(&db, sql, 1),
            expected_bag(oracle, "expected_terminal_bag"),
            "{id}: full terminal output includes duplicate occurrences"
        );

        let predicate = oracle["predicate"].as_str().expect("original predicate");
        let mut not_true = expected_bag(oracle, "expected_false_rows");
        not_true.extend(expected_bag(oracle, "expected_unknown_rows"));
        not_true.sort();
        assert_eq!(
            rows(&db, &format!("SELECT id, amount FROM orders WHERE ({predicate}) IS TRUE"), 2),
            expected_bag(oracle, "expected_selected_rows"),
            "{id}: TRUE bag"
        );
        assert_eq!(
            rows(&db, &format!("SELECT id, amount FROM orders WHERE ({predicate}) IS FALSE"), 2),
            expected_bag(oracle, "expected_false_rows"),
            "{id}: FALSE bag"
        );
        assert_eq!(
            rows(&db, &format!("SELECT id, amount FROM orders WHERE ({predicate}) IS UNKNOWN"), 2),
            expected_bag(oracle, "expected_unknown_rows"),
            "{id}: UNKNOWN bag"
        );
        assert_eq!(
            rows(&db, &format!("SELECT id, amount FROM orders WHERE ({predicate}) IS NOT TRUE"), 2),
            not_true,
            "{id}: rejected direction includes FALSE and UNKNOWN"
        );

        for cell in frozen["cells"].as_array().expect("13 parser dialects") {
            let name = cell["dialect"].as_str().expect("dialect");
            let dialect = dialect_from_name(name).expect("project parser");
            let protocol = analyze_sql(sql, name, dialect.as_ref())
                .unwrap_or_else(|error| panic!("{id}/{name}: {error}"));
            let [ProtocolStatement::Query(query)] = protocol.statements() else {
                panic!("{id}/{name}: source query lost its typed semantic output")
            };
            let [column] = query.output().columns() else {
                panic!("{id}/{name}: wrong terminal projection")
            };
            if id == "88-C177-S001" {
                let ValueDomain::Set(domain) = column.domain() else {
                    panic!("{id}/{name}: exact amount=10 set missing")
                };
                assert_eq!(domain.mode(), SetMode::Include);
                assert_eq!(domain.values().len(), 1);
                assert_eq!(domain.values()[0].value(), &LiteralValue::Number("10".into()));
            }
            tested += 1;
        }
    }
    assert_eq!(tested, 39);
}
