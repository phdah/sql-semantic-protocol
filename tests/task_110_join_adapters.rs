//! TASK-110 adapter contract: compiled dbt and ODCS must share the typed
//! physical SQL join-chain construction, not manufacture intermediate rows.

#[cfg(feature = "odcs")]
#[test]
fn compiled_dbt_and_odcs_three_way_join_preserve_one_physical_proof() {
    use sql_semantic_protocol::{
        analyze_configured_inputs_with_catalog, analyze_dbt_manifest_with_schemas,
        dialect_from_name, parse_dbt_manifest, parse_odcs_yaml, physical_joint_source_plan,
        ConfiguredSqlInput, RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
        WitnessDirection, WitnessObligation,
    };

    let sql = "SELECT l.k FROM warehouse.raw.left_input AS l                JOIN warehouse.raw.middle_input AS r ON l.k=r.k                JOIN warehouse.raw.right_input AS t ON r.k=t.k";
    let manifest = serde_json::json!({
        "metadata": {
            "dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v12.json",
            "dbt_version": "1.11.8",
            "adapter_type": "postgres"
        },
        "nodes": {
            "model.demo.joined": {
                "unique_id": "model.demo.joined",
                "resource_type": "model",
                "relation_name": "warehouse.analytics.joined",
                "database": "warehouse",
                "schema": "analytics",
                "original_file_path": "models/joined.sql",
                "language": "sql",
                "raw_code": sql,
                "compiled_code": sql,
                "depends_on": {
                    "nodes": [
                        "source.demo.left_input",
                        "source.demo.middle_input",
                        "source.demo.right_input"
                    ]
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
            "source.demo.middle_input": {
                "unique_id": "source.demo.middle_input",
                "resource_type": "source",
                "relation_name": "warehouse.raw.middle_input",
                "database": "warehouse",
                "schema": "raw",
                "name": "middle_input",
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

    let relations = [
        "warehouse.raw.left_input",
        "warehouse.raw.middle_input",
        "warehouse.raw.right_input",
    ];
    let names = RelationCatalog::new(&relations).expect("physical relations");
    let odcs_yaml = r#"
apiVersion: v3.2.0
kind: DataContract
id: task110
name: Chain join inputs
version: 1.0.0
status: active
schema:
  - name: left_input
    physicalName: warehouse.raw.left_input
    properties:
      - name: k
        physicalType: INTEGER
  - name: middle_input
    physicalName: warehouse.raw.middle_input
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
    let odcs_catalog = RelationCatalog::from_schemas(odcs.source_schemas()).expect("ODCS catalog");
    let direct_schemas = relations
        .iter()
        .map(|relation| {
            RelationSchema::new(
                *relation,
                vec![SchemaColumn::from_sql_type("k", "INTEGER", "postgresql")
                    .expect("physical source key")],
            )
            .expect("typed source")
        })
        .collect::<Vec<_>>();
    let direct_catalog = RelationCatalog::from_schemas(&direct_schemas).expect("direct catalog");
    let source = SqlInput::inline(sql);
    let configured = [ConfiguredSqlInput::new(
        "task110",
        &source,
        "postgresql",
        parser.as_ref(),
    )];
    let direct = analyze_configured_inputs_with_catalog(&configured, &direct_catalog)
        .expect("direct SQL analysis");
    let from_odcs = analyze_configured_inputs_with_catalog(&configured, &odcs_catalog)
        .expect("ODCS source analysis");

    for (kind, analyzed) in [
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
                    .all(|relation| layer.consumes().iter().any(|name| name == relation))
            })
            .expect("one terminal consumes all three inputs");
        let plan = physical_joint_source_plan(analyzed, &[(layer.id(), 4)]);
        let WitnessDirection::Feasible(cases) = plan.outcome() else {
            panic!("{kind}: complete physical join proof required: {plan:?}");
        };
        assert!(
            cases.iter().any(|case| {
                case.obligations().iter().any(|obligation| {
                    matches!(
                        obligation,
                        WitnessObligation::JoinChainPopulation {
                            left_rows: 2,
                            middle_rows: 2,
                            right_rows: 1,
                            output_rows: 4,
                            closed_world: true,
                            ..
                        }
                    )
                })
            }),
            "{kind}: adapter must preserve one exact three-instance bag"
        );
    }
}
