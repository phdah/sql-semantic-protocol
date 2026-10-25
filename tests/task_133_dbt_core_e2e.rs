//! TASK-133: compiled dbt Core model, independent DuckDB source/terminal oracle,
//! and full canonical physical page proof from the executed source state.
use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_dbt_artifacts, parse_dbt_catalog, parse_dbt_manifest, physical_finite_page_bag_plan,
    LiteralValue, PhysicalFinitePageProof, PhysicalWriteRow, PhysicalWriteState, SetMode,
    ValueDomain,
};
use sqlparser::dialect::dialect_from_str;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

#[test]
#[ignore = "requires actual dbt seed/run/docs; invoked by make dbt-e2e"]
fn dbt_core_compiled_page_matches_native_full_source_and_terminal() {
    let project=PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let target=project.join("tests/fixtures/dbt_core_project/target");
    let full_manifest:Value=serde_json::from_str(&fs::read_to_string(target.join("manifest.json")).unwrap()).unwrap();
    let full_catalog:Value=serde_json::from_str(&fs::read_to_string(target.join("catalog.json")).unwrap()).unwrap();
    let model_id="model.sql_semantic_protocol_e2e.task133_pagination";
    let source_id="source.sql_semantic_protocol_e2e.raw.task133_source";
    let model=&full_manifest["nodes"][model_id];
    let source=&full_manifest["sources"][source_id];
    assert_eq!(model["resource_type"],"model");
    assert_eq!(source["columns"]["k"]["data_type"],"INTEGER");
    let compiled=model["compiled_code"].as_str().unwrap();
    assert!(!compiled.contains("{{"),"uncompiled Jinja is not a SQL oracle");
    assert!(compiled.to_ascii_lowercase().contains("limit 2 offset 1"));
    let source_relation=source["relation_name"].as_str().unwrap();
    let target_relation=model["relation_name"].as_str().unwrap();
    let db=Connection::open(std::env::var("DBT_E2E_DATABASE").unwrap()).unwrap();
    let native_source=db.prepare(&format!("SELECT k FROM {source_relation} ORDER BY k")).unwrap()
        .query_map([],|row|row.get::<_,i64>(0)).unwrap()
        .collect::<Result<Vec<_>,_>>().unwrap();
    assert_eq!(native_source,vec![1,2,3,4],"complete executed physical seed");
    let native_terminal=db.prepare(&format!("SELECT k FROM {target_relation} ORDER BY k")).unwrap()
        .query_map([],|row|row.get::<_,i64>(0)).unwrap()
        .collect::<Result<Vec<_>,_>>().unwrap();
    assert_eq!(native_terminal,vec![2,3],"actual dbt materialized terminal");

    let mut isolated_manifest=full_manifest.clone();
    isolated_manifest["nodes"].as_object_mut().unwrap().retain(|id,_|id==model_id);
    isolated_manifest["sources"].as_object_mut().unwrap().retain(|id,_|id==source_id);
    let mut isolated_catalog=full_catalog.clone();
    isolated_catalog["nodes"].as_object_mut().unwrap().retain(|id,_|id==model_id);
    isolated_catalog["sources"].as_object_mut().unwrap().retain(|id,_|id==source_id);
    let manifest=parse_dbt_manifest(&isolated_manifest.to_string()).unwrap();
    let catalog=parse_dbt_catalog(&isolated_catalog.to_string()).unwrap();
    let dialect=dialect_from_str(manifest.adapter_type()).unwrap();
    let bundle=analyze_dbt_artifacts(&manifest,&catalog,manifest.adapter_type(),dialect.as_ref()).unwrap();
    let physical_name=bundle.source_schemas().iter()
        .find(|schema|schema.relation().contains("task133_source")).unwrap().relation();
    let layer=bundle.layers().iter().find(|layer|
        layer.consumes().iter().any(|name|name==physical_name)
    ).unwrap();
    let rows=native_source.iter().map(|n|{
        PhysicalWriteRow::new(BTreeMap::from([("k".to_string(),LiteralValue::Number(n.to_string()))])).unwrap()
    }).collect();
    let state=PhysicalWriteState::new(physical_name,rows).unwrap();
    let proof=physical_finite_page_bag_plan(&bundle,layer.id(),&[state]);
    let PhysicalFinitePageProof::Exact(exact)=proof else {
        panic!("compiled dbt lost canonical page-bag proof: {proof:?}");
    };
    assert_eq!(exact.rows.len(),4);
    let selected=exact.rows.iter().filter(|row|row.survives).map(|row|row.source_row_id.clone()).collect::<Vec<_>>();
    assert_eq!(selected,vec![format!("{physical_name}#1"),format!("{physical_name}#2")]);
    let rejected=exact.rows.iter().filter(|row|!row.survives).map(|row|row.source_row_id.clone()).collect::<Vec<_>>();
    assert_eq!(rejected,vec![format!("{physical_name}#0"),format!("{physical_name}#3")]);
    let terminal=exact.terminal.iter().map(|row|{
        match row.get("k").unwrap(){LiteralValue::Number(n)=>n.parse::<i64>().unwrap(),other=>panic!("{other:?}")}
    }).collect::<Vec<_>>();
    assert_eq!(terminal,native_terminal);
    let ValueDomain::Set(domain)=&exact.terminal_domains["k"] else {panic!("not strongest finite domain");};
    assert_eq!(domain.mode(),SetMode::Include);
    assert_eq!(domain.values().iter().map(|lit|lit.value().clone()).collect::<Vec<_>>(),
        vec![LiteralValue::Number("2".into()),LiteralValue::Number("3".into())]);
    assert_eq!(exact.terminal_domains["k"].admits_null(),Some(false));
}
