//! TASK-196 S191/S193: authoritative source metadata is not native execution.
//!
//! The original SQL reads a source relation; dbt compiled SQL, catalog
//! introspection, manifest fallback and ODCS only supply independently
//! typed facts. A conflicting declaration never overrides a catalog.

use serde_json::{json, Value};
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_artifacts, analyze_inputs,
    dialect_from_name, parse_dbt_catalog, parse_dbt_manifest, ComposedSemantics,
    ConfiguredSqlInput, DataType, DbtArtifactsError, RelationCatalog, SchemaSourceKind, SqlInput,
    ValueDomain,
};

#[cfg(feature = "odcs")]
use sql_semantic_protocol::analyze_dbt_manifest_with_schemas;

const FREEZE: &str = include_str!("../docs/task-196-acceptance-freeze.json");

fn original(id: &str) -> String {
    let frozen: Value = serde_json::from_str(FREEZE).expect("frozen SQL inventory");
    frozen["cases"]
        .as_array()
        .expect("38 cases")
        .iter()
        .find(|case| case["id"] == id)
        .expect("original metadata case")["sql"]
        .as_str()
        .expect("SQL text")
        .to_owned()
}

fn manifest(source_type: Option<&str>, sql: &str) -> Value {
    let mut source = json!({
        "unique_id": "source.task196.orders",
        "resource_type": "source",
        "name": "orders",
        "relation_name": "orders"
    });
    if let Some(source_type) = source_type {
        source["columns"] = json!({
            "id": {"name":"id", "data_type":source_type},
            "amount": {"name":"amount","data_type":"DECIMAL(10,2)"}
        });
    }
    json!({
        "metadata": {
            "dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v12.json",
            "dbt_version": "1.11.8",
            "adapter_type": "duckdb"
        },
        "nodes": {
            "model.task196.read": {
                "unique_id": "model.task196.read",
                "resource_type": "model",
                "name": "read",
                "language": "sql",
                "config": {"materialized":"view"},
                "relation_name": "task196_read",
                "compiled_code": sql,
                "depends_on": {"nodes":["source.task196.orders"]}
            }
        },
        "sources": {"source.task196.orders":source}
    })
}

fn catalog(data_type: Option<&str>) -> Value {
    let mut sources = json!({});
    if let Some(data_type) = data_type {
        sources["source.task196.orders"] = json!({
            "unique_id":"source.task196.orders",
            "metadata":{
                "type":"BASE TABLE", "database":null, "schema":null, "name":"orders"
            },
            "columns":{
                "id":{"name":"id", "type":data_type, "index":1},
                "amount":{"name":"amount", "type":"DECIMAL(10,2)", "index":2}
            },
            "stats":{}
        });
    }
    json!({
        "metadata": {
            "dbt_schema_version":"https://schemas.getdbt.com/dbt/catalog/v1.json",
            "dbt_version":"1.11.8"
        },
        "nodes":{}, "sources":sources, "errors":null
    })
}

fn dbt_bundle(
    declared_type: Option<&str>,
    catalog_type: Option<&str>,
    sql: &str,
) -> Result<sql_semantic_protocol::AnalysisBundle, DbtArtifactsError> {
    let manifest = parse_dbt_manifest(&manifest(declared_type, sql).to_string())
        .expect("valid compiled dbt manifest");
    let catalog =
        parse_dbt_catalog(&catalog(catalog_type).to_string()).expect("valid warehouse catalog");
    let dialect = dialect_from_name("duckdb").expect("source analyzer dialect");
    analyze_dbt_artifacts(&manifest, &catalog, "duckdb", dialect.as_ref())
}

fn typed_output(catalog: &RelationCatalog, sql: &str, dialect_name: &str) -> Vec<ValueDomain> {
    let dialect = dialect_from_name(dialect_name).expect("named project dialect");
    let input = SqlInput::inline(sql);
    let configured = [ConfiguredSqlInput::new(
        "task196-original",
        &input,
        dialect_name,
        dialect.as_ref(),
    )];
    let bundle = analyze_configured_inputs_with_catalog(&configured, catalog)
        .unwrap_or_else(|error| panic!("{dialect_name}: original SQL: {error}"));
    let ComposedSemantics::Resolved(semantics) = bundle
        .layers()
        .last()
        .expect("source read terminal")
        .composed_semantics()
    else {
        panic!("{dialect_name}: original SQL must retain explicit typed output");
    };
    semantics
        .output()
        .columns()
        .iter()
        .map(|column| column.domain().clone())
        .collect()
}

#[test]
fn both_original_metadata_reads_prefer_warehouse_types_and_fail_closed_on_absence() {
    for id in ["88-C177-S191", "88-C177-S193"] {
        let sql = original(id);
        assert_eq!(sql, "SELECT id FROM orders");
        let catalog_bundle = dbt_bundle(Some("TEXT"), Some("INTEGER"), &sql).expect("catalog wins");
        let [schema] = catalog_bundle.source_schemas() else {
            panic!("{id}: one warehouse source must be retained");
        };
        assert_eq!(schema.relation(), "orders");
        assert_eq!(schema.source_kind(), Some(SchemaSourceKind::DbtCatalog));
        assert_eq!(
            schema
                .columns()
                .iter()
                .find(|column| column.name() == "id")
                .expect("id column")
                .data_type(),
            &DataType::SignedInteger { bits: Some(32) },
            "{id}: declared manifest TEXT must not defeat introspection"
        );
        let fallback = dbt_bundle(Some("INTEGER"), None, &sql)
            .expect("complete manifest-declared fallback is authoritative without catalog");
        let [fallback_schema] = fallback.source_schemas() else {
            panic!("{id}: manifest fallback must preserve one source");
        };
        assert_eq!(
            fallback_schema.source_kind(),
            Some(SchemaSourceKind::DbtManifest)
        );
        assert_eq!(
            fallback_schema
                .columns()
                .iter()
                .find(|column| column.name() == "id")
                .expect("id column")
                .data_type(),
            &DataType::SignedInteger { bits: Some(32) }
        );
        assert!(
            dbt_bundle(None, None, &sql).is_err(),
            "{id}: absent catalog and declarations cannot create typed evidence"
        );

        let direct_dialect = dialect_from_name("duckdb").expect("source dialect");
        let direct = analyze_inputs(
            &[SqlInput::inline(
                "CREATE TABLE orders(id INTEGER PRIMARY KEY,amount DECIMAL(10,2))",
            )],
            "duckdb",
            direct_dialect.as_ref(),
        )
        .expect("authoritative SQL DDL");
        let from_direct =
            RelationCatalog::from_schemas(direct.source_schemas()).expect("DDL source type");
        let from_catalog = RelationCatalog::from_schemas(catalog_bundle.source_schemas())
            .expect("authoritative warehouse source");
        let from_manifest = RelationCatalog::from_schemas(fallback.source_schemas())
            .expect("declared complete manifest source");
        let frozen: Value = serde_json::from_str(FREEZE).expect("named parser matrix");
        for dialect in frozen["dialects"].as_array().expect("13 named dialects") {
            let name = dialect.as_str().expect("source parser name");
            let expected = typed_output(&from_direct, &sql, name);
            assert_eq!(
                typed_output(&from_catalog, &sql, name),
                expected,
                "{id}/{name}: warehouse facts must preserve canonical output"
            );
            assert_eq!(
                typed_output(&from_manifest, &sql, name),
                expected,
                "{id}/{name}: complete declared source facts must preserve output"
            );
        }
    }
}

#[cfg(feature = "odcs")]
#[test]
fn original_metadata_reads_match_odcs_and_direct_typed_facts_without_execution_attestation() {
    let sql = original("88-C177-S191");
    let parsed = sql_semantic_protocol::parse_odcs_yaml(
        "apiVersion: v3.2.0
kind: DataContract
id: task196
name: Schema authority
version: 1.0.0
status: active
schema:
  - name: orders
    physicalName: orders
    properties:
      - name: id
        physicalType: INTEGER
      - name: amount
        physicalType: DECIMAL(10,2)
",
        "duckdb",
        &RelationCatalog::new(&["orders"]).expect("physical source name"),
    )
    .expect("ODCS authoritative source schema");
    let odcs =
        RelationCatalog::from_schemas(parsed.source_schemas()).expect("typed contract catalog");
    let duckdb = dialect_from_name("duckdb").expect("source parser");
    let ddl = analyze_inputs(
        &[SqlInput::inline(
            "CREATE TABLE orders(id INTEGER,amount DECIMAL(10,2))",
        )],
        "duckdb",
        duckdb.as_ref(),
    )
    .expect("typed direct DDL");
    let direct = RelationCatalog::from_schemas(ddl.source_schemas()).expect("source DDL catalog");
    let frozen: Value = serde_json::from_str(FREEZE).expect("original dialects");
    for name in frozen["dialects"].as_array().expect("13 dialects") {
        let name = name.as_str().expect("public parser name");
        assert_eq!(
            typed_output(&odcs, &sql, name),
            typed_output(&direct, &sql, name),
            "S191/S193/{name}: contract types must preserve canonical source read"
        );
    }
    let parsed_manifest = parse_dbt_manifest(&manifest(Some("INTEGER"), &sql).to_string())
        .expect("compiled dbt source");
    let compiled = analyze_dbt_manifest_with_schemas(&parsed_manifest, "duckdb", duckdb.as_ref())
        .expect("compiled source SQL with complete manifest declarations");
    assert_eq!(
        typed_output(
            &RelationCatalog::from_schemas(compiled.source_schemas())
                .expect("manifest source catalog"),
            &sql,
            "duckdb",
        ),
        typed_output(&direct, &sql, "duckdb")
    );
}
