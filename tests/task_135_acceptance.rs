//! TASK-135: executable acceptance inventory and canonical emission guard.
//! Independent whole-bag DuckDB oracles are in the bounded CTE/set tests.
use serde_json::{json, Value};
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, to_bundle_json, ConfiguredSqlInput,
    RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
};

fn inventory() -> Value {
    serde_json::from_str(include_str!("../docs/task-135-acceptance-freeze.json")).unwrap()
}

fn analyze(sql: &str) -> Value {
    let dialect = dialect_from_name("duckdb").unwrap();
    let input = SqlInput::inline(sql);
    let inputs = [ConfiguredSqlInput::new(
        "TASK-135",
        &input,
        "duckdb",
        dialect.as_ref(),
    )];
    let catalog = RelationCatalog::from_schemas(&[RelationSchema::new(
        "s",
        vec![SchemaColumn::from_sql_type("k", "INTEGER", "postgresql").unwrap()],
    )
    .unwrap()])
    .unwrap();
    let bundle = analyze_configured_inputs_with_catalog(&inputs, &catalog).unwrap();
    serde_json::from_str(&to_bundle_json(&bundle)).unwrap()
}

#[test]
fn freeze_covers_every_required_variant_with_typed_domains_and_both_witness_directions() {
    let freeze = inventory();
    assert_eq!(freeze["version"], 2);
    assert_eq!(freeze["task"], "TASK-135");
    assert_eq!(freeze["fixture_ids"], json!(["76-C135-P", "76-C135-N"]));
    assert_eq!(freeze["mapped_parent_ac"], json!([2, 4, 5, 6]));
    assert_eq!(
        freeze["acceptance_contract"]["task_ac"],
        json!([1, 2, 3, 4, 5])
    );
    let base = freeze["cases"].as_array().unwrap();
    let additional = freeze["additional_mandatory_cases"].as_array().unwrap();
    let conditional = freeze["conditional_cases"].as_array().unwrap();
    assert_eq!((base.len(), additional.len(), conditional.len()), (2, 4, 3));
    let mandatory = base.iter().chain(additional);
    let mut ids = std::collections::BTreeSet::new();
    for case in mandatory {
        let id = case["id"].as_str().unwrap();
        assert!(id.starts_with("76-C135-"));
        assert!(ids.insert(id));
        assert_eq!(case["classification"], "mandatory");
        assert!(!case["sql"].as_str().unwrap().is_empty());
        assert!(case["source_bag"].is_array());
        assert!(case["terminal_bag"].is_array());
        assert!(case.get("terminal_domains").is_some() || case.get("domain").is_some());
        assert!(case.get("source_row_ids").is_some() || case.get("source_ids").is_some());
        assert!(
            case.get("selected_source_ids").is_some()
                || case.get("selected_ids").is_some()
                || case.get("left_selected").is_some()
        );
        assert!(
            case.get("rejected_source_ids").is_some()
                || case.get("rejected_ids").is_some()
                || case.get("left_page_rejected").is_some()
        );
    }
    assert_eq!(
        ids,
        [
            "76-C135-P-cte-nested",
            "76-C135-N-empty",
            "76-C135-P-renamed-producer",
            "76-C135-P-set-branch",
            "76-C135-N-set-zero",
            "76-C135-N-null-duplicates",
        ]
        .into_iter()
        .collect(),
        "a mandatory fixture may not disappear behind an unchanged count"
    );
    assert_eq!(
        freeze["acceptance_contract"]["mandatory_ids"],
        json!([
            "76-C135-P-cte-nested",
            "76-C135-N-empty",
            "76-C135-P-renamed-producer",
            "76-C135-P-set-branch",
            "76-C135-N-set-zero",
            "76-C135-N-null-duplicates"
        ])
    );
    assert_eq!(
        freeze["dialect_matrix"]["dialects"]
            .as_array()
            .unwrap()
            .len(),
        13
    );
    assert_eq!(
        freeze["adapter_matrix"]["sources"],
        json!(["direct_sql", "dbt_compiled", "odcs"])
    );
    for case in conditional {
        assert_eq!(case["status"], "Residual");
        assert_eq!(case["owner"], "TASK-135");
        assert!(case["reason"].as_str().unwrap().len() > 20);
        assert!(case["release_disposition"]
            .as_str()
            .unwrap()
            .starts_with("reviewed_"));
    }
    assert!(freeze["unverified_scope"].as_array().unwrap().is_empty());
    assert_eq!(freeze["signoff"]["parent_done"], false);
}

#[test]
fn canonical_json_retains_separate_cte_and_set_branch_windows() {
    let freeze = inventory();
    let nested_sql = freeze["cases"][0]["sql"].as_str().unwrap();
    let serialized = analyze(nested_sql);
    let canonical = serialized.to_string();
    assert!(canonical.contains("nested_page"));
    assert!(canonical.contains("inner"));
    assert!(canonical.contains("outer"));
    assert!(canonical.contains("projected_columns"));
    let schema = include_str!("../schema/protocol.schema.json");
    assert!(schema.contains("\"nested_page\""));
    assert!(schema.contains("Branch-local ORDER BY and LIMIT/FETCH"));

    let set_sql = freeze["additional_mandatory_cases"][1]["sql"]
        .as_str()
        .unwrap();
    let output = analyze(set_sql);
    let serialized = output.to_string();
    assert!(serialized.contains("set_operation"));
    assert!(serialized.contains("row_shaping"));
    assert!(serialized.contains("membership"));
}
