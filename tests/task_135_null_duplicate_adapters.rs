//! TASK-135 nullable, duplicate-key page semantics from SQL, dbt and ODCS.
#![cfg(feature = "odcs")]

use serde_json::{json, Value};
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_manifest_with_schemas,
    dialect_from_name, parse_dbt_manifest, parse_odcs_yaml,
    physical_finite_composed_page_plan, ConfiguredSqlInput, LiteralValue,
    PhysicalComposedPageProof, PhysicalWriteRow, PhysicalWriteState,
    RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
};
use std::collections::BTreeMap;

const SOURCE: &str = "warehouse.raw.s";

#[test]
fn nullable_duplicate_source_has_identical_closed_world_proofs_on_all_adapters() {
    let freeze: Value = serde_json::from_str(include_str!("../docs/task-135-acceptance-freeze.json")).unwrap();
    let case = freeze["additional_mandatory_cases"].as_array().unwrap().iter()
        .find(|case| case["id"] == "76-C135-N-null-duplicates").unwrap();
    let known = RelationCatalog::new(&[SOURCE]).unwrap();
    let contract = parse_odcs_yaml(
        r#"
apiVersion: v3.2.0
kind: DataContract
id: task135nullable
name: Nullable producer page
version: 1.0.0
status: active
schema:
  - name: s
    physicalName: warehouse.raw.s
    properties:
      - name: k
        physicalType: INTEGER
      - name: seq
        physicalType: INTEGER
"#,
        "postgresql", &known
    ).unwrap();
    let contract_schema = RelationCatalog::from_schemas(contract.source_schemas()).unwrap();
    let direct_schema = RelationCatalog::from_schemas(&[
        RelationSchema::new(SOURCE, vec![
            SchemaColumn::from_sql_type("k", "INTEGER", "postgresql").unwrap(),
            SchemaColumn::from_sql_type("seq", "INTEGER", "postgresql").unwrap()
        ]).unwrap()
    ]).unwrap();
    let sql = case["sql"].as_str().unwrap().replace("FROM s", "FROM warehouse.raw.s");
    let dialect = dialect_from_name("postgresql").unwrap();
    let input = SqlInput::inline(&sql);
    let configured = [ConfiguredSqlInput::new("TASK-135-null", &input, "postgresql", dialect.as_ref())];
    let direct = analyze_configured_inputs_with_catalog(&configured, &direct_schema).unwrap();
    let odcs = analyze_configured_inputs_with_catalog(&configured, &contract_schema).unwrap();
    let manifest_json = json!({
        "metadata": {"dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v12.json", "dbt_version": "1.11.8", "adapter_type": "postgres"},
        "nodes": {"model.demo.task135null": {
            "unique_id": "model.demo.task135null", "resource_type": "model",
            "relation_name": "warehouse.analytics.task135null",
            "database": "warehouse", "schema": "analytics",
            "original_file_path": "models/task135null.sql", "language": "sql",
            "raw_code": sql, "compiled_code": sql,
            "depends_on": {"nodes": ["source.demo.s"]}
        }},
        "sources": {"source.demo.s": {
            "unique_id": "source.demo.s", "resource_type": "source",
            "relation_name": SOURCE, "database": "warehouse", "schema": "raw", "name": "s",
            "columns": {
                "k": {"name": "k", "data_type": "INTEGER"},
                "seq": {"name": "seq", "data_type": "INTEGER"}
            }
        }}
    });
    let manifest = parse_dbt_manifest(&manifest_json.to_string()).unwrap();
    let dbt = analyze_dbt_manifest_with_schemas(&manifest, "postgresql", dialect.as_ref()).unwrap();
    let state = PhysicalWriteState::new(
        SOURCE,
        case["source_bag"].as_array().unwrap().iter().map(|value| {
            PhysicalWriteRow::new(BTreeMap::from([
                ("k".to_string(), value["k"].as_i64().map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string()))),
                ("seq".to_string(), LiteralValue::Number(value["seq"].as_i64().unwrap().to_string()))
            ])).unwrap()
        }).collect()
    ).unwrap();
    let expected = case["terminal_bag"].as_array().unwrap().iter().map(|row| {
        BTreeMap::from([
            ("k".to_string(), row["k"].as_i64().map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string()))),
            ("seq".to_string(), LiteralValue::Number(row["seq"].as_i64().unwrap().to_string()))
        ])
    }).collect::<Vec<_>>();
    let mut proofs = Vec::new();
    for (adapter, bundle) in [("direct_sql", direct), ("dbt_compiled", dbt), ("odcs", odcs)] {
        let layer = bundle.layers().last().unwrap().id();
        let proof = physical_finite_composed_page_plan(&bundle, layer, std::slice::from_ref(&state));
        let PhysicalComposedPageProof::Exact(exact) = proof else {
            panic!("{adapter}: nullable/duplicate page must be exact: {proof:?}");
        };
        assert_eq!(exact.terminal, expected, "{adapter}");
        assert_eq!(exact.source_row_ids.len(), 5, "{adapter}");
        assert_eq!(exact.selected_source_ids, [format!("{SOURCE}#2"), format!("{SOURCE}#3")], "{adapter}");
        assert_eq!(exact.terminal_domains["k"].admits_null(), Some(false), "{adapter}");
        assert_eq!(exact.terminal_domains["seq"].admits_null(), Some(false), "{adapter}");
        proofs.push((exact.terminal, exact.source_row_ids, exact.selected_source_ids, exact.rejected_source_ids, exact.terminal_domains));
    }
    assert_eq!(proofs[0], proofs[1], "direct/dbt parity");
    assert_eq!(proofs[0], proofs[2], "direct/ODCS parity");
}
