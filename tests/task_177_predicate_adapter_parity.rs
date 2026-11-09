//! TASK-177 source-independent typed-domain parity for ten portable filters.
//! Direct DDL, dbt compiled manifest and ODCS source metadata must agree on
//! canonical output value domains. The adapter's provenance and execution
//! claims must remain separate from the facts those sources can actually prove.

use serde_json::{json, Value};
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_manifest_with_schemas, analyze_inputs,
    dialect_from_name, parse_dbt_manifest, ComposedSemantics, ConfiguredSqlInput,
    ProducerExecution, RelationCatalog, SqlInput, ValueDomain,
};

const ORACLES: &str = include_str!("../docs/task-177-predicate-oracles.json");

fn composed_domain(bundle: &sql_semantic_protocol::AnalysisBundle) -> ValueDomain {
    let layer = bundle.layers().last().expect("one terminal producer");
    let ComposedSemantics::Resolved(semantics) = layer.composed_semantics() else {
        panic!("source-complete, simple filter must resolve");
    };
    semantics.output().columns()[0].domain().clone()
}

fn direct_ddl_catalog() -> RelationCatalog {
    let dialect = dialect_from_name("duckdb").expect("duckdb parser");
    let bundle = analyze_inputs(
        &[SqlInput::inline(
            "CREATE TABLE orders (id INTEGER, amount INTEGER)",
        )],
        "duckdb",
        dialect.as_ref(),
    )
    .expect("direct DDL relation metadata");
    RelationCatalog::from_schemas(bundle.source_schemas()).expect("direct catalog")
}

fn dbt_source_bundle(sql: &str) -> sql_semantic_protocol::AnalysisBundle {
    let manifest = json!({
        "metadata": {
            "dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v12.json",
            "dbt_version": "1.11.8",
            "adapter_type": "duckdb"
        },
        "nodes": {
            "model.task177.filter": {
                "unique_id": "model.task177.filter",
                "resource_type": "model",
                "name": "filter",
                "relation_name": "task177_filter",
                "language": "sql",
                "config": {"materialized": "view"},
                "compiled_code": sql,
                "depends_on": {"nodes": ["source.task177.orders"]}
            }
        },
        "sources": {
            "source.task177.orders": {
                "unique_id": "source.task177.orders",
                "resource_type": "source",
                "name": "orders",
                "relation_name": "orders",
                "columns": {
                    "id": {"name": "id", "data_type": "INTEGER"},
                    "amount": {"name": "amount", "data_type": "INTEGER"}
                }
            }
        }
    });
    let manifest = parse_dbt_manifest(&manifest.to_string()).expect("dbt manifest");
    let dialect = dialect_from_name("duckdb").expect("duckdb parser");
    analyze_dbt_manifest_with_schemas(&manifest, "duckdb", dialect.as_ref())
        .expect("dbt compiled schema-aware semantics")
}

fn direct_domain(sql: &str, catalog: &RelationCatalog) -> ValueDomain {
    let dialect = dialect_from_name("duckdb").expect("duckdb parser");
    let input = SqlInput::inline(sql);
    let configured = [ConfiguredSqlInput::new(
        "task177-source-fact",
        &input,
        "duckdb",
        dialect.as_ref(),
    )];
    let bundle = analyze_configured_inputs_with_catalog(&configured, catalog)
        .expect("canonical SQL with source metadata");
    composed_domain(&bundle)
}

#[cfg(feature = "odcs")]
fn odcs_source_catalog() -> RelationCatalog {
    let metadata = "apiVersion: v3.2.0\nkind: DataContract\nid: task177-filter\nname: TASK-177\nversion: 1.0.0\nstatus: active\nschema:\n  - name: orders\n    properties:\n      - name: id\n        physicalType: INTEGER\n      - name: amount\n        physicalType: INTEGER\n";
    let base = RelationCatalog::new(&["orders"]).expect("orders relation identity");
    let bundle = sql_semantic_protocol::parse_odcs_yaml(metadata, "duckdb", &base)
        .expect("representable ODCS typed source facts");
    RelationCatalog::from_schemas(bundle.source_schemas()).expect("ODCS catalog")
}

#[test]
fn task_177_compiled_dbt_and_direct_sql_preserve_ten_typed_filter_domains() {
    let fixture: Value = serde_json::from_str(ORACLES).expect("oracle fixture");
    let catalog = direct_ddl_catalog();
    let ids = [
        "88-C177-S001",
        "88-C177-S002",
        "88-C177-S003",
        "88-C177-S004",
        "88-C177-S005",
        "88-C177-S006",
        "88-C177-S007",
        "88-C177-S008",
        "88-C177-S009",
        "88-C177-S012",
    ];
    for id in ids {
        let case = fixture["cases"]
            .as_array()
            .expect("cases")
            .iter()
            .find(|value| value["id"] == id)
            .expect("fixed sample");
        let sql = case["sql"].as_str().expect("compiled SQL");
        let direct = direct_domain(sql, &catalog);
        assert!(
            matches!(direct, ValueDomain::Set(_) | ValueDomain::Ranges(_)),
            "{id}: the typed predicate domain must not degrade to unbounded"
        );
        let dbt = dbt_source_bundle(sql);
        assert_eq!(
            composed_domain(&dbt),
            direct,
            "{id}: compiled dbt source facts must preserve the direct typed domain"
        );
        let [producer] = dbt.producer_evidence() else {
            panic!("{id}: the dbt view must have one materialization declaration");
        };
        assert_eq!(
            producer.execution(),
            ProducerExecution::Unattested,
            "{id}: adapter metadata must not claim warehouse execution"
        );
    }
}

#[cfg(feature = "odcs")]
#[test]
fn task_177_odcs_parity_preserves_exact_same_filter_domains_as_sql_and_dbt() {
    let fixture: Value = serde_json::from_str(ORACLES).expect("oracle fixture");
    let direct = direct_ddl_catalog();
    let odcs = odcs_source_catalog();
    for case in fixture["cases"].as_array().expect("cases") {
        if case["expected_projected_domain"].is_null() {
            continue;
        }
        let id = case["id"].as_str().expect("stable ID");
        let sql = case["sql"].as_str().expect("SQL");
        let expected = direct_domain(sql, &direct);
        assert_eq!(
            direct_domain(sql, &odcs),
            expected,
            "{id}: ODCS physical type metadata changed canonical semantics"
        );
        assert_eq!(
            composed_domain(&dbt_source_bundle(sql)),
            expected,
            "{id}: compiled dbt and ODCS source semantics diverged"
        );
    }
}
