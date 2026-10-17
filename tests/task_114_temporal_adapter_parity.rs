//! TASK-114: exact TIMESTAMP WITHOUT TIME ZONE comparison sources through SQL/dbt/ODCS.
#[cfg(feature = "odcs")]
#[test]
fn timestamp_inequality_has_source_attested_adapter_parity() {
    use serde_json::json;
    use sql_semantic_protocol::{
        analyze_configured_inputs_with_catalog, analyze_dbt_manifest_with_schemas,
        dialect_from_name, parse_dbt_manifest, parse_odcs_yaml, physical_joint_source_plan,
        ConfiguredSqlInput, RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
        WitnessDirection, WitnessObligation,
    };

    let names = ["warehouse.raw.l", "warehouse.raw.r"];
    let source_catalog = RelationCatalog::new(&names).unwrap();
    let contract = r#"
apiVersion: v3.2.0
kind: DataContract
id: task114
name: Typed range input
version: 1.0.0
status: active
schema:
  - name: l
    physicalName: warehouse.raw.l
    properties:
      - name: t
        physicalType: TIMESTAMP WITHOUT TIME ZONE
  - name: r
    physicalName: warehouse.raw.r
    properties:
      - name: t
        physicalType: TIMESTAMP WITHOUT TIME ZONE
"#;
    let odcs = parse_odcs_yaml(contract, "generic", &source_catalog).unwrap();
    let odcs_catalog = RelationCatalog::from_schemas(odcs.source_schemas()).unwrap();
    let catalog = RelationCatalog::from_schemas(
        &names
            .iter()
            .map(|name| {
                RelationSchema::new(
                    *name,
                    vec![SchemaColumn::from_sql_type("t", "TIMESTAMP WITHOUT TIME ZONE", "postgresql").unwrap()],
                )
                .unwrap()
            })
            .collect::<Vec<_>>(),
    )
    .unwrap();
    let parser = dialect_from_name("generic").unwrap();
    for (predicate, law) in [("l.t >= r.t", "timestamp_greater_than_or_equal")] {
        let sql = format!(
            "SELECT l.t FROM warehouse.raw.l AS l JOIN warehouse.raw.r AS r ON {predicate}"
        );
        let manifest = json!({
            "metadata": {
                "dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v12.json",
                "dbt_version": "1.11.8",
                "adapter_type": "postgres"
            },
            "nodes": {
                "model.demo.task114": {
                    "unique_id": "model.demo.task114",
                    "resource_type": "model",
                    "relation_name": "warehouse.analytics.task114",
                    "database": "warehouse",
                    "schema": "analytics",
                    "original_file_path": "models/task114.sql",
                    "language": "sql",
                    "raw_code": sql,
                    "compiled_code": sql,
                    "depends_on": {
                        "nodes": ["source.demo.l", "source.demo.r"]
                    }
                }
            },
            "sources": {
                "source.demo.l": {
                    "unique_id": "source.demo.l",
                    "resource_type": "source",
                    "relation_name": "warehouse.raw.l",
                    "database": "warehouse",
                    "schema": "raw",
                    "name": "l",
                    "columns": {"t": {"name": "t", "data_type": "TIMESTAMP WITHOUT TIME ZONE"}}
                },
                "source.demo.r": {
                    "unique_id": "source.demo.r",
                    "resource_type": "source",
                    "relation_name": "warehouse.raw.r",
                    "database": "warehouse",
                    "schema": "raw",
                    "name": "r",
                    "columns": {"t": {"name": "t", "data_type": "TIMESTAMP WITHOUT TIME ZONE"}}
                }
            }
        });
        let parsed = parse_dbt_manifest(&manifest.to_string()).unwrap();
        let dbt = analyze_dbt_manifest_with_schemas(&parsed, "generic", parser.as_ref()).unwrap();
        let source = SqlInput::inline(sql);
        let input = [ConfiguredSqlInput::new(
            "task114",
            &source,
            "generic",
            parser.as_ref(),
        )];
        let direct = analyze_configured_inputs_with_catalog(&input, &catalog).unwrap();
        let odcs = analyze_configured_inputs_with_catalog(&input, &odcs_catalog).unwrap();
        for (adapter, bundle) in [("direct", &direct), ("dbt", &dbt), ("odcs", &odcs)] {
            let layer = bundle
                .layers()
                .iter()
                .find(|layer| {
                    names
                        .iter()
                        .all(|name| layer.consumes().iter().any(|relation| relation == name))
                })
                .unwrap();
            let plan = physical_joint_source_plan(bundle, &[(layer.id(), 3)]);
            let WitnessDirection::Feasible(cases) = plan.outcome() else {
                panic!("{predicate}/{adapter}: complete two-source plan missing: {plan:?}");
            };
            assert!(
                cases.iter().any(|case| {
                    case.obligations()
                        .iter()
                        .filter(|obligation| {
                            matches!(obligation, WitnessObligation::ClosedWorld { .. })
                        })
                        .count()
                        == 2
                        && case.obligations().iter().any(|obligation| {
                            matches!(obligation, WitnessObligation::TemporalJoinPopulation {
                                law: observed, output_rows: 3, closed_world: true, ..
                            } if observed.as_str() == law)
                        })
                }),
                "{predicate}/{adapter}: identical canonical range law and physical completeness"
            );
        }
    }
}
