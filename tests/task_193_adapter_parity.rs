//! TASK-193: authoritative direct DDL, compiled dbt source metadata and
//! ODCS parity for join source datatypes and resulting canonical domains.
//! No catalog adapter certifies an external SQL engine or source-row proof.

use serde_json::{json, Value};
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_manifest_with_schemas, analyze_inputs,
    dialect_from_name, parse_dbt_manifest, ComposedSemantics, ConfiguredSqlInput, ProducerExecution,
    RelationCatalog, SqlInput, ValueDomain,
};

const FREEZE: &str = include_str!("../docs/task-193-acceptance-freeze.json");

fn direct_catalog() -> RelationCatalog {
    let dialect = dialect_from_name("duckdb").expect("project dialect");
    let bundle = analyze_inputs(
        &[SqlInput::inline(
            "CREATE TABLE orders(id INTEGER, amount INTEGER);
             CREATE TABLE customers(id INTEGER, amount INTEGER);
             CREATE TABLE products(id INTEGER)",
        )],
        "duckdb",
        dialect.as_ref(),
    )
    .expect("direct source DDL");
    RelationCatalog::from_schemas(bundle.source_schemas()).expect("source metadata")
}

fn dbt_catalog(sql: &str) -> RelationCatalog {
    let mut sources = vec!["source.task193.orders"];
    if sql.contains("customers") {
        sources.push("source.task193.customers");
    }
    if sql.contains("products") {
        sources.push("source.task193.products");
    }
    let manifest = json!({
        "metadata": {
            "dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v12.json",
            "dbt_version": "1.11.8",
            "adapter_type": "duckdb"
        },
        "nodes": {
            "model.task193.join": {
                "unique_id": "model.task193.join",
                "resource_type": "model",
                "name": "join", "language": "sql",
                "config": {"materialized": "view"},
                "relation_name": "task193_join",
                "compiled_code": sql,
                "depends_on": {"nodes": sources}
            }
        },
        "sources": {
            "source.task193.orders": {
                "unique_id": "source.task193.orders",
                "resource_type": "source", "name": "orders",
                "relation_name": "orders",
                "columns": {
                    "id": {"name": "id", "data_type": "INTEGER"},
                    "amount": {"name": "amount", "data_type": "INTEGER"}
                }
            },
            "source.task193.customers": {
                "unique_id": "source.task193.customers",
                "resource_type": "source", "name": "customers",
                "relation_name": "customers",
                "columns": {
                    "id": {"name": "id", "data_type": "INTEGER"},
                    "amount": {"name": "amount", "data_type": "INTEGER"}
                }
            },
            "source.task193.products": {
                "unique_id": "source.task193.products",
                "resource_type": "source", "name": "products",
                "relation_name": "products",
                "columns": {
                    "id": {"name": "id", "data_type": "INTEGER"}
                }
            }
        }
    });
    let compiled = parse_dbt_manifest(&manifest.to_string()).expect("real compiled dbt SQL");
    let dialect = dialect_from_name("duckdb").expect("public dbt dialect");
    let analyzed = analyze_dbt_manifest_with_schemas(&compiled, "duckdb", dialect.as_ref())
        .expect("source and compiled model");
    assert!(
        analyzed
            .producer_evidence()
            .iter()
            .all(|producer| producer.execution() == ProducerExecution::Unattested),
        "dbt source declarations are not native engine execution"
    );
    RelationCatalog::from_schemas(analyzed.source_schemas()).expect("dbt authoritative facts")
}

#[cfg(feature = "odcs")]
fn odcs_catalog() -> RelationCatalog {
    let contract = "apiVersion: v3.2.0
kind: DataContract
id: task193
name: TASK-193 adapter parity
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
  - name: products
    physicalName: products
    properties:
      - name: id
        physicalType: INTEGER
";
    let parsed = sql_semantic_protocol::parse_odcs_yaml(
        contract,
        "duckdb",
        &RelationCatalog::new(&["orders", "customers", "products"])
            .expect("three physical source identities"),
    )
    .expect("schema and type facts");
    RelationCatalog::from_schemas(parsed.source_schemas()).expect("ODCS source catalog")
}

fn canonical_domains(catalog: &RelationCatalog, sql: &str, name: &str) -> Vec<ValueDomain> {
    let dialect = dialect_from_name(name).expect("one of 13 exposed parsers");
    let input = SqlInput::inline(sql);
    let configured = [ConfiguredSqlInput::new(
        "task193-join-parity",
        &input,
        name,
        dialect.as_ref(),
    )];
    let bundle = analyze_configured_inputs_with_catalog(&configured, catalog)
        .expect("typed source-backed original SQL");
    let ComposedSemantics::Resolved(semantics) = bundle
        .layers()
        .last()
        .expect("one joined output")
        .composed_semantics()
    else {
        panic!("{name}: expected typed canonical join output, not silent unsupported");
    };
    semantics
        .output()
        .columns()
        .iter()
        .map(|column| column.domain().clone())
        .collect()
}

fn source_representable_cases() -> Vec<(String, String)> {
    let freeze: Value = serde_json::from_str(FREEZE).expect("TASK-193 frozen inventory");
    freeze["cases"]
        .as_array()
        .expect("24 original joins")
        .iter()
        .filter(|case| {
            // TASK-178's 21 native join SQLs already have independent DuckDB
            // output oracles. LEFT SEMI/ANTI native spelling is not equivalent
            // to original syntax across all engines; those are separate parse
            // boundaries. The MSSQL LATERAL AST difference remains TASK-179/191.
            case["id"] != "88-C177-S054"
                && case["id"] != "88-C177-S055"
                && case["id"] != "88-C177-S152"
        })
        .map(|case| {
            (
                case["id"].as_str().expect("source ID").to_string(),
                case["sql"].as_str().expect("original SQL").to_string(),
            )
        })
        .collect()
}

#[test]
fn every_21_original_native_join_cases_keep_typed_sql_and_dbt_parity() {
    let direct = direct_catalog();
    let cases = source_representable_cases();
    assert_eq!(cases.len(), 21);
    let dialects = serde_json::from_str::<Value>(FREEZE).expect("dialects");
    for (id, sql) in &cases {
        let dbt = dbt_catalog(sql);
        for dialect in dialects["dialects"].as_array().expect("13 names") {
            let name = dialect.as_str().expect("dialect");
            let expected = canonical_domains(&direct, sql, name);
            assert!(!expected.is_empty(), "{id}/{name}: missing output columns");
            assert_eq!(
                canonical_domains(&dbt, sql, name),
                expected,
                "{id}/{name}: compiled dbt must preserve the same typed source facts"
            );
        }
    }
}

#[cfg(feature = "odcs")]
#[test]
fn every_21_original_native_join_cases_keep_typed_odcs_parity() {
    let direct = direct_catalog();
    let odcs = odcs_catalog();
    let dialects = serde_json::from_str::<Value>(FREEZE).expect("dialects");
    for (id, sql) in source_representable_cases() {
        for name in dialects["dialects"].as_array().expect("13 names") {
            let name = name.as_str().expect("dialect");
            assert_eq!(
                canonical_domains(&odcs, &sql, name),
                canonical_domains(&direct, &sql, name),
                "{id}/{name}: authoritative equivalent source schema must not change output domains"
            );
        }
    }
}

#[test]
fn exceptional_original_syntax_and_vendor_ast_cells_are_not_approved_as_portable_native() {
    let freeze: Value = serde_json::from_str(FREEZE).expect("acceptance freeze");
    for (id, expected_boundary) in [
        ("88-C177-S054", "original_LEFT_SEMI_rejected_by_duckdb_parser"),
        ("88-C177-S055", "original_LEFT_ANTI_rejected_by_duckdb_parser"),
        (
            "88-C177-S152",
            "MSSQL_different_AST_requires_TASK_179_TASK_191_adjudication",
        ),
    ] {
        let case = freeze["cases"]
            .as_array()
            .expect("original joins")
            .iter()
            .find(|case| case["id"] == id)
            .expect("explicit original syntax boundary");
        assert_eq!(case["original_duckdb_syntax_boundary"], expected_boundary);
    }
    assert_eq!(freeze["external_vendor_execution_attested"], false);
}
