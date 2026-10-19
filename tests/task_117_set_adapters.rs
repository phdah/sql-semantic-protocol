//! TASK-117 source-independent adapter parity for exact two-column set bags.

#[cfg(feature = "odcs")]
#[test]
fn direct_dbt_and_odcs_retain_the_same_nullable_tuple_histograms() {
    use std::collections::BTreeMap;

    use serde_json::json;
    use sql_semantic_protocol::{
        analyze_configured_inputs_with_catalog, analyze_dbt_manifest_with_schemas,
        dialect_from_name, parse_dbt_manifest, parse_odcs_yaml, physical_finite_set_bag_plan,
        ConfiguredSqlInput, LiteralValue, PhysicalFiniteSetProof, PhysicalWriteRow,
        PhysicalWriteState, RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
    };

    let freeze: serde_json::Value =
        serde_json::from_str(include_str!("../docs/task-117-acceptance-freeze.json"))
            .expect("frozen twelve-case set matrix");
    let names = ["warehouse.raw.a", "warehouse.raw.b"];
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
id: task117setbag
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
"#;
    let known = RelationCatalog::new(&names).expect("known physical names");
    let odcs = parse_odcs_yaml(odcs_yaml, "postgresql", &known).expect("ODCS");
    let odcs_catalog = RelationCatalog::from_schemas(odcs.source_schemas()).expect("ODCS catalog");

    let parser = dialect_from_name("postgresql").expect("postgresql parser");
    for case in freeze["cases"].as_array().expect("frozen cases") {
        let id = case["id"].as_str().expect("case id");
        let sql = case["sql"]
            .as_str()
            .expect("SQL")
            .replace(" FROM a ", " FROM warehouse.raw.a ")
            .replace(" FROM b", " FROM warehouse.raw.b");
        let manifest = json!({
            "metadata": {
                "dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v12.json",
                "dbt_version": "1.11.8",
                "adapter_type": "postgres"
            },
            "nodes": {
                "model.demo.task117": {
                    "unique_id": "model.demo.task117",
                    "resource_type": "model",
                    "relation_name": "warehouse.analytics.task117",
                    "database": "warehouse",
                    "schema": "analytics",
                    "original_file_path": "models/task117.sql",
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

        let physical = names
            .iter()
            .zip(["a", "b"])
            .map(|(relation, key)| {
                let rows = case["source_bags"][key]
                    .as_array()
                    .expect("whole source bag")
                    .iter()
                    .map(|row| {
                        let values = row
                            .as_object()
                            .expect("row")
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
                        PhysicalWriteRow::new(values).expect("row")
                    })
                    .collect();
                PhysicalWriteState::new(*relation, rows).expect("physical source")
            })
            .collect::<Vec<_>>();

        let mut expected = None;
        for (adapter, bundle) in [("direct", &direct), ("dbt", &dbt), ("odcs", &odcs)] {
            let layer = bundle
                .layers()
                .iter()
                .find(|layer| {
                    names
                        .iter()
                        .all(|relation| layer.consumes().iter().any(|source| source == relation))
                })
                .unwrap_or_else(|| panic!("{id}/{adapter}: no two-source set layer"));
            let proof = physical_finite_set_bag_plan(bundle, layer.id(), &physical);
            let PhysicalFiniteSetProof::Exact(exact) = proof else {
                panic!("{id}/{adapter}: adapter lost canonical set proof: {proof:?}");
            };
            let tuple_counts = exact
                .tuples
                .iter()
                .map(|tuple| (tuple.values, tuple.output_count))
                .collect::<BTreeMap<_, _>>();
            assert_eq!(
                exact.terminal.len(),
                case["expected_count"].as_u64().expect("count") as usize,
                "{id}/{adapter}: total"
            );
            if let Some(first) = &expected {
                assert_eq!(&tuple_counts, first, "{id}/{adapter}: tuple parity");
            } else {
                expected = Some(tuple_counts);
            }
        }
    }
}
