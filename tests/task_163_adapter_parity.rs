//! TASK-163: SQL, dbt and ODCS may supply the same canonical typed
//! source schema, but cannot attest a vendor session law.

use std::collections::BTreeMap;

use serde_json::json;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_artifacts, dialect_from_name,
    parse_dbt_catalog, parse_dbt_manifest, physical_duckdb_transaction_plan_with_laws,
    ConfiguredSqlInput, LiteralValue, PhysicalScriptCatalog, PhysicalTransactionScriptProof,
    PhysicalTransactionSessionLaw, PhysicalTransactionStep, PhysicalWriteRow, PhysicalWriteState,
    RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
};

fn direct_schema() -> RelationCatalog {
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

fn dbt_schema() -> RelationCatalog {
    let manifest = json!({
        "metadata": {
            "dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v12.json",
            "dbt_version": "1.11.8", "adapter_type": "duckdb"
        },
        "nodes": {
            "model.task163.probe": {
                "unique_id": "model.task163.probe", "resource_type": "model",
                "name": "probe", "relation_name": "task163_probe",
                "original_file_path": "models/probe.sql", "language": "sql",
                "raw_code": "SELECT id, v FROM t",
                "compiled_code": "SELECT id, v FROM t",
                "depends_on": {"nodes": ["source.task163.t"]}
            }
        },
        "sources": {
            "source.task163.t": {
                "unique_id": "source.task163.t", "resource_type": "source",
                "name": "t", "relation_name": "t",
                "columns": {
                    "id": {"name": "id", "data_type": "INTEGER"},
                    "v": {"name": "v", "data_type": "INTEGER"}
                }
            }
        }
    });
    let catalog = json!({
        "metadata": {
            "dbt_schema_version": "https://schemas.getdbt.com/dbt/catalog/v1.json",
            "dbt_version": "1.11.8"
        },
        "nodes": {},
        "sources": {
            "source.task163.t": {
                "unique_id": "source.task163.t",
                "metadata": {"type": "BASE TABLE", "database": "main", "schema": "main", "name": "t"},
                "columns": {
                    "id": {"name": "id", "type": "INTEGER", "index": 1},
                    "v": {"name": "v", "type": "INTEGER", "index": 2}
                },
                "stats": {}
            }
        },
        "errors": null
    });
    let manifest = parse_dbt_manifest(&manifest.to_string()).unwrap();
    let catalog = parse_dbt_catalog(&catalog.to_string()).unwrap();
    let dialect = dialect_from_name("duckdb").unwrap();
    let bundle = analyze_dbt_artifacts(&manifest, &catalog, "duckdb", dialect.as_ref()).unwrap();
    RelationCatalog::from_schemas(bundle.source_schemas()).unwrap()
}

#[cfg(feature = "odcs")]
fn odcs_schema() -> RelationCatalog {
    let yaml = r#"apiVersion: v3.2.0
kind: DataContract
id: task163
name: TASK-163 transaction metadata parity
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
    let adapted = sql_semantic_protocol::parse_odcs_yaml(
        yaml,
        "duckdb",
        &RelationCatalog::new(&["t"]).unwrap(),
    )
    .unwrap();
    RelationCatalog::from_schemas(adapted.source_schemas()).unwrap()
}

fn row(id: i64, value: Option<i64>) -> PhysicalWriteRow {
    PhysicalWriteRow::new(BTreeMap::from([
        ("id".to_string(), LiteralValue::Number(id.to_string())),
        (
            "v".to_string(),
            value.map_or(LiteralValue::Null, |v| LiteralValue::Number(v.to_string())),
        ),
    ]))
    .unwrap()
}

type ExactOutcome = (Vec<PhysicalWriteRow>, Vec<Vec<(LiteralValue, u64)>>);

fn prove(catalog: &RelationCatalog) -> ExactOutcome {
    let dialect = dialect_from_name("duckdb").unwrap();
    let input = SqlInput::inline("UPDATE t SET v = 20 WHERE id = 1");
    let configured = ConfiguredSqlInput::new("task163-step", &input, "duckdb", dialect.as_ref());
    let bundle = analyze_configured_inputs_with_catalog(&[configured], catalog).unwrap();
    let initial = PhysicalScriptCatalog {
        states: vec![
            PhysicalWriteState::new("t", vec![row(1, Some(10)), row(2, None), row(2, None)])
                .unwrap(),
        ],
        schemas: catalog.schemas().to_vec(),
    };
    let steps = [
        PhysicalTransactionStep::Begin,
        PhysicalTransactionStep::Sql(&bundle),
        PhysicalTransactionStep::Commit,
    ];
    let exact = physical_duckdb_transaction_plan_with_laws(&steps, &initial, &[]);
    let PhysicalTransactionScriptProof::Exact(exact) = exact else {
        panic!("equivalent metadata must preserve proof: {exact:?}");
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
    let residual = physical_duckdb_transaction_plan_with_laws(
        &steps,
        &initial,
        &[PhysicalTransactionSessionLaw::ConcurrentIsolation],
    );
    assert!(
        matches!(residual, PhysicalTransactionScriptProof::Residual { step_index: 0, .. }),
        "metadata cannot attest isolation: {residual:?}"
    );
    let domains = exact.final_catalog.exact_domains().unwrap();
    (
        exact.final_catalog.states[0].rows().to_vec(),
        domains[0].1.iter().map(|domain| domain.values().to_vec()).collect(),
    )
}

#[test]
fn direct_and_dbt_evidence_preserve_exact_rows_domains_and_isolation_residual() {
    assert_eq!(prove(&direct_schema()), prove(&dbt_schema()));
    assert_eq!(
        prove(&direct_schema()).0,
        vec![row(1, Some(20)), row(2, None), row(2, None)]
    );
}

#[cfg(feature = "odcs")]
#[test]
fn direct_and_odcs_evidence_preserve_exact_rows_domains_and_isolation_residual() {
    assert_eq!(prove(&direct_schema()), prove(&odcs_schema()));
}
