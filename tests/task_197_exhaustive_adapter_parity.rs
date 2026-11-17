//! TASK-197: exhaustive original-SQL cross-adapter public protocol contract.
//!
//! Direct SQL DDL, compiled dbt manifest metadata and ODCS contain the same
//! source type facts but have distinct provenance. Metadata never certifies
//! native vendor execution or the source/terminal physical bag.

use serde_json::{json, Value};
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_artifacts,
    analyze_dbt_manifest_with_schemas, analyze_inputs, dialect_from_name, parse_dbt_catalog,
    parse_dbt_manifest, to_bundle_json, ConfiguredSqlInput, ProducerExecution, RelationCatalog,
    SchemaSourceKind, SqlInput,
};

const FREEZE: &str = include_str!("../docs/task-197-acceptance-ledger-v2.json");
const SOURCE_FACTS: &str = include_str!("../docs/task-197-source-fact-matrix.json");

fn original() -> Value {
    serde_json::from_str(FREEZE).expect("immutable TASK-197 source SQL ledger")
}

fn source_ddl(amount_type: &str) -> String {
    format!(
        "CREATE TABLE orders(id INTEGER, amount {amount_type}, created_at TIMESTAMP, __physical_row_id VARCHAR);
         CREATE TABLE customers(id INTEGER, amount INTEGER, name VARCHAR, __physical_row_id VARCHAR);
         CREATE TABLE products(id INTEGER)"
    )
}

fn direct_catalog(amount_type: &str) -> RelationCatalog {
    let dialect = dialect_from_name("duckdb").expect("source parser");
    let ddl = source_ddl(amount_type);
    let sources = analyze_inputs(&[SqlInput::inline(&ddl)], "duckdb", dialect.as_ref())
        .expect("typed source DDL");
    RelationCatalog::from_schemas(sources.source_schemas()).expect("DDL physical source types")
}

fn dbt_manifest_source(amount_type: &str) -> Value {
    let manifest = json!({
        "metadata": {
            "dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v12.json",
            "dbt_version": "1.11.8",
            "adapter_type": "duckdb"
        },
        "nodes": {
            "model.task197.source_probe": {
                "unique_id": "model.task197.source_probe",
                "resource_type": "model", "name": "source_probe",
                "language": "sql", "relation_name": "source_probe",
                "config": {"materialized": "view"},
                "compiled_code": "SELECT o.id FROM orders o LEFT JOIN customers c ON o.id = c.id LEFT JOIN products p ON p.id = o.id",
                "depends_on": {"nodes": ["source.task197.orders", "source.task197.customers", "source.task197.products"]}
            }
        },
        "sources": {
            "source.task197.orders": {
                "unique_id": "source.task197.orders",
                "resource_type": "source", "name": "orders", "relation_name": "orders",
                "columns": {
                    "id": {"name": "id", "data_type": "INTEGER"},
                    "amount": {"name": "amount", "data_type": amount_type},
                    "created_at": {"name": "created_at", "data_type": "TIMESTAMP"},
                    "__physical_row_id": {"name": "__physical_row_id", "data_type": "VARCHAR"}
                }
            },
            "source.task197.customers": {
                "unique_id": "source.task197.customers",
                "resource_type": "source", "name": "customers", "relation_name": "customers",
                "columns": {
                    "id": {"name": "id", "data_type": "INTEGER"},
                    "amount": {"name": "amount", "data_type": "INTEGER"},
                    "name": {"name": "name", "data_type": "VARCHAR"},
                    "__physical_row_id": {"name": "__physical_row_id", "data_type": "VARCHAR"}
                }
            },
            "source.task197.products": {
                "unique_id": "source.task197.products",
                "resource_type": "source", "name": "products", "relation_name": "products",
                "columns": {"id": {"name": "id", "data_type": "INTEGER"}}
            }
        }
    });
    manifest
}

fn dbt_catalog(amount_type: &str) -> RelationCatalog {
    let parsed = parse_dbt_manifest(&dbt_manifest_source(amount_type).to_string())
        .expect("compiled dbt source manifest");
    let dialect = dialect_from_name("duckdb").expect("metadata dialect");
    let bundle = analyze_dbt_manifest_with_schemas(&parsed, "duckdb", dialect.as_ref())
        .expect("source declarations and compiled probe");
    assert!(bundle
        .producer_evidence()
        .iter()
        .all(|p| p.execution() == ProducerExecution::Unattested));
    RelationCatalog::from_schemas(bundle.source_schemas()).expect("dbt authoritative facts")
}

fn dbt_warehouse_catalog(amount_type: &str) -> RelationCatalog {
    let manifest = parse_dbt_manifest(&dbt_manifest_source(amount_type).to_string())
        .expect("complete dbt manifest source identities");
    let mut sources = serde_json::Map::new();
    for (name, columns) in [
        (
            "orders",
            vec![
                ("id", "INTEGER"),
                ("amount", amount_type),
                ("created_at", "TIMESTAMP"),
                ("__physical_row_id", "VARCHAR"),
            ],
        ),
        (
            "customers",
            vec![
                ("id", "INTEGER"),
                ("amount", "INTEGER"),
                ("name", "VARCHAR"),
                ("__physical_row_id", "VARCHAR"),
            ],
        ),
        ("products", vec![("id", "INTEGER")]),
    ] {
        let mut column_map = serde_json::Map::new();
        for (index, (column, data_type)) in columns.into_iter().enumerate() {
            column_map.insert(
                column.to_string(),
                json!({"name": column, "type": data_type, "index": index + 1}),
            );
        }
        let id = format!("source.task197.{name}");
        sources.insert(
            id.clone(),
            json!({
                "unique_id": id,
                "metadata": {
                    "type": "BASE TABLE",
                    "database": null,
                    "schema": null,
                    "name": name
                },
                "columns": column_map,
                "stats": {}
            }),
        );
    }
    let raw_catalog = json!({
        "metadata": {
            "dbt_schema_version": "https://schemas.getdbt.com/dbt/catalog/v1.json",
            "dbt_version": "1.11.8"
        },
        "nodes": {}, "sources": sources, "errors": null
    });
    let warehouse = parse_dbt_catalog(&raw_catalog.to_string()).expect("native warehouse catalog");
    let dialect = dialect_from_name("duckdb").expect("dbt source dialect");
    let bundle = analyze_dbt_artifacts(&manifest, &warehouse, "duckdb", dialect.as_ref())
        .expect("introspected catalog is authoritative for source types");
    assert!(bundle
        .producer_evidence()
        .iter()
        .all(|p| p.execution() == ProducerExecution::Unattested));
    assert!(bundle
        .source_schemas()
        .iter()
        .all(|schema| schema.source_kind() == Some(SchemaSourceKind::DbtCatalog)));
    RelationCatalog::from_schemas(bundle.source_schemas())
        .expect("warehouse introspected source types")
}

#[cfg(feature = "odcs")]
fn odcs_catalog(amount_type: &str) -> RelationCatalog {
    let yaml = format!(
        "apiVersion: v3.2.0
kind: DataContract
id: task197
name: Exhaustive original source type parity
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
      - name: __physical_row_id
        physicalType: VARCHAR
  - name: customers
    physicalName: customers
    properties:
      - name: id
        physicalType: INTEGER
      - name: amount
        physicalType: INTEGER
      - name: name
        physicalType: VARCHAR
      - name: __physical_row_id
        physicalType: VARCHAR
  - name: products
    physicalName: products
    properties:
      - name: id
        physicalType: INTEGER
"
    );
    let parsed = sql_semantic_protocol::parse_odcs_yaml(
        &yaml,
        "duckdb",
        &RelationCatalog::new(&["orders", "customers", "products"]).expect("physical source names"),
    )
    .expect("ODCS typed source declaration");
    RelationCatalog::from_schemas(parsed.source_schemas()).expect("ODCS source types")
}

fn canonical_semantics(catalog: &RelationCatalog, sql: &str, dialect: &str) -> Value {
    let parser = dialect_from_name(dialect).expect("frozen public dialect");
    let input = SqlInput::inline(sql);
    let configured = [ConfiguredSqlInput::new(
        "task197-exhaustive",
        &input,
        dialect,
        parser.as_ref(),
    )];
    let analyzed = analyze_configured_inputs_with_catalog(&configured, catalog)
        .unwrap_or_else(|error| panic!("{dialect}: original source-aware SQL: {error}"));
    let document: Value =
        serde_json::from_str(&to_bundle_json(&analyzed)).expect("complete public JSON contract");
    json!({
        "statements": document["inputs"][0]["statements"],
        "layers": document["layers"],
        "graph": document["graph"],
    })
}

#[test]
fn every_parser_accepted_original_statement_preserves_full_canonical_semantics_across_equal_sources(
) {
    let fixture = original();
    let source_facts: Value =
        serde_json::from_str(SOURCE_FACTS).expect("TASK-197 authoritative source facts");
    let cases = fixture["cases"]
        .as_array()
        .expect("all original SQL variants");
    let fact_cases = source_facts["cases"]
        .as_array()
        .expect("exact original per-case source authorities");
    assert_eq!(cases.len(), fact_cases.len(), "source-fact inventory drift");
    let direct_integer = direct_catalog("INTEGER");
    let direct_float = direct_catalog("DOUBLE");
    let direct_decimal = direct_catalog("DECIMAL(10,2)");
    let dbt_integer = dbt_catalog("INTEGER");
    let dbt_float = dbt_catalog("DOUBLE");
    let dbt_decimal = dbt_catalog("DECIMAL(10,2)");
    let warehouse_integer = dbt_warehouse_catalog("INTEGER");
    let warehouse_float = dbt_warehouse_catalog("DOUBLE");
    let warehouse_decimal = dbt_warehouse_catalog("DECIMAL(10,2)");
    #[cfg(feature = "odcs")]
    let odcs_integer = odcs_catalog("INTEGER");
    #[cfg(feature = "odcs")]
    let odcs_float = odcs_catalog("DOUBLE");
    #[cfg(feature = "odcs")]
    let odcs_decimal = odcs_catalog("DECIMAL(10,2)");
    let mut checked = 0;
    let mut bypassed = 0;
    let mut parser_boundaries = 0;

    for (case, fact) in cases.iter().zip(fact_cases) {
        let id = case["id"].as_str().expect("stable fixture id");
        let sql = case["sql"].as_str().expect("original SQL");
        assert_eq!(fact["id"], case["id"], "{id}: source-fact identity drift");
        assert_eq!(fact["sql"], case["sql"], "{id}: source-fact SQL drift");
        assert_eq!(
            fact["source_relations"], case["source_relations_mentioned"],
            "{id}: source relation drift"
        );
        let no_external_fact = case["source_relations_mentioned"]
            .as_array()
            .expect("named source scope")
            .is_empty();
        let fact_set = fact["source_fact_set"].as_str();
        assert_eq!(
            fact_set.is_none(),
            no_external_fact,
            "{id}: missing or unexpected authoritative source types"
        );
        // The frozen source-fact matrix, not a second hard-coded case-ID list,
        // determines which equivalent type authorities each adapter receives.
        let (direct, dbt, warehouse) = match fact_set {
            Some("integer_amount") => (&direct_integer, &dbt_integer, &warehouse_integer),
            Some("floating_amount") => (&direct_float, &dbt_float, &warehouse_float),
            Some("decimal_amount") => (&direct_decimal, &dbt_decimal, &warehouse_decimal),
            None => (&direct_integer, &dbt_integer, &warehouse_integer),
            Some(other) => panic!("{id}: unknown source fact set {other}"),
        };
        #[cfg(feature = "odcs")]
        let odcs = match fact_set {
            Some("floating_amount") => &odcs_float,
            Some("decimal_amount") => &odcs_decimal,
            Some("integer_amount") | None => &odcs_integer,
            Some(other) => panic!("{id}: unknown ODCS source fact set {other}"),
        };

        for cell in case["cells"].as_array().expect("all 13 parser identities") {
            let name = cell["dialect"].as_str().expect("parser name");
            let status = cell["original_observation"]
                .as_str()
                .expect("historical status");
            if status == "R" || status == "U" {
                assert_eq!(cell["status"], "unrepresentable", "{id}/{name}");
                parser_boundaries += 1;
                continue;
            }
            if no_external_fact {
                assert_eq!(case["adapter_fact_scope"], "no_external_source_fact");
                bypassed += 1;
                continue;
            }
            // Compare the entire public statement, resolved/composed terminal,
            // lineage graph and physical plan, not merely a projected datatype.
            // D means an upstream cross-*dialect* document difference, not a
            // permission for equal sources to diverge within this dialect.
            let expected = canonical_semantics(direct, sql, name);
            assert_eq!(
                canonical_semantics(dbt, sql, name),
                expected,
                "{id}/{name}: dbt authoritative source metadata changed canonical facts"
            );
            assert_eq!(
                canonical_semantics(warehouse, sql, name),
                expected,
                "{id}/{name}: dbt catalog introspection changed canonical facts"
            );
            #[cfg(feature = "odcs")]
            assert_eq!(
                canonical_semantics(odcs, sql, name),
                expected,
                "{id}/{name}: ODCS authoritative source metadata changed canonical facts"
            );
            checked += 1;
        }
    }
    assert_eq!(checked + bypassed + parser_boundaries, 2119);
    assert!(
        checked > 1900,
        "every source-backed parser-accepted original needs parity"
    );
    assert_eq!(
        parser_boundaries, 82,
        "historical R/U boundaries remain explicit"
    );
    assert_eq!(
        bypassed, 24,
        "24 accepted source-free cells; two others are parser boundaries"
    );
}
