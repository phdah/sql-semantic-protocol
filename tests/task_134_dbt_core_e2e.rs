//! TASK-134: executed dbt Core DISTINCT model, native DuckDB source and terminal bags.
use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_dbt_artifacts, parse_dbt_catalog, parse_dbt_manifest,
    physical_finite_page_bag_plan, LiteralValue, PhysicalFinitePageProof,
    PhysicalWriteRow, PhysicalWriteState, ValueDomain,
};
use sqlparser::dialect::dialect_from_str;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

#[test]
#[ignore = "requires actual dbt seed/run/docs; invoked by make dbt-e2e"]
fn executed_dbt_distinct_preserves_duplicate_sources_but_emits_distinct_terminal_bag() {
    let project = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let target = project.join("tests/fixtures/dbt_core_project/target");
    let mut manifest_json: Value =
        serde_json::from_str(&fs::read_to_string(target.join("manifest.json")).unwrap()).unwrap();
    let mut catalog_json: Value =
        serde_json::from_str(&fs::read_to_string(target.join("catalog.json")).unwrap()).unwrap();
    let model_id = "model.sql_semantic_protocol_e2e.task134_distinct";
    let source_id = "source.sql_semantic_protocol_e2e.raw.task134_source";
    let compiled = manifest_json["nodes"][model_id]["compiled_code"].as_str().unwrap();
    assert!(compiled.contains("distinct") && compiled.contains("limit 3"));
    assert!(!compiled.contains("{{"), "dbt model was not compiled");
    let source_relation = manifest_json["sources"][source_id]["relation_name"].as_str().unwrap().to_string();
    let target_relation = manifest_json["nodes"][model_id]["relation_name"].as_str().unwrap().to_string();
    let db = Connection::open(std::env::var("DBT_E2E_DATABASE").unwrap()).unwrap();
    let source_values = db.prepare(&format!("SELECT k FROM {source_relation} ORDER BY k")).unwrap()
        .query_map([], |row| row.get::<_, i64>(0)).unwrap()
        .collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(source_values, vec![1, 1, 2, 3]);
    let terminal_values = db.prepare(&format!("SELECT k FROM {target_relation} ORDER BY k")).unwrap()
        .query_map([], |row| row.get::<_, i64>(0)).unwrap()
        .collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(terminal_values, vec![1, 2, 3]);

    manifest_json["nodes"].as_object_mut().unwrap().retain(|id, _| id == model_id);
    manifest_json["sources"].as_object_mut().unwrap().retain(|id, _| id == source_id);
    catalog_json["nodes"].as_object_mut().unwrap().retain(|id, _| id == model_id);
    catalog_json["sources"].as_object_mut().unwrap().retain(|id, _| id == source_id);
    let manifest = parse_dbt_manifest(&manifest_json.to_string()).unwrap();
    let catalog = parse_dbt_catalog(&catalog_json.to_string()).unwrap();
    let dialect = dialect_from_str(manifest.adapter_type()).unwrap();
    let bundle = analyze_dbt_artifacts(
        &manifest, &catalog, manifest.adapter_type(), dialect.as_ref()
    ).unwrap();
    let physical = bundle.source_schemas().iter()
        .find(|schema| schema.relation().contains("task134_source")).unwrap().relation();
    let layer = bundle.layers().iter()
        .find(|layer| layer.consumes().iter().any(|r| r == physical)).unwrap();
    let state = PhysicalWriteState::new(physical, source_values.iter().map(|number| {
        PhysicalWriteRow::new(BTreeMap::from([(
            "k".to_string(), LiteralValue::Number(number.to_string())
        )])).unwrap()
    }).collect()).unwrap();
    let proof = physical_finite_page_bag_plan(&bundle, layer.id(), &[state]);
    let PhysicalFinitePageProof::Exact(exact) = proof else {
        panic!("compiled model did not preserve canonical distinct proof: {proof:?}");
    };
    assert_eq!(exact.rows.len(), 4);
    assert!(exact.rows.iter().all(|r| r.survives));
    assert_eq!(exact.terminal.len(), 3);
    assert_eq!(exact.terminal_contributors[0].len(), 2);
    let values = exact.terminal.iter().map(|row| match row.get("k").unwrap() {
        LiteralValue::Number(value) => value.parse::<i64>().unwrap(),
        other => panic!("unexpected dbt terminal value: {other:?}"),
    }).collect::<Vec<_>>();
    assert_eq!(values, terminal_values);
    assert!(matches!(exact.terminal_domains["k"], ValueDomain::Set(_)));
    assert_eq!(exact.terminal_domains["k"].admits_null(), Some(false));
}
