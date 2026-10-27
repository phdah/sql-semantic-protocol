//! TASK-146 frozen parser classifications and provenance-neutral physical-state parity.

use std::collections::BTreeMap;

use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_artifacts, analyze_inputs,
    dialect_from_name, parse_dbt_catalog, parse_dbt_manifest, physical_ordered_write_state_plan,
    CheckCompareOperator, ConfiguredSqlInput, ConstraintEnforcement, ConstraintEvidence,
    ConstraintProvenance, ConstraintSourceKind, ConstraintValue, LiteralValue, PhysicalWriteRow,
    PhysicalWriteState, PhysicalWriteStatePlan, ProtocolStatement, RelationCatalog,
    RelationConstraint, RelationConstraintSet, RelationSchema, SchemaColumn, SqlInput, WriteKind,
};

const DIALECTS: [&str; 13] = [
    "generic",
    "mysql",
    "postgresql",
    "hive",
    "sqlite",
    "snowflake",
    "redshift",
    "mssql",
    "clickhouse",
    "bigquery",
    "ansi",
    "duckdb",
    "databricks",
];
const TARGET: &str = "warehouse.analytics.stg_orders";
const SOURCE: &str = "warehouse.raw.orders";

fn freeze() -> serde_json::Value {
    serde_json::from_str(include_str!("../docs/task-146-acceptance-freeze.json")).unwrap()
}

fn direct_catalog() -> RelationCatalog {
    RelationCatalog::from_schemas(&[TARGET, SOURCE].map(|relation| {
        RelationSchema::new(
            relation,
            ["id", "amount"]
                .into_iter()
                .map(|column| SchemaColumn::from_sql_type(column, "INTEGER", "postgresql").unwrap())
                .collect(),
        )
        .unwrap()
    }))
    .unwrap()
}

fn dbt_catalog() -> RelationCatalog {
    let manifest = parse_dbt_manifest(include_str!("fixtures/dbt/manifest-v12.json")).unwrap();
    let catalog = parse_dbt_catalog(include_str!("fixtures/dbt/catalog-v1.json")).unwrap();
    let dialect = dialect_from_name("postgresql").unwrap();
    let dbt = analyze_dbt_artifacts(&manifest, &catalog, "postgresql", dialect.as_ref()).unwrap();
    RelationCatalog::from_schemas(dbt.source_schemas()).unwrap()
}

#[cfg(feature = "odcs")]
fn odcs_catalog() -> RelationCatalog {
    use sql_semantic_protocol::parse_odcs_yaml;
    let known = RelationCatalog::new(&[TARGET, SOURCE]).unwrap();
    let contract = parse_odcs_yaml(
        r#"
apiVersion: v3.2.0
kind: DataContract
id: task146parity
name: Task 146 equivalent source and target metadata
version: 1.0.0
status: active
schema:
  - name: stg_orders
    physicalName: warehouse.analytics.stg_orders
    properties:
      - name: id
        physicalType: INTEGER
      - name: amount
        physicalType: INTEGER
  - name: orders
    physicalName: warehouse.raw.orders
    properties:
      - name: id
        physicalType: INTEGER
      - name: amount
        physicalType: INTEGER
"#,
        "postgresql",
        &known,
    )
    .unwrap();
    RelationCatalog::from_schemas(contract.source_schemas()).unwrap()
}

fn state(relation: &str, rows: &[(Option<i64>, Option<i64>)]) -> PhysicalWriteState {
    PhysicalWriteState::new(
        relation,
        rows.iter()
            .map(|(id, amount)| {
                PhysicalWriteRow::new(BTreeMap::from([
                    (
                        "id".to_string(),
                        id.map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string())),
                    ),
                    (
                        "amount".to_string(),
                        amount.map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string())),
                    ),
                ]))
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
}

fn evidence() -> Vec<ConstraintEvidence> {
    vec![ConstraintEvidence::new(
        ConstraintProvenance::new(ConstraintSourceKind::ExternalMetadata, "task-146").unwrap(),
        ConstraintEnforcement::Enforced,
    )]
}

fn normalize_sql(sql: &str) -> String {
    sql.replace("target", TARGET)
        .replace("source", SOURCE)
        .replace("score", "amount")
}

fn parity_case(case: &serde_json::Value, catalog: &RelationCatalog) -> PhysicalWriteStatePlan {
    let sql = normalize_sql(case["sql"].as_str().unwrap());
    let input = SqlInput::inline(&sql);
    let dialect = dialect_from_name("postgresql").unwrap();
    let configured =
        ConfiguredSqlInput::new("task-146-parity", &input, "postgresql", dialect.as_ref());
    let mut bundle = analyze_configured_inputs_with_catalog(&[configured], catalog).unwrap();
    let id = case["id"].as_str().unwrap();
    let mut constraints = if id.ends_with("pk-collision") {
        vec![RelationConstraint::primary_key(vec!["id".to_string()], evidence()).unwrap()]
    } else if id.ends_with("check") {
        vec![RelationConstraint::check_comparison(
            "amount",
            CheckCompareOperator::Gte,
            ConstraintValue::Integer(0),
            evidence(),
        )
        .unwrap()]
    } else {
        vec![]
    };
    let fk = id.ends_with("fk-safe") || id.ends_with("fk-violation");
    let mut sets = Vec::new();
    if fk {
        constraints.push(
            RelationConstraint::foreign_key(
                vec!["amount".into()],
                SOURCE,
                vec!["id".into()],
                evidence(),
            )
            .unwrap(),
        );
        sets.push(
            RelationConstraintSet::new(
                SOURCE,
                vec![RelationConstraint::primary_key(vec!["id".into()], evidence()).unwrap()],
            )
            .unwrap(),
        );
    }
    sets.push(RelationConstraintSet::new(TARGET, constraints).unwrap());
    bundle.enrich_relation_constraints(&sets);
    let mut states = vec![state(TARGET, &json_rows(&case["prior_target_bag"]))];
    if !case["source_row_bag"].is_null() {
        states.push(state(SOURCE, &json_rows(&case["source_row_bag"])));
    }
    physical_ordered_write_state_plan(&bundle, &states)
}

fn json_rows(rows: &serde_json::Value) -> Vec<(Option<i64>, Option<i64>)> {
    rows.as_array()
        .unwrap()
        .iter()
        .map(|row| (row["id"].as_i64(), row["score"].as_i64()))
        .collect()
}

#[test]
fn all_117_frozen_parser_cells_classify_exact_or_unsupported_without_inference() {
    let matrix = freeze();
    let mut cells = 0;
    let mut mismatch = Vec::new();
    for case in matrix["cases"].as_array().unwrap() {
        for dialect in DIALECTS {
            let parser = dialect_from_name(dialect).unwrap();
            let sql = case["sql"].as_str().unwrap();
            let observed = match analyze_inputs(&[SqlInput::inline(sql)], dialect, parser.as_ref())
            {
                Err(_) => "parser_boundary",
                Ok(bundle) => match &bundle.inputs()[0].statements()[0] {
                    ProtocolStatement::Unsupported(_) => "canonical_unsupported",
                    ProtocolStatement::Query(query) => {
                        assert_eq!(query.write().expect("INSERT").kind(), WriteKind::Append);
                        if sql.contains("WITH ") && !sql.contains("RECURSIVE") {
                            if query.write().unwrap().insert_cte_source().is_none() {
                                "canonical_unsupported"
                            } else {
                                "parsed_canonical"
                            }
                        } else if sql.contains("RECURSIVE") {
                            "canonical_unsupported"
                        } else {
                            "parsed_canonical"
                        }
                    }
                    _ => panic!("unexpected future protocol statement form"),
                },
            };
            let expected = case["dialect_parser_cells"][dialect]["state"]
                .as_str()
                .unwrap();
            if observed != expected {
                mismatch.push(format!(
                    "{} / {}: {}",
                    case["id"].as_str().unwrap(),
                    dialect,
                    observed
                ));
            }
            cells += 1;
        }
    }
    assert_eq!(cells, matrix["cases"].as_array().unwrap().len() * 13);
    assert!(
        mismatch.is_empty(),
        "dialect classifications: {}",
        mismatch.join("; ")
    );
}

#[test]
fn equivalent_source_target_catalogs_prove_identical_closed_world_states() {
    let freeze = freeze();
    let direct = direct_catalog();
    let dbt = dbt_catalog();
    #[cfg(feature = "odcs")]
    let catalogs = vec![
        ("direct_sql", direct),
        ("dbt_compiled", dbt),
        ("odcs_schema", odcs_catalog()),
    ];
    #[cfg(not(feature = "odcs"))]
    let catalogs = vec![("direct_sql", direct), ("dbt_compiled", dbt)];

    for case in freeze["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        let mut expected = None;
        for (origin, catalog) in &catalogs {
            let proof = parity_case(case, catalog);
            if let Some(baseline) = &expected {
                assert_eq!(
                    &proof, baseline,
                    "{id}: {origin} diverged from equivalent direct metadata"
                );
            } else {
                expected = Some(proof.clone());
            }
            match case["proof"].as_str().unwrap() {
                "Exact" => assert!(
                    matches!(proof, PhysicalWriteStatePlan::Feasible { .. }),
                    "{origin}/{id}: {proof:?}"
                ),
                "Impossible" => assert!(
                    matches!(proof, PhysicalWriteStatePlan::Impossible { .. }),
                    "{origin}/{id}: {proof:?}"
                ),
                "Residual" => assert!(
                    matches!(proof, PhysicalWriteStatePlan::Residual { .. }),
                    "{origin}/{id}: {proof:?}"
                ),
                other => panic!("unrecognized frozen proof: {other}"),
            };
        }
    }
}

#[test]
fn vendor_qualifiers_never_become_unproved_plain_append() {
    let boundaries = [
        "INSERT IGNORE INTO target(id,score) SELECT id,score FROM source",
        "INSERT OVERWRITE INTO target SELECT id,score FROM source",
        "INSERT INTO target(id,score) SELECT id,score FROM source RETURNING id",
        "INSERT INTO target(id,score) SELECT id,score FROM source ON CONFLICT DO NOTHING",
    ];
    for sql in boundaries {
        for dialect in DIALECTS {
            let parser = dialect_from_name(dialect).unwrap();
            if let Ok(bundle) = analyze_inputs(&[SqlInput::inline(sql)], dialect, parser.as_ref()) {
                let unsupported = match &bundle.inputs()[0].statements()[0] {
                    ProtocolStatement::Unsupported(_) => true,
                    ProtocolStatement::Query(q) => q.write().is_none(),
                    _ => true,
                };
                assert!(unsupported, "{dialect}: unproved vendor append law: {sql}");
            }
        }
    }
}
