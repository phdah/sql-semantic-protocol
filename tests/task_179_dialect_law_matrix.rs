//! TASK-179 independent public dialect and vendor-law disposition checks.
//!
//! Historical parser/analyzer statuses are source evidence, not an assertion
//! that any external engine executed the original syntax or shares a session law.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;
use sql_semantic_protocol::{analyze_sql, dialect_from_name, Error, ProtocolStatement};

const MATRIX: &str = include_str!("../docs/task-179-variant-matrix-v2.json");
const ORIGINAL: &str = include_str!("../docs/task-177-parser-samples.json");
const PARSERS: &str = include_str!("../docs/task-177-parser-snapshot.json");
const EQUIVALENCE: &str = include_str!("../docs/task-177-equivalence-snapshot.json");
const COVERAGE: &str = include_str!("../docs/coverage-manifest.json");

fn read(source: &str) -> Value {
    serde_json::from_str(source).expect("versioned fixture")
}

#[test]
fn frozen_named_law_inventory_preserves_all_original_identity_and_dialect_boundaries() {
    let frozen = read(MATRIX);
    let source = read(ORIGINAL);
    let snapshot = read(PARSERS);
    let equivalence = read(EQUIVALENCE);
    let coverage = read(COVERAGE);
    assert_eq!(frozen["schema_version"], 2);
    assert_eq!(frozen["task_id"], "TASK-179");
    assert_eq!(frozen["parent"], "TASK-88");
    assert_eq!(frozen["dialects"], source["dialects"]);
    assert_eq!(frozen["dialects"], snapshot["dialects"]);
    assert_eq!(frozen["dialects"], equivalence["dialects"]);
    assert_eq!(
        coverage["task_179_vendor_session_laws"]["variant_matrix"],
        "docs/task-179-variant-matrix-v2.json"
    );
    assert_eq!(frozen["external_vendor_runtime_proven"], false);
    let cases = frozen["cases"].as_array().expect("all task variants");
    let names = frozen["dialects"].as_array().expect("thirteen dialects");
    assert_eq!(cases.len(), 37);
    assert_eq!(names.len(), 13);
    assert_eq!(frozen["case_count"], cases.len());
    assert_eq!(frozen["dialect_cell_count"], cases.len() * names.len());
    let expected_families = BTreeMap::from([
        ("null_ordering", 6_usize),
        ("collation", 9),
        ("implicit_coercion", 3),
        ("float_timezone", 7),
        ("dml_transaction", 10),
        ("identifier_case", 2),
    ]);
    let mut actual_families = BTreeMap::new();
    let mut identities = BTreeSet::new();
    let original = source["cases"].as_array().expect("TASK-66 original SQL");
    for case in cases {
        let id = case["id"].as_str().expect("stable TASK-179 variant");
        assert!(
            identities.insert(id),
            "duplicate task fixture identity {id}"
        );
        let family = case["family"].as_str().expect("named vendor law");
        *actual_families.entry(family).or_insert(0_usize) += 1;
        assert!(!case["sql"].as_str().expect("SQL fixture").is_empty());
        assert_eq!(case["fixture_positive"], "88-C179-P");
        assert_eq!(case["fixture_rejected"], "88-C179-N");
        assert!(case["requirements"].as_str().is_some());
        assert!(case["portable_expected_domain"].as_str().is_some());
        assert!(case["source_instance"].as_str().is_some());
        assert!(case["physical_source_terminal_disposition"]
            .as_str()
            .is_some());
        assert!(case["adapter_disposition"].as_str().is_some());
        if let Some(original_id) = case["source_case_id"].as_str() {
            let entry = original
                .iter()
                .find(|entry| entry["id"] == original_id)
                .unwrap_or_else(|| panic!("{id}: original sample {original_id} missing"));
            assert_eq!(case["feature_id"], entry["feature_id"]);
            assert_eq!(case["variant"], entry["variant"]);
            assert_eq!(case["sql"], entry["sql"]);
            assert_eq!(case["parser_mask"], snapshot["matrix"][original_id]);
            assert_eq!(
                case["analyzer_mask"],
                snapshot["canonical_analysis_matrix"][original_id]
            );
            assert_eq!(
                case["canonical_dialect_mask"],
                equivalence["matrix"][original_id]
            );
        } else {
            assert_eq!(family, "identifier_case");
            assert!(case["parser_mask"].is_null());
            assert!(case["analyzer_mask"].is_null());
        }
        let dialect_cells = case["dialect_cells"].as_array().expect("owned 13 cells");
        assert_eq!(dialect_cells.len(), names.len(), "{id}: no omissions");
        for (cell, name) in dialect_cells.iter().zip(names) {
            assert_eq!(cell["dialect"], *name, "{id}: wrong identity order");
            assert_eq!(cell["native_vendor"], "unattested");
            assert_eq!(cell["conditional_owner"], "TASK-179");
            assert_eq!(cell["release_disposition"], "unapproved_vendor_law");
            assert_eq!(cell["review_status"], "awaiting_independent_review");
        }
    }
    assert_eq!(actual_families, expected_families);
}

#[test]
fn all_original_task179_parser_and_analysis_cells_match_the_frozen_public_boundary() {
    let matrix = read(MATRIX);
    let cases = matrix["cases"].as_array().expect("vendor law rows");
    let dialects = matrix["dialects"]
        .as_array()
        .expect("13 dialect identities");
    let mut observations = 0_usize;
    for case in cases {
        let Some(id) = case["source_case_id"].as_str() else {
            continue;
        };
        let sql = case["sql"].as_str().expect("immutable original SQL");
        let parser_mask = case["parser_mask"].as_str().expect("13 parser statuses");
        let analysis_mask = case["analyzer_mask"].as_str().expect("13 public analyses");
        assert_eq!(parser_mask.len(), 13);
        assert_eq!(analysis_mask.len(), 13);
        for (index, (name, cell)) in dialects
            .iter()
            .zip(case["dialect_cells"].as_array().expect("complete dialects"))
            .enumerate()
        {
            let dialect_name = name.as_str().expect("public dialect");
            let parser = dialect_from_name(dialect_name).expect("all dialect names exposed");
            let observed = analyze_sql(sql, dialect_name, parser.as_ref());
            let parsed = parser_mask.as_bytes()[index] == b'P';
            let expected = analysis_mask.as_bytes()[index];
            let classification = match observed {
                Err(Error::Parse(_)) => b'R',
                Err(Error::Analysis(_)) if parsed => b'E',
                Err(error) => panic!("{id}/{dialect_name}: unexpected public error {error}"),
                Ok(bundle) if !parsed => {
                    panic!("{id}/{dialect_name}: rejected original spelling now parses: {bundle:?}")
                }
                Ok(bundle)
                    if bundle.statements().iter().any(|statement| {
                        matches!(statement, ProtocolStatement::Unsupported(_))
                    }) =>
                {
                    b'U'
                }
                Ok(_) => b'A',
            };
            assert_eq!(
                classification, expected,
                "{id}/{dialect_name}: frozen public parser/analyzer law drift"
            );
            assert_eq!(
                cell["parser_status"],
                if parsed { "parsed" } else { "rejected" }
            );
            let expected_status = match expected {
                b'A' => "analyzed",
                b'U' => "unsupported",
                b'E' => "analysis_error",
                b'R' => "parser_rejected",
                _ => panic!("{id}: invalid frozen status"),
            };
            assert_eq!(cell["analyzer_status"], expected_status);
            observations += 1;
        }
    }
    assert_eq!(observations, 35 * 13);
}
