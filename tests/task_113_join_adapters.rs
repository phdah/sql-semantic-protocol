//! TASK-113: direct SQL, dbt compiled artifacts and ODCS share one physical proof.

#[cfg(feature = "odcs")]
#[test]
fn compiled_dbt_odcs_and_direct_sql_have_equivalent_portable_join_evidence() {
    use serde_json::json;
    use sql_semantic_protocol::{
        analyze_configured_inputs_with_catalog, analyze_dbt_manifest_with_schemas,
        dialect_from_name, parse_dbt_manifest, parse_odcs_yaml, physical_joint_source_plan,
        ConfiguredSqlInput, JoinPopulationPattern, RelationCatalog, RelationSchema, SchemaColumn,
        SqlInput, WitnessDirection, WitnessObligation,
    };

    let relations = ["warehouse.raw.left_input", "warehouse.raw.right_input"];
    let names = RelationCatalog::new(&relations).unwrap();
    let yaml = r#"
apiVersion: v3.2.0
kind: DataContract
id: task113
name: Portable joined inputs
version: 1.0.0
status: active
schema:
  - name: left_input
    physicalName: warehouse.raw.left_input
    properties:
      - name: k
        physicalType: INTEGER
      - name: lid
        physicalType: VARCHAR
  - name: right_input
    physicalName: warehouse.raw.right_input
    properties:
      - name: k
        physicalType: INTEGER
      - name: rid
        physicalType: VARCHAR
"#;
    let odcs = parse_odcs_yaml(yaml, "generic", &names).unwrap();
    let odcs_catalog = RelationCatalog::from_schemas(odcs.source_schemas()).unwrap();
    let schemas = relations
        .iter()
        .map(|relation| {
            let id = if *relation == relations[0] {
                "lid"
            } else {
                "rid"
            };
            RelationSchema::new(
                *relation,
                vec![
                    SchemaColumn::from_sql_type("k", "INTEGER", "postgresql").unwrap(),
                    SchemaColumn::from_sql_type(id, "VARCHAR", "postgresql").unwrap(),
                ],
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let catalog = RelationCatalog::from_schemas(&schemas).unwrap();

    for (variant, join_expression, dialect) in [
        ("cross", "CROSS JOIN", "generic"),
        ("using", "JOIN", "generic"),
        ("natural", "NATURAL JOIN", "generic"),
        ("or_on", "JOIN", "generic"),
        ("non_equi", "JOIN", "generic"),
        ("is_not_distinct_from", "JOIN", "generic"),
        ("null_safe_equality", "JOIN", "mysql"),
    ] {
        let right = "warehouse.raw.right_input AS r";
        let join = match variant {
            "cross" => format!("{join_expression} {right}"),
            "using" => format!("{join_expression} {right} USING (k)"),
            "natural" => format!("{join_expression} {right}"),
            "or_on" => format!("{join_expression} {right} ON l.k = r.k OR l.k IS NULL"),
            "non_equi" => format!("{join_expression} {right} ON l.k < r.k"),
            "is_not_distinct_from" => {
                format!("{join_expression} {right} ON l.k IS NOT DISTINCT FROM r.k")
            }
            "null_safe_equality" => format!("{join_expression} {right} ON l.k <=> r.k"),
            _ => unreachable!(),
        };
        let sql = format!("SELECT l.k FROM warehouse.raw.left_input AS l {join}");
        let manifest = json!({
            "metadata": {
                "dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v12.json",
                "dbt_version": "1.11.8",
                "adapter_type": "postgres"
            },
            "nodes": {
                "model.demo.task113": {
                    "unique_id": "model.demo.task113",
                    "resource_type": "model",
                    "relation_name": "warehouse.analytics.task113",
                    "database": "warehouse",
                    "schema": "analytics",
                    "original_file_path": "models/task113.sql",
                    "language": "sql",
                    "raw_code": sql,
                    "compiled_code": sql,
                    "depends_on": {
                        "nodes": ["source.demo.left_input", "source.demo.right_input"]
                    }
                }
            },
            "sources": {
                "source.demo.left_input": {
                    "unique_id": "source.demo.left_input",
                    "resource_type": "source",
                    "relation_name": "warehouse.raw.left_input",
                    "database": "warehouse",
                    "schema": "raw",
                    "name": "left_input",
                    "columns": {
                        "k": {"name": "k", "data_type": "INTEGER"},
                        "lid": {"name": "lid", "data_type": "VARCHAR"}
                    }
                },
                "source.demo.right_input": {
                    "unique_id": "source.demo.right_input",
                    "resource_type": "source",
                    "relation_name": "warehouse.raw.right_input",
                    "database": "warehouse",
                    "schema": "raw",
                    "name": "right_input",
                    "columns": {
                        "k": {"name": "k", "data_type": "INTEGER"},
                        "rid": {"name": "rid", "data_type": "VARCHAR"}
                    }
                }
            }
        });
        let parser = dialect_from_name(dialect).unwrap();
        let parsed_manifest = parse_dbt_manifest(&manifest.to_string()).unwrap();
        let dbt_bundle =
            analyze_dbt_manifest_with_schemas(&parsed_manifest, dialect, parser.as_ref()).unwrap();
        let source = SqlInput::inline(sql);
        let configured = [ConfiguredSqlInput::new(
            "task-113",
            &source,
            dialect,
            parser.as_ref(),
        )];
        let direct_bundle = analyze_configured_inputs_with_catalog(&configured, &catalog).unwrap();
        let odcs_bundle =
            analyze_configured_inputs_with_catalog(&configured, &odcs_catalog).unwrap();

        for (adapter, bundle) in [
            ("sql", &direct_bundle),
            ("dbt", &dbt_bundle),
            ("odcs", &odcs_bundle),
        ] {
            let layer = bundle
                .layers()
                .iter()
                .find(|layer| {
                    relations
                        .iter()
                        .all(|name| layer.consumes().iter().any(|c| c == name))
                })
                .unwrap();
            let plan = physical_joint_source_plan(bundle, &[(layer.id(), 2)]);
            let WitnessDirection::Feasible(cases) = plan.outcome() else {
                panic!("{variant}/{adapter}: source-backed proof needed: {plan:?}");
            };
            assert!(cases.iter().any(|case| {
                let closed = case.obligations().iter().filter(|obligation| {
                    matches!(obligation, WitnessObligation::ClosedWorld { .. })
                }).count() == 2;
                let population = case.obligations().iter().any(|obligation| {
                    matches!(
                        obligation,
                        WitnessObligation::JoinPopulation {
                            output_rows: 2,
                            pattern: JoinPopulationPattern::CommonMatched,
                            closed_world: true,
                            ..
                        } | WitnessObligation::UniformJoinPopulation {
                            output_rows: 2,
                            closed_world: true,
                            ..
                        }
                    )
                });
                closed && population
            }), "{variant}/{adapter}: exact two-source bag proof");
        }
    }
}
