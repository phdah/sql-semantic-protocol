//! TASK-119: equivalent typed SQL, dbt compiled manifest and ODCS schema adapters.
//! Only source-type evidence may change; the physical proof is protocol-owned.

#![cfg(feature = "odcs")]

use std::collections::BTreeMap;

use serde_json::{json, Value};
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_manifest_with_schemas, dialect_from_name,
    parse_dbt_manifest, parse_odcs_yaml, physical_finite_set_tree_plan, ConfiguredSqlInput,
    LiteralValue, PhysicalFiniteSetTreeProof, PhysicalWriteRow, PhysicalWriteState,
    RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
};

fn sources() -> Vec<RelationSchema> {
    ["warehouse.raw.a", "warehouse.raw.b"]
        .iter()
        .map(|source| {
            RelationSchema::new(
                *source,
                ["id", "k", "v"]
                    .iter()
                    .map(|column| {
                        SchemaColumn::from_sql_type(
                            *column,
                            if *column == "id" {
                                "VARCHAR"
                            } else {
                                "INTEGER"
                            },
                            "postgresql",
                        )
                        .expect("schema type")
                    })
                    .collect(),
            )
            .expect("source relation")
        })
        .collect()
}

fn snapshots(case: &Value) -> Vec<PhysicalWriteState> {
    case["source_bags"]
        .as_object()
        .expect("source populations")
        .iter()
        .map(|(source, rows)| {
            let rows = rows
                .as_array()
                .expect("physical rows")
                .iter()
                .map(|row| {
                    let values = row
                        .as_object()
                        .expect("complete row")
                        .iter()
                        .map(|(name, value)| {
                            let scalar = if value.is_null() {
                                LiteralValue::Null
                            } else if let Some(number) = value.as_i64() {
                                LiteralValue::Number(number.to_string())
                            } else {
                                LiteralValue::Text(value.as_str().expect("stable ID").to_string())
                            };
                            (name.to_string(), scalar)
                        })
                        .collect::<BTreeMap<_, _>>();
                    PhysicalWriteRow::new(values).expect("physical source")
                })
                .collect();
            PhysicalWriteState::new(format!("warehouse.raw.{source}"), rows)
                .expect("named physical source")
        })
        .collect()
}

fn result(
    bundle: &sql_semantic_protocol::AnalysisBundle,
    states: &[PhysicalWriteState],
) -> BTreeMap<Vec<Option<i128>>, u64> {
    let target = bundle
        .layers()
        .iter()
        .find(|layer| {
            states.iter().all(|source| {
                layer
                    .consumes()
                    .iter()
                    .any(|used| used == source.relation())
            })
        })
        .expect("set terminal with complete physical inputs")
        .id();
    let proof = physical_finite_set_tree_plan(bundle, target, states);
    let PhysicalFiniteSetTreeProof::Exact(exact) = proof else {
        panic!("adapter lost full physical set proof: {proof:?}");
    };
    exact
        .tuples
        .into_iter()
        .filter(|tuple| tuple.output_count > 0)
        .map(|tuple| (tuple.values, tuple.output_count))
        .collect()
}

#[test]
fn bounded_set_bags_have_direct_dbt_and_odcs_parity() {
    let freeze: Value =
        serde_json::from_str(include_str!("../docs/task-119-acceptance-freeze.json"))
            .expect("frozen cases");
    let direct_catalog = RelationCatalog::from_schemas(&sources()).expect("direct schemas");
    let known =
        RelationCatalog::new(&["warehouse.raw.a", "warehouse.raw.b"]).expect("ODCS identities");
    let odcs = parse_odcs_yaml(
        r#"
apiVersion: v3.2.0
kind: DataContract
id: task119setalignment
name: Two positional set source populations
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
"#,
        "postgresql",
        &known,
    )
    .expect("ODCS typed schemas");
    let odcs_catalog = RelationCatalog::from_schemas(odcs.source_schemas()).expect("ODCS");
    let dialect = dialect_from_name("postgresql").expect("parser");
    for case in freeze["cases"]
        .as_array()
        .expect("frozen cases")
        .iter()
        .filter(|case| case["classification"] == "mandatory_exact")
    {
        let sql = case["sql"]
            .as_str()
            .expect("SQL")
            .replace("FROM a", "FROM warehouse.raw.a")
            .replace("FROM b", "FROM warehouse.raw.b");
        let input = SqlInput::inline(&sql);
        let configured = [ConfiguredSqlInput::new(
            "task119-adapter",
            &input,
            "postgresql",
            dialect.as_ref(),
        )];
        let direct = analyze_configured_inputs_with_catalog(&configured, &direct_catalog)
            .expect("typed SQL");
        let with_odcs = analyze_configured_inputs_with_catalog(&configured, &odcs_catalog)
            .expect("typed ODCS SQL");

        let manifest = json!({
            "metadata": {
                "dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v12.json",
                "dbt_version": "1.11.8",
                "adapter_type": "postgres"
            },
            "nodes": {
                "model.demo.task119": {
                    "unique_id": "model.demo.task119",
                    "resource_type": "model",
                    "relation_name": "warehouse.analytics.task119",
                    "database": "warehouse",
                    "schema": "analytics",
                    "original_file_path": "models/task119.sql",
                    "language": "sql",
                    "raw_code": sql,
                    "compiled_code": sql,
                    "depends_on": {
                        "nodes": ["source.demo.a", "source.demo.b"]
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
                        "id": {"name": "id", "data_type": "VARCHAR"},
                        "k": {"name": "k", "data_type": "INTEGER"},
                        "v": {"name": "v", "data_type": "INTEGER"}
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
                        "id": {"name": "id", "data_type": "VARCHAR"},
                        "k": {"name": "k", "data_type": "INTEGER"},
                        "v": {"name": "v", "data_type": "INTEGER"}
                    }
                }
            }
        });
        let artifacts = parse_dbt_manifest(&manifest.to_string()).expect("manifest v12");
        let dbt = analyze_dbt_manifest_with_schemas(&artifacts, "postgresql", dialect.as_ref())
            .expect("compiled dbt manifest");
        let physical = snapshots(case);
        let expected = result(&direct, &physical);
        assert_eq!(
            expected,
            result(&with_odcs, &physical),
            "ODCS: {}",
            case["id"]
        );
        assert_eq!(expected, result(&dbt, &physical), "dbt: {}", case["id"]);
    }
}
