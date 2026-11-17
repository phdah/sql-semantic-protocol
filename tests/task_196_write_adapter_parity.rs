//! TASK-196 authoritative source-fact parity for original DML SQL.
//!
//! dbt manifest/catalog and ODCS provide source evidence, not executable
//! INSERT/UPDATE/DELETE/MERGE programs or foreign vendor runtime attestation.

use serde_json::{json, Value};
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_manifest_with_schemas, analyze_inputs,
    dialect_from_name, parse_dbt_manifest, to_bundle_json, ConfiguredSqlInput, ProducerExecution,
    RelationCatalog, SqlInput,
};

const FREEZE: &str = include_str!("../docs/task-196-acceptance-freeze.json");

fn original() -> Value {
    serde_json::from_str(FREEZE).expect("original 38 statement inventory")
}

fn source_catalog_from_direct() -> RelationCatalog {
    let dialect = dialect_from_name("duckdb").expect("public dialect");
    let ddl = analyze_inputs(
        &[SqlInput::inline(
            "CREATE TABLE orders(id INTEGER, amount INTEGER);
             CREATE TABLE customers(id INTEGER, amount INTEGER)",
        )],
        "duckdb",
        dialect.as_ref(),
    )
    .expect("direct source schema");
    RelationCatalog::from_schemas(ddl.source_schemas()).expect("typed direct physical source")
}

fn source_catalog_from_dbt() -> RelationCatalog {
    let sources = ["orders", "customers"]
        .into_iter()
        .map(|relation| {
            let id = format!("source.task196.{relation}");
            (
                id.clone(),
                json!({
                    "unique_id":id, "resource_type":"source", "name":relation,
                    "relation_name":relation,
                    "columns":{
                        "id":{"name":"id","data_type":"INTEGER"},
                        "amount":{"name":"amount","data_type":"INTEGER"}
                    }
                }),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    let manifest = json!({
        "metadata":{
            "dbt_schema_version":"https://schemas.getdbt.com/dbt/manifest/v12.json",
            "dbt_version":"1.11.8", "adapter_type":"duckdb"
        },
        "nodes":{
            "model.task196.read":{
                "unique_id":"model.task196.read","resource_type":"model",
                "name":"read","language":"sql","relation_name":"task196_read",
                "config":{"materialized":"view"},
                "compiled_code":"SELECT orders.id FROM orders JOIN customers ON orders.id=customers.id",
                "depends_on":{"nodes":["source.task196.orders","source.task196.customers"]}
            }
        },
        "sources":sources
    });
    let parsed = parse_dbt_manifest(&manifest.to_string()).expect("compiled source references");
    let dialect = dialect_from_name("duckdb").expect("dbt compiled dialect");
    let bundle = analyze_dbt_manifest_with_schemas(&parsed, "duckdb", dialect.as_ref())
        .expect("authoritative complete manifest source types");
    assert!(
        bundle
            .producer_evidence()
            .iter()
            .all(|producer| producer.execution() == ProducerExecution::Unattested),
        "dbt compiled model is not evidence of physical execution"
    );
    RelationCatalog::from_schemas(bundle.source_schemas()).expect("dbt typed source facts")
}

#[cfg(feature = "odcs")]
fn source_catalog_from_odcs() -> RelationCatalog {
    let contract = "apiVersion: v3.2.0
kind: DataContract
id: task196-writes
name: TASK-196 source metadata
version: 1.0.0
status: active
schema:
  - name: orders
    physicalName: orders
    properties:
      - name: id
        physicalType: INTEGER
      - name: amount
        physicalType: INTEGER
  - name: customers
    physicalName: customers
    properties:
      - name: id
        physicalType: INTEGER
      - name: amount
        physicalType: INTEGER
";
    let parsed = sql_semantic_protocol::parse_odcs_yaml(
        contract,
        "duckdb",
        &RelationCatalog::new(&["orders", "customers"]).expect("contract physical names"),
    )
    .expect("ODCS source columns");
    RelationCatalog::from_schemas(parsed.source_schemas()).expect("ODCS typed source")
}

fn observed_statement(catalog: &RelationCatalog, sql: &str, dialect_name: &str) -> Value {
    let dialect = dialect_from_name(dialect_name).expect("named project dialect");
    let source = SqlInput::inline(sql);
    let configured = [ConfiguredSqlInput::new(
        "task196-source-parity",
        &source,
        dialect_name,
        dialect.as_ref(),
    )];
    let bundle = analyze_configured_inputs_with_catalog(&configured, catalog)
        .unwrap_or_else(|error| panic!("{dialect_name}: original DML: {error}"));
    let document: Value =
        serde_json::from_str(&to_bundle_json(&bundle)).expect("canonical document JSON");
    document["inputs"][0]["statements"][0].clone()
}

#[test]
fn all_original_18_dml_programs_preserve_canonical_wire_across_direct_and_dbt_sources() {
    let freeze = original();
    let direct = source_catalog_from_direct();
    let dbt = source_catalog_from_dbt();
    let mut compared = 0;
    let mut rejected = 0;
    for case in freeze["cases"].as_array().expect("38 stable originals") {
        if !case["feature_id"]
            .as_str()
            .expect("case family")
            .starts_with("dml.")
        {
            continue;
        }
        let id = case["id"].as_str().expect("original ID");
        let sql = case["sql"].as_str().expect("original SQL");
        for cell in case["cells"].as_array().expect("13 dialect rows") {
            let name = cell["dialect"].as_str().expect("dialect");
            if cell["original_observed_status"] == "R" {
                rejected += 1;
                continue;
            }
            let expected = observed_statement(&direct, sql, name);
            let actual = observed_statement(&dbt, sql, name);
            assert_eq!(
                actual, expected,
                "{id}/{name}: compiled dbt source facts cannot change the SQL write contract"
            );
            assert!(
                expected.is_object(),
                "{id}/{name}: no silent missing statement"
            );
            compared += 1;
        }
    }
    assert_eq!(compared + rejected, 18 * 13);
    assert_eq!(
        rejected, 10,
        "REPLACE INTO has ten parser-specific rejections"
    );
}

#[cfg(feature = "odcs")]
#[test]
fn all_original_224_parsed_dml_cells_preserve_odcs_source_fact_parity() {
    let freeze = original();
    let direct = source_catalog_from_direct();
    let odcs = source_catalog_from_odcs();
    let mut checked = 0;
    for case in freeze["cases"].as_array().expect("original variants") {
        if !case["feature_id"]
            .as_str()
            .expect("family")
            .starts_with("dml.")
        {
            continue;
        }
        let id = case["id"].as_str().expect("original SQL");
        let sql = case["sql"].as_str().expect("SQL");
        for cell in case["cells"].as_array().expect("dialects") {
            if cell["original_observed_status"] == "R" {
                continue;
            }
            let name = cell["dialect"].as_str().expect("dialect name");
            assert_eq!(
                observed_statement(&odcs, sql, name),
                observed_statement(&direct, sql, name),
                "{id}/{name}: ODCS datatype facts changed canonical mutation result"
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 224);
}
