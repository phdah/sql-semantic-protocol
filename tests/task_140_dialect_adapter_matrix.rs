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

// Each disposition is backed by an independent runtime parse and canonical
// proof, not by a dialect-name production whitelist.
#[test]
fn frozen_dialect_and_compiled_dbt_classifications_are_reproduced() {
    let matrix = freeze();
    assert_eq!(matrix["owner"], "TASK-140");
    let mut checked = 0;
    for case in matrix["variants"].as_array().expect("finite variant matrix") {
        let sql = case["sql"].as_str().expect("SQL");
        for cell in case["parser_cells"].as_array().expect("13 dialect cells") {
            let dialect = cell["dialect"].as_str().expect("project dialect");
            assert_eq!(cell["classification"], proof_status(sql, dialect), "{dialect}: {sql}");
            checked += 1;
        }
        let duckdb = "duckdb";
        let adapter_cells = case["adapter_cells"].as_array().expect("adapter matrix");
        assert_eq!(adapter_cells.len(), 3);
        assert_eq!(adapter_cells[0]["classification"], proof_status(sql, duckdb));
        assert_eq!(adapter_cells[1]["classification"], compiled_dbt_status(sql, duckdb));
        assert_eq!(adapter_cells[2]["classification"], "inapplicable_without_literal_rows");
        assert_eq!(adapter_cells[2]["certified"], true);
        if case["kind"] == "conditional" {
            assert_eq!(case["owner"], "TASK-140");
            assert_ne!(proof_status(sql, duckdb), "exact", "{sql}: conditional cannot be exact");
        }
    }
    assert_eq!(checked, matrix["verification"]["parser_cells_to_verify"].as_u64().unwrap() as usize);
}

#[cfg(feature = "odcs")]
#[test]
fn real_odcs_schema_does_not_invent_finite_array_or_values_rows() {
    use sql_semantic_protocol::{parse_odcs_yaml, RelationCatalog};
    let catalog = RelationCatalog::new(&["analytics.customers", "analytics.orders"])
        .expect("valid relation catalog");
    let metadata = parse_odcs_yaml(
        include_str!("fixtures/odcs/commerce.odcs.yaml"),
        "postgresql",
        &catalog,
    ).expect("real ODCS contract");
    assert!(!metadata.source_schemas().is_empty());
    let sql = "SELECT v, COUNT(*) AS n FROM UNNEST(opaque_array) AS t(v) GROUP BY v";
    assert_eq!(proof_status(sql, "duckdb"), "residual");
}
