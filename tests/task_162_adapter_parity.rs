//! TASK-162: equivalent SQL/dbt/ODCS schema evidence must preserve the
//! canonical typed transaction outcome. Adapters do not supply execution laws.

use std::collections::BTreeMap;

use serde_json::json;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_artifacts, dialect_from_name,
    parse_dbt_catalog, parse_dbt_manifest, physical_duckdb_transaction_plan, ConfiguredSqlInput,
    LiteralValue, PhysicalScriptCatalog, PhysicalTransactionScriptProof, PhysicalTransactionStep,
    PhysicalWriteRow, PhysicalWriteState, RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
};

fn direct() -> RelationCatalog {
    RelationCatalog::from_schemas(&[RelationSchema::new(
        "t",
        vec![
            SchemaColumn::from_sql_type("id", "INTEGER", "duckdb").unwrap(),
            SchemaColumn::from_sql_type("v", "INTEGER", "duckdb").unwrap(),
        ],
    )
    .unwrap()])
    .unwrap()
}

fn dbt() -> RelationCatalog {
    let manifest = json!({
        "metadata": {
            "dbt_schema_version":"https://schemas.getdbt.com/dbt/manifest/v12.json",
            "dbt_version":"1.11.8","adapter_type":"duckdb"
        },
        "nodes": {
            "model.task162.probe": {
                "unique_id":"model.task162.probe", "resource_type":"model",
                "name":"probe", "relation_name":"task162_probe",
                "original_file_path":"models/probe.sql","language":"sql",
                "raw_code":"SELECT id, v FROM t",
                "compiled_code":"SELECT id, v FROM t",
                "depends_on":{"nodes":["source.task162.t"]}
            }
        },
        "sources": {
            "source.task162.t": {
                "unique_id":"source.task162.t", "resource_type":"source",
                "name":"t", "relation_name":"t",
                "columns":{
                    "id":{"name":"id","data_type":"INTEGER"},
                    "v":{"name":"v","data_type":"INTEGER"}
                }
            }
        }
    });
    let catalog = json!({
        "metadata": {
            "dbt_schema_version":"https://schemas.getdbt.com/dbt/catalog/v1.json",
            "dbt_version":"1.11.8"
        },
        "nodes":{},
        "sources":{
            "source.task162.t":{
                "unique_id":"source.task162.t",
                "metadata":{"type":"BASE TABLE","database":"main","schema":"main","name":"t"},
                "columns":{
                    "id":{"name":"id","type":"INTEGER","index":1},
                    "v":{"name":"v","type":"INTEGER","index":2}
                },
                "stats":{}
            }
        },
        "errors":null
    });
    let manifest = parse_dbt_manifest(&manifest.to_string()).unwrap();
    let catalog = parse_dbt_catalog(&catalog.to_string()).unwrap();
    let dialect = dialect_from_name("duckdb").unwrap();
    let bundle = analyze_dbt_artifacts(&manifest, &catalog, "duckdb", dialect.as_ref()).unwrap();
    RelationCatalog::from_schemas(bundle.source_schemas()).unwrap()
}

#[cfg(feature = "odcs")]
fn odcs() -> RelationCatalog {
    use sql_semantic_protocol::parse_odcs_yaml;
    let document = r#"apiVersion: v3.2.0
kind: DataContract
id: task162
name: TASK-162 transaction parity
version: 1.0.0
status: active
schema:
  - name: t
    physicalName: t
    properties:
      - name: id
        physicalType: INTEGER
      - name: v
        physicalType: INTEGER
"#;
    let parsed =
        parse_odcs_yaml(document, "duckdb", &RelationCatalog::new(&["t"]).unwrap()).unwrap();
    RelationCatalog::from_schemas(parsed.source_schemas()).unwrap()
}

fn row(id: i64, v: Option<i64>) -> PhysicalWriteRow {
    PhysicalWriteRow::new(BTreeMap::from([
        ("id".to_string(), LiteralValue::Number(id.to_string())),
        (
            "v".to_string(),
            v.map_or(LiteralValue::Null, |x| LiteralValue::Number(x.to_string())),
        ),
    ]))
    .unwrap()
}

fn outcome(catalog: &RelationCatalog) -> (Vec<PhysicalWriteRow>, Vec<Vec<(LiteralValue, u64)>>) {
    let dialect = dialect_from_name("duckdb").unwrap();
    let input = SqlInput::inline("UPDATE t SET v = 20 WHERE id = 1");
    let configured = ConfiguredSqlInput::new("task162-step", &input, "duckdb", dialect.as_ref());
    let bundle = analyze_configured_inputs_with_catalog(&[configured], catalog).unwrap();
    let initial = PhysicalScriptCatalog {
        states: vec![PhysicalWriteState::new(
            "t",
            vec![row(1, Some(10)), row(2, None), row(2, None)],
        )
        .unwrap()],
        schemas: catalog.schemas().to_vec(),
    };
    let proof = physical_duckdb_transaction_plan(
        &[
            PhysicalTransactionStep::Begin,
            PhysicalTransactionStep::Sql(&bundle),
            PhysicalTransactionStep::Commit,
        ],
        &initial,
    );
    let PhysicalTransactionScriptProof::Exact(exact) = proof else {
        panic!("equivalent schema evidence must prove the same transaction: {proof:?}");
    };
    assert_eq!(exact.checkpoints[1].before.states[0].rows().len(), 3);
    assert_eq!(
        exact.checkpoints[1]
            .statement
            .as_ref()
            .unwrap()
            .write
            .as_ref()
            .unwrap()
            .affected()
            .updated(),
        1
    );
    let state = &exact.final_catalog.states[0];
    let domains = exact.final_catalog.exact_domains().unwrap();
    (
        state.rows().to_vec(),
        domains[0]
            .1
            .iter()
            .map(|domain| domain.values().to_vec())
            .collect(),
    )
}

#[test]
fn direct_and_dbt_metadata_prove_same_transaction_rows_and_domains() {
    assert_eq!(outcome(&direct()), outcome(&dbt()));
    assert_eq!(
        outcome(&direct()).0,
        vec![row(1, Some(20)), row(2, None), row(2, None)]
    );
}

#[cfg(feature = "odcs")]
#[test]
fn direct_and_odcs_metadata_prove_same_transaction_rows_and_domains() {
    assert_eq!(outcome(&direct()), outcome(&odcs()));
}
