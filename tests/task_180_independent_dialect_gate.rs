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
    assert_eq!(
        freeze["release_policy"]["all_mandatory_cells_finally_verified"],
        false
    );
    assert_eq!(freeze["release_policy"]["parent_88_signed_off"], false);
    assert_eq!(freeze["release_policy"]["task_91_release_approved"], false);

    let cases = freeze["cases"]
        .as_array()
        .expect("task-owned 163-case freeze");
    let source_cases = original["cases"].as_array().expect("original 163 SQLs");
    let adapter_cases = adapter["cases"]
        .as_array()
        .expect("reviewed adapter matrix");
    assert_eq!(
        (cases.len(), source_cases.len(), adapter_cases.len()),
        (163, 163, 163)
    );
    let mut ids = BTreeSet::new();
    let mut cells_seen = BTreeSet::new();
    let mut categories = [0_usize; 4];

    for case in cases {
        let id = case["id"].as_str().expect("stable original fixture");
        assert!(ids.insert(id.to_string()), "{id}: duplicate original SQL");
        // The original inventory and adapter ledger have different ordering.
        // Stable SQL identities, not array positions, define reconciliation.
        let source_case = source_cases
            .iter()
            .find(|entry| entry["id"] == id)
            .expect("original SQL identity");
        let adapter_case = adapter_cases
            .iter()
            .find(|entry| entry["id"] == id)
            .expect("reviewed adapter identity");
        assert_eq!(case["sql"], source_case["sql"]);
        assert_eq!(case["sql"], adapter_case["sql"]);
        assert_eq!(case["feature_id"], source_case["feature_id"]);
        assert_eq!(case["owner"], adapter_case["owner"]);
        assert_eq!(
            case["source_relations"],
            adapter_case["source_relations_mentioned"]
        );
        assert_eq!(case["positive_fixture"], "88-C180-P");
        assert_eq!(case["negative_fixture"], "88-C180-N");
        assert!(
            case["typed_oracle"].is_object(),
            "{id}: missing independent typed oracle"
        );
        assert!(case["owner_review_ref"]
            .as_str()
            .is_some_and(|reference| reference.starts_with("docs/task-")));
        let source_cells = source_case["dialect_cells"]
            .as_array()
            .expect("13 original parser outcomes");
        let reviewed_cells = adapter_case["cells"]
            .as_array()
            .expect("13 independent adapter decisions");
        let frozen_cells = case["cells"].as_array().expect("13 task-owned cells");
        assert_eq!(frozen_cells.len(), 13, "{id}: a dialect was skipped");
        assert_eq!(source_cells.len(), 13);
        assert_eq!(reviewed_cells.len(), 13);

        for (dialect_index, frozen) in frozen_cells.iter().enumerate() {
            let name = freeze["dialects"][dialect_index].as_str().expect("dialect");
            let key = format!("{id}/{name}");
            assert!(
                cells_seen.insert(key.clone()),
                "{key}: duplicated frozen cell"
            );
            assert_eq!(frozen["id"], key);
            assert_eq!(frozen["dialect"], name);
            assert_eq!(frozen["dialect"], source_cells[dialect_index]["dialect"]);
            assert_eq!(frozen["dialect"], reviewed_cells[dialect_index]["dialect"]);
            assert_eq!(
                frozen["original_canonical_class"],
                source_cells[dialect_index]["observed_status"]
            );
            assert_eq!(
                frozen["original_canonical_class"],
                reviewed_cells[dialect_index]["original_observation"]
            );
            assert_eq!(
                frozen["independent_bounded_review"],
                reviewed_cells[dialect_index]["reviewed_semantic_disposition"]
            );
            assert_eq!(frozen["external_native_vendor"], "unattested");
            assert_eq!(
                reviewed_cells[dialect_index]["external_native_execution"],
                "unattested"
            );

            let class = frozen["original_canonical_class"]
                .as_str()
                .expect("historical canonical observation");
            let (slot, required) = match class {
                "C" => (
                    0,
                    "requires_full_independent_canonical_and_physical_signoff",
                ),
                "D" => (1, "divergent_canonical_requires_adjudication"),
                "U" => (2, "mandatory_unsupported_requires_implementation"),
                "R" => (3, "parser_rejection_requires_reviewed_disposition"),
                other => panic!("{key}: unknown canonical observation {other}"),
            };
            assert_eq!(
                frozen["release_gate"], required,
                "{key}: release claim upgraded"
            );
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
    assert_eq!(
        manifest["task_191_boundary_disposition_acceptance"]["full_matrix_release_approved"],
        false
    );
    assert_eq!(
        manifest["task_180_complete_dialect_gate"]["complete_matrix_release_approved"],
        false
    );
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

fn sound_numeric_domain(domain: &ValueDomain, value: Option<i64>) -> bool {
    match domain {
        ValueDomain::Unbounded | ValueDomain::Unknown(_) => true,
        ValueDomain::Empty => false,
        ValueDomain::Set(values) => {
            let literal = match value {
                Some(number) => LiteralValue::Number(number.to_string()),
                None => LiteralValue::Null,
            };
            let present = values.values().iter().any(|entry| entry.value() == &literal);
            match values.mode() {
                SetMode::Include => present,
                SetMode::Exclude => !present,
            }
        }
        ValueDomain::Ranges(values) => {
            let Some(number) = value.map(i128::from) else {
                return false;
            };
            values.ranges().iter().any(|interval| {
                let lower = interval.lower().is_none_or(|bound| {
                    let LiteralValue::Number(raw) = bound.value().value() else {
                        return false;
                    };
                    raw.parse::<i128>().is_ok_and(|edge| {
                        number > edge || (number == edge && bound.inclusive())
                    })
                });
                let upper = interval.upper().is_none_or(|bound| {
                    let LiteralValue::Number(raw) = bound.value().value() else {
                        return false;
                    };
                    raw.parse::<i128>().is_ok_and(|edge| {
                        number < edge || (number == edge && bound.inclusive())
                    })
                });
                lower && upper
            })
        }
        // A new, unexamined domain variant is not an automatic certificate.
        _ => false,
    }
}

fn check_exact_bound(
    actual: Option<&sql_semantic_protocol::IntervalBound>,
    expected: &Value,
    id: &str,
) {
    if expected.is_null() {
        assert!(actual.is_none(), "{id}: unexpected numeric interval bound");
        return;
    }
    let parts = expected.as_array().expect("numeric bound + inclusivity");
    let bound = actual.expect("missing exact bound");
    let expected_number = parts[0].as_i64().expect("integer bound");
    assert_eq!(
        bound.value().value(),
        &LiteralValue::Number(expected_number.to_string()),
        "{id}: wrong typed bound"
    );
    assert_eq!(
        bound.inclusive(),
        parts[1].as_bool().expect("inclusive flag"),
        "{id}: inclusive/exclusive law changed"
    );
}

fn check_exact_projected_domain(domain: &ValueDomain, expected: &Value, id: &str) {
    match expected["kind"].as_str().expect("explicit domain kind") {
        "set_include" | "set_exclude" => {
            let ValueDomain::Set(set) = domain else {
                panic!("{id}: an exact set must not become unknown");
            };
            let mode = if expected["kind"] == "set_include" {
                SetMode::Include
            } else {
                SetMode::Exclude
            };
            assert_eq!(set.mode(), mode, "{id}: set inclusion direction changed");
            let values = expected["values"].as_array().expect("explicit numeric set");
            assert_eq!(set.values().len(), values.len(), "{id}: missing set entry");
            for item in values {
                let literal = match item.as_i64() {
                    Some(number) => LiteralValue::Number(number.to_string()),
                    None if item.is_null() => LiteralValue::Null,
                    None => panic!("{id}: invalid source expected value"),
                };
                assert!(
                    set.values().iter().any(|entry| entry.value() == &literal),
                    "{id}: missing typed set value {literal:?}"
                );
            }
        }
        "range" | "range_union" => {
            let ValueDomain::Ranges(ranges) = domain else {
                panic!("{id}: an exact numeric range must not become unknown");
            };
            let expected_ranges = if expected["kind"] == "range" {
                vec![expected]
            } else {
                expected["ranges"]
                    .as_array()
                    .expect("exact union of ranges")
                    .iter()
                    .collect()
            };
            assert_eq!(
                ranges.ranges().len(),
                expected_ranges.len(),
                "{id}: interval union cardinality changed"
            );
            for (actual, expected_range) in ranges.ranges().iter().zip(expected_ranges) {
                check_exact_bound(actual.lower(), &expected_range["lower"], id);
                check_exact_bound(actual.upper(), &expected_range["upper"], id);
            }
        }
        other => panic!("{id}: unrecognized frozen domain kind {other}"),
    }
}

#[test]
fn native_complete_source_bags_truth_partitions_and_typed_domains_are_executed_for_all_25() {
    let matrix = read(FREEZE);
    let goldens = read(PREDICATES);
    let db = Connection::open_in_memory().expect("native DuckDB");
    db.execute_batch(goldens["source_setup"].as_str().expect("source SQL"))
        .expect("complete fixture setup");
    assert_eq!(
        rows(&db, "SELECT id, amount FROM orders", 2),
        expected_bag(&goldens, "orders_source_rows")
    );
    assert_eq!(
        rows(&db, "SELECT id, amount FROM customers", 2),
        expected_bag(&goldens, "customers_source_rows")
    );

    let mut cases = BTreeSet::new();
    let mut dialect_cells = 0;
    let mut sound_terminal_values = 0;
    for oracle in goldens["cases"].as_array().expect("25 original oracles") {
        let id = oracle["id"].as_str().expect("original SQL identity");
        assert!(cases.insert(id), "{id}: duplicate original native oracle");
        let frozen = matrix["cases"]
            .as_array()
            .expect("163 SQL cases")
            .iter()
            .find(|case| case["id"] == id)
            .expect("case owned by final gate");
        let sql = oracle["sql"].as_str().expect("original SQL");
        assert_eq!(sql, frozen["sql"].as_str().expect("frozen source SQL"));
        let complete_output = expected_bag(oracle, "expected_terminal_bag");
        assert_eq!(rows(&db, sql, 1), complete_output, "{id}: full terminal bag");

        let predicate = oracle["predicate"].as_str().expect("original predicate");
        let source = if id == "88-C177-S013" { "orders o" } else { "orders" };
        let selection = if id == "88-C177-S013" { "o.id, o.amount" } else { "id, amount" };
        let mut not_true = expected_bag(oracle, "expected_false_rows");
        not_true.extend(expected_bag(oracle, "expected_unknown_rows"));
        not_true.sort();
        for (truth, expectation) in [
            ("IS TRUE", "expected_selected_rows"),
            ("IS FALSE", "expected_false_rows"),
            ("IS UNKNOWN", "expected_unknown_rows"),
        ] {
            let sql = format!(
                "SELECT {selection} FROM {source} WHERE ({predicate}) {truth}"
            );
            assert_eq!(
                rows(&db, &sql, 2),
                expected_bag(oracle, expectation),
                "{id}: independently frozen {truth} bag"
            );
        }
        assert_eq!(
            rows(
                &db,
                &format!("SELECT {selection} FROM {source} WHERE ({predicate}) IS NOT TRUE"),
                2
            ),
            not_true,
            "{id}: FALSE and UNKNOWN must be rejected without losing occurrences"
        );

        for cell in frozen["cells"].as_array().expect("13 dialect parser cells") {
            let name = cell["dialect"].as_str().expect("dialect");
            let dialect = dialect_from_name(name).expect("project dialect");
            let analyzed = analyze_sql(sql, name, dialect.as_ref());
            let protocol = match analyzed {
                Ok(value) => value,
                Err(sql_semantic_protocol::Error::Parse(_))
                    if cell["original_canonical_class"] == "R" =>
                {
                    dialect_cells += 1;
                    continue;
                }
                Err(error) => panic!("{id}/{name}: unexpected source error: {error}"),
            };
            match protocol.statements() {
                [ProtocolStatement::Query(query)] => {
                    let [column] = query.output().columns() else {
                        panic!("{id}/{name}: original one-column output lost");
                    };
                    for value in &complete_output {
                        let integer = value[0];
                        assert!(
                            sound_numeric_domain(column.domain(), integer),
                            "{id}/{name}: native terminal value {integer:?} contradicted by emitted domain {:?}",
                            column.domain()
                        );
                        sound_terminal_values += 1;
                    }
                    if !oracle["expected_projected_domain"].is_null() {
                        check_exact_projected_domain(
                            column.domain(),
                            &oracle["expected_projected_domain"],
                            id,
                        );
                    }
                }
                [ProtocolStatement::Unsupported(_)]
                    if cell["original_canonical_class"] == "U" => {}
                _ => panic!("{id}/{name}: original terminal is missing or unsupported without an approved boundary"),
            }
            dialect_cells += 1;
        }
    }
    assert_eq!(cases.len(), 25);
    assert_eq!(dialect_cells, 325, "every original predicate x 13 public dialects");
    assert!(sound_terminal_values > 1000, "verify full output occurrences, not a count-only oracle");
}
