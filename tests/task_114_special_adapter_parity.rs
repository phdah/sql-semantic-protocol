//! TASK-114: the same canonical ASOF and correlated physical proofs from all
//! representable source evidence adapters, not consumer-side SQL reparsing.
#[cfg(feature = "odcs")]
#[test]
fn compiled_dbt_direct_sql_and_odcs_attest_identical_asof_and_apply_bags() {
    use serde_json::json;
    use sql_semantic_protocol::{
        analyze_configured_inputs_with_catalog, analyze_dbt_manifest_with_schemas,
        dialect_from_name, parse_dbt_manifest, parse_odcs_yaml, physical_joint_source_plan,
        ConfiguredSqlInput, RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
        WitnessDirection, WitnessObligation,
    };

    let sources = ["warehouse.raw.l", "warehouse.raw.r"];
    let known = RelationCatalog::new(&sources).unwrap();
    let yaml = r#"
apiVersion: v3.2.0
kind: DataContract
id: task114correlated
name: Correlated source contracts
version: 1.0.0
status: active
schema:
  - name: l
    physicalName: warehouse.raw.l
    properties:
      - name: id
        physicalType: VARCHAR
      - name: t
        physicalType: INTEGER
  - name: r
    physicalName: warehouse.raw.r
    properties:
      - name: id
        physicalType: VARCHAR
      - name: t
        physicalType: INTEGER
"#;
    let odcs = parse_odcs_yaml(yaml, "generic", &known).unwrap();
    let odcs_catalog = RelationCatalog::from_schemas(odcs.source_schemas()).unwrap();
    let catalog = RelationCatalog::from_schemas(
        &sources
            .iter()
            .map(|name| {
                RelationSchema::new(
                    *name,
                    vec![
                        SchemaColumn::from_sql_type("id", "VARCHAR", "postgresql").unwrap(),
                        SchemaColumn::from_sql_type("t", "INTEGER", "postgresql").unwrap(),
                    ],
                )
                .unwrap()
            })
            .collect::<Vec<_>>(),
    )
    .unwrap();
    for (variant, dialect, operator, law) in [
        (
            "asof",
            "snowflake",
            "ASOF JOIN warehouse.raw.r AS r MATCH_CONDITION(l.t >= r.t)",
            "asof_nearest_prior_inclusive",
        ),
        (
            "lateral",
            "generic",
            "CROSS JOIN LATERAL (SELECT r.id FROM warehouse.raw.r AS r WHERE r.t = l.t) AS q",
            "correlated_inner_equality",
        ),
        (
            "cross_apply",
            "mssql",
            "CROSS APPLY (SELECT r.id FROM warehouse.raw.r AS r WHERE r.t = l.t) AS q",
            "correlated_inner_equality",
        ),
        (
            "outer_apply",
            "mssql",
            "OUTER APPLY (SELECT r.id FROM warehouse.raw.r AS r WHERE r.t = l.t) AS q",
            "correlated_outer_equality",
        ),
    ] {
        let sql = if variant == "asof" {
            format!("SELECT l.id,r.id FROM warehouse.raw.l AS l {operator}")
        } else {
            format!("SELECT l.id,q.id FROM warehouse.raw.l AS l {operator}")
        };
        let manifest = json!({
            "metadata": {
                "dbt_schema_version":"https://schemas.getdbt.com/dbt/manifest/v12.json",
                "dbt_version":"1.11.8",
                "adapter_type":"postgres"
            },
            "nodes": {
                "model.demo.task114": {
                    "unique_id":"model.demo.task114",
                    "resource_type":"model",
                    "relation_name":"warehouse.analytics.task114",
                    "database":"warehouse",
                    "schema":"analytics",
                    "original_file_path":"models/task114.sql",
                    "language":"sql",
                    "raw_code":sql,
                    "compiled_code":sql,
                    "depends_on":{"nodes":["source.demo.l","source.demo.r"]}
                }
            },
            "sources": {
                "source.demo.l": {
                    "unique_id":"source.demo.l",
                    "resource_type":"source",
                    "relation_name":"warehouse.raw.l",
                    "database":"warehouse",
                    "schema":"raw",
                    "name":"l",
                    "columns":{"id":{"name":"id","data_type":"VARCHAR"},"t":{"name":"t","data_type":"INTEGER"}}
                },
                "source.demo.r": {
                    "unique_id":"source.demo.r",
                    "resource_type":"source",
                    "relation_name":"warehouse.raw.r",
                    "database":"warehouse",
                    "schema":"raw",
                    "name":"r",
                    "columns":{"id":{"name":"id","data_type":"VARCHAR"},"t":{"name":"t","data_type":"INTEGER"}}
                }
            }
        });
        let parser = dialect_from_name(dialect).unwrap();
        let parsed = parse_dbt_manifest(&manifest.to_string()).unwrap();
        let dbt = analyze_dbt_manifest_with_schemas(&parsed, dialect, parser.as_ref()).unwrap();
        let input_sql = SqlInput::inline(sql);
        let input = [ConfiguredSqlInput::new(
            "task114",
            &input_sql,
            dialect,
            parser.as_ref(),
        )];
        let direct = analyze_configured_inputs_with_catalog(&input, &catalog).unwrap();
        let odcs = analyze_configured_inputs_with_catalog(&input, &odcs_catalog).unwrap();
        let mut proof_bags = Vec::new();
        for (adapter, bundle) in [("direct", &direct), ("dbt", &dbt), ("odcs", &odcs)] {
            let layer = bundle
                .layers()
                .iter()
                .find(|layer| {
                    sources
                        .iter()
                        .all(|source| layer.consumes().iter().any(|name| name == source))
                })
                .unwrap();
            let plan = physical_joint_source_plan(bundle, &[(layer.id(), 2)]);
            let WitnessDirection::Feasible(cases) = plan.outcome() else {
                panic!("{variant}/{adapter}: no physical proof: {plan:?}");
            };
            let bag = cases
                .iter()
                .find_map(|case| {
                    case.obligations().iter().find_map(|obligation| {
                        if let WitnessObligation::FiniteJoinPopulation {
                            law: observed,
                            left_values,
                            right_values,
                            output_rows: 2,
                            closed_world: true,
                            ..
                        } = obligation
                        {
                            (observed.as_str() == law).then(|| {
                                (
                                    left_values.clone(),
                                    right_values.clone(),
                                    observed.selected_rows(left_values, right_values).unwrap(),
                                    observed
                                        .unselected_pairs(left_values, right_values)
                                        .unwrap(),
                                )
                            })
                        } else {
                            None
                        }
                    })
                })
                .unwrap_or_else(|| panic!("{variant}/{adapter}: missing canonical row-bag law"));
            proof_bags.push(bag);
        }
        assert_eq!(
            proof_bags[0], proof_bags[1],
            "{variant}: dbt canonical parity"
        );
        assert_eq!(
            proof_bags[0], proof_bags[2],
            "{variant}: ODCS canonical parity"
        );
    }
}
