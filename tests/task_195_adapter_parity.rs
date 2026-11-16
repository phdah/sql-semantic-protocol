//! TASK-195: original 36 SQL statements analyzed against equivalent typed
//! source facts from direct DDL, compiled dbt and ODCS. This is metadata
//! parity only. It must not be promoted to a vendor-native execution claim.

use serde_json::{json, Value};
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_manifest_with_schemas, analyze_inputs,
    dialect_from_name, parse_dbt_manifest, ComposedSemantics, ConfiguredSqlInput,
    ProducerExecution, RelationCatalog, SqlInput, ValueDomain,
};

const FREEZE: &str = include_str!("../docs/task-195-acceptance-freeze.json");

fn frozen() -> Value {
    serde_json::from_str(FREEZE).expect("complete original TASK-195 SQL")
}

fn direct_catalog() -> RelationCatalog {
    let dialect = dialect_from_name("duckdb").expect("supported source parser");
    let bundle = analyze_inputs(
        &[SqlInput::inline(
            "CREATE TABLE orders(id INTEGER, amount INTEGER);
             CREATE TABLE customers(id INTEGER, amount INTEGER);
             CREATE TABLE products(id INTEGER)",
        )],
        "duckdb",
        dialect.as_ref(),
    )
    .expect("direct authoritative DDL source columns");
    RelationCatalog::from_schemas(bundle.source_schemas()).expect("direct source schema")
}

fn compiled_dbt_catalog(sql: &str) -> RelationCatalog {
    let mut dependencies = vec!["source.task195.orders"];
    if sql.contains("customers") {
        dependencies.push("source.task195.customers");
    }
    let manifest = json!({
        "metadata": {
            "dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v12.json",
            "dbt_version": "1.11.8",
            "adapter_type": "duckdb"
        },
        "nodes": {
            "model.task195.result": {
                "unique_id": "model.task195.result",
                "resource_type": "model",
                "name": "result",
                "language": "sql",
                "config": {"materialized": "view"},
                "relation_name": "task195_result",
                "compiled_code": sql,
                "depends_on": {"nodes": dependencies}
            }
        },
        "sources": {
            "source.task195.orders": {
                "unique_id": "source.task195.orders",
                "resource_type": "source",
                "name": "orders",
                "relation_name": "orders",
                "columns": {
                    "id": {"name": "id", "data_type": "INTEGER"},
                    "amount": {"name": "amount", "data_type": "INTEGER"}
                }
            },
            "source.task195.customers": {
                "unique_id": "source.task195.customers",
                "resource_type": "source",
                "name": "customers",
                "relation_name": "customers",
                "columns": {
                    "id": {"name": "id", "data_type": "INTEGER"},
                    "amount": {"name": "amount", "data_type": "INTEGER"}
                }
            }
        }
    });
    let parsed = parse_dbt_manifest(&manifest.to_string()).expect("original compiled SQL");
    let dialect = dialect_from_name("duckdb").expect("same dbt SQL parser");
    let bundle = analyze_dbt_manifest_with_schemas(&parsed, "duckdb", dialect.as_ref())
        .expect("compiled SQL and authoritative dbt source types");
    assert!(
        bundle
            .producer_evidence()
            .iter()
            .all(|producer| producer.execution() == ProducerExecution::Unattested),
        "dbt compiled model metadata is not a native runtime attestation"
    );
    RelationCatalog::from_schemas(bundle.source_schemas()).expect("dbt typed source catalog")
}

#[cfg(feature = "odcs")]
fn odcs_catalog() -> RelationCatalog {
    let contract = r#"apiVersion: v3.2.0
kind: DataContract
id: task195
name: Window and ordering typed metadata
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
"#;
    let parsed = sql_semantic_protocol::parse_odcs_yaml(
        contract,
        "duckdb",
        &RelationCatalog::new(&["orders", "customers"]).expect("physical source names"),
    )
    .expect("ODCS exact matching source facts");
    RelationCatalog::from_schemas(parsed.source_schemas()).expect("ODCS typed source catalog")
}

fn typed_domains(
    catalog: &RelationCatalog,
    sql: &str,
    dialect_name: &str,
) -> Option<Vec<ValueDomain>> {
    let dialect = dialect_from_name(dialect_name).expect("named project dialect");
    let input = SqlInput::inline(sql);
    let configured = [ConfiguredSqlInput::new(
        "task195-original",
        &input,
        dialect_name,
        dialect.as_ref(),
    )];
    let bundle =
        analyze_configured_inputs_with_catalog(&configured, catalog).unwrap_or_else(|error| {
            panic!("{dialect_name}: expected source-backed original SQL: {error}")
        });
    let terminal = bundle
        .layers()
        .last()
        .expect("terminal source-aware output layer");
    match terminal.composed_semantics() {
        ComposedSemantics::Resolved(semantics) => Some(
            semantics
                .output()
                .columns()
                .iter()
                .map(|column| column.domain().clone())
                .collect(),
        ),
        _ => None,
    }
}

#[test]
fn every_supported_original_dialect_preserves_direct_and_compiled_dbt_typed_domains() {
    let fixture = frozen();
    let direct = direct_catalog();
    let mut cells = 0;
    let mut unresolved = 0;
    for case in fixture["cases"].as_array().expect("36 original SQL cases") {
        let id = case["id"].as_str().expect("stable original ID");
        let sql = case["sql"].as_str().expect("original SQL");
        let dbt = compiled_dbt_catalog(sql);
        for cell in case["cells"].as_array().expect("13 dialect variants") {
            if cell["original_observed_status"] == "R" {
                continue;
            }
            let dialect = cell["dialect"].as_str().expect("named parser dialect");
            let expected = typed_domains(&direct, sql, dialect);
            let actual = typed_domains(&dbt, sql, dialect);
            assert_eq!(
                actual, expected,
                "{id}/{dialect}: compiled dbt source evidence must match direct DDL"
            );
            if let Some(domains) = expected {
                assert!(
                    !domains.is_empty(),
                    "{id}/{dialect}: supported SQL cannot silently discard its output"
                );
            } else {
                unresolved += 1;
            }
            cells += 1;
        }
    }
    assert_eq!(
        cells, 467,
        "468 cells with one pinned MSSQL parser rejection"
    );
    assert_eq!(
        unresolved, 0,
        "supported semantics must not disappear from source-typed composition"
    );
}

#[cfg(feature = "odcs")]
#[test]
fn all_original_467_parser_accepted_cells_match_odcs_and_direct_typed_source_facts() {
    let fixture = frozen();
    let direct = direct_catalog();
    let odcs = odcs_catalog();
    let mut cells = 0;
    for case in fixture["cases"].as_array().expect("36 original SQL cases") {
        let id = case["id"].as_str().expect("original SQL ID");
        let sql = case["sql"].as_str().expect("immutable original SQL");
        for cell in case["cells"].as_array().expect("13 dialect variants") {
            if cell["original_observed_status"] == "R" {
                continue;
            }
            let name = cell["dialect"].as_str().expect("named parser dialect");
            assert_eq!(
                typed_domains(&odcs, sql, name),
                typed_domains(&direct, sql, name),
                "{id}/{name}: ODCS schema metadata cannot change canonical SQL outcomes"
            );
            cells += 1;
        }
    }
    assert_eq!(cells, 467);
}
