//! TASK-192: compare the same typed predicate source facts through direct
//! SQL DDL, compiled dbt metadata and ODCS where each can represent them.
//! Adapter parity does not independently establish a native vendor law.

use serde_json::{json, Value};
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_manifest_with_schemas, analyze_inputs,
    dialect_from_name, parse_dbt_manifest, ComposedSemantics, ConfiguredSqlInput,
    ProducerExecution, RelationCatalog, SqlInput, ValueDomain,
};

const MATRIX: &str = include_str!("../docs/task-192-acceptance-freeze.json");

fn direct_catalog(amount_type: &str) -> RelationCatalog {
    let dialect = dialect_from_name("duckdb").expect("public dialect");
    let sql = format!(
        "CREATE TABLE orders (id INTEGER, amount {amount_type}, created_at TIMESTAMP);
         CREATE TABLE customers (id INTEGER, name VARCHAR)"
    );
    let bundle = analyze_inputs(&[SqlInput::inline(&sql)], "duckdb", dialect.as_ref())
        .expect("direct physical SQL source datatypes");
    RelationCatalog::from_schemas(bundle.source_schemas()).expect("direct schemas")
}

fn dbt_source_bundle(
    amount_type: &str,
    sql: &str,
    dialect_name: &str,
) -> sql_semantic_protocol::AnalysisBundle {
    // Fixture dependency edges are explicit physical sources referenced in the
    // original SQL, not all hypothetical sources in the catalog. This tests
    // the compiled model itself, not a convenient substitute query.
    let mut dependencies = Vec::new();
    if sql.contains("orders") {
        dependencies.push("source.task192.orders");
    }
    if sql.contains("customers") {
        dependencies.push("source.task192.customers");
    }
    assert!(
        !dependencies.is_empty(),
        "all 41 source-bearing cases require sources"
    );

    let manifest = json!({
        "metadata": {
            "dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v12.json",
            "dbt_version": "1.11.8",
            "adapter_type": "duckdb"
        },
        "nodes": {
            "model.task192.predicate_probe": {
                "unique_id": "model.task192.predicate_probe",
                "resource_type": "model",
                "name": "predicate_probe",
                "language": "sql",
                "config": {"materialized": "view"},
                "relation_name": "predicate_probe",
                "compiled_code": sql,
                "depends_on": {"nodes": dependencies}
            }
        },
        "sources": {
            "source.task192.orders": {
                "unique_id": "source.task192.orders",
                "resource_type": "source",
                "name": "orders",
                "relation_name": "orders",
                "columns": {
                    "id": {"name": "id", "data_type": "INTEGER"},
                    "amount": {"name": "amount", "data_type": amount_type},
                    "created_at": {"name": "created_at", "data_type": "TIMESTAMP"}
                }
            },
            "source.task192.customers": {
                "unique_id": "source.task192.customers",
                "resource_type": "source",
                "name": "customers",
                "relation_name": "customers",
                "columns": {
                    "id": {"name": "id", "data_type": "INTEGER"},
                    "name": {"name": "name", "data_type": "VARCHAR"}
                }
            }
        }
    });
    let parsed = parse_dbt_manifest(&manifest.to_string()).expect("compiled dbt v12 manifest");
    let dialect = dialect_from_name(dialect_name).expect("public parser");
    analyze_dbt_manifest_with_schemas(&parsed, dialect_name, dialect.as_ref())
        .expect("compiled dbt source and query semantics")
}

fn compiled_dbt_domains(
    bundle: &sql_semantic_protocol::AnalysisBundle,
) -> Option<Vec<ValueDomain>> {
    let layer = bundle.layers().last().expect("compiled dbt terminal");
    match layer.composed_semantics() {
        ComposedSemantics::Resolved(semantics) => Some(
            semantics
                .output()
                .columns()
                .iter()
                .map(|c| c.domain().clone())
                .collect(),
        ),
        _ => None,
    }
}

#[cfg(feature = "odcs")]
fn odcs_catalog(amount_type: &str) -> RelationCatalog {
    let contract = format!(
        "apiVersion: v3.2.0
kind: DataContract
id: task192
name: TASK-192
version: 1.0.0
status: active
schema:
  - name: orders
    physicalName: orders
    properties:
      - name: id
        physicalType: INTEGER
      - name: amount
        physicalType: {amount_type}
      - name: created_at
        physicalType: TIMESTAMP
  - name: customers
    physicalName: customers
    properties:
      - name: id
        physicalType: INTEGER
      - name: name
        physicalType: VARCHAR
"
    );
    let parsed = sql_semantic_protocol::parse_odcs_yaml(
        &contract,
        "duckdb",
        &RelationCatalog::new(&["orders", "customers"]).expect("two named sources"),
    )
    .expect("ODCS source schema evidence");
    RelationCatalog::from_schemas(parsed.source_schemas()).expect("ODCS schemas")
}

fn domains(catalog: &RelationCatalog, dialect_name: &str, sql: &str) -> Option<Vec<ValueDomain>> {
    let dialect = dialect_from_name(dialect_name).expect("public SQL dialect");
    let input = SqlInput::inline(sql);
    let configured = [ConfiguredSqlInput::new(
        "task192-adapter-evidence",
        &input,
        dialect_name,
        dialect.as_ref(),
    )];
    let bundle = analyze_configured_inputs_with_catalog(&configured, catalog)
        .expect("source-typed public analyzer");
    let layer = bundle.layers().last().expect("one query layer");
    match layer.composed_semantics() {
        ComposedSemantics::Resolved(semantics) => Some(
            semantics
                .output()
                .columns()
                .iter()
                .map(|c| c.domain().clone())
                .collect(),
        ),
        _ => None,
    }
}

fn cases() -> Value {
    serde_json::from_str(MATRIX).expect("task-owned versioned matrix")
}

#[test]
fn every_representable_original_predicate_keeps_dbt_and_direct_typed_domains() {
    let matrix = cases();
    let dialects = matrix["dialects"].as_array().expect("13 dialects");
    let integer = direct_catalog("INTEGER");
    let float = direct_catalog("DOUBLE");
    let mut verified = 0;
    for case in matrix["cases"].as_array().expect("43 original cases") {
        let id = case["id"].as_str().expect("named case");
        if id == "88-C177-S033" || id == "88-C177-S179" {
            // The original scalar inputs have no physical table facts for dbt
            // or ODCS to attest; this is an explicit not-applicable boundary.
            continue;
        }
        let sql = case["sql"].as_str().expect("original SQL");
        let floating = matches!(id, "88-C177-S040" | "88-C177-S041");
        let (direct, amount_type) = if floating {
            (&float, "DOUBLE")
        } else {
            (&integer, "INTEGER")
        };
        for name in dialects {
            let name = name.as_str().expect("dialect");
            let expected = domains(direct, name, sql);
            assert!(
                expected.as_ref().is_some_and(|c| !c.is_empty()),
                "{id}/{name}: missing typed output"
            );
            let dbt = dbt_source_bundle(amount_type, sql, name);
            let actual = compiled_dbt_domains(&dbt);
            assert_eq!(
                actual, expected,
                "{id}/{name}: original compiled dbt SQL parity"
            );
            let sourced = RelationCatalog::from_schemas(dbt.source_schemas())
                .expect("dbt exported typed source facts");
            assert_eq!(
                domains(&sourced, name, sql),
                expected,
                "{id}/{name}: dbt source facts changed when exported to a catalog"
            );
            assert!(
                dbt.producer_evidence()
                    .iter()
                    .all(|producer| producer.execution() == ProducerExecution::Unattested),
                "{id}/{name}: compiled dbt metadata is not native execution"
            );
            verified += 1;
        }
    }
    assert_eq!(verified, 41 * 13);
}

#[cfg(feature = "odcs")]
#[test]
fn every_representable_original_predicate_keeps_odcs_and_direct_typed_domains() {
    let matrix = cases();
    let dialects = matrix["dialects"].as_array().expect("13 dialects");
    let direct_integer = direct_catalog("INTEGER");
    let direct_float = direct_catalog("DOUBLE");
    let odcs_integer = odcs_catalog("INTEGER");
    let odcs_float = odcs_catalog("DOUBLE");
    let mut verified = 0;
    for case in matrix["cases"].as_array().expect("43 original cases") {
        let id = case["id"].as_str().expect("original ID");
        if id == "88-C177-S033" || id == "88-C177-S179" {
            continue;
        }
        let sql = case["sql"].as_str().expect("original SQL");
        let floating = matches!(id, "88-C177-S040" | "88-C177-S041");
        let (direct, odcs) = if floating {
            (&direct_float, &odcs_float)
        } else {
            (&direct_integer, &odcs_integer)
        };
        for name in dialects {
            let name = name.as_str().expect("dialect");
            let expected = domains(direct, name, sql);
            let actual = domains(odcs, name, sql);
            assert!(
                expected.as_ref().is_some_and(|c| !c.is_empty()),
                "{id}/{name}: missing source domain"
            );
            assert_eq!(actual, expected, "{id}/{name}: ODCS source-fact parity");
            verified += 1;
        }
    }
    assert_eq!(verified, 41 * 13);
}
