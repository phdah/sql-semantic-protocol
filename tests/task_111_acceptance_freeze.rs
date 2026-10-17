//! TASK-111 finite frozen acceptance inventory, schema and conditional boundaries.
use std::collections::BTreeSet;

use serde_json::Value;

const FREEZE: &str = include_str!("../docs/task-111-acceptance-freeze.json");
const MANIFEST: &str = include_str!("../docs/coverage-manifest.json");
const SCHEMA: &str = include_str!("../schema/protocol.schema.json");
const AUDIT: &str = include_str!("../docs/task-111-acceptance-audit.md");

#[test]
fn frozen_task111_variants_have_complete_sql_ast_oracle_and_dialect_cells() {
    let freeze: Value = serde_json::from_str(FREEZE).expect("frozen matrix");
    assert_eq!(freeze["version"], 2);
    assert_eq!(
        freeze["fixture_ids"],
        serde_json::json!(["71-C111-P", "71-C111-N"])
    );
    let expected = BTreeSet::from([
        "71-C111-P-LEFT-EMPTY",
        "71-C111-P-LEFT-REVERSED",
        "71-C111-P-RIGHT-EMPTY",
        "71-C111-P-FULL-EMPTY",
        "71-C111-N-LEFT-MIXED",
        "71-C111-N-RIGHT-MIXED",
        "71-C111-N-FULL-MIXED",
        "71-C111-N-BOTH-EMPTY",
    ]);
    let cases = freeze["cases"]
        .as_array()
        .expect("finite mandatory variants");
    let actual = cases
        .iter()
        .map(|case| case["id"].as_str().expect("stable fixture ID"))
        .collect::<BTreeSet<_>>();
    assert_eq!(actual, expected);
    let dialects = freeze["dialects"].as_array().expect("dialects");
    assert_eq!(dialects.len(), 13);
    let mut cells = 0;
    for case in cases {
        let id = case["id"].as_str().unwrap();
        assert_eq!(case["classification"], "exact_required", "{id}");
        assert!(case["sql"].as_str().is_some(), "{id}");
        assert_eq!(case["source_instances"]["l"], "l", "{id}");
        assert_eq!(case["source_instances"]["r"], "r", "{id}");
        assert_eq!(case["source_column_types"]["k"], "INTEGER", "{id}");
        assert_eq!(case["source_column_types"]["id"], "VARCHAR", "{id}");
        assert_eq!(
            case["canonical_ast_expectation"]["on"]["operator"], "eq",
            "{id}"
        );
        assert_eq!(case["canonical_ast_expectation"]["joins"], 1, "{id}");
        assert!(
            case["constructive_oracle"]["expected_pattern"].is_string(),
            "{id}"
        );
        assert_eq!(
            case["constructive_oracle"]["left_physical_count"],
            case["left"].as_array().unwrap().len(),
            "{id}: physical left count"
        );
        assert_eq!(
            case["constructive_oracle"]["right_physical_count"],
            case["right"].as_array().unwrap().len(),
            "{id}: physical right count"
        );
        assert_eq!(
            case["expected"].as_array().unwrap().len(),
            case["expected_output_count"].as_u64().unwrap() as usize,
            "{id}: entire output bag"
        );
        assert_eq!(case["adapter_facts"].as_array().unwrap().len(), 3, "{id}");
        for domain in case["domains"].as_array().expect("outcome domains") {
            assert_eq!(domain["type"], "signed_integer", "{id}");
            assert_eq!(domain["lower_inclusive"], true, "{id}");
            assert_eq!(domain["upper_inclusive"], true, "{id}");
            assert!(domain["contains_null"].is_boolean(), "{id}");
        }
        for dialect in dialects {
            let name = dialect.as_str().expect("dialect name");
            let cell = &case["dialects"][name];
            assert_eq!(cell["parser"], "parser_equivalent", "{id}/{name}");
            assert_eq!(cell["canonical"], "equivalent_when_parsed", "{id}/{name}");
            assert_eq!(
                cell["native_execution"],
                if name == "duckdb" {
                    "duckdb_oracle"
                } else {
                    "not_claimed"
                },
                "{id}/{name}: no vendor engine inference"
            );
            cells += 1;
        }
    }
    assert_eq!(cells, 104);
    let boundaries = freeze["conditional_boundaries"].as_array().unwrap();
    assert_eq!(boundaries.len(), 3);
    for boundary in boundaries {
        assert_eq!(boundary["owner"], "TASK-111");
        assert_eq!(boundary["status"], "reviewed_bounded_residual");
        assert_eq!(boundary["release_disposition"], "pending_release_review");
        assert!(boundary["missing_evidence"].as_str().unwrap().len() > 20);
        assert!(boundary["test"].is_string());
    }
}

#[test]
fn frozen_task111_contract_is_linked_to_active_schema_and_review_audit() {
    let manifest: Value = serde_json::from_str(MANIFEST).expect("coverage manifest");
    assert_eq!(
        manifest["task_111_join_acceptance"]["acceptance_freeze"],
        "docs/task-111-acceptance-freeze.json"
    );
    assert_eq!(
        manifest["task_111_join_acceptance"]["release_approved"],
        false
    );
    let schema: Value = serde_json::from_str(SCHEMA).expect("canonical schema");
    let joins = schema["$defs"]["constructiveObligation"]["oneOf"]
        .as_array()
        .expect("constructive obligation variants");
    let join = joins
        .iter()
        .find(|v| v["properties"]["kind"]["const"] == "join_population")
        .expect("join population wire variant");
    let patterns = join["properties"]["pattern"]["enum"].as_array().unwrap();
    for pattern in [
        "empty_right",
        "empty_left",
        "disjoint_keys",
        "mixed_matched_unmatched",
        "mixed_duplicate_right",
    ] {
        assert!(patterns.contains(&Value::String(pattern.to_string())));
    }
    assert!(AUDIT.contains("Matt two-axis review"));
    assert!(AUDIT.contains("TASK-111-C-NULLABILITY"));
    assert!(AUDIT.contains("TASK-111-C-VENDOR"));
    assert!(AUDIT.contains("TASK-111-C-NULLSAFE"));
}
