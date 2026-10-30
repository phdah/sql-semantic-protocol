//! TASK-141 direct SQL and compiled dbt lexical-source parity.
//! Adapter parity is about canonical source identities, not an independent
//! proof that a complete physical CTE evaluator exists.

use serde_json::{json, Map, Value};
use sql_semantic_protocol::{
    analyze_dbt_manifest_with_schemas, analyze_inputs, dialect_from_name, parse_dbt_manifest,
    AnalysisBundle, ProtocolStatement, QueryStatement, SqlInput,
};

fn compiled_dbt(sql: &str, initial_bags: &Value) -> AnalysisBundle {
    let mut sources = Map::new();
    let mut dependencies = Vec::new();
    for name in initial_bags.as_object().unwrap().keys() {
        let unique_id = format!("source.demo.{name}");
        dependencies.push(unique_id.clone());
        sources.insert(
            unique_id.clone(),
            json!({
                "unique_id": unique_id,
                "resource_type": "source",
                "relation_name": name,
                "database": "main",
                "schema": "raw",
                "name": name,
                "columns": {"k": {"name": "k", "data_type": "INTEGER"}}
            }),
        );
    }
    let manifest = json!({
        "metadata": {
            "dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v12.json",
            "dbt_version": "1.11.8",
            "adapter_type": "duckdb"
        },
        "nodes": {"model.demo.task141": {
            "unique_id": "model.demo.task141",
            "resource_type": "model",
            "relation_name": "main.task141",
            "database": "main",
            "schema": "main",
            "name": "task141",
            "original_file_path": "models/task141.sql",
            "language": "sql",
            "raw_code": "SELECT 0 WHERE FALSE",
            "compiled_code": sql,
            "depends_on": {"nodes": dependencies}
        }},
        "sources": sources
    });
    let parsed = parse_dbt_manifest(&manifest.to_string()).unwrap();
    let dialect = dialect_from_name("duckdb").unwrap();
    analyze_dbt_manifest_with_schemas(&parsed, "duckdb", dialect.as_ref()).unwrap()
}

fn model_query(bundle: &AnalysisBundle) -> &QueryStatement {
    bundle
        .inputs()
        .iter()
        .flat_map(|input| input.statements())
        .find_map(|statement| match statement {
            ProtocolStatement::Query(query) => Some(query),
            ProtocolStatement::Unsupported(_) => None,
        })
        .expect("compiled model query")
}

#[test]
fn frozen_cte_source_instances_match_direct_and_compiled_dbt() {
    let freeze: Value =
        serde_json::from_str(include_str!("../docs/task-141-acceptance-freeze.json")).unwrap();
    let dialect = dialect_from_name("duckdb").unwrap();
    let mut cases = 0;
    for case in freeze["cases"].as_array().unwrap() {
        if case["classification"] != "mandatory_exact" {
            continue;
        }
        let id = case["id"].as_str().unwrap();
        let sql = case["sql"].as_str().unwrap();
        let direct = analyze_inputs(&[SqlInput::inline(sql)], "duckdb", dialect.as_ref()).unwrap();
        let compiled = compiled_dbt(sql, &case["initial_bags"]);
        let direct = model_query(&direct);
        let compiled = model_query(&compiled);
        assert_eq!(compiled.dependencies(), direct.dependencies(), "{id}: physical dependencies");
        assert_eq!(compiled.sources(), direct.sources(), "{id}: distinct source instances");
        assert_eq!(
            compiled
                .output()
                .columns()
                .iter()
                .map(|column| column.name())
                .collect::<Vec<_>>(),
            direct
                .output()
                .columns()
                .iter()
                .map(|column| column.name())
                .collect::<Vec<_>>(),
            "{id}: output identities"
        );
        cases += 1;
    }
    assert_eq!(cases, 4);
}
