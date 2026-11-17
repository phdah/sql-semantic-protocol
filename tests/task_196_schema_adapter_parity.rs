//! TASK-196 authoritative original schema PK and DECIMAL adapter facts.
//!
//! Independent dbt/ODCS declarations may supply the same canonical key and
//! column types, but neither attests physical enforcement or vendor DDL.

use serde_json::{json, Value};
use sql_semantic_protocol::{
    analyze_dbt_manifest_with_schemas, analyze_inputs, dialect_from_name, parse_dbt_manifest,
    DataType, RelationCatalog, RelationConstraint, RelationConstraintSet, SchemaSourceKind,
    SqlInput,
};

const FREEZE: &str = include_str!("../docs/task-196-acceptance-freeze.json");

fn primary_key(sets: &[RelationConstraintSet]) -> Vec<String> {
    sets.iter()
        .find(|set| set.relation() == "orders")
        .expect("authoritative original orders constraint facts")
        .constraints()
        .iter()
        .find_map(|fact| match fact {
            RelationConstraint::PrimaryKey(key) => Some(key.columns().to_vec()),
            _ => None,
        })
        .expect("canonical id primary key")
}

fn assert_decimal(schemas: &[sql_semantic_protocol::RelationSchema]) {
    let orders = schemas
        .iter()
        .find(|schema| schema.relation() == "orders")
        .expect("typed original relation");
    assert_eq!(
        orders
            .columns()
            .iter()
            .find(|column| column.name() == "amount")
            .expect("original amount source")
            .data_type(),
        &DataType::Decimal {
            precision: Some(10),
            scale: Some(2)
        }
    );
    assert_eq!(
        orders
            .columns()
            .iter()
            .find(|column| column.name() == "id")
            .expect("original id source")
            .data_type(),
        &DataType::SignedInteger { bits: Some(32) }
    );
}

fn compiled_dbt() -> sql_semantic_protocol::AnalysisBundle {
    let metadata = json!({
        "metadata":{
            "dbt_schema_version":"https://schemas.getdbt.com/dbt/manifest/v12.json",
            "dbt_version":"1.11.8","adapter_type":"duckdb"
        },
        "nodes":{
            "model.task196.orders_read":{
                "unique_id":"model.task196.orders_read","resource_type":"model",
                "name":"orders_read","language":"sql",
                "relation_name":"orders_read",
                "config":{"materialized":"view"},
                "compiled_code":"SELECT id, amount FROM orders",
                "depends_on":{"nodes":["source.task196.orders"]}
            }
        },
        "sources":{
            "source.task196.orders":{
                "unique_id":"source.task196.orders","resource_type":"source",
                "name":"orders","relation_name":"orders",
                "columns":{
                    "id":{"name":"id","data_type":"INTEGER"},
                    "amount":{"name":"amount","data_type":"DECIMAL(10,2)"}
                },
                "constraints":[{"type":"primary_key","columns":["id"]}]
            }
        }
    });
    let parsed = parse_dbt_manifest(&metadata.to_string()).expect("compiled source metadata");
    let dialect = dialect_from_name("duckdb").expect("source parser");
    analyze_dbt_manifest_with_schemas(&parsed, "duckdb", dialect.as_ref())
        .expect("complete dbt declared source schema")
}

#[test]
fn original_decimal_primary_keys_are_canonical_across_sql_and_dbt_with_distinct_provenance() {
    let frozen: Value = serde_json::from_str(FREEZE).expect("original SQL");
    let dbt = compiled_dbt();
    assert_decimal(dbt.source_schemas());
    assert_eq!(primary_key(dbt.relation_constraints()), vec!["id"]);
    assert_eq!(
        dbt.source_schemas()[0].source_kind(),
        Some(SchemaSourceKind::DbtManifest)
    );

    let mut checked = 0;
    for suffix in ["S190", "S192"] {
        let id = format!("88-C177-{suffix}");
        let case = frozen["cases"]
            .as_array()
            .expect("original variants")
            .iter()
            .find(|entry| entry["id"] == id)
            .expect("original SQL DDL");
        for cell in case["cells"].as_array().expect("13 frozen parser cells") {
            let name = cell["dialect"].as_str().expect("dialect");
            let parser = dialect_from_name(name).expect("public parser");
            let ddl = analyze_inputs(
                &[SqlInput::inline(
                    case["sql"].as_str().expect("immutable SQL"),
                )],
                name,
                parser.as_ref(),
            )
            .unwrap_or_else(|err| panic!("{id}/{name}: {err}"));
            assert_decimal(ddl.source_schemas());
            assert_eq!(
                primary_key(ddl.relation_constraints()),
                primary_key(dbt.relation_constraints()),
                "{id}/{name}: SQL and dbt canonical key names"
            );
            let source = RelationCatalog::from_schemas(dbt.source_schemas())
                .expect("complete declared source");
            assert!(
                source
                    .schemas()
                    .iter()
                    .any(|schema| schema.relation() == "orders"),
                "{id}/{name}: compiled dbt source not a physical execution attestation"
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 26);
}

#[cfg(feature = "odcs")]
#[test]
fn original_decimal_primary_keys_match_odcs_typed_metadata_without_enforcement_claims() {
    let contract = "apiVersion: v3.2.0
kind: DataContract
id: task196-schema
name: TASK-196 PK and DECIMAL
version: 1.0.0
status: active
schema:
  - name: orders
    physicalName: orders
    properties:
      - name: id
        physicalType: INTEGER
        primaryKey: true
      - name: amount
        physicalType: DECIMAL(10,2)
";
    let parsed = sql_semantic_protocol::parse_odcs_yaml(
        contract,
        "duckdb",
        &RelationCatalog::new(&["orders"]).expect("explicit physical source"),
    )
    .expect("ODCS typed table declaration");
    assert_decimal(parsed.source_schemas());
    assert_eq!(
        parsed.source_schemas()[0].source_kind(),
        Some(SchemaSourceKind::ExternalMetadata)
    );
    let dbt = compiled_dbt();
    assert_eq!(
        primary_key(parsed.relation_constraints()),
        primary_key(dbt.relation_constraints()),
        "canonical PK identities agree while enforcement provenance stays distinct"
    );
}
