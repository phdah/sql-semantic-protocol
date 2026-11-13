//! TASK-193 freezes exactly the original 24 joins/scopes variants and their
//! 312 observed dialect cells, then tests the public document for each.
//! One known MSSQL LATERAL AST drift remains explicitly unapproved and belongs
//! to TASK-179/TASK-191; equivalent parser shapes cannot silently diverge.

use std::collections::BTreeSet;

use serde_json::Value;
use sql_semantic_protocol::{analyze_sql, dialect_from_name, to_json, ProtocolStatement};

const FREEZE: &str = include_str!("../docs/task-193-acceptance-freeze.json");
const ORIGINAL: &str = include_str!("../docs/task-178-full-variant-inventory.json");
const COVERAGE: &str = include_str!("../docs/coverage-manifest.json");

fn parsed(s: &str) -> Value {
    serde_json::from_str(s).expect("committed TASK-193 acceptance contract")
}

fn without_input_dialect(mut wire: Value) -> Value {
    for input in wire["inputs"].as_array_mut().expect("canonical input array") {
        input["dialect"] = Value::Null;
    }
    wire
}

#[test]
fn freeze_preserves_all_original_sql_and_observed_parser_cells() {
    let freeze = parsed(FREEZE);
    let original = parsed(ORIGINAL);
    let coverage = parsed(COVERAGE);
    assert_eq!(freeze["schema_version"], 1);
    assert_eq!(freeze["task_id"], "TASK-193");
    assert_eq!(freeze["case_count"], 24);
    assert_eq!(freeze["parser_cells"], 312);
    assert_eq!(freeze["dialects"], original["dialects"]);
    assert_eq!(freeze["task_complete"], false);
    assert_eq!(freeze["release_approved"], false);
    assert_eq!(
        coverage["task_193_join_and_source_instance_acceptance"]["freeze"],
        "docs/task-193-acceptance-freeze.json"
    );
    let originals = original["cases"].as_array().expect("original 163 SQL variants");
    let mut seen = BTreeSet::new();
    let mut cells = BTreeSet::new();
    for case in freeze["cases"].as_array().expect("24 cases") {
        let id = case["id"].as_str().expect("original fixture identity");
        assert!(seen.insert(id), "{id}: duplicate original SQL variant");
        let prior = originals
            .iter()
            .find(|row| row["id"] == id)
            .expect("source SQL exists in original reviewed inventory");
        assert_eq!(case["sql"], prior["sql"], "{id}: SQL cannot be silently edited");
        assert_eq!(case["variant"], prior["variant"], "{id}: fixed variant");
        assert_eq!(case["feature_id"], prior["feature_id"], "{id}: family owner");
        assert!(
            case["feature_id"]
                .as_str()
                .is_some_and(|family| family.starts_with("joins.") || family.starts_with("scopes.")),
            "{id}: TASK-193 owns joins and correlated scopes only"
        );
        assert_eq!(
            case["observed_canonical_mask"],
            prior["full_document_classification"],
            "{id}: original 13-dialect observation cannot be replaced with a passing claim"
        );
        let variants = case["cells"].as_array().expect("13 named dialect cells");
        assert_eq!(variants.len(), 13);
        for (index, cell) in variants.iter().enumerate() {
            let name = cell["dialect"].as_str().expect("parser dialect");
            let identity = cell["id"].as_str().expect("stable cell identity");
            assert_eq!(name, freeze["dialects"][index]);
            assert_eq!(identity, format!("{id}/{name}"));
            assert!(cells.insert(identity), "{identity}: repeated dialect cell");
            assert_eq!(cell["native_vendor_execution"], "unattested");
            assert_eq!(cell["observed_status"], prior["dialect_cells"][index]["observed_status"]);
        }
    }
    assert_eq!(seen.len(), 24);
    assert_eq!(cells.len(), 312);
}

#[test]
fn all_shared_join_sql_forms_emit_identical_public_documents_across_13_parsers() {
    let freeze = parsed(FREEZE);
    let mut checked = 0;
    for case in freeze["cases"].as_array().expect("24 original SQLs") {
        let id = case["id"].as_str().expect("stable fixture");
        let sql = case["sql"].as_str().expect("original SQL");
        let mut reference: Option<Value> = None;
        for cell in case["cells"].as_array().expect("frozen cells") {
            let name = cell["dialect"].as_str().expect("named parser");
            let dialect = dialect_from_name(name).expect("exposed parser dialect");
            let protocol = analyze_sql(sql, name, dialect.as_ref())
                .unwrap_or_else(|error| panic!("{id}/{name}: original SQL failed: {error}"));
            assert!(
                protocol
                    .statements()
                    .iter()
                    .all(|statement| !matches!(statement, ProtocolStatement::Unsupported(_))),
                "{id}/{name}: successful parsing is not an excuse to drop semantics"
            );
            let document = without_input_dialect(parsed(&to_json(&protocol)));
            let columns = document["inputs"][0]["statements"][0]["output"]["columns"]
                .as_array()
                .expect("every SQL output has canonical columns");
            assert_eq!(
                columns.len(),
                case["expected_columns"].as_u64().expect("native projection arity") as usize,
                "{id}/{name}: canonical output arity"
            );
            for column in columns {
                assert!(
                    column["domain"].is_object(),
                    "{id}/{name}: output domains cannot disappear"
                );
            }
            if let Some(baseline) = &reference {
                if id == "88-C177-S152" && name == "mssql" {
                    assert_eq!(cell["observed_status"], "D");
                    assert_ne!(
                        &document, baseline,
                        "MSSQL LATERAL AST drift needs explicit TASK-179/TASK-191 adjudication"
                    );
                } else {
                    assert_eq!(cell["observed_status"], "C");
                    assert_eq!(
                        &document, baseline,
                        "{id}/{name}: identical SQL shape changed canonical outcomes"
                    );
                }
            } else {
                assert_eq!(cell["observed_status"], "C");
                reference = Some(document);
            }
            checked += 1;
        }
    }
    assert_eq!(checked, 312);
}
