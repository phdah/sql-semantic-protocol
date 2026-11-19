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
const JOINS: &str = include_str!("../docs/task-193-acceptance-freeze.json");
const SETS: &str = include_str!("../docs/task-194-acceptance-freeze.json");
const SET_BAGS: &str = include_str!("../docs/task-194-independent-bag-review.json");
const WINDOW: &str = include_str!("../docs/task-195-acceptance-freeze.json");
const EXCEPTIONS: &str = include_str!("../docs/task-191-independent-review-v4.json");
const MANIFEST: &str = include_str!("../docs/coverage-manifest.json");
const SIGNOFF: &str = include_str!("../docs/task-180-final-signoff-evidence.json");
const VENDOR: &str = include_str!("../docs/task-179-variant-matrix-v2.json");
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
    let signoff = read(SIGNOFF);

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
        signoff["all_required_independent_reviews_approved"]
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
        signoff["task180_signed_off"]
    );
    if signoff["task180_signed_off"] == true {
        assert!(TASK.contains("status: Done"));
    } else {
        assert!(TASK.contains("status: In Progress"));
        assert!(!TASK.contains("status: Done"));
    }
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
            let present = values
                .values()
                .iter()
                .any(|entry| entry.value() == &literal);
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
                    raw.parse::<i128>()
                        .is_ok_and(|edge| number > edge || (number == edge && bound.inclusive()))
                });
                let upper = interval.upper().is_none_or(|bound| {
                    let LiteralValue::Number(raw) = bound.value().value() else {
                        return false;
                    };
                    raw.parse::<i128>()
                        .is_ok_and(|edge| number < edge || (number == edge && bound.inclusive()))
                });
                lower && upper
            })
        }
        // A new, unexamined domain variant is not an automatic certificate.
        _ => false,
    }
}

fn check_exact_bound(actual: Option<&sql_semantic_protocol::Bound>, expected: &Value, id: &str) {
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
        assert_eq!(
            rows(&db, sql, 1),
            complete_output,
            "{id}: full terminal bag"
        );

        let predicate = oracle["predicate"].as_str().expect("original predicate");
        let source = if id == "88-C177-S013" {
            "orders o"
        } else {
            "orders"
        };
        let selection = if id == "88-C177-S013" {
            "o.id, o.amount"
        } else {
            "id, amount"
        };
        let mut not_true = expected_bag(oracle, "expected_false_rows");
        not_true.extend(expected_bag(oracle, "expected_unknown_rows"));
        not_true.sort();
        for (truth, expectation) in [
            ("IS TRUE", "expected_selected_rows"),
            ("IS FALSE", "expected_false_rows"),
            ("IS UNKNOWN", "expected_unknown_rows"),
        ] {
            let sql = format!("SELECT {selection} FROM {source} WHERE ({predicate}) {truth}");
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
    assert_eq!(
        dialect_cells, 325,
        "every original predicate x 13 public dialects"
    );
    assert!(
        sound_terminal_values > 1000,
        "verify full output occurrences, not a count-only oracle"
    );
}

fn verify_native_public_output(sql: &str, expected: &[Vec<Option<i64>>], width: usize, id: &str) {
    let parser = dialect_from_name("duckdb").expect("native dialect parser");
    let protocol = analyze_sql(sql, "duckdb", parser.as_ref())
        .unwrap_or_else(|error| panic!("{id}: public original SQL rejected: {error}"));
    let [ProtocolStatement::Query(query)] = protocol.statements() else {
        panic!("{id}: an original supported query became silently unsupported");
    };
    assert_eq!(
        query.output().columns().len(),
        width,
        "{id}: original terminal column arity"
    );
    for row in expected {
        for (column, value) in query.output().columns().iter().zip(row) {
            assert!(
                sound_numeric_domain(column.domain(), *value),
                "{id}: public output domain excludes actual native {value:?} for {}",
                column.name()
            );
        }
    }
}

#[test]
fn all_24_original_join_source_populations_have_exact_native_terminal_bags() {
    let freeze = read(JOINS);
    let db = Connection::open_in_memory().expect("native DuckDB");
    db.execute_batch(freeze["source_setup"].as_str().expect("closed source SQL"))
        .expect("native source population");
    assert_eq!(
        rows(&db, "SELECT id, amount FROM orders", 2),
        expected_bag(&freeze["complete_source_bags"], "orders"),
        "source rows include physical duplicates and NULL"
    );
    assert_eq!(
        rows(&db, "SELECT id, amount FROM customers", 2),
        expected_bag(&freeze["complete_source_bags"], "customers"),
        "join partners include physical duplicate keys"
    );

    let mut visited = BTreeSet::new();
    for case in freeze["cases"].as_array().expect("24 original join cases") {
        let id = case["id"].as_str().expect("stable SQL ID");
        assert!(visited.insert(id), "{id}: duplicate join oracle");
        let original = case["sql"].as_str().expect("original SQL");
        let executable = case["native_duckdb_sql"]
            .as_str()
            .expect("reviewed native SQL");
        if case["original_duckdb_syntax_boundary"]
            .as_str()
            .is_some_and(|value| value.contains("rejected_by_duckdb_parser"))
        {
            assert!(
                db.prepare(original).is_err(),
                "{id}: original vendor syntax must not be silently certified as DuckDB"
            );
        }
        let count = case["expected_columns"].as_u64().expect("terminal arity") as usize;
        let oracle = expected_bag(case, "expected_terminal_bag");
        assert_eq!(
            rows(&db, executable, count),
            oracle,
            "{id}: complete join output, NULL extension and duplicate multiplicities"
        );
        verify_native_public_output(original, &oracle, count, id);
    }
    assert_eq!(visited.len(), 24);
}

#[test]
fn all_20_native_set_bags_and_rejected_candidate_occurrences_are_replayed() {
    let freeze = read(SETS);
    let independent = read(SET_BAGS);
    let db = Connection::open_in_memory().expect("native DuckDB");
    db.execute_batch(
        freeze["source_setup"]
            .as_str()
            .expect("source population SQL"),
    )
    .expect("complete source populations");
    assert_eq!(
        rows(&db, "SELECT id, amount FROM orders", 2),
        expected_bag(&freeze["complete_source_bags"], "orders")
    );
    assert_eq!(
        rows(&db, "SELECT id, amount FROM customers", 2),
        expected_bag(&freeze["complete_source_bags"], "customers")
    );

    let mut executable = 0;
    let mut original_boundaries = 0;
    for case in freeze["cases"]
        .as_array()
        .expect("22 original set variants")
    {
        let id = case["id"].as_str().expect("stable original set ID");
        let source = independent["cases"]
            .as_array()
            .expect("separately reviewed source and candidate bags")
            .iter()
            .find(|item| item["case_id"] == id)
            .expect("source and output ledger entry");
        let Some(native_sql) = case["native_duckdb_sql"].as_str() else {
            assert!(
                matches!(id, "88-C177-S084" | "88-C177-S086"),
                "{id}: unreviewed native syntax exclusion"
            );
            assert!(case["expected_failure"].as_str().is_some());
            original_boundaries += 1;
            continue;
        };
        let width = case["expected_output_columns"]
            .as_u64()
            .expect("native output width") as usize;
        let expected = expected_bag(case, "expected_terminal_bag");
        assert_eq!(
            rows(&db, native_sql, width),
            expected,
            "{id}: source-accurate output tuple multiset"
        );
        assert_eq!(
            expected,
            expected_bag(source, "independent_terminal_bag"),
            "{id}: native output must match the separate independent review"
        );
        let candidate_sql = source["candidate_sql"]
            .as_str()
            .expect("candidate source population SQL");
        let candidates = expected_bag(source, "independent_source_candidate_bag");
        assert_eq!(
            rows(&db, candidate_sql, width),
            candidates,
            "{id}: complete source candidate multiset"
        );
        let mut partition = expected.clone();
        partition.extend(expected_bag(
            source,
            "independent_non_emitted_candidate_bag",
        ));
        partition.sort();
        assert_eq!(
            partition, candidates,
            "{id}: every unmatched/rejected source candidate is accounted for"
        );
        verify_native_public_output(
            case["sql"].as_str().expect("original SQL"),
            &expected,
            width,
            id,
        );
        executable += 1;
    }
    assert_eq!(executable, 20);
    assert_eq!(original_boundaries, 2);
}

#[test]
fn all_28_deterministic_native_group_window_and_ordered_bags_are_replayed() {
    let freeze = read(WINDOW);
    let db = Connection::open_in_memory().expect("native DuckDB");
    db.execute_batch(freeze["source_setup"].as_str().expect("physical input SQL"))
        .expect("complete physical source bags");
    assert_eq!(
        rows(&db, "SELECT id, amount FROM orders", 2),
        expected_bag(&freeze["complete_source_bags"], "orders")
    );
    assert_eq!(
        rows(&db, "SELECT id, amount FROM customers", 2),
        expected_bag(&freeze["complete_source_bags"], "customers")
    );

    let mut integer_bags = 0;
    let mut floating_bags = 0;
    let mut conditional_cases = 0;
    for case in freeze["cases"]
        .as_array()
        .expect("36 original window/group/order cases")
    {
        let id = case["id"].as_str().expect("original SQL identity");
        let sql = case["sql"].as_str().expect("original SQL");
        let native = &case["native_oracle"];
        match native["kind"].as_str().expect("reviewed native law kind") {
            "prior_original_integer_bag"
            | "original_integer_bag"
            | "unordered_original_source_bag" => {
                let width = native["columns"].as_u64().expect("projection width") as usize;
                let expected = expected_bag(native, "expected_terminal_bag");
                assert_eq!(
                    rows(&db, sql, width),
                    expected,
                    "{id}: original native terminal bag, including NULL and duplicates"
                );
                verify_native_public_output(sql, &expected, width, id);
                integer_bags += 1;
            }
            "original_float_bag" => {
                let expected = native["expected_terminal_bag"]
                    .as_array()
                    .expect("original float rows");
                assert_eq!(native["columns"], 1, "{id}: float oracle width");
                let mut statement = db.prepare(sql).expect("original native float SQL");
                let native_rows = statement
                    .query_map([], |row| row.get::<usize, Option<f64>>(0))
                    .expect("native floating outputs");
                let mut actual: Vec<_> = native_rows
                    .map(|row| row.expect("native float value"))
                    .collect();
                actual.sort_by(|a, b| a.partial_cmp(b).expect("finite native float fixtures"));
                let mut expected_values: Vec<Option<f64>> = expected
                    .iter()
                    .map(|row| {
                        let value = &row[0];
                        if value.is_null() {
                            None
                        } else {
                            Some(value.as_f64().expect("literal expected fraction"))
                        }
                    })
                    .collect();
                expected_values
                    .sort_by(|a, b| a.partial_cmp(b).expect("finite frozen float fixtures"));
                assert_eq!(
                    actual.len(),
                    expected_values.len(),
                    "{id}: output multiplicity"
                );
                for (observed, wanted) in actual.iter().zip(expected_values.iter()) {
                    match (observed, wanted) {
                        (Some(a), Some(b)) => assert!(
                            (a - b).abs() <= 1e-9,
                            "{id}: native window or aggregate scalar {a} != {b}"
                        ),
                        (None, None) => {}
                        _ => panic!("{id}: native SQL NULL result differed from frozen oracle"),
                    }
                }
                floating_bags += 1;
            }
            "native_parser_rejection" | "unstable_tie_membership" | "stochastic_conditional" => {
                // These have no single deterministic source-to-terminal bag
                // under the original SQL law; claiming one would be unsound.
                assert!(native["expected_terminal_bag"].is_null());
                conditional_cases += 1;
            }
            other => panic!("{id}: unreviewed native oracle classification {other}"),
        }
    }
    assert_eq!((integer_bags, floating_bags, conditional_cases), (25, 3, 8));
}

// A final approval must reference the actual frozen source, owner and dialect.
// Free-text evidence descriptions alone are not an independent review.
fn reviewed_cell_matches_frozen_source(review: &Value, case: &Value, cell: &Value) -> bool {
    let provenance_matches = [
        ("id", &cell["id"]),
        ("dialect", &cell["dialect"]),
        ("source_case_id", &case["id"]),
        ("original_sql", &case["sql"]),
        ("feature_id", &case["feature_id"]),
        ("semantic_owner", &case["owner"]),
        ("owner_review_ref", &case["owner_review_ref"]),
        ("adapter_review_ref", &case["adapter_review_ref"]),
        (
            "original_canonical_class",
            &cell["original_canonical_class"],
        ),
    ]
    .iter()
    .all(|(field, expected)| &review[*field] == *expected);
    let class = cell["original_canonical_class"].as_str();
    let status = review["proof_status"].as_str();
    let exceptional_proof = review["exception_resolution_ref"]
        .as_str()
        .is_some_and(|reference| !reference.trim().is_empty());

    provenance_matches
        // Rejected SQL cannot acquire an Exact physical claim just by filling
        // free-text signoff fields.
        && !(class == Some("R") && status == Some("Exact"))
        // Historical Unsupported or dialect-drift observations require a
        // specific independent resolution when promoted beyond Residual.
        && (class == Some("C")
            || status == Some("reviewed_conditional_residual")
            || exceptional_proof)
}

#[test]
fn independently_reviewed_cell_provenance_cannot_be_substituted_or_upgraded() {
    let freeze = read(FREEZE);
    let cases = freeze["cases"].as_array().expect("frozen original SQL");
    let case = &cases[0];
    let cell = &case["cells"][0];
    assert_eq!(cell["original_canonical_class"], "C");
    let mut review = serde_json::json!({
        "id": cell["id"],
        "dialect": cell["dialect"],
        "source_case_id": case["id"],
        "original_sql": case["sql"],
        "feature_id": case["feature_id"],
        "semantic_owner": case["owner"],
        "owner_review_ref": case["owner_review_ref"],
        "adapter_review_ref": case["adapter_review_ref"],
        "original_canonical_class": cell["original_canonical_class"],
        "proof_status": "Exact",
    });
    assert!(reviewed_cell_matches_frozen_source(&review, case, cell));

    for field in [
        "id",
        "dialect",
        "source_case_id",
        "original_sql",
        "feature_id",
        "semantic_owner",
        "owner_review_ref",
        "adapter_review_ref",
        "original_canonical_class",
    ] {
        let original = review[field].clone();
        review[field] = serde_json::json!("substituted evidence");
        assert!(
            !reviewed_cell_matches_frozen_source(&review, case, cell),
            "{field}: a different SQL/case/owner must not pass signoff"
        );
        review[field] = original;
    }

    let rejected = cases
        .iter()
        .flat_map(|case| {
            case["cells"]
                .as_array()
                .expect("13 parser cells")
                .iter()
                .map(move |cell| (case, cell))
        })
        .find(|(_, cell)| cell["original_canonical_class"] == "R")
        .expect("original parser rejection");
    let (rejected_case, rejected_cell) = rejected;
    for field in [
        ("id", &rejected_cell["id"]),
        ("dialect", &rejected_cell["dialect"]),
        ("source_case_id", &rejected_case["id"]),
        ("original_sql", &rejected_case["sql"]),
        ("feature_id", &rejected_case["feature_id"]),
        ("semantic_owner", &rejected_case["owner"]),
        ("owner_review_ref", &rejected_case["owner_review_ref"]),
        ("adapter_review_ref", &rejected_case["adapter_review_ref"]),
        (
            "original_canonical_class",
            &rejected_cell["original_canonical_class"],
        ),
    ] {
        review[field.0] = field.1.clone();
    }
    review["exception_resolution_ref"] = serde_json::json!("docs/some-independent-review.json");
    assert!(
        !reviewed_cell_matches_frozen_source(&review, rejected_case, rejected_cell),
        "a parser-rejected original cannot be independently signed Exact"
    );
    review["proof_status"] = serde_json::json!("reviewed_conditional_residual");
    assert!(reviewed_cell_matches_frozen_source(
        &review,
        rejected_case,
        rejected_cell
    ));

    let unsupported = cases
        .iter()
        .flat_map(|case| {
            case["cells"]
                .as_array()
                .expect("13 parser cells")
                .iter()
                .map(move |cell| (case, cell))
        })
        .find(|(_, cell)| cell["original_canonical_class"] == "U")
        .expect("original analyzer Unsupported");
    let (unsupported_case, unsupported_cell) = unsupported;
    for (field, value) in [
        ("id", &unsupported_cell["id"]),
        ("dialect", &unsupported_cell["dialect"]),
        ("source_case_id", &unsupported_case["id"]),
        ("original_sql", &unsupported_case["sql"]),
        ("feature_id", &unsupported_case["feature_id"]),
        ("semantic_owner", &unsupported_case["owner"]),
        ("owner_review_ref", &unsupported_case["owner_review_ref"]),
        (
            "adapter_review_ref",
            &unsupported_case["adapter_review_ref"],
        ),
        (
            "original_canonical_class",
            &unsupported_cell["original_canonical_class"],
        ),
    ] {
        review[field] = value.clone();
    }
    review["proof_status"] = serde_json::json!("Exact");
    review["exception_resolution_ref"] = serde_json::Value::Null;
    assert!(
        !reviewed_cell_matches_frozen_source(&review, unsupported_case, unsupported_cell),
        "unsupported original SQL needs a separately attested semantic resolution"
    );
}

#[test]
fn acceptance_cannot_be_marked_done_without_per_cell_independent_evidence() {
    let signoff = read(SIGNOFF);
    let freeze = read(FREEZE);
    let manifest = read(MANIFEST);
    assert_eq!(signoff["schema_version"], 1);
    assert_eq!(signoff["task_id"], "TASK-180");
    assert_eq!(signoff["required_original_cases"], 163);
    assert_eq!(signoff["required_dialect_cells"], 2119);

    let expected: BTreeSet<String> = freeze["cases"]
        .as_array()
        .expect("complete original variants")
        .iter()
        .flat_map(|case| case["cells"].as_array().expect("all dialect cells"))
        .map(|cell| {
            cell["id"]
                .as_str()
                .expect("original case/dialect identity")
                .to_owned()
        })
        .collect();
    assert_eq!(expected.len(), 2119);

    let approvals = signoff["approved_cells"]
        .as_array()
        .expect("individual independently reviewed cells");
    let mut observed = BTreeSet::new();
    for review in approvals {
        let id = review["id"].as_str().expect("reviewed source case/dialect");
        assert!(
            expected.contains(id),
            "{id}: an extra review cannot replace a required cell"
        );
        let (source_case, source_cell) = freeze["cases"]
            .as_array()
            .expect("original SQL cases")
            .iter()
            .find_map(|case| {
                case["cells"]
                    .as_array()
                    .expect("original dialect cells")
                    .iter()
                    .find(|cell| cell["id"] == id)
                    .map(|cell| (case, cell))
            })
            .expect("every approval belongs to the frozen matrix");
        assert!(
            reviewed_cell_matches_frozen_source(review, source_case, source_cell),
            "{id}: signoff cannot borrow another SQL, dialect, owner or historical parser class; unproved exceptions remain Residual"
        );
        assert!(
            observed.insert(id.to_owned()),
            "{id}: duplicate release review"
        );
        for field in [
            "reviewer",
            "reviewed_commit",
            "native_source_and_terminal_oracle",
            "typed_domain_and_null_bounds",
            "negative_or_impossible_direction",
            "sql_dbt_odcs_evidence",
            "vendor_session_disposition",
        ] {
            assert!(
                review[field]
                    .as_str()
                    .is_some_and(|value| !value.trim().is_empty()),
                "{id}: independent review is missing {field}"
            );
        }
        assert!(
            matches!(
                review["proof_status"].as_str(),
                Some("Exact" | "Impossible" | "reviewed_conditional_residual")
            ),
            "{id}: every final classification needs an explicit physical proof status"
        );
        assert!(
            !review["native_execution_attested"]
                .as_bool()
                .unwrap_or(false)
                || review["native_execution_evidence"]
                    .as_str()
                    .is_some_and(|evidence| !evidence.trim().is_empty()),
            "{id}: vendor runtime cannot be certified without execution"
        );
    }

    let done = signoff["task180_signed_off"]
        .as_bool()
        .expect("explicit independent acceptance decision");
    let manifest_done = manifest["task_180_complete_dialect_gate"]
        ["complete_matrix_release_approved"]
        .as_bool()
        .expect("explicit canonical release gate");
    assert_eq!(done, manifest_done);
    assert_eq!(
        manifest["task_180_complete_dialect_gate"]["all_mandatory_cells_signed_off"],
        signoff["all_required_independent_reviews_approved"]
    );
    assert_eq!(
        signoff["all_required_independent_reviews_approved"], done,
        "task signoff needs a complete independent cell review"
    );
    if done {
        assert_eq!(
            observed, expected,
            "every mandatory cell needs independent approval"
        );
        for field in ["independent_reviewer", "reviewed_code_commit"] {
            assert!(signoff[field].as_str().is_some_and(|s| !s.is_empty()));
        }
        let verify = &signoff["verification"];
        for field in [
            "protocol_exact_head_ci",
            "pinned_sql_tdg_consumer_ci",
            "pinned_consumer_commit",
        ] {
            assert!(verify[field].as_str().is_some_and(|s| !s.is_empty()));
        }
        assert_eq!(verify["protocol_ci_jobs_passed"], 9);
        assert_eq!(verify["dbt_e2e_verified"], true);
        assert_eq!(verify["active_schema_golden_verified"], true);
        assert_eq!(
            signoff["vendor_boundary_review"]["reviewed_conditions"], 481,
            "every reviewed vendor/session cell needs a final release disposition"
        );
        assert!(signoff["vendor_boundary_review"]["independent_owner"]
            .as_str()
            .is_some_and(|s| !s.is_empty()));
        let vendor = read(VENDOR);
        let required_vendor: BTreeSet<_> = vendor["cases"]
            .as_array()
            .expect("37 original vendor/session laws")
            .iter()
            .flat_map(|case| {
                let id = case["id"].as_str().expect("original vendor law ID");
                case["dialect_cells"]
                    .as_array()
                    .expect("13 named vendor dialects")
                    .iter()
                    .map(move |cell| {
                        format!("{id}/{}", cell["dialect"].as_str().expect("vendor dialect"))
                    })
            })
            .collect();
        assert_eq!(required_vendor.len(), 481);
        let mut reviewed_vendor = BTreeSet::new();
        for record in signoff["vendor_boundary_review"]["approved_dispositions"]
            .as_array()
            .expect("independent individual vendor-law dispositions")
        {
            let id = record["id"].as_str().expect("reviewed vendor law/dialect");
            assert!(
                reviewed_vendor.insert(id.to_owned()),
                "{id}: duplicate vendor signoff"
            );
            assert!(
                required_vendor.contains(id),
                "{id}: not an original vendor-law cell"
            );
            assert!(record["reviewer"].as_str().is_some_and(|v| !v.is_empty()));
            assert!(record["reviewed_law_and_session"]
                .as_str()
                .is_some_and(|v| !v.is_empty()));
            assert!(matches!(
                record["release_disposition"].as_str(),
                Some("approved_explicit_residual" | "native_execution_attested")
            ));
            if record["release_disposition"] == "native_execution_attested" {
                assert!(record["native_evidence_url"]
                    .as_str()
                    .is_some_and(|evidence| !evidence.is_empty()));
            }
        }
        assert_eq!(
            reviewed_vendor, required_vendor,
            "all 481 vendor laws need individually reviewed release dispositions"
        );
        assert!(TASK.contains("status: Done"));
    } else {
        assert!(
            observed.len() < expected.len(),
            "all cells already have evidence but no signoff"
        );
        assert!(
            !TASK.contains("status: Done"),
            "task cannot close without all approvals"
        );
    }
}
