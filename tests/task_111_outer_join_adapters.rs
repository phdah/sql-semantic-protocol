//! TASK-111: direct SQL, compiled dbt and ODCS evidence must produce
//! the same canonical physical outer join witness, not inferred SQL variants.

#[cfg(feature = "odcs")]
#[test]
fn compiled_dbt_and_odcs_outer_join_use_one_typed_source_assignment() {
    use sql_semantic_protocol::{
        analyze_configured_inputs_with_catalog, analyze_dbt_manifest_with_schemas,
        dialect_from_name, parse_dbt_manifest, parse_odcs_yaml, physical_joint_source_plan,
        ConfiguredSqlInput, JoinKind, JoinPopulationPattern, RelationCatalog, RelationSchema,
        SchemaColumn, SqlInput, WitnessDirection, WitnessObligation,
    };

    for (expected_kind, join_sql, expected_left, expected_right) in [
        (JoinKind::Left, "LEFT JOIN", 3, 2),
        (JoinKind::Right, "RIGHT JOIN", 2, 3),
        (JoinKind::Full, "FULL OUTER JOIN", 2, 2),
    ] {
        let sql = format!(
            "SELECT l.k FROM warehouse.raw.left_input AS l {join_sql} warehouse.raw.right_input AS r ON r.k=l.k"
        );
        let manifest = serde_json::json!({
            "metadata": {
                "dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v12.json",
                "dbt_version": "1.11.8",
                "adapter_type": "postgres"
            },
            "nodes": {
                "model.demo.outer_joined": {
                    "unique_id": "model.demo.outer_joined",
                    "resource_type": "model",
                    "relation_name": "warehouse.analytics.outer_joined",
                    "database": "warehouse",
                    "schema": "analytics",
                    "original_file_path": "models/outer_joined.sql",
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
                    "columns": { "k": {"name": "k", "data_type": "INTEGER"} }
                },
                "source.demo.right_input": {
                    "unique_id": "source.demo.right_input",
                    "resource_type": "source",
                    "relation_name": "warehouse.raw.right_input",
                    "database": "warehouse",
                    "schema": "raw",
                    "name": "right_input",
                    "columns": { "k": {"name": "k", "data_type": "INTEGER"} }
                }
            }
        });
        let parser = dialect_from_name("postgresql").expect("PostgreSQL");
        let parsed_manifest = parse_dbt_manifest(&manifest.to_string()).expect("compiled dbt v12");
        let from_dbt =
            analyze_dbt_manifest_with_schemas(&parsed_manifest, "postgresql", parser.as_ref())
                .expect("typed dbt source schemas");
        let relations = ["warehouse.raw.left_input", "warehouse.raw.right_input"];
        let names = RelationCatalog::new(&relations).expect("physical relations");
        let odcs_yaml = r#"
    apiVersion: v3.2.0
    kind: DataContract
    id: task111
    name: Outer join inputs
    version: 1.0.0
    status: active
    schema:
      - name: left_input
        physicalName: warehouse.raw.left_input
        properties:
          - name: k
            physicalType: INTEGER
      - name: right_input
        physicalName: warehouse.raw.right_input
        properties:
          - name: k
            physicalType: INTEGER
    "#;
        let odcs =
            parse_odcs_yaml(odcs_yaml, "postgresql", &names).expect("typed ODCS physical metadata");
        let odcs_catalog =
            RelationCatalog::from_schemas(odcs.source_schemas()).expect("ODCS catalog");
        let schemas = relations
            .iter()
            .map(|relation| {
                RelationSchema::new(
                    *relation,
                    vec![SchemaColumn::from_sql_type("k", "INTEGER", "postgresql")
                        .expect("physical key")],
                )
                .expect("source")
            })
            .collect::<Vec<_>>();
        let catalog = RelationCatalog::from_schemas(&schemas).expect("direct catalog");
        let source = SqlInput::inline(sql);
        let configured = [ConfiguredSqlInput::new(
            "task111",
            &source,
            "postgresql",
            parser.as_ref(),
        )];
        let direct = analyze_configured_inputs_with_catalog(&configured, &catalog)
            .expect("direct SQL analysis");
        let from_odcs = analyze_configured_inputs_with_catalog(&configured, &odcs_catalog)
            .expect("ODCS source analysis");

        for (adapter, analyzed) in [
            ("direct", &direct),
            ("dbt", &from_dbt),
            ("odcs", &from_odcs),
        ] {
            let layer = analyzed
                .layers()
                .iter()
                .find(|layer| {
                    relations
                        .iter()
                        .all(|name| layer.consumes().iter().any(|v| v == name))
                })
                .expect("outer join terminal consumes both sources");
            let plan = physical_joint_source_plan(analyzed, &[(layer.id(), 3)]);
            let WitnessDirection::Feasible(cases) = plan.outcome() else {
                panic!("{adapter}: physical outer proof required: {plan:?}");
            };
            assert!(
                cases
                    .iter()
                    .any(|case| case.obligations().iter().any(|o| matches!(
                        o,
                        WitnessObligation::JoinPopulation {
                            join_kind,
                            pattern: JoinPopulationPattern::MixedMatchedUnmatched,
                            left_rows,
                            right_rows,
                            output_rows: 3,
                            closed_world: true,
                            ..
                        } if *join_kind == expected_kind
                            && *left_rows == expected_left
                            && *right_rows == expected_right
                    ))),
                "{adapter}: {expected_kind:?} compiled facts must retain exact mixed outer bag"
            );
        }
    }
}
