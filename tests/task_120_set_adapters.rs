//! TASK-120 source-independent adapter parity for exact two-column set bags.

#[cfg(feature = "odcs")]
#[test]
fn all_mandatory_set_trees_have_exact_direct_dbt_and_odcs_parity() {
    use std::collections::BTreeMap;

    use serde_json::json;
    use sql_semantic_protocol::{
        analyze_configured_inputs_with_catalog, analyze_dbt_manifest_with_schemas,
        dialect_from_name, parse_dbt_manifest, parse_odcs_yaml, physical_finite_set_bag_plan,
        physical_finite_set_tree_plan, ConfiguredSqlInput, LiteralValue, PhysicalFiniteSetProof,
        PhysicalFiniteSetTreeProof, PhysicalWriteRow, PhysicalWriteState, RelationCatalog,
        RelationSchema, SchemaColumn, SqlInput,
    };

    let freeze: serde_json::Value =
        serde_json::from_str(include_str!("../docs/task-120-acceptance-freeze.json"))
            .expect("task-owned 21-case set matrix");
    let names = ["warehouse.raw.a", "warehouse.raw.b", "warehouse.raw.c"];
    let sql_type = |name: &str| {
        if name == "id" {
            "VARCHAR"
        } else {
            "INTEGER"
        }
    };
    let schemas = names
        .iter()
        .map(|relation| {
            RelationSchema::new(
                *relation,
                ["id", "k", "v"]
                    .iter()
                    .map(|column| {
                        SchemaColumn::from_sql_type(*column, sql_type(column), "postgresql")
                            .expect("typed column")
                    })
                    .collect(),
            )
            .expect("direct schema")
        })
        .collect::<Vec<_>>();
    let direct_catalog = RelationCatalog::from_schemas(&schemas).expect("direct catalog");

    let odcs_yaml = r#"
apiVersion: v3.2.0
kind: DataContract
id: task120setbag
name: Complete source tuple bag
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
"#;
    let known = RelationCatalog::new(&names).expect("known physical names");
    let odcs = parse_odcs_yaml(odcs_yaml, "postgresql", &known).expect("ODCS");
    let odcs_catalog = RelationCatalog::from_schemas(odcs.source_schemas()).expect("ODCS catalog");

    let parser = dialect_from_name("postgresql").expect("postgresql parser");
    for case in freeze["mandatory_cases"]
        .as_array()
        .expect("all 21 mandatory cases")
    {
        let id = case["id"].as_str().expect("case id");
        let mut sql = case["sql"].as_str().expect("SQL").to_string();
        for source in ["a", "b", "c"] {
            sql = sql.replace(
                &format!("FROM {source}"),
                &format!("FROM warehouse.raw.{source}"),
            );
        }
        let mut manifest = json!({
            "metadata": {
                "dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v12.json",
                "dbt_version": "1.11.8",
                "adapter_type": "postgres"
            },
            "nodes": {
                "model.demo.task120": {
                    "unique_id": "model.demo.task120",
                    "resource_type": "model",
                    "relation_name": "warehouse.analytics.task120",
                    "database": "warehouse",
                    "schema": "analytics",
                    "original_file_path": "models/task120.sql",
                    "language": "sql",
                    "raw_code": sql,
                    "compiled_code": sql,
                    "depends_on": {"nodes": ["source.demo.a", "source.demo.b"]}
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
                        "id": {"name":"id","data_type":"VARCHAR"},
                        "k": {"name":"k","data_type":"INTEGER"},
                        "v": {"name":"v","data_type":"INTEGER"}
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
                        "id": {"name":"id","data_type":"VARCHAR"},
                        "k": {"name":"k","data_type":"INTEGER"},
                        "v": {"name":"v","data_type":"INTEGER"}
                    }
                }
            }
        });
        // The same typed source catalog is visible to every adapter, even when
        // a particular frozen tree consumes only a subset of the sources.
        let mut third_source = manifest["sources"]["source.demo.b"].clone();
        third_source["unique_id"] = json!("source.demo.c");
        third_source["name"] = json!("c");
        third_source["relation_name"] = json!("warehouse.raw.c");
        manifest["sources"]["source.demo.c"] = third_source;
        let dependencies = case["source_bags"]
            .as_object()
            .expect("actual physical dependencies")
            .keys()
            .map(|name| format!("source.demo.{name}"))
            .collect::<Vec<_>>();
        manifest["nodes"]["model.demo.task120"]["depends_on"]["nodes"] = json!(dependencies);
        let parsed = parse_dbt_manifest(&manifest.to_string()).expect("compiled dbt manifest");
        let dbt = analyze_dbt_manifest_with_schemas(&parsed, "postgresql", parser.as_ref())
            .expect("dbt canonical analysis");
        let source = SqlInput::inline(&sql);
        let configured = [ConfiguredSqlInput::new(
            id,
            &source,
            "postgresql",
            parser.as_ref(),
        )];
        let direct = analyze_configured_inputs_with_catalog(&configured, &direct_catalog)
            .expect("direct canonical analysis");
        let odcs = analyze_configured_inputs_with_catalog(&configured, &odcs_catalog)
            .expect("ODCS canonical analysis");

        let physical = case["source_bags"]
            .as_object()
            .expect("closed-world inputs")
            .iter()
            .map(|(source, rows)| {
                let values = rows
                    .as_array()
                    .expect("whole physical bag")
                    .iter()
                    .map(|row| {
                        let values = row
                            .as_object()
                            .expect("source tuple")
                            .iter()
                            .map(|(column, value)| {
                                let value = if value.is_null() {
                                    LiteralValue::Null
                                } else if let Some(number) = value.as_i64() {
                                    LiteralValue::Number(number.to_string())
                                } else {
                                    LiteralValue::Text(
                                        value.as_str().expect("physical row id").to_string(),
                                    )
                                };
                                (column.to_string(), value)
                            })
                            .collect::<BTreeMap<_, _>>();
                        PhysicalWriteRow::new(values).expect("valid physical row")
                    })
                    .collect();
                PhysicalWriteState::new(format!("warehouse.raw.{source}"), values)
                    .expect("physical source identity")
            })
            .collect::<Vec<_>>();

        let mut expected = None;
        for (adapter, bundle) in [("direct", &direct), ("dbt", &dbt), ("odcs", &odcs)] {
            let layer = bundle
                .layers()
                .iter()
                .find(|layer| {
                    physical.iter().all(|source| {
                        layer
                            .consumes()
                            .iter()
                            .any(|used| used == source.relation())
                    })
                })
                .unwrap_or_else(|| panic!("{id}/{adapter}: no two-source set layer"));
            let (positive, all_counts, count, domains) =
                if case["variant"] == "six_operator_nullable_bag" {
                    let proof = physical_finite_set_bag_plan(bundle, layer.id(), &physical);
                    let PhysicalFiniteSetProof::Exact(exact) = proof else {
                        panic!("{id}/{adapter}: adapter lost exact tuple bag: {proof:?}");
                    };
                    (
                        exact
                            .tuples
                            .iter()
                            .filter(|tuple| tuple.output_count > 0)
                            .map(|tuple| (tuple.values.to_vec(), tuple.output_count))
                            .collect::<BTreeMap<_, _>>(),
                        exact
                            .tuples
                            .iter()
                            .map(|tuple| (tuple.values.to_vec(), tuple.output_count))
                            .collect::<BTreeMap<_, _>>(),
                        exact.terminal.len(),
                        exact.terminal_domains.to_vec(),
                    )
                } else {
                    let proof = physical_finite_set_tree_plan(bundle, layer.id(), &physical);
                    let PhysicalFiniteSetTreeProof::Exact(exact) = proof else {
                        panic!("{id}/{adapter}: adapter lost exact set tree: {proof:?}");
                    };
                    (
                        exact
                            .tuples
                            .iter()
                            .filter(|tuple| tuple.output_count > 0)
                            .map(|tuple| (tuple.values.clone(), tuple.output_count))
                            .collect::<BTreeMap<_, _>>(),
                        exact
                            .tuples
                            .iter()
                            .map(|tuple| (tuple.values.clone(), tuple.output_count))
                            .collect::<BTreeMap<_, _>>(),
                        exact.terminal.len(),
                        exact.terminal_domains,
                    )
                };
            let frozen_counts = case["expected_terminal_tuple_histogram"]
                .as_array()
                .expect("independently frozen complete tuple bag")
                .iter()
                .map(|row| {
                    let key = row["tuple"]
                        .as_array()
                        .expect("two columns")
                        .iter()
                        .map(|value| value.as_i64().map(i128::from))
                        .collect::<Vec<_>>();
                    (key, row["count"].as_u64().expect("frequency"))
                })
                .collect::<BTreeMap<_, _>>();
            assert_eq!(
                positive, frozen_counts,
                "{id}/{adapter}: complete tuple bag"
            );
            assert_eq!(
                count,
                case["expected_count"].as_u64().expect("count") as usize,
                "{id}/{adapter}: complete terminal multiplicity"
            );
            for (index, domain) in domains.iter().enumerate() {
                let frozen = &case["typed_terminal_domains"][index];
                let values = frozen["allowed"]
                    .as_array()
                    .expect("frozen finite domain")
                    .iter()
                    .map(|value| i128::from(value.as_i64().expect("integer")))
                    .collect::<Vec<_>>();
                assert_eq!(
                    domain.values, values,
                    "{id}/{adapter}: column {index} values"
                );
                assert_eq!(
                    domain.contains_null,
                    frozen["contains_null"].as_bool().expect("NULL"),
                    "{id}/{adapter}: column {index} nullable"
                );
                let bounds = match (frozen["lower"].as_i64(), frozen["upper"].as_i64()) {
                    (Some(lower), Some(upper)) => Some((i128::from(lower), i128::from(upper))),
                    _ => None,
                };
                assert_eq!(
                    domain.bounds(),
                    bounds,
                    "{id}/{adapter}: column {index} bounds"
                );
            }
            let tuple_counts = all_counts;
            if let Some(first) = &expected {
                assert_eq!(&tuple_counts, first, "{id}/{adapter}: tuple parity");
            } else {
                expected = Some(tuple_counts);
            }
        }
    }
}
