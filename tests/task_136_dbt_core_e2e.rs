//! Executed TASK-136 dbt seed, compiled model and native DuckDB oracle.
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
#[ignore = "requires real dbt seed/run/docs and DuckDB; invoked by make dbt-e2e"]
fn executed_dbt_core_page_has_full_source_and_typed_terminal_bag() {
    let project = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let target = project.join("tests/fixtures/dbt_core_project/target");
    let mut manifest_json: Value =
        serde_json::from_str(&fs::read_to_string(target.join("manifest.json")).unwrap()).unwrap();
    let mut catalog_json: Value =
        serde_json::from_str(&fs::read_to_string(target.join("catalog.json")).unwrap()).unwrap();
    let model_id = "model.sql_semantic_protocol_e2e.task136_page";
    let source_id = "source.sql_semantic_protocol_e2e.raw.task136_source";
    let compiled = manifest_json["nodes"][model_id]["compiled_code"].as_str().unwrap();
    assert!(compiled.contains("order by k") && compiled.contains("limit 2"));
    assert!(!compiled.contains("{{"));
    let source_name = manifest_json["sources"][source_id]["relation_name"].as_str().unwrap().to_string();
    let target_name = manifest_json["nodes"][model_id]["relation_name"].as_str().unwrap().to_string();
    let db = Connection::open(std::env::var("DBT_E2E_DATABASE").unwrap()).unwrap();
    let physical = db.prepare(&format!("SELECT k FROM {source_name} ORDER BY k")).unwrap()
        .query_map([], |row| row.get::<_,i64>(0)).unwrap()
        .collect::<Result<Vec<_>,_>>().unwrap();
    assert_eq!(physical,vec![1,2,3]);
    let terminal = db.prepare(&format!("SELECT k FROM {target_name} ORDER BY k")).unwrap()
        .query_map([], |row| row.get::<_,i64>(0)).unwrap()
        .collect::<Result<Vec<_>,_>>().unwrap();
    assert_eq!(terminal,vec![1,2]);
    manifest_json["nodes"].as_object_mut().unwrap().retain(|key,_|key==model_id);
    manifest_json["sources"].as_object_mut().unwrap().retain(|key,_|key==source_id);
    catalog_json["nodes"].as_object_mut().unwrap().retain(|key,_|key==model_id);
    catalog_json["sources"].as_object_mut().unwrap().retain(|key,_|key==source_id);
    let manifest = parse_dbt_manifest(&manifest_json.to_string()).unwrap();
    let catalog = parse_dbt_catalog(&catalog_json.to_string()).unwrap();
    let parser = dialect_from_str(manifest.adapter_type()).unwrap();
    let bundle = analyze_dbt_artifacts(&manifest,&catalog,manifest.adapter_type(),parser.as_ref()).unwrap();
    let source = bundle.source_schemas().iter().find(|schema|
        schema.relation().contains("task136_source")).unwrap().relation().to_string();
    let layer = bundle.layers().iter().find(|layer|
        layer.consumes().iter().any(|dep|dep==&source)).unwrap();
    let state = PhysicalWriteState::new(&source,physical.iter().map(|n|
        PhysicalWriteRow::new(BTreeMap::from([(
            "k".to_string(),LiteralValue::Number(n.to_string())
        )])).unwrap()
    ).collect()).unwrap();
    let proof = physical_finite_page_bag_plan(&bundle,layer.id(),&[state]);
    let PhysicalFinitePageProof::Exact(exact) = proof else {
        panic!("TASK-136 real compiled dbt page must preserve source bag: {proof:?}");
    };
    assert_eq!(exact.source,source);
    assert_eq!(exact.rows.len(),3);
    let actual = exact.terminal.iter().map(|row|match &row["k"] {
        LiteralValue::Number(n) => n.parse::<i64>().unwrap(),
        other => panic!("unexpected typed row: {other:?}")
    }).collect::<Vec<_>>();
    assert_eq!(actual,terminal);
    assert_eq!(exact.rows.iter().filter(|r|r.survives).map(|r|r.source_row_id.clone()).collect::<Vec<_>>(),
        vec![format!("{source}#0"),format!("{source}#1")]);
    assert_eq!(exact.rows.iter().filter(|r|!r.survives).map(|r|r.source_row_id.clone()).collect::<Vec<_>>(),
        vec![format!("{source}#2")]);
    let ValueDomain::Set(domain) = &exact.terminal_domains["k"] else {
        panic!("terminal domain must be finite");
    };
    assert_eq!(domain.values().len(),2);
    assert_eq!(exact.terminal_domains["k"].admits_null(),Some(false));
}
