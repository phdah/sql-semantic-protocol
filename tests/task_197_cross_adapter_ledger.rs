//! TASK-197: reject omissions and false certification in the original
//! 163-variant/13-dialect cross-adapter ledger, then exercise public typed
//! output domains through all three representable source-evidence adapters.
//! Neither metadata equality nor parser acceptance proves vendor execution.

use serde_json::{json, Value};
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_manifest_with_schemas, analyze_inputs,
    dialect_from_name, parse_dbt_manifest, ComposedSemantics, ConfiguredSqlInput,
    ProducerExecution, RelationCatalog, SqlInput, ValueDomain,
};

const LEDGER: &str = include_str!("../docs/task-197-acceptance-freeze.json");
const ORIGINAL: &str = include_str!("../docs/task-178-full-variant-inventory.json");

fn ledger() -> Value {
    serde_json::from_str(LEDGER).expect("task-owned cross-adapter freeze")
}

fn upstream_freezes() -> Vec<Value> {
    [
        include_str!("../docs/task-192-acceptance-freeze.json"),
        include_str!("../docs/task-193-acceptance-freeze.json"),
        include_str!("../docs/task-194-acceptance-freeze.json"),
        include_str!("../docs/task-195-acceptance-freeze.json"),
        include_str!("../docs/task-196-acceptance-freeze.json"),
    ]
    .iter()
    .map(|text| serde_json::from_str(text).expect("versioned upstream freeze"))
    .collect()
}

fn observed_status(cell: &Value) -> &str {
    [
        "observed_parser_canonical_status",
        "observed_status",
        "original_observed_status",
    ]
    .iter()
    .find_map(|field| cell[*field].as_str())
    .expect("all upstream parser cells require an explicit observation")
}

#[test]
fn every_original_variant_dialect_and_owner_has_an_unapproved_evidence_disposition() {
    let frozen = ledger();
    let source: Value = serde_json::from_str(ORIGINAL).expect("original 163 SQL variants");
    let cases = frozen["cases"].as_array().expect("ledger cases");
    let original = source["cases"].as_array().expect("original case inventory");
    let dialects = frozen["dialects"].as_array().expect("13 dialect names");
    assert_eq!(dialects.len(), 13);
    assert_eq!(cases.len(), 163);
    assert_eq!(original.len(), 163);
    assert_eq!(frozen["task197_signoff"], false);
    assert_eq!(frozen["parent_task88_signoff"], false);
    assert_eq!(frozen["release_gate"], false);

    let mut observed_ids = std::collections::BTreeSet::new();
    let mut observed_cells = 0;
    for freeze in upstream_freezes() {
        let task = freeze["task_id"].as_str().expect("source owner");
        for upstream in freeze["cases"].as_array().expect("frozen original cases") {
            let id = upstream["id"].as_str().expect("stable source ID");
            assert!(observed_ids.insert(id.to_string()), "{id}: duplicated family owner");
            let case = cases.iter().find(|entry| entry["id"] == id)
                .unwrap_or_else(|| panic!("{id}: missing TASK-197 ledger case"));
            let original_case = original.iter().find(|entry| entry["id"] == id)
                .unwrap_or_else(|| panic!("{id}: missing original TASK-178 SQL"));
            assert_eq!(case["owner"], task, "{id}: owner drift");
            assert_eq!(case["feature_id"], upstream["feature_id"], "{id}: feature drift");
            assert_eq!(case["sql"], upstream["sql"], "{id}: owner SQL drift");
            assert_eq!(case["sql"], original_case["sql"], "{id}: original SQL drift");
            assert_eq!(case["evidence"]["owner_freeze"],
                format!("docs/task-{}-acceptance-freeze.json", &task[5..]));
            assert!(case["evidence"]["direct_sql"].as_array().is_some_and(|v| !v.is_empty()));
            assert!(case["evidence"]["independent_semantic_review"].as_str().is_some());

            let source_cells = upstream["dialect_cells"].as_array()
                .or_else(|| upstream["cells"].as_array())
                .expect("source dialect obligations");
            let ledger_cells = case["cells"].as_array().expect("ledger dialect obligations");
            assert_eq!(source_cells.len(), 13, "{id}: incomplete source dialect inventory");
            assert_eq!(ledger_cells.len(), 13, "{id}: incomplete adapter inventory");
            for (index, (source_cell, entry)) in source_cells.iter().zip(ledger_cells).enumerate() {
                let name = dialects[index].as_str().expect("dialect name");
                let status = observed_status(source_cell);
                assert_eq!(source_cell["dialect"], name, "{id}: source dialect drift");
                assert_eq!(entry["dialect"], name, "{id}: ledger dialect drift");
                assert_eq!(entry["original_observation"], status, "{id}/{name}: observation drift");
                let expected = if status == "R" || status == "U" {
                    "unrepresentable"
                } else {
                    "conditional"
                };
                assert_eq!(entry["status"], expected,
                    "{id}/{name}: unsupported or unattested evidence promoted");
                assert_eq!(entry["external_native_execution"], "unattested");
                assert!(entry["reason"].as_str().is_some_and(|value| !value.is_empty()));
                observed_cells += 1;
            }
        }
    }
    assert_eq!(observed_ids.len(), 163);
    assert_eq!(observed_cells, 2119);
    assert_eq!(frozen["case_count"], 163);
    assert_eq!(frozen["cell_count"], 2119);
    assert!(cases.iter().all(|case| case["id"].as_str().is_some_and(|id| observed_ids.contains(id))));
}

fn direct_catalog() -> RelationCatalog {
    let dialect = dialect_from_name("duckdb").expect("public dialect");
    let ddl = "CREATE TABLE orders(id INTEGER, amount INTEGER);
               CREATE TABLE customers(id INTEGER, amount INTEGER);
               CREATE TABLE products(id INTEGER)";
    let bundle = analyze_inputs(&[SqlInput::inline(ddl)], "duckdb", dialect.as_ref())
        .expect("authoritative direct DDL");
    RelationCatalog::from_schemas(bundle.source_schemas()).expect("direct typed source")
}

fn dbt_catalog(sql: &str) -> RelationCatalog {
    let dependencies = ["orders", "customers", "products"]
        .iter()
        .filter(|name| sql.contains(*name))
        .map(|name| format!("source.task197.{name}"))
        .collect::<Vec<_>>();
    let manifest = json!({
        "metadata": {
            "dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v12.json",
            "dbt_version": "1.11.8",
            "adapter_type": "duckdb"
        },
        "nodes": {
            "model.task197.probe": {
                "unique_id": "model.task197.probe",
                "resource_type": "model",
                "name": "probe",
                "language": "sql",
                "relation_name": "task197_probe",
                "config": {"materialized": "view"},
                "compiled_code": sql,
                "depends_on": {"nodes": dependencies}
            }
        },
        "sources": {
            "source.task197.orders": {
                "unique_id": "source.task197.orders",
                "resource_type": "source", "name": "orders", "relation_name": "orders",
                "columns": {
                    "id": {"name": "id", "data_type": "INTEGER"},
                    "amount": {"name": "amount", "data_type": "INTEGER"}
                }
            },
            "source.task197.customers": {
                "unique_id": "source.task197.customers",
                "resource_type": "source", "name": "customers", "relation_name": "customers",
                "columns": {
                    "id": {"name": "id", "data_type": "INTEGER"},
                    "amount": {"name": "amount", "data_type": "INTEGER"}
                }
            },
            "source.task197.products": {
                "unique_id": "source.task197.products",
                "resource_type": "source", "name": "products", "relation_name": "products",
                "columns": {"id": {"name": "id", "data_type": "INTEGER"}}
            }
        }
    });
    let parsed = parse_dbt_manifest(&manifest.to_string()).expect("compiled manifest");
    let dialect = dialect_from_name("duckdb").expect("dbt dialect");
    let bundle = analyze_dbt_manifest_with_schemas(&parsed, "duckdb", dialect.as_ref())
        .expect("compiled source and model");
    assert!(bundle.producer_evidence().iter().all(|p| p.execution() == ProducerExecution::Unattested));
    RelationCatalog::from_schemas(bundle.source_schemas()).expect("dbt authoritative catalog")
}

#[cfg(feature = "odcs")]
fn odcs_catalog() -> RelationCatalog {
    let contract = "apiVersion: v3.2.0
kind: DataContract
id: task197
name: Cross adapter source parity
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
            .expect("declared physical relations"),
    ).expect("ODCS typed source declarations");
    RelationCatalog::from_schemas(parsed.source_schemas()).expect("ODCS catalog")
}

fn output_domains(catalog: &RelationCatalog, sql: &str, dialect: &str) -> Vec<ValueDomain> {
    let parser = dialect_from_name(dialect).expect("one of thirteen parsers");
    let input = SqlInput::inline(sql);
    let configured = [ConfiguredSqlInput::new("task197-parity", &input, dialect, parser.as_ref())];
    let bundle = analyze_configured_inputs_with_catalog(&configured, catalog)
        .unwrap_or_else(|error| panic!("{dialect}: source-aware public analysis: {error}"));
    let terminal = bundle.layers().last().expect("one terminal SQL projection");
    match terminal.composed_semantics() {
        ComposedSemantics::Resolved(semantics) => semantics.output().columns()
            .iter().map(|column| column.domain().clone()).collect(),
        _ => panic!("{dialect}: expected a resolved typed terminal"),
    }
}

const PORTABLE_CASES: &[&str] = &[
    "88-C177-S001", // predicate
    "88-C177-S043", // join
    "88-C177-S066", // set
    "88-C177-S106", // window
    "88-C177-S191", // schema-authority read
];

#[test]
fn original_cross_family_typed_reads_agree_for_matching_sql_dbt_and_odcs_source_facts() {
    let evidence = ledger();
    let direct = direct_catalog();
    #[cfg(feature = "odcs")]
    let odcs = odcs_catalog();
    let mut compared = 0;
    for id in PORTABLE_CASES {
        let case = evidence["cases"].as_array().expect("all original SQL")
            .iter().find(|c| c["id"] == *id).expect("original cross-family exemplar");
        let sql = case["sql"].as_str().expect("original SQL");
        let dbt = dbt_catalog(sql);
        for cell in case["cells"].as_array().expect("thirteen dialects") {
            let name = cell["dialect"].as_str().expect("named parser");
            assert_eq!(cell["original_observation"], "C", "{id}/{name}: portable SQL");
            let expected = output_domains(&direct, sql, name);
            assert_eq!(output_domains(&dbt, sql, name), expected,
                "{id}/{name}: manifest/catalog cannot weaken source-typed output domain");
            #[cfg(feature = "odcs")]
            assert_eq!(output_domains(&odcs, sql, name), expected,
                "{id}/{name}: ODCS cannot weaken source-typed output domain");
            compared += 1;
        }
    }
    assert_eq!(compared, 5 * 13);
}
