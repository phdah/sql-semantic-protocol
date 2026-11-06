//! TASK-164: equivalent SQL/dbt/ODCS source schemas retain canonical mixed
//! MERGE, DELETE and read semantics without inventing transaction session laws.

use std::collections::BTreeMap;

use serde_json::json;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_artifacts, dialect_from_name,
    parse_dbt_catalog, parse_dbt_manifest, physical_duckdb_transaction_plan, ConfiguredSqlInput,
    LiteralValue, PhysicalScriptCatalog, PhysicalTransactionScriptProof, PhysicalTransactionStep,
    PhysicalWriteRow, PhysicalWriteState, RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
};

fn direct() -> RelationCatalog {
    RelationCatalog::from_schemas(
        &["t", "s"]
            .into_iter()
            .map(|relation| {
                RelationSchema::new(
                    relation,
                    ["id", "v"]
                        .into_iter()
                        .map(|column| {
                            SchemaColumn::from_sql_type(column, "INTEGER", "duckdb").unwrap()
                        })
                        .collect(),
                )
                .unwrap()
            })
            .collect::<Vec<_>>(),
    )
    .unwrap()
}

fn dbt() -> RelationCatalog {
    let manifest = json!({
        "metadata": {
            "dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v12.json",
            "dbt_version": "1.11.8",
            "adapter_type": "duckdb"
        },
        "nodes": {
            "model.task164.probe": {
                "unique_id": "model.task164.probe",
                "resource_type": "model",
                "name": "probe",
                "relation_name": "task164_probe",
                "original_file_path": "models/probe.sql",
                "language": "sql",
                "raw_code": "SELECT id, v FROM t",
                "compiled_code": "SELECT id, v FROM t",
                "depends_on": {"nodes": ["source.task164.t"]}
            }
        },
        "sources": {
            "source.task164.t": {
                "unique_id": "source.task164.t",
                "resource_type": "source",
                "name": "t",
                "relation_name": "t",
                "columns": {
                    "id": {"name":"id", "data_type":"INTEGER"},
                    "v": {"name":"v", "data_type":"INTEGER"}
                }
            },
            "source.task164.s": {
                "unique_id": "source.task164.s",
                "resource_type": "source",
                "name": "s",
                "relation_name": "s",
                "columns": {
                    "id": {"name":"id", "data_type":"INTEGER"},
                    "v": {"name":"v", "data_type":"INTEGER"}
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
            "source.task164.t": {
                "unique_id": "source.task164.t",
                "metadata": {"type":"BASE TABLE","database":"main","schema":"main","name":"t"},
                "columns": {
                    "id":{"name":"id","type":"INTEGER","index":1},
                    "v":{"name":"v","type":"INTEGER","index":2}
                },
                "stats": {}
            },
            "source.task164.s": {
                "unique_id": "source.task164.s",
                "metadata": {"type":"BASE TABLE","database":"main","schema":"main","name":"s"},
                "columns": {
                    "id":{"name":"id","type":"INTEGER","index":1},
                    "v":{"name":"v","type":"INTEGER","index":2}
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
fn odcs() -> RelationCatalog {
    let yaml = r#"apiVersion: v3.2.0
kind: DataContract
id: task164
name: TASK-164 mixed script metadata
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
  - name: s
    physicalName: s
    properties:
      - name: id
        physicalType: INTEGER
      - name: v
        physicalType: INTEGER
"#;
    let adapted = sql_semantic_protocol::parse_odcs_yaml(
        yaml,
        "duckdb",
        &RelationCatalog::new(&["t", "s"]).unwrap(),
    )
    .unwrap();
    RelationCatalog::from_schemas(adapted.source_schemas()).unwrap()
}

fn row(id: i64, value: Option<i64>) -> PhysicalWriteRow {
    PhysicalWriteRow::new(BTreeMap::from([
        ("id".to_string(), LiteralValue::Number(id.to_string())),
        (
            "v".to_string(),
            value.map_or(LiteralValue::Null, |value| {
                LiteralValue::Number(value.to_string())
            }),
        ),
    ]))
    .unwrap()
}

fn prove(catalog: &RelationCatalog) -> (Vec<PhysicalWriteRow>, Vec<usize>) {
    let dialect = dialect_from_name("duckdb").unwrap();
    let sql = [
        "MERGE INTO t USING s ON t.id = s.id WHEN MATCHED THEN UPDATE SET v = s.v",
        "DELETE FROM t WHERE id = 2",
        "SELECT id, v FROM t WHERE v >= 20",
    ];
    let inputs = sql
        .iter()
        .map(|sql| SqlInput::inline(*sql))
        .collect::<Vec<_>>();
    let ids = ["task164-merge", "task164-delete", "task164-read"];
    let bundles = inputs
        .iter()
        .zip(ids)
        .map(|(input, id)| {
            analyze_configured_inputs_with_catalog(
                &[ConfiguredSqlInput::new(
                    id,
                    input,
                    "duckdb",
                    dialect.as_ref(),
                )],
                catalog,
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let steps = bundles
        .iter()
        .map(PhysicalTransactionStep::Sql)
        .collect::<Vec<_>>();
    // Catalogs normalize by relation name. Pair each complete population with
    // exactly its authoritative schema, not an independent input vector order.
    let schemas = catalog
        .schemas()
        .iter()
        .filter(|schema| matches!(schema.relation(), "t" | "s"))
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(schemas.len(), 2);
    let states = schemas
        .iter()
        .map(|schema| {
            let rows = match schema.relation() {
                "t" => vec![row(1, Some(10)), row(2, None)],
                "s" => vec![row(1, Some(30))],
                other => panic!("unexpected physical relation {other}"),
            };
            PhysicalWriteState::new(schema.relation(), rows).unwrap()
        })
        .collect();
    let initial = PhysicalScriptCatalog { states, schemas };
    let proof = physical_duckdb_transaction_plan(&steps, &initial);
    let PhysicalTransactionScriptProof::Exact(exact) = proof else {
        panic!("equivalent source facts must preserve mixed-script proof: {proof:?}");
    };
    assert_eq!(exact.checkpoints.len(), 3);
    let merge = exact.checkpoints[0]
        .statement
        .as_ref()
        .unwrap()
        .write
        .as_ref()
        .unwrap();
    assert_eq!(merge.affected().updated(), 1);
    let deletion = exact.checkpoints[1]
        .statement
        .as_ref()
        .unwrap()
        .write
        .as_ref()
        .unwrap();
    assert_eq!(deletion.affected().deleted(), 1);
    let read = exact.checkpoints[2]
        .statement
        .as_ref()
        .unwrap()
        .read
        .as_ref()
        .unwrap();
    assert!(read.rejected_source_ordinals.is_empty());
    assert_eq!(read.output.rows(), &[row(1, Some(30))]);
    let target = exact
        .final_catalog
        .states
        .iter()
        .find(|state| state.relation() == "t")
        .unwrap();
    let domains = exact.final_catalog.exact_domains().unwrap();
    assert!(domains
        .iter()
        .find(|(relation, _)| relation == "t")
        .unwrap()
        .1
        .iter()
        .all(|domain| !domain.values().is_empty()));
    (
        target.rows().to_vec(),
        read.rejected_source_ordinals.clone(),
    )
}

#[test]
fn direct_sql_and_dbt_compiled_metadata_prove_the_same_terminal_bag() {
    assert_eq!(prove(&direct()), prove(&dbt()));
    assert_eq!(prove(&direct()).0, vec![row(1, Some(30))]);
}

#[cfg(feature = "odcs")]
#[test]
fn direct_sql_and_odcs_contract_metadata_prove_the_same_terminal_bag() {
    assert_eq!(prove(&direct()), prove(&odcs()));
}
