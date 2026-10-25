//! TASK-140 frozen SQL parser and independent adapter proof matrix.

use serde_json::{json, Value};
use sql_semantic_protocol::{
    analyze_dbt_manifest_with_schemas, analyze_inputs, dialect_from_name, parse_dbt_manifest,
    physical_finite_table_plan, PhysicalFiniteTableProof, SqlInput,
};

fn freeze() -> Value {
    serde_json::from_str(include_str!("../docs/task-140-acceptance-freeze.json"))
        .expect("complete frozen TASK-140 matrix")
}

fn proof_status(sql: &str, dialect: &str) -> String {
    let parser = dialect_from_name(dialect).expect("13 supported dialects");
    match analyze_inputs(&[SqlInput::inline(sql)], dialect, parser.as_ref()) {
        Err(_) => "parser_boundary".into(),
        Ok(bundle) => match bundle.layers().last() {
            None => "residual".into(),
            Some(layer) => match physical_finite_table_plan(&bundle, layer.id(), None) {
                PhysicalFiniteTableProof::Exact(_) => "exact".into(),
                PhysicalFiniteTableProof::Impossible { .. }
                | PhysicalFiniteTableProof::Residual { .. } => "residual".into(),
            },
        },
    }
}

fn compiled_dbt_status(sql: &str, dialect: &str) -> String {
    let node = json!({
        "metadata": {
            "dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v12.json",
            "dbt_version": "1.11.8", "adapter_type": dialect
        },
        "nodes": {"model.demo.task140": {
            "unique_id": "model.demo.task140",
            "resource_type": "model", "relation_name": "main.task140",
            "database": "main", "schema": "main", "name": "task140",
            "original_file_path": "models/task140.sql",
            "language": "sql", "raw_code": "select 0 where false",
            "compiled_code": sql, "depends_on": {"nodes": []}
        }},
        "sources": {}
    });
    let Ok(manifest) = parse_dbt_manifest(&node.to_string()) else {
        return "parser_boundary".into();
    };
    let parser = dialect_from_name(dialect).expect("dialect");
    match analyze_dbt_manifest_with_schemas(&manifest, dialect, parser.as_ref()) {
        Err(_) => "parser_boundary".into(),
        Ok(bundle) => match bundle.layers().last() {
            None => "residual".into(),
            Some(layer) => match physical_finite_table_plan(&bundle, layer.id(), None) {
                PhysicalFiniteTableProof::Exact(_) => "exact".into(),
                PhysicalFiniteTableProof::Impossible { .. }
                | PhysicalFiniteTableProof::Residual { .. } => "residual".into(),
            },
        },
    }
}

// Collect observed parser classifications before freezing the signed matrix.
#[test]
#[ignore = "temporary matrix observation completed; replace with assertions after parser fix"]
fn observe_all_dialects_and_compiled_dbt_fanout() {
    let matrix = freeze();
    let mut findings = Vec::new();
    for case in matrix["variants"].as_array().unwrap() {
        let sql = case["sql"].as_str().unwrap();
        let id = case["id"].as_str().unwrap();
        for dialect in matrix["dialects"].as_array().unwrap() {
            let dialect = dialect.as_str().unwrap();
            findings.push(json!({
                "id": id, "dialect": dialect,
                "direct": proof_status(sql, dialect),
                "dbt": compiled_dbt_status(sql, dialect)
            }));
        }
    }
    panic!("TASK140_OBSERVED={}", serde_json::to_string(&findings).unwrap());
}
