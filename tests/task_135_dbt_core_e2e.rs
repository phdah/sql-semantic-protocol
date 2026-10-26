//! Executed dbt Core fixture: physical seed -> compiled CTE page -> materialized DuckDB view.
use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_dbt_artifacts, parse_dbt_catalog, parse_dbt_manifest,
    physical_finite_composed_page_plan, LiteralValue, PhysicalComposedPageProof, PhysicalWriteRow,
    PhysicalWriteState,
};
use sqlparser::dialect::dialect_from_str;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

#[test]
#[ignore = "requires actual dbt seed/run/docs; invoked by make dbt-e2e"]
fn executed_dbt_nested_cte_page_preserves_entire_source_and_final_output_bag() {
    let project = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let target = project.join("tests/fixtures/dbt_core_project/target");
    let mut manifest_json: Value =
        serde_json::from_str(&fs::read_to_string(target.join("manifest.json")).unwrap()).unwrap();
    let mut catalog_json: Value =
        serde_json::from_str(&fs::read_to_string(target.join("catalog.json")).unwrap()).unwrap();
    let model_id = "model.sql_semantic_protocol_e2e.task135_nested_page";
    let source_id = "source.sql_semantic_protocol_e2e.raw.task135_source";
    let compiled = manifest_json["nodes"][model_id]["compiled_code"]
        .as_str()
        .unwrap();
    assert!(compiled.contains("with page as") && compiled.contains("limit 4"));
    assert!(compiled.contains("limit 1 offset 2"));
    assert!(!compiled.contains("{{"));
    let source_relation = manifest_json["sources"][source_id]["relation_name"]
        .as_str()
        .unwrap()
        .to_string();
    let target_relation = manifest_json["nodes"][model_id]["relation_name"]
        .as_str()
        .unwrap()
        .to_string();
    let db = Connection::open(std::env::var("DBT_E2E_DATABASE").unwrap()).unwrap();
    let physical = db
        .prepare(&format!("SELECT k FROM {source_relation} ORDER BY k"))
        .unwrap()
        .query_map([], |row| row.get::<_, i64>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(physical, [1, 2, 3, 4, 5]);
    let final_rows = db
        .prepare(&format!("SELECT k FROM {target_relation} ORDER BY k"))
        .unwrap()
        .query_map([], |row| row.get::<_, i64>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(final_rows, [3]);
    manifest_json["nodes"]
        .as_object_mut()
        .unwrap()
        .retain(|key, _| key == model_id);
    manifest_json["sources"]
        .as_object_mut()
        .unwrap()
        .retain(|key, _| key == source_id);
    catalog_json["nodes"]
        .as_object_mut()
        .unwrap()
        .retain(|key, _| key == model_id);
    catalog_json["sources"]
        .as_object_mut()
        .unwrap()
        .retain(|key, _| key == source_id);
    let manifest = parse_dbt_manifest(&manifest_json.to_string()).unwrap();
    let catalog = parse_dbt_catalog(&catalog_json.to_string()).unwrap();
    let dialect = dialect_from_str(manifest.adapter_type()).unwrap();
    let bundle = analyze_dbt_artifacts(
        &manifest,
        &catalog,
        manifest.adapter_type(),
        dialect.as_ref(),
    )
    .unwrap();
    let source = bundle
        .source_schemas()
        .iter()
        .find(|schema| schema.relation().contains("task135_source"))
        .unwrap()
        .relation()
        .to_string();
    let layer = bundle
        .layers()
        .iter()
        .find(|layer| layer.consumes().iter().any(|dep| dep == &source))
        .unwrap();
    let state = PhysicalWriteState::new(
        &source,
        physical
            .iter()
            .map(|value| {
                PhysicalWriteRow::new(BTreeMap::from([(
                    "k".to_string(),
                    LiteralValue::Number(value.to_string()),
                )]))
                .unwrap()
            })
            .collect(),
    )
    .unwrap();
    let proof = physical_finite_composed_page_plan(&bundle, layer.id(), &[state]);
    let PhysicalComposedPageProof::Exact(exact) = proof else {
        panic!("dbt compiled query must preserve exact nested page: {proof:?}");
    };
    assert_eq!(exact.source, source);
    assert_eq!(exact.source_row_ids.len(), physical.len());
    assert_eq!(exact.selected_source_ids, [format!("{source}#2")]);
    assert_eq!(exact.rejected_source_ids.len(), 4);
    let output = exact
        .terminal
        .iter()
        .map(|row| match row.get("k").unwrap() {
            LiteralValue::Number(value) => value.parse::<i64>().unwrap(),
            other => panic!("unexpected typed output {other:?}"),
        })
        .collect::<Vec<_>>();
    assert_eq!(output, final_rows);
    assert_eq!(exact.terminal_domains["k"].admits_null(), Some(false));
}
