//! TASK-192: preserve the 43 predicate/type cases without promoting parser
//! equivalence or individual DuckDB goldens to full semantic certification.
//! This task-owned increment verifies four previously uncovered closed-world
//! string predicate bags; remaining conditional cells stay unsigned.

use std::collections::BTreeSet;

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{analyze_sql, dialect_from_name, to_json};

const FREEZE: &str = include_str!("../docs/task-192-acceptance-freeze.json");
const SOURCE: &str = include_str!("../docs/task-178-full-variant-inventory.json");
const PRIOR: &str = include_str!("../docs/task-177-predicate-oracles.json");
const COVERAGE: &str = include_str!("../docs/coverage-manifest.json");
const REVIEW: &str = include_str!("../docs/task-192-review-queue.json");

fn json(source: &str) -> Value {
    serde_json::from_str(source).expect("committed versioned task evidence")
}

fn field<'a>(value: &'a Value, name: &str) -> &'a str {
    value[name].as_str().expect("task evidence string")
}

fn ids(db: &Connection, sql: &str) -> Vec<i64> {
    let mut statement = db.prepare(sql).expect("independent native SQL query");
    let rows = statement
        .query_map([], |row| row.get::<_, i64>(0))
        .expect("integer source identity rows");
    let mut result: Vec<_> = rows.map(|row| row.expect("native row")).collect();
    result.sort();
    result
}

fn expected_ids(case: &Value, key: &str) -> Vec<i64> {
    serde_json::from_value(case[key].clone()).expect("frozen source identities")
}

#[test]
fn task_192_inventory_contains_all_original_cases_without_false_semantic_certification() {
    let freeze = json(FREEZE);
    let original = json(SOURCE);
    let prior = json(PRIOR);
    let coverage = json(COVERAGE);
    let review = json(REVIEW);
    let signed_off = review["independent_signoff"]
        .as_bool()
        .expect("explicit independent signoff");
    assert_eq!(freeze["schema_version"], 1);
    assert_eq!(freeze["task_id"], "TASK-192");
    assert_eq!(
        freeze["status"],
        if signed_off {
            "complete_semantic_acceptance"
        } else {
            "in_progress_v1_incomplete_semantic_acceptance"
        }
    );
    assert_eq!(freeze["positive_fixture_id"], "88-C192-P");
    assert_eq!(freeze["negative_fixture_id"], "88-C192-N");
    assert_eq!(freeze["case_count"], 43);
    assert_eq!(freeze["dialect_cell_count"], 559);
    assert_eq!(
        freeze["preexisting_duckdb_truth_partition_oracle_count"],
        25
    );
    assert_eq!(freeze["new_task_owned_duckdb_oracle_count"], 4);
    assert_eq!(freeze["new_task_owned_additional_truth_partitions"], 11);
    assert_eq!(freeze["non_predicate_duckdb_evidence_count"], 3);
    assert_eq!(
        freeze["native_positive_negative_or_conditional_case_count"],
        43
    );
    assert_eq!(freeze["unproved_native_truth_partition_case_count"], 0);
    assert_eq!(freeze["full_typed_semantic_signoff"], signed_off);
    assert_eq!(freeze["release_approved"], false);
    assert_eq!(freeze["independent_review_signed_off"], signed_off);
    assert_eq!(
        coverage["task_192_predicate_type_acceptance"]["task_done"],
        signed_off
    );
    assert_eq!(
        coverage["task_192_predicate_type_acceptance"]["independent_signoff"],
        signed_off
    );

    let dialects = freeze["dialects"].as_array().expect("public dialects");
    assert_eq!(dialects.len(), 13);
    assert_eq!(&freeze["dialects"], &original["dialects"]);
    let known: BTreeSet<_> = prior["cases"]
        .as_array()
        .expect("earlier independently verified cases")
        .iter()
        .map(|case| field(case, "id"))
        .collect();
    assert_eq!(known.len(), 25);
    let new: BTreeSet<_> = freeze["task_owned_string_pattern_duckdb_oracles"]["cases"]
        .as_array()
        .expect("independent string predicate cases")
        .iter()
        .map(|case| field(case, "id"))
        .collect();
    assert_eq!(new.len(), 4);
    assert!(known.is_disjoint(&new));

    let mut seen = BTreeSet::new();
    let mut types = std::collections::BTreeMap::<&str, usize>::new();
    let mut cells = 0;
    for case in freeze["cases"].as_array().expect("task-owned inventory") {
        let id = field(case, "id");
        assert!(seen.insert(id), "{id}: duplicate case");
        let original_case = original["cases"]
            .as_array()
            .expect("original variant registry")
            .iter()
            .find(|item| item["id"] == id)
            .expect("original case must be preserved");
        assert!(
            field(case, "feature_id").starts_with("predicates.")
                || field(case, "feature_id").starts_with("types."),
            "{id}: this child cannot claim other family coverage"
        );
        assert_eq!(case["sql"], original_case["sql"], "{id}: SQL changed");
        assert_eq!(case["variant"], original_case["variant"]);
        let observations = case["dialect_cells"].as_array().expect("13 dialect cells");
        assert_eq!(observations.len(), dialects.len(), "{id}");
        for (index, observation) in observations.iter().enumerate() {
            assert_eq!(observation["dialect"], dialects[index], "{id}");
            assert_eq!(
                observation["observed_parser_canonical_status"],
                original_case["dialect_cells"][index]["observed_status"],
                "{id}: cannot retroactively alter observed parser evidence"
            );
            assert_eq!(
                observation["semantic_certification"],
                if signed_off {
                    "reviewed_bounded_task_192_semantics"
                } else {
                    "pending_independent_task_192_verification"
                },
                "{id}: parser equality is not semantic proof"
            );
            assert_eq!(observation["native_vendor_certification"], "unattested");
            cells += 1;
        }
        assert_eq!(case["release_approved"], false);
        let claim = &case["typed_output_expectation"];
        let kind = claim["kind"]
            .as_str()
            .expect("independent typed expectation");
        *types.entry(kind).or_default() += 1;
        if kind == "precise_constraint" {
            let source = prior["cases"]
                .as_array()
                .expect("prior exact domain goldens")
                .iter()
                .find(|item| item["id"] == id)
                .expect("original independent exact predicate case");
            assert_eq!(
                claim["literal_domain"], source["expected_projected_domain"],
                "{id}: precise typed interval, signed bounds, or set changed"
            );
            assert_eq!(claim["nullable"], false);
        }
        assert!(
            case["source_and_terminal_duckdb_oracle"].is_object(),
            "{id}: every original case requires pinned native positive or negative evidence"
        );
        if !signed_off {
            assert!(
                !case["missing_evidence"]
                    .as_array()
                    .expect("pending requirements")
                    .is_empty(),
                "{id}: unsatisfied semantic or independent review gates cannot disappear"
            );
        }
        if known.contains(id) || new.contains(id) {
            assert_eq!(
                case["source_and_terminal_duckdb_oracle"]
                    ["complete_physical_true_false_unknown_bags"],
                true,
                "{id}: previously proven predicate cannot lose its complete truth partition"
            );
        }
    }
    assert_eq!(seen.len(), 43);
    assert_eq!(cells, 559);
    assert_eq!(types["precise_constraint"], 10);
    assert_eq!(types["unconstrained_nullable_id"], 31);
    assert_eq!(types["native_evaluation_error"], 1);
    assert_eq!(types["unsupported_structural_cast"], 1);
}

#[test]
fn four_original_string_patterns_have_complete_duckdb_true_false_unknown_bags() {
    let freeze = json(FREEZE);
    let source = &freeze["task_owned_string_pattern_duckdb_oracles"];
    let db = Connection::open_in_memory().expect("DuckDB");
    db.execute_batch(field(source, "source_setup"))
        .expect("complete pinned physical string source");
    assert_eq!(
        ids(&db, "SELECT id FROM customers"),
        vec![1, 2, 3, 4, 5, 6, 7],
        "source row identities and multiplicity"
    );
    let mut names = db
        .prepare("SELECT name FROM customers ORDER BY id")
        .expect("name source bag");
    let actual: Vec<Option<String>> = names
        .query_map([], |row| row.get::<_, Option<String>>(0))
        .expect("nullable name query")
        .map(|row| row.expect("source name"))
        .collect();
    let frozen: Vec<Option<String>> = source["source_rows"]
        .as_array()
        .expect("source rows")
        .iter()
        .map(|row| row[1].as_str().map(ToOwned::to_owned))
        .collect();
    assert_eq!(actual, frozen, "duplicate string values and NULL retained");

    for case in source["cases"]
        .as_array()
        .expect("task-owned native goldens")
    {
        let id = field(case, "id");
        let original = freeze["cases"]
            .as_array()
            .expect("inventory")
            .iter()
            .find(|row| row["id"] == id)
            .expect("original SQL variant");
        let predicate = field(case, "predicate");
        let selected = expected_ids(case, "selected_ids");
        assert_eq!(
            ids(&db, field(original, "sql")),
            selected,
            "{id}: exact independent original terminal bag"
        );
        let mut all = Vec::new();
        for (suffix, key) in [
            ("IS TRUE", "selected_ids"),
            ("IS FALSE", "false_ids"),
            ("IS UNKNOWN", "unknown_ids"),
        ] {
            let expected = expected_ids(case, key);
            assert_eq!(
                ids(
                    &db,
                    &format!("SELECT id FROM customers WHERE ({predicate}) {suffix}")
                ),
                expected,
                "{id}: {suffix} entire physical bag"
            );
            all.extend(expected);
        }
        all.sort();
        assert_eq!(
            all,
            vec![1, 2, 3, 4, 5, 6, 7],
            "{id}: closed-world partition"
        );
        let mut not_true = expected_ids(case, "false_ids");
        not_true.extend(expected_ids(case, "unknown_ids"));
        not_true.sort();
        assert_eq!(
            ids(
                &db,
                &format!("SELECT id FROM customers WHERE ({predicate}) IS NOT TRUE")
            ),
            not_true,
            "{id}: rejected direction includes UNKNOWN"
        );

        // The public protocol is checked at its external API boundary, not
        // through parser internals. This does not certify vendor execution.
        let dialect = dialect_from_name("duckdb").expect("supported dialect");
        let output = analyze_sql(field(original, "sql"), "duckdb", dialect.as_ref())
            .unwrap_or_else(|error| panic!("{id}: public analysis failed: {error}"));
        let emitted = json(&to_json(&output));
        assert_eq!(
            emitted["inputs"][0]["statements"][0]["kind"], "query",
            "{id}: query cannot be silently dropped"
        );
        assert!(
            emitted["inputs"][0]["statements"][0]["output"]["columns"][0]["domain"].is_object(),
            "{id}: explicit projected domain/unknown required"
        );
    }
}

#[test]
fn observed_portable_dialect_documents_remain_equivalent_without_claiming_native_execution() {
    let freeze = json(FREEZE);
    let dialects = freeze["dialects"].as_array().expect("13 dialects");
    for case in freeze["cases"].as_array().expect("frozen cases") {
        let id = field(case, "id");
        let sql = field(case, "sql");
        let mut reference: Option<Value> = None;
        let cells = case["dialect_cells"].as_array().expect("13 observed cells");
        let mut comparison_count = 0;
        for (dialect, cell) in dialects.iter().zip(cells.iter()) {
            if cell["observed_parser_canonical_status"] != "C" {
                assert!(
                    !cell["semantic_certification"].is_null(),
                    "{id}: parser exception must preserve its unverified state"
                );
                continue;
            }
            let name = dialect.as_str().expect("dialect name");
            let parser = dialect_from_name(name).expect("public dialect");
            let protocol = analyze_sql(sql, name, parser.as_ref())
                .unwrap_or_else(|error| panic!("{id}/{name}: expected portable parse: {error}"));
            let mut document = json(&to_json(&protocol));
            for input in document["inputs"].as_array_mut().expect("public inputs") {
                input["dialect"] = Value::Null;
            }
            if let Some(expected) = &reference {
                assert_eq!(&document, expected, "{id}/{name}: canonical drift");
            } else {
                reference = Some(document);
            }
            comparison_count += 1;
        }
        // Predicate spellings are portable to all 13 parsers. The
        // structural CAST has three explicit parser/AST boundaries; the
        // other ten canonical documents must still agree exactly.
        let expected_count = if id == "88-C177-S179" { 10 } else { 13 };
        assert_eq!(
            comparison_count, expected_count,
            "{id}: observed portable dialect count drifted"
        );
    }
}

#[test]
fn struct_cast_parser_rejections_and_repaired_duckdb_drift_are_explicit() {
    let freeze = json(FREEZE);
    let structure = freeze["cases"]
        .as_array()
        .expect("43 fixed cases")
        .iter()
        .find(|case| case["id"] == "88-C177-S179")
        .expect("one structural exception");
    let sql = field(structure, "sql");
    let mut portable: Option<Value> = None;
    let mut accepted = 0;
    let mut rejected = 0;
    let mut repaired = 0;
    for observation in structure["dialect_cells"]
        .as_array()
        .expect("13 parser cells")
    {
        let dialect_name = field(observation, "dialect");
        let historical = field(observation, "observed_parser_canonical_status");
        let dialect = dialect_from_name(dialect_name).expect("registered SQL parser");
        let result = analyze_sql(sql, dialect_name, dialect.as_ref());
        if historical == "R" {
            assert!(
                matches!(result, Err(sql_semantic_protocol::Error::Parse(_))),
                "{dialect_name}: original syntax rejection needs an actual parser error"
            );
            assert!(
                matches!(dialect_name, "bigquery" | "generic"),
                "unexpected unsupported parser identity"
            );
            rejected += 1;
            continue;
        }
        assert!(
            matches!(historical, "C" | "D"),
            "{dialect_name}: unexplained STRUCT dialect cell"
        );
        let protocol = result.unwrap_or_else(|error| {
            panic!("{dialect_name}: original STRUCT should remain an explicit semantic boundary: {error}")
        });
        let mut wire = json(&to_json(&protocol));
        let query = &wire["inputs"][0]["statements"][0];
        assert_eq!(
            query["output"]["columns"][0]["domain"]["kind"], "unknown",
            "{dialect_name}: a native DuckDB STRUCT value does not certify portable CAST semantics"
        );
        assert!(
            query["diagnostics"]
                .as_array()
                .expect("public diagnostic list")
                .iter()
                .any(|diagnostic| diagnostic["code"]
                    .as_str()
                    .is_some_and(|code| code.contains("unsupported"))),
            "{dialect_name}: unsupported STRUCT must be observable, not silently exact"
        );
        wire["inputs"][0]["dialect"] = Value::Null;
        if let Some(expected) = &portable {
            assert_eq!(&wire, expected, "{dialect_name}: full document differs");
        } else {
            portable = Some(wire);
        }
        if historical == "D" {
            assert_eq!(
                dialect_name, "duckdb",
                "historical drift was a one-cell DuckDB repair"
            );
            repaired += 1;
        }
        accepted += 1;
    }
    assert_eq!((accepted, rejected, repaired), (11, 2, 1));
}

#[test]
fn original_is_unknown_of_nullable_numeric_comparison_is_a_certified_null_test() {
    use sql_semantic_protocol::{
        ConditionExactnessStatus, Predicate, ProtocolStatement, ValueDomain,
    };

    let freeze = json(FREEZE);
    let sql = field(
        freeze["cases"]
            .as_array()
            .expect("43 original SQLs")
            .iter()
            .find(|case| case["id"] == "88-C177-S018")
            .expect("original nullable comparison"),
        "sql",
    );
    for name in freeze["dialects"].as_array().expect("13 dialects") {
        let name = name.as_str().expect("dialect");
        let parser = dialect_from_name(name).expect("supported dialect");
        let protocol = analyze_sql(sql, name, parser.as_ref())
            .expect("portable original three-valued predicate");
        let [ProtocolStatement::Query(query)] = protocol.statements() else {
            panic!("{name}: original numeric truth test was dropped");
        };
        let Some(Predicate::IsNull(null_test)) = query.predicates().where_predicate() else {
            panic!(
                "{name}: IS UNKNOWN of a null-propagating numeric comparison must be a null test"
            );
        };
        assert!(!null_test.negated(), "{name}: IS UNKNOWN selects NULL");
        assert_eq!(
            query.condition_exactness().status(),
            ConditionExactnessStatus::Exact,
            "{name}: numeric NULL truth must not be a computed-expression residual"
        );
        assert!(
            matches!(
                query.output().columns()[0].domain(),
                ValueDomain::Unbounded | ValueDomain::Unknown(_)
            ),
            "{name}: the predicate tests amount, not projected id"
        );
    }
}
