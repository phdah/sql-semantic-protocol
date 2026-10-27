//! TASK-146 real compiled dbt Core CTE source and native DuckDB INSERT oracle.
//! Executed after make dbt-e2e materializes the seed and view.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_inputs, dialect_from_name,
    physical_ordered_write_state_plan, ConfiguredSqlInput, LiteralValue, PhysicalWriteRow,
    PhysicalWriteState, PhysicalWriteStatePlan, ProtocolStatement, RelationCatalog,
    RelationSchema, SchemaColumn, SqlInput,
};

fn state(name: &str, rows: &[(i64, i64)]) -> PhysicalWriteState {
    PhysicalWriteState::new(
        name,
        rows.iter().map(|(id,score)| {
            PhysicalWriteRow::new(BTreeMap::from([
                ("id".to_string(),LiteralValue::Number(id.to_string())),
                ("score".to_string(),LiteralValue::Number(score.to_string()))
            ])).unwrap()
        }).collect()
    ).unwrap()
}

#[test]
#[ignore = "requires real dbt seed/run/docs generate via make dbt-e2e"]
fn compiled_cte_select_drives_an_exact_insert_into_a_complete_target() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let project = root.join("tests/fixtures/dbt_core_project/target");
    let manifest: Value = serde_json::from_str(
        &fs::read_to_string(project.join("manifest.json")).expect("dbt manifest")
    ).unwrap();
    let catalog: Value = serde_json::from_str(
        &fs::read_to_string(project.join("catalog.json")).expect("dbt catalog")
    ).unwrap();

    let model_id = "model.sql_semantic_protocol_e2e.task146_insert_source";
    let node = &manifest["nodes"][model_id];
    assert_eq!(node["resource_type"],"model");
    assert!(catalog["nodes"][model_id].is_object(), "actual dbt catalog model");
    let compiled = node["compiled_code"].as_str().expect("compiled dbt SELECT");
    assert!(!compiled.contains("{{"), "compiled SQL cannot contain Jinja");
    assert!(compiled.to_lowercase().contains("with staged as"));
    let relation = node["relation_name"].as_str().expect("dbt materialized relation");

    let dialect = dialect_from_name("duckdb").unwrap();
    let insert_sql = format!("INSERT INTO target(score,id) {compiled}");
    let input = SqlInput::inline(&insert_sql);
    let preliminary = analyze_inputs(&[SqlInput::inline(&insert_sql)], "duckdb", dialect.as_ref()).unwrap();
    let ProtocolStatement::Query(query) = &preliminary.inputs()[0].statements()[0] else {
        panic!("compiled dbt CTE must normalize into canonical INSERT")
    };
    let stage = query.write().unwrap().insert_cte_source().expect("canonical CTE stages");
    assert_eq!(stage.name(),"staged");
    let source = stage.producer().sources().first().expect("physical compiled source")
        .name().to_string();

    let schemas = [source.as_str(), "target"].map(|relation| RelationSchema::new(
        relation,
        ["id","score"].into_iter().map(|col|
            SchemaColumn::from_sql_type(col,"INTEGER","postgresql").unwrap()
        ).collect()
    ).unwrap());
    let typed_catalog = RelationCatalog::from_schemas(&schemas).unwrap();
    let configured = ConfiguredSqlInput::new(
        "task-146-dbt-core", &input, "duckdb", dialect.as_ref()
    );
    let bundle = analyze_configured_inputs_with_catalog(&[configured], &typed_catalog).unwrap();
    let result = physical_ordered_write_state_plan(
        &bundle,
        &[state(&source,&[(1,10),(2,20),(3,30)]), state("target",&[])]
    );
    let PhysicalWriteStatePlan::Feasible { transitions, final_states } = result else {
        panic!("compiled dbt SELECT must prove CTE source and target: {result:?}")
    };
    assert_eq!(transitions.len(),1);
    assert_eq!(transitions[0].affected().inserted(),2);
    assert_eq!(transitions[0].intermediate_states().len(),1);
    assert_eq!(transitions[0].intermediate_states()[0].rows(),
        state("staged",&[(2,20),(3,30)]).rows());
    let target = final_states.iter().find(|s|s.relation()=="target").unwrap();
    assert_eq!(target.rows(),state("target",&[(2,20),(3,30)]).rows());

    let db = Connection::open(std::env::var("DBT_E2E_DATABASE").expect("warehouse path"))
        .expect("actual dbt DuckDB warehouse");
    let mut stmt = db.prepare(&format!("SELECT score,id FROM {relation} ORDER BY id"))
        .expect("compiled dbt materialized view");
    let actual = stmt.query_map([],|row|
        Ok((row.get::<_,i64>(0)?,row.get::<_,i64>(1)?))
    ).unwrap().map(|row|row.unwrap()).collect::<Vec<_>>();
    assert_eq!(actual,vec![(20,2),(30,3)]);
}
