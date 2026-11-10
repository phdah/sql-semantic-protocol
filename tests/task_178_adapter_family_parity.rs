//! TASK-178: independent source/adapter metadata parity for the shared
//! portable read families. Adapter provenance does not attest execution.

use serde_json::json;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_manifest_with_schemas, analyze_inputs,
    dialect_from_name, parse_dbt_manifest, ComposedSemantics, ConfiguredSqlInput,
    ProducerExecution, RelationCatalog, SqlInput, ValueDomain,
};

const DIALECTS: &[&str] = &[
    "ansi",
    "bigquery",
    "clickhouse",
    "databricks",
    "duckdb",
    "generic",
    "hive",
    "mssql",
    "mysql",
    "postgresql",
    "redshift",
    "snowflake",
    "sqlite",
];

const CASES: &[(&str, &str)] = &[
    ("predicate", "SELECT amount FROM orders WHERE amount >= 10"),
    (
        "join",
        "SELECT o.id FROM orders o INNER JOIN customers c ON o.id = c.id",
    ),
    (
        "set",
        "SELECT id FROM orders UNION ALL SELECT id FROM customers",
    ),
    (
        "window",
        "SELECT ROW_NUMBER() OVER (ORDER BY id) AS rn FROM orders",
    ),
    ("pagination", "SELECT id FROM orders ORDER BY id LIMIT 2"),
];

fn direct_catalog() -> RelationCatalog {
    let dialect = dialect_from_name("duckdb").expect("exposed parser");
    let ddl = analyze_inputs(
        &[SqlInput::inline(
            "CREATE TABLE orders (id INTEGER, amount INTEGER);
             CREATE TABLE customers (id INTEGER, amount INTEGER)",
        )],
        "duckdb",
        dialect.as_ref(),
    )
    .expect("direct DDL source metadata");
    RelationCatalog::from_schemas(ddl.source_schemas()).expect("complete direct source catalog")
}

fn dbt_catalog(sql: &str) -> RelationCatalog {
    let manifest = json!({
        "metadata": {
            "dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v12.json",
            "dbt_version": "1.11.8", "adapter_type": "duckdb"
        },
        "nodes": {
            "model.task178.read": {
                "unique_id": "model.task178.read", "resource_type": "model",
                "name": "read", "language": "sql",
                "config": {"materialized": "view"},
                "relation_name": "task178_read", "compiled_code": sql,
                "depends_on": {
                    "nodes": ["source.task178.orders", "source.task178.customers"]
                }
            }
        },
        "sources": {
            "source.task178.orders": {
                "unique_id": "source.task178.orders", "resource_type": "source",
                "name": "orders", "relation_name": "orders",
                "columns": {
                    "id": {"name": "id", "data_type": "INTEGER"},
                    "amount": {"name": "amount", "data_type": "INTEGER"}
                }
            },
            "source.task178.customers": {
                "unique_id": "source.task178.customers", "resource_type": "source",
                "name": "customers", "relation_name": "customers",
                "columns": {
                    "id": {"name": "id", "data_type": "INTEGER"},
                    "amount": {"name": "amount", "data_type": "INTEGER"}
                }
            }
        }
    });
    let manifest = parse_dbt_manifest(&manifest.to_string()).expect("compiled dbt v12");
    let dialect = dialect_from_name("duckdb").expect("dbt compile dialect");
    let analyzed = analyze_dbt_manifest_with_schemas(&manifest, "duckdb", dialect.as_ref())
        .expect("dbt compiled read model");
    assert!(
        analyzed
            .producer_evidence()
            .iter()
            .all(|producer| producer.execution() == ProducerExecution::Unattested),
        "dbt materialization metadata must not be promoted to physical execution"
    );
    RelationCatalog::from_schemas(analyzed.source_schemas())
        .expect("dbt source types, not guessed SQL output")
}

#[cfg(feature = "odcs")]
fn odcs_catalog() -> RelationCatalog {
    let contract = "apiVersion: v3.2.0
kind: DataContract
id: task178
name: TASK-178 adapter parity
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
        &RelationCatalog::new(&["orders", "customers"]).expect("named physical relations"),
    )
    .expect("contract source schema");
    RelationCatalog::from_schemas(parsed.source_schemas()).expect("contract schema")
}

fn output_domains(catalog: &RelationCatalog, name: &str, sql: &str) -> Vec<ValueDomain> {
    let dialect = dialect_from_name(name).expect("one of 13 dialects");
    let input = SqlInput::inline(sql);
    let configured = [ConfiguredSqlInput::new(
        "task178-adapter-read",
        &input,
        name,
        dialect.as_ref(),
    )];
    let bundle = analyze_configured_inputs_with_catalog(&configured, catalog)
        .expect("source-typed analysis");
    let ComposedSemantics::Resolved(semantics) = bundle
        .layers()
        .last()
        .expect("one terminal")
        .composed_semantics()
    else {
        panic!("{name}: parser supported shared SQL but not its typed output");
    };
    semantics
        .output()
        .columns()
        .iter()
        .map(|column| column.domain().clone())
        .collect()
}

#[test]
fn all_five_portable_read_families_have_identical_direct_and_dbt_domains_in_13_dialects() {
    let direct = direct_catalog();
    for (family, sql) in CASES {
        let dbt = dbt_catalog(sql);
        for name in DIALECTS {
            let expected = output_domains(&direct, name, sql);
            let from_dbt = output_domains(&dbt, name, sql);
            assert!(
                !expected.is_empty(),
                "{family}/{name}: missing typed output columns"
            );
            assert_eq!(
                from_dbt, expected,
                "{family}/{name}: compiled dbt source types changed canonical output domains"
            );
        }
    }
}

#[cfg(feature = "odcs")]
#[test]
fn all_five_portable_read_families_preserve_odcs_source_type_parity() {
    let direct = direct_catalog();
    let odcs = odcs_catalog();
    for (family, sql) in CASES {
        let dbt = dbt_catalog(sql);
        for name in DIALECTS {
            let expected = output_domains(&direct, name, sql);
            assert_eq!(
                output_domains(&dbt, name, sql),
                expected,
                "{family}/{name}: compiled dbt metadata changed output domains"
            );
            assert_eq!(
                output_domains(&odcs, name, sql),
                expected,
                "{family}/{name}: ODCS source datatype changed output domains"
            );
        }
    }
}
