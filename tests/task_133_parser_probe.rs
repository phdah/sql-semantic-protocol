//! Temporary parser observation to freeze exact TASK-133 dialect classifications.
use serde_json::{json, Map, Value};
use sql_semantic_protocol::{analyze_configured_inputs_with_catalog, dialect_from_name, physical_finite_page_bag_plan, ConfiguredSqlInput, PhysicalFinitePageProof, PhysicalWriteState, PhysicalWriteRow, LiteralValue, RelationCatalog, RelationSchema, SchemaColumn, SqlInput};
use sqlparser::parser::Parser;
use std::collections::BTreeMap;
#[test]
fn observe_all_parser_cells() {
 let freeze: Value=serde_json::from_str(include_str!("../docs/task-133-acceptance-freeze.json")).unwrap();
 let dialects=["generic","mysql","postgresql","hive","sqlite","snowflake","redshift","mssql","clickhouse","bigquery","ansi","duckdb","databricks"];
 let catalog=RelationCatalog::from_schemas(&[RelationSchema::new("s",vec![SchemaColumn::from_sql_type("k","INTEGER","postgresql").unwrap()]).unwrap()]).unwrap();
 let mut output=Map::new();
 for case in freeze["cases"].as_array().unwrap() {
  let mut statuses=Map::new(); let sql=case["sql"].as_str().unwrap();
  for dialect in dialects {
   let parser=dialect_from_name(dialect).unwrap();
   let status=match Parser::parse_sql(parser.as_ref(),sql) {
    Err(_) => "parser_rejected".to_string(),
    Ok(_) => {
     let input=SqlInput::inline(sql);
     match analyze_configured_inputs_with_catalog(&[ConfiguredSqlInput::new("test",&input,dialect,parser.as_ref())],&catalog) {
      Err(err) => format!("analysis_error:{err}"),
      Ok(bundle) => {
       let source=case["source_bag"].as_array().unwrap().iter().map(|v|{
        PhysicalWriteRow::new(BTreeMap::from([("k".to_string(),if v.is_null(){LiteralValue::Null}else{LiteralValue::Number(v.as_i64().unwrap().to_string())})])).unwrap()
       }).collect();
       let state=PhysicalWriteState::new("s",source).unwrap();
       let layer=bundle.layers().last().unwrap().id();
       match physical_finite_page_bag_plan(&bundle,layer,&[state]){
        PhysicalFinitePageProof::Exact(_)=>"exact",
        PhysicalFinitePageProof::Residual{..}=>"residual",
        PhysicalFinitePageProof::Impossible{..}=>"impossible",
       }.to_string()
      }
     }
    }
   };
   statuses.insert(dialect.to_string(),json!(status));
  }
  output.insert(case["id"].as_str().unwrap().to_string(),Value::Object(statuses));
 }
 panic!("TASK133_PARSER_OBSERVATION_START\n{}\nTASK133_PARSER_OBSERVATION_END",Value::Object(output));
}
