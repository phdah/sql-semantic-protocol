//! TASK-118: nested and repeated source-branch adapter parity at the public API.

#![cfg(feature = "odcs")]

use std::collections::BTreeMap;

use serde_json::{json, Value};
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_manifest_with_schemas, dialect_from_name,
    parse_dbt_manifest, parse_odcs_yaml, physical_finite_set_tree_plan, ConfiguredSqlInput,
    LiteralValue, PhysicalFiniteSetTreeProof, PhysicalWriteRow, PhysicalWriteState,
    RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
};

fn source_schemas() -> Vec<RelationSchema> {
    ["warehouse.raw.a", "warehouse.raw.b", "warehouse.raw.c"]
        .map(|relation| {
            RelationSchema::new(
                relation,
                ["id", "k", "v"]
                    .into_iter()
                    .map(|column| {
                        SchemaColumn::from_sql_type(
                            column,
                            if column == "id" { "VARCHAR" } else { "INTEGER" },
                            "postgresql",
                        )
                        .expect("typed schema")
                    })
                    .collect(),
            )
            .expect("source schema")
        })
        .to_vec()
}

fn snapshots(case: &Value) -> Vec<PhysicalWriteState> {
    case["source_bags"]
        .as_object()
        .expect("physical sources")
        .iter()
        .map(|(source, bag)| {
            let rows = bag
                .as_array()
                .expect("rows")
                .iter()
                .map(|row| {
                    let data = row
                        .as_object()
                        .expect("row")
                        .iter()
                        .map(|(column, value)| {
                            let scalar = if value.is_null() {
                                LiteralValue::Null
                            } else if let Some(n) = value.as_i64() {
                                LiteralValue::Number(n.to_string())
                            } else {
                                LiteralValue::Text(value.as_str().expect("id").to_string())
                            };
                            (column.to_string(), scalar)
                        })
                        .collect::<BTreeMap<_, _>>();
                    PhysicalWriteRow::new(data).expect("physical row")
                })
                .collect();
            PhysicalWriteState::new(format!("warehouse.raw.{source}"), rows)
                .expect("source population")
        })
        .collect()
}

fn full_counts(
    bundle: &sql_semantic_protocol::AnalysisBundle,
    states: &[PhysicalWriteState],
) -> BTreeMap<Vec<Option<i128>>, u64> {
    let target = bundle
        .layers()
        .iter()
        .find(|layer| {
            states
                .iter()
                .all(|source| layer.consumes().iter().any(|s| s == source.relation()))
        })
        .expect("complete set source consumer")
        .id();
    let proof = physical_finite_set_tree_plan(bundle, target, states);
    let PhysicalFiniteSetTreeProof::Exact(exact) = proof else {
        panic!("adapter lost canonical source bag proof: {proof:?}");
    };
    exact
        .tuples
        .into_iter()
        .map(|tuple| (tuple.values, tuple.output_count))
        .collect()
}

#[test]
fn direct_dbt_and_odcs_share_identical_nested_nullable_source_obligations() {
    let freeze: Value =
        serde_json::from_str(include_str!("../docs/task-118-acceptance-freeze.json"))
            .expect("task-118 frozen cases");
    let schemas = source_schemas();
    let direct_catalog = RelationCatalog::from_schemas(&schemas).expect("direct catalog");
    let names = ["warehouse.raw.a", "warehouse.raw.b", "warehouse.raw.c"];
    let known = RelationCatalog::new(&names).expect("ODCS source identities");
    let odcs = parse_odcs_yaml(
        r#"
apiVersion: v3.2.0
kind: DataContract
id: task118nestedset
name: Nested set source population
version: 1.0.0
status: active
schema:
  - name: a
    physicalName: warehouse.raw.a
    properties:
      - name: id
        physicalType: VARCHAR
      - name: k
        physicalType: INTEGER
      - name: v
        physicalType: INTEGER
  - name: b
    physicalName: warehouse.raw.b
    properties:
      - name: id
        physicalType: VARCHAR
      - name: k
        physicalType: INTEGER
      - name: v
        physicalType: INTEGER
  - name: c
    physicalName: warehouse.raw.c
    properties:
      - name: id
        physicalType: VARCHAR
      - name: k
        physicalType: INTEGER
      - name: v
        physicalType: INTEGER
"#,
        "postgresql",
        &known,
    )
    .expect("ODCS");
    let odcs_catalog =
        RelationCatalog::from_schemas(odcs.source_schemas()).expect("ODCS canonical catalog");
    let parser = dialect_from_name("postgresql").expect("dialect");
    for case in freeze["cases"]
        .as_array()
        .expect("all cases")
        .iter()
        .filter(|case| {
            case["statements"].as_array().expect("producer").is_empty()
                && !case["variants"]
                    .as_array()
                    .expect("variants")
                    .iter()
                    .any(|v| v == "cte")
        })
    {
        let sql = case["sql"]
            .as_str()
            .expect("SQL")
            .replace("FROM a", "FROM warehouse.raw.a")
            .replace("FROM b", "FROM warehouse.raw.b")
            .replace("FROM c", "FROM warehouse.raw.c");
        let input = SqlInput::inline(&sql);
        let configured = [ConfiguredSqlInput::new(
            "task-118-adapter",
            &input,
            "postgresql",
            parser.as_ref(),
        )];
        let direct = analyze_configured_inputs_with_catalog(&configured, &direct_catalog)
            .expect("direct SQL");
        let adapted = analyze_configured_inputs_with_catalog(&configured, &odcs_catalog)
            .expect("ODCS enriched SQL");
        let manifest = json!({
            "metadata": {
                "dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v12.json",
                "dbt_version": "1.11.8",
                "adapter_type": "postgres"
            },
            "nodes": {
                "model.demo.task118": {
                    "unique_id": "model.demo.task118",
                    "resource_type": "model",
                    "relation_name": "warehouse.analytics.task118",
                    "database": "warehouse",
                    "schema": "analytics",
                    "original_file_path": "models/task118.sql",
                    "language": "sql",
                    "raw_code": sql,
                    "compiled_code": sql,
                    "depends_on": {
                        "nodes": ["source.demo.a", "source.demo.b", "source.demo.c"]
                    }
                }
            },
            "sources": {
                "source.demo.a": {
                    "unique_id": "source.demo.a",
                    "resource_type": "source",
                    "relation_name": "warehouse.raw.a",
                    "database": "warehouse",
                    "schema": "raw",
                    "name": "a",
                    "columns": {
                        "id": {"name": "id","data_type":"VARCHAR"},
                        "k": {"name": "k","data_type":"INTEGER"},
                        "v": {"name": "v","data_type":"INTEGER"}
                    }
                },
                "source.demo.b": {
                    "unique_id": "source.demo.b",
                    "resource_type": "source",
                    "relation_name": "warehouse.raw.b",
                    "database": "warehouse",
                    "schema": "raw",
                    "name": "b",
                    "columns": {
                        "id": {"name": "id","data_type":"VARCHAR"},
                        "k": {"name": "k","data_type":"INTEGER"},
                        "v": {"name": "v","data_type":"INTEGER"}
                    }
                },
                "source.demo.c": {
                    "unique_id": "source.demo.c",
                    "resource_type": "source",
                    "relation_name": "warehouse.raw.c",
                    "database": "warehouse",
                    "schema": "raw",
                    "name": "c",
                    "columns": {
                        "id": {"name": "id","data_type":"VARCHAR"},
                        "k": {"name": "k","data_type":"INTEGER"},
                        "v": {"name": "v","data_type":"INTEGER"}
                    }
                }
            }
        });
        let dbt = parse_dbt_manifest(&manifest.to_string()).expect("compiled model");
        let dbt_bundle =
            analyze_dbt_manifest_with_schemas(&dbt, "postgresql", parser.as_ref()).expect("dbt");
        let states = snapshots(case);
        let expected = full_counts(&direct, &states);
        assert_eq!(
            expected,
            full_counts(&adapted, &states),
            "ODCS: {}",
            case["id"]
        );
        assert_eq!(
            expected,
            full_counts(&dbt_bundle, &states),
            "dbt: {}",
            case["id"]
        );
    }
}
