//! TASK-115: equivalent source metadata must produce the same canonical filtered
//! three-way physical proof. These positive-parity checks do not certify the
//! mixed/NULL/PK-FK mandatory bags or downstream grouped terminal.
#[cfg(feature = "odcs")]
#[test]
fn filtered_three_way_chain_has_direct_dbt_and_odcs_source_parity() {
    use serde_json::json;
    use sql_semantic_protocol::{
        analyze_configured_inputs_with_catalog, analyze_dbt_manifest_with_schemas,
        dialect_from_name, parse_dbt_manifest, parse_odcs_yaml, physical_joint_source_plan,
        ConfiguredSqlInput, RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
        WitnessDirection, WitnessObligation,
    };

    let sources = ["warehouse.raw.l", "warehouse.raw.r", "warehouse.raw.t"];
    let sql = "SELECT l.k FROM warehouse.raw.l AS l JOIN warehouse.raw.r AS r ON l.k=r.k JOIN warehouse.raw.t AS t ON r.k=t.k WHERE l.a>0";
    let known = RelationCatalog::new(&sources).expect("ODCS relation names");
    let yaml = r#"
apiVersion: v3.2.0
kind: DataContract
id: task115filtered
name: Filtered three-way source contracts
version: 1.0.0
status: active
schema:
  - name: l
    physicalName: warehouse.raw.l
    properties:
      - name: k
        physicalType: INTEGER
      - name: a
        physicalType: INTEGER
  - name: r
    physicalName: warehouse.raw.r
    properties:
      - name: k
        physicalType: INTEGER
  - name: t
    physicalName: warehouse.raw.t
    properties:
      - name: k
        physicalType: INTEGER
"#;
    let odcs = parse_odcs_yaml(yaml, "postgresql", &known).expect("ODCS typed sources");
    let odcs_catalog = RelationCatalog::from_schemas(odcs.source_schemas()).expect("ODCS catalog");
    let direct_catalog = RelationCatalog::from_schemas(
        &sources
            .iter()
            .enumerate()
            .map(|(index, name)| {
                let mut columns = vec![SchemaColumn::from_sql_type("k", "INTEGER", "postgresql")
                    .expect("typed join key")];
                if index == 0 {
                    columns.push(
                        SchemaColumn::from_sql_type("a", "INTEGER", "postgresql")
                            .expect("typed filter input"),
                    );
                }
                RelationSchema::new(*name, columns).expect("source schema")
            })
            .collect::<Vec<_>>(),
    )
    .expect("direct catalog");
    let manifest = json!({
        "metadata": {
            "dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v12.json",
            "dbt_version": "1.11.8",
            "adapter_type": "postgres"
        },
        "nodes": {
            "model.demo.task115": {
                "unique_id": "model.demo.task115",
                "resource_type": "model",
                "relation_name": "warehouse.analytics.task115",
                "database": "warehouse",
                "schema": "analytics",
                "original_file_path": "models/task115.sql",
                "language": "sql",
                "raw_code": sql,
                "compiled_code": sql,
                "depends_on": {
                    "nodes": ["source.demo.l", "source.demo.r", "source.demo.t"]
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
                "columns": {
                    "k": {"name": "k", "data_type": "INTEGER"},
                    "a": {"name": "a", "data_type": "INTEGER"}
                }
            },
            "source.demo.r": {
                "unique_id": "source.demo.r",
                "resource_type": "source",
                "relation_name": "warehouse.raw.r",
                "database": "warehouse",
                "schema": "raw",
                "name": "r",
                "columns": {
                    "k": {"name": "k", "data_type": "INTEGER"}
                }
            },
            "source.demo.t": {
                "unique_id": "source.demo.t",
                "resource_type": "source",
                "relation_name": "warehouse.raw.t",
                "database": "warehouse",
                "schema": "raw",
                "name": "t",
                "columns": {
                    "k": {"name": "k", "data_type": "INTEGER"}
                }
            }
        }
    });

    let parser = dialect_from_name("postgresql").expect("dialect");
    let parsed = parse_dbt_manifest(&manifest.to_string()).expect("dbt artifact");
    let dbt = analyze_dbt_manifest_with_schemas(&parsed, "postgresql", parser.as_ref())
        .expect("dbt analysis");
    let inline = SqlInput::inline(sql);
    let input = [ConfiguredSqlInput::new(
        "task115-direct",
        &inline,
        "postgresql",
        parser.as_ref(),
    )];
    let direct =
        analyze_configured_inputs_with_catalog(&input, &direct_catalog).expect("direct analysis");
    let odcs =
        analyze_configured_inputs_with_catalog(&input, &odcs_catalog).expect("ODCS analysis");

    let mut canonical = Vec::new();
    for (adapter, bundle) in [("direct", &direct), ("dbt", &dbt), ("odcs", &odcs)] {
        let target = bundle
            .layers()
            .iter()
            .find(|layer| {
                sources
                    .iter()
                    .all(|source| layer.consumes().iter().any(|name| name == source))
            })
            .unwrap_or_else(|| panic!("{adapter}: missing canonical joined layer"));
        let proof = physical_joint_source_plan(bundle, &[(target.id(), 4)]);
        let WitnessDirection::Feasible(cases) = proof.outcome() else {
            panic!("{adapter}: filtered chain lacks complete physical proof: {proof:?}");
        };
        let bag = cases
            .iter()
            .find_map(|case| {
                let population = case.obligations().iter().find_map(|obligation| {
                    if let WitnessObligation::JoinChainPopulation {
                        left_row,
                        middle_row,
                        right_row,
                        left_rows: 2,
                        middle_rows: 2,
                        right_rows: 1,
                        output_rows: 4,
                        matched_value: 1,
                        closed_world: true,
                        ..
                    } = obligation
                    {
                        Some((
                            left_row.relation().to_string(),
                            middle_row.relation().to_string(),
                            right_row.relation().to_string(),
                        ))
                    } else {
                        None
                    }
                })?;
                let physical = case
                    .obligations()
                    .iter()
                    .filter(|obligation| {
                        matches!(obligation, WitnessObligation::ClosedWorld { .. })
                    })
                    .count();
                let filtered = case.obligations().iter().any(|obligation| {
                    matches!(obligation, WitnessObligation::Rows {
                    boundary, predicate, ..
                } if boundary.relation() == sources[0]
                    && !format!("{predicate:?}").contains("All([])"))
                });
                (physical == 3 && filtered).then_some(population)
            })
            .unwrap_or_else(|| panic!("{adapter}: missing same-source closed TRUE bag"));
        canonical.push(bag);
    }
    assert_eq!(
        canonical[0], canonical[1],
        "dbt must carry canonical source facts"
    );
    assert_eq!(
        canonical[0], canonical[2],
        "ODCS must carry canonical source facts"
    );
}
