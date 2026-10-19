//! TASK-118: nested and repeated source-branch adapter parity at the public API.

#![cfg(feature = "odcs")]

use std::collections::BTreeMap;

use serde_json::{json, Value};
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_artifacts,
    analyze_dbt_manifest_with_schemas, dialect_from_name, parse_dbt_catalog, parse_dbt_manifest,
    parse_odcs_yaml, physical_finite_set_tree_plan, ConfiguredSqlInput, LiteralValue,
    PhysicalFiniteSetTreeProof, PhysicalWriteRow, PhysicalWriteState, RelationCatalog,
    RelationSchema, SchemaColumn, SqlInput,
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
        .filter(|case| case["statements"].as_array().expect("producer").is_empty())
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
        let referenced_sources = case["source_bags"]
            .as_object()
            .expect("physical source bag names")
            .keys()
            .map(|source| format!("source.demo.{source}"))
            .collect::<Vec<_>>();
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
                        "nodes": referenced_sources
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

#[test]
fn produced_model_bag_proofs_reuse_one_source_assignment_across_sql_dbt_and_odcs() {
    let freeze: Value =
        serde_json::from_str(include_str!("../docs/task-118-acceptance-freeze.json"))
            .expect("frozen model");
    let case = freeze["cases"]
        .as_array()
        .expect("cases")
        .iter()
        .find(|case| case["id"] == "72-C118-P-produced-model")
        .expect("task model variant");
    let names = ["a", "b", "c"];
    let producer_names = ["sa", "sb", "sc"];
    let model_relation = |relation: &str| format!("warehouse.raw.{relation}");
    let direct_schemas = names
        .iter()
        .map(|name| {
            source_schemas()
                .into_iter()
                .find(|schema| schema.relation() == model_relation(name))
                .expect("physical typed schema")
        })
        .chain(producer_names.iter().map(|name| {
            RelationSchema::new(
                model_relation(name),
                ["k", "v"]
                    .into_iter()
                    .map(|column| {
                        SchemaColumn::from_sql_type(column, "INTEGER", "postgresql")
                            .expect("model output type")
                    })
                    .collect(),
            )
            .expect("named model schema")
        }))
        .collect::<Vec<_>>();
    let known = direct_schemas
        .iter()
        .map(|schema| schema.relation())
        .collect::<Vec<_>>();
    let known = RelationCatalog::new(&known).expect("all external and named relations");
    let contract_entries = names.iter()
        .chain(producer_names.iter())
        .map(|name| format!(
            "  - name: {name}\n    physicalName: warehouse.raw.{name}\n    properties:\n      - name: k\n        physicalType: INTEGER\n      - name: v\n        physicalType: INTEGER\n{}",
            if producer_names.contains(name) { String::new() } else {
                "      - name: id\n        physicalType: VARCHAR\n".to_string()
            }
        )).collect::<Vec<_>>().join("");
    let contract = format!(
        "apiVersion: v3.2.0\nkind: DataContract\nid: task118models\nname: Nested typed producers\nversion: 1.0.0\nstatus: active\nschema:\n{contract_entries}"
    );
    let odcs = parse_odcs_yaml(&contract, "postgresql", &known).expect("ODCS schemas");
    let odcs_catalog = RelationCatalog::from_schemas(odcs.source_schemas()).expect("ODCS catalog");
    let direct_catalog = RelationCatalog::from_schemas(&direct_schemas)
        .expect("full direct source and model schema");
    let parser = dialect_from_name("postgresql").expect("parser");
    let mut sql = names
        .iter()
        .zip(producer_names.iter())
        .map(|(source, target)| {
            format!("CREATE TABLE warehouse.raw.{target} AS SELECT k,v FROM warehouse.raw.{source}")
        })
        .collect::<Vec<_>>();
    sql.push(
        "(SELECT k,v FROM warehouse.raw.sa UNION ALL SELECT k,v FROM warehouse.raw.sb) EXCEPT ALL SELECT k,v FROM warehouse.raw.sc".to_string()
    );
    let input = sql.iter().map(SqlInput::inline).collect::<Vec<_>>();
    let identities = (0..sql.len())
        .map(|n| format!("model-{n}"))
        .collect::<Vec<_>>();
    let configured = input
        .iter()
        .zip(identities.iter())
        .map(|(sql, name)| ConfiguredSqlInput::new(name, sql, "postgresql", parser.as_ref()))
        .collect::<Vec<_>>();
    let direct = analyze_configured_inputs_with_catalog(&configured, &direct_catalog)
        .expect("direct materialized model");
    let from_odcs = analyze_configured_inputs_with_catalog(&configured, &odcs_catalog)
        .expect("ODCS source producer analysis");

    let mut nodes = serde_json::Map::new();
    for (source, target) in names.iter().zip(producer_names.iter()) {
        let key = format!("model.demo.{target}");
        let compiled = format!("SELECT k,v FROM warehouse.raw.{source}");
        nodes.insert(
            key.clone(),
            json!({
                "unique_id": key, "resource_type": "model",
                "relation_name": model_relation(target), "database": "warehouse",
                "schema": "raw", "original_file_path": format!("models/{target}.sql"),
                "language": "sql", "raw_code": compiled, "compiled_code": compiled,
                "depends_on": {"nodes": [format!("source.demo.{source}")]},
                "columns": {"k": {"name": "k", "data_type": "INTEGER"},
                            "v": {"name": "v", "data_type": "INTEGER"}}
            }),
        );
    }
    nodes.insert(
        "model.demo.task118".to_string(),
        json!({
            "unique_id": "model.demo.task118", "resource_type": "model",
            "relation_name": "warehouse.analytics.task118",
            "database": "warehouse", "schema": "analytics",
            "original_file_path": "models/task118.sql", "language": "sql",
            "raw_code": sql.last().expect("terminal model"),
            "compiled_code": sql.last().expect("terminal model"),
            "depends_on": {"nodes": ["model.demo.sa","model.demo.sb","model.demo.sc"]},
            "columns": {"k": {"name": "k", "data_type": "INTEGER"},
                        "v": {"name": "v", "data_type": "INTEGER"}}
        }),
    );
    let mut sources = serde_json::Map::new();
    for source in names {
        let key = format!("source.demo.{source}");
        sources.insert(
            key.clone(),
            json!({
                "unique_id": key, "resource_type": "source",
                "relation_name": model_relation(source), "database": "warehouse",
                "schema": "raw", "name": source,
                "columns": {"id": {"name": "id", "data_type": "VARCHAR"},
                            "k": {"name": "k", "data_type": "INTEGER"},
                            "v": {"name": "v", "data_type": "INTEGER"}}
            }),
        );
    }
    let manifest = json!({
        "metadata": {
            "dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v12.json",
            "dbt_version": "1.11.8", "adapter_type": "postgres"
        },
        "nodes": nodes, "sources": sources,
    });
    let parsed = parse_dbt_manifest(&manifest.to_string()).expect("compiled model sources");
    // Manifest-declared intermediate model columns alone are not authoritative
    // evidence that no materialized cast occurred. The paired dbt catalog
    // supplies the equal typed model schemas required for exact transport.
    let mut model_catalog_nodes = serde_json::Map::new();
    for name in producer_names {
        let key = format!("model.demo.{name}");
        model_catalog_nodes.insert(
            key.clone(),
            json!({
                "unique_id": key,
                "metadata": {
                    "type": "VIEW", "database": "warehouse",
                    "schema": "raw", "name": name
                },
                "columns": {
                    "k": {"name": "k", "type": "INTEGER", "index": 1},
                    "v": {"name": "v", "type": "INTEGER", "index": 2}
                },
                "stats": {}
            }),
        );
    }
    let catalog_artifact = json!({
        "metadata": {
            "dbt_schema_version": "https://schemas.getdbt.com/dbt/catalog/v1.json",
            "dbt_version": "1.11.8"
        },
        "nodes": model_catalog_nodes,
        "sources": {},
        "errors": null,
    });
    let parsed_catalog =
        parse_dbt_catalog(&catalog_artifact.to_string()).expect("paired model relation catalog");
    let dbt = analyze_dbt_artifacts(&parsed, &parsed_catalog, "postgresql", parser.as_ref())
        .expect("typed dbt materialized models");
    let states = snapshots(case);
    let proof = |bundle: &sql_semantic_protocol::AnalysisBundle| {
        let target = bundle.layers().last().expect("terminal layer").id();
        let result = physical_finite_set_tree_plan(bundle, target, &states);
        let PhysicalFiniteSetTreeProof::Exact(exact) = result else {
            panic!("named produced source lost full physical bag proof: {result:?}");
        };
        (
            exact.sources,
            exact
                .tuples
                .into_iter()
                .map(|tuple| (tuple.values, tuple.output_count))
                .collect::<BTreeMap<_, _>>(),
        )
    };
    let expected = proof(&direct);
    assert_eq!(expected, proof(&from_odcs), "ODCS model outputs");
    assert_eq!(expected, proof(&dbt), "dbt model outputs");
    assert_eq!(
        expected.0,
        names
            .iter()
            .map(|name| model_relation(name))
            .collect::<Vec<_>>(),
        "the only writable inputs are original physical relations",
    );
    assert_eq!(
        expected.1.get(&vec![Some(1), None]),
        Some(&2),
        "independent frozen nested SQL oracle",
    );
}
