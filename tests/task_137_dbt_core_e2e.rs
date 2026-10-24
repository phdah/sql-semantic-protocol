//! TASK-137: executed dbt Core model, catalog and complete native warehouse bag.
//! Run with make dbt-e2e after materialization and dbt docs generate.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_dbt_artifacts, parse_dbt_catalog, parse_dbt_manifest, physical_finite_table_plan,
    LiteralValue, PhysicalFiniteTableProof,
};
use sqlparser::dialect::dialect_from_str;

fn sorted(rows: &[BTreeMap<String, LiteralValue>]) -> Vec<String> {
    let mut result = rows
        .iter()
        .map(|row| format!("{row:?}"))
        .collect::<Vec<_>>();
    result.sort();
    result
}

#[test]
#[ignore = "requires dbt run and docs generate; run make dbt-e2e"]
fn source_free_unnest_real_dbt_artifacts_match_full_native_warehouse_bag() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let target = root.join("tests/fixtures/dbt_core_project/target");
    let mut manifest_json: Value =
        serde_json::from_str(&fs::read_to_string(target.join("manifest.json")).unwrap()).unwrap();
    let mut catalog_json: Value =
        serde_json::from_str(&fs::read_to_string(target.join("catalog.json")).unwrap()).unwrap();
    let model_id = "model.sql_semantic_protocol_e2e.task137_literal_unnest";
    let model = &manifest_json["nodes"][model_id];
    assert_eq!(model["resource_type"], "model");
    let compiled = model["compiled_code"].as_str().unwrap();
    assert!(!compiled.contains("{{"), "compiled SQL contains Jinja");
    assert!(compiled.to_ascii_lowercase().contains("unnest("));
    let relation = model["relation_name"].as_str().unwrap().to_string();

    // The closed source-free model is the whole proof boundary. Other dbt
    // models and sources cannot be treated as nonexistent physical data.
    manifest_json["nodes"]
        .as_object_mut()
        .unwrap()
        .retain(|id, _| id == model_id);
    manifest_json["sources"].as_object_mut().unwrap().clear();
    catalog_json["nodes"]
        .as_object_mut()
        .unwrap()
        .retain(|id, _| id == model_id);
    catalog_json["sources"].as_object_mut().unwrap().clear();

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
    let layer = bundle
        .layers()
        .last()
        .expect("real compiled dbt model output");
    let PhysicalFiniteTableProof::Exact(exact) =
        physical_finite_table_plan(&bundle, layer.id(), None)
    else {
        panic!("real dbt compiled SQL lost the complete literal UNNEST proof");
    };

    let db_path = std::env::var("DBT_E2E_DATABASE").unwrap();
    let db = Connection::open(db_path).unwrap();
    let mut statement = db.prepare(&format!("SELECT v FROM {relation}")).unwrap();
    let native = statement
        .query_map([], |row| {
            let v = row.get::<_, Option<i64>>(0)?;
            Ok(BTreeMap::from([(
                "v".to_string(),
                v.map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string())),
            )]))
        })
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    assert_eq!(sorted(&exact.terminal), sorted(&native));
    assert_eq!(
        exact.row_ids,
        ["unnest:1", "unnest:2", "unnest:3", "unnest:4"]
    );
    assert!(exact.nodes.iter().all(|node| node.inputs().is_empty()));
    assert_eq!(exact.terminal_domains["v"].admits_null(), Some(true));
    assert_eq!(exact.terminal.len(), 4);
    let absent = BTreeMap::from([("v".to_string(), LiteralValue::Number("9".to_string()))]);
    assert!(matches!(
        physical_finite_table_plan(&bundle, layer.id(), Some(&absent)),
        PhysicalFiniteTableProof::Impossible { .. }
    ));
}
