//! TASK-123 source-independent aggregate semantics for SQL, dbt and ODCS.
#[cfg(feature = "odcs")]
#[test]
fn joined_aggregate_adapters_prove_exact_or_explicit_residual() {
    use std::collections::BTreeMap;

    use serde_json::{json, Value};
    use sql_semantic_protocol::{
        analyze_configured_inputs_with_catalog, analyze_dbt_manifest_with_schemas,
        dialect_from_name, parse_dbt_manifest, parse_odcs_yaml,
        physical_finite_join_aggregate_plan, ConfiguredSqlInput, LiteralValue,
        PhysicalJoinedAggregateProof, PhysicalWriteRow, PhysicalWriteState, RelationCatalog,
        RelationSchema, SchemaColumn, SqlInput,
    };

    let inventory: Value =
        serde_json::from_str(include_str!("../docs/task-123-acceptance-freeze.json"))
            .expect("TASK-123 immutable acceptance matrix");
    let relations = ["warehouse.raw.l", "warehouse.raw.r"];
    let schemas = relations
        .into_iter()
        .map(|name| {
            RelationSchema::new(
                name,
                ["id", "k", "x"]
                    .into_iter()
                    .map(|column| {
                        SchemaColumn::from_sql_type(
                            column,
                            if column == "id" { "VARCHAR" } else { "INTEGER" },
                            "postgresql",
                        )
                        .expect("physical source datatype")
                    })
                    .collect(),
            )
            .expect("typed physical relation")
        })
        .collect::<Vec<_>>();
    let catalog = RelationCatalog::from_schemas(&schemas).expect("direct SQL catalog");
    let known = RelationCatalog::new(&relations).expect("authoritative physical relations");
    let odcs = parse_odcs_yaml(
        r#"
apiVersion: v3.2.0
kind: DataContract
id: task123joinaggregation
name: Typed physical joined aggregate inputs
version: 1.0.0
status: active
schema:
  - name: l
    physicalName: warehouse.raw.l
    properties:
      - name: id
        physicalType: VARCHAR
      - name: k
        physicalType: INTEGER
      - name: x
        physicalType: INTEGER
  - name: r
    physicalName: warehouse.raw.r
    properties:
      - name: id
        physicalType: VARCHAR
      - name: k
        physicalType: INTEGER
      - name: x
        physicalType: INTEGER
"#,
        "postgresql",
        &known,
    )
    .expect("ODCS parsed source schemas");
    let odcs_catalog =
        RelationCatalog::from_schemas(odcs.source_schemas()).expect("ODCS typed catalog");
    let parser = dialect_from_name("postgresql").expect("portable parser");

    for case in inventory["cases"].as_array().expect("frozen cases") {
        let mandatory = case["kind"] == "mandatory";
        let fixture = case["id"].as_str().expect("fixture");
        let sql = case["sql"].as_str().expect("SQL").replace(
            "FROM l INNER JOIN r",
            "FROM warehouse.raw.l AS l INNER JOIN warehouse.raw.r AS r",
        );
        let input = SqlInput::inline(&sql);
        let config = ConfiguredSqlInput::new(fixture, &input, "postgresql", parser.as_ref());
        let direct = analyze_configured_inputs_with_catalog(&[config], &catalog)
            .map_err(|error| error.to_string());
        let config = ConfiguredSqlInput::new(fixture, &input, "postgresql", parser.as_ref());
        let odcs_bundle = analyze_configured_inputs_with_catalog(&[config], &odcs_catalog)
            .map_err(|error| error.to_string());

        let manifest = json!({
            "metadata": {
                "dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v12.json",
                "dbt_version": "1.11.8",
                "adapter_type": "postgres"
            },
            "nodes": {
                "model.demo.task123": {
                    "unique_id": "model.demo.task123",
                    "resource_type": "model",
                    "relation_name": "warehouse.analytics.task123",
                    "database": "warehouse",
                    "schema": "analytics",
                    "original_file_path": "models/task123.sql",
                    "language": "sql",
                    "raw_code": sql,
                    "compiled_code": sql,
                    "depends_on": {"nodes": ["source.demo.l", "source.demo.r"]}
                }
            },
            "sources": {
                "source.demo.l": {
                    "unique_id": "source.demo.l", "resource_type": "source",
                    "relation_name": "warehouse.raw.l", "database": "warehouse",
                    "schema": "raw", "name": "l",
                    "columns": {
                        "id": {"name": "id", "data_type": "VARCHAR"},
                        "k": {"name": "k", "data_type": "INTEGER"},
                        "x": {"name": "x", "data_type": "INTEGER"}
                    }
                },
                "source.demo.r": {
                    "unique_id": "source.demo.r", "resource_type": "source",
                    "relation_name": "warehouse.raw.r", "database": "warehouse",
                    "schema": "raw", "name": "r",
                    "columns": {
                        "id": {"name": "id", "data_type": "VARCHAR"},
                        "k": {"name": "k", "data_type": "INTEGER"},
                        "x": {"name": "x", "data_type": "INTEGER"}
                    }
                }
            }
        });
        let parsed = parse_dbt_manifest(&manifest.to_string()).expect("compiled dbt manifest");
        let dbt = analyze_dbt_manifest_with_schemas(&parsed, "postgresql", parser.as_ref())
            .map_err(|error| error.to_string());

        let states = ["l", "r"]
            .into_iter()
            .zip(relations)
            .map(|(short, relation)| {
                PhysicalWriteState::new(
                    relation,
                    case["source_bags"][short]
                        .as_array()
                        .expect("complete physical source")
                        .iter()
                        .map(|row| {
                            PhysicalWriteRow::new(
                                row.as_object()
                                    .expect("row")
                                    .iter()
                                    .map(|(column, value)| {
                                        let scalar = if value.is_null() {
                                            LiteralValue::Null
                                        } else if let Some(number) = value.as_i64() {
                                            LiteralValue::Number(number.to_string())
                                        } else {
                                            LiteralValue::Text(
                                                value
                                                    .as_str()
                                                    .expect("source identity")
                                                    .to_string(),
                                            )
                                        };
                                        (column.clone(), scalar)
                                    })
                                    .collect::<BTreeMap<_, _>>(),
                            )
                            .expect("row values")
                        })
                        .collect(),
                )
                .expect("closed source")
            })
            .collect::<Vec<_>>();
        let mut proofs = Vec::new();
        for (name, bundle) in [("SQL", &direct), ("dbt", &dbt), ("ODCS", &odcs_bundle)] {
            let Ok(bundle) = bundle else {
                assert!(
                    !mandatory,
                    "{name}:{fixture}: mandatory analysis failed: {bundle:?}"
                );
                continue;
            };
            let target = bundle
                .layers()
                .iter()
                .find(|layer| layer.consumes().iter().any(|input| input == relations[0]))
                .unwrap_or_else(|| panic!("{name}:{fixture}: no physical input producer"))
                .id();
            let result = physical_finite_join_aggregate_plan(bundle, target, &states);
            if !mandatory {
                assert!(
                    matches!(result, PhysicalJoinedAggregateProof::Residual { .. }),
                    "{name}:{fixture}: conditional grouping must fail closed: {result:?}"
                );
                continue;
            }
            let PhysicalJoinedAggregateProof::Exact(exact) = result else {
                panic!("{name}:{fixture}: missing exact joined canonical proof: {result:?}");
            };
            assert_eq!(exact.sources, relations.map(str::to_string));
            proofs.push((
                exact.selected.clone(),
                exact.rejected.clone(),
                exact.groups.clone(),
                exact.terminal.clone(),
                exact.terminal_domains.clone(),
            ));
        }
        if mandatory {
            assert_eq!(proofs[0], proofs[1], "{fixture}: compiled dbt parity");
            assert_eq!(proofs[0], proofs[2], "{fixture}: ODCS schema parity");
        }
    }
}
