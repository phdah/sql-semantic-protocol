//! TASK-151: parser and evidence-source parity, without native vendor-law claims.

use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_artifacts, analyze_inputs,
    dialect_from_name, parse_dbt_catalog, parse_dbt_manifest, physical_ordered_write_state_plan,
    ConfiguredSqlInput, LiteralValue, PhysicalWriteRow, PhysicalWriteState, PhysicalWriteStatePlan,
    ProtocolStatement, RelationCatalog, RelationSchema, SchemaColumn, SqlInput, WriteKind,
};
use std::collections::BTreeMap;

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

const CASES: [&str; 8] = [
    "UPDATE target AS t SET score = 40 WHERE t.id <= 2 RETURNING t.id",
    "DELETE FROM target AS t WHERE t.id = 2 RETURNING t.id, t.score",
    "UPDATE target AS t SET score = 99 WHERE EXISTS (SELECT 1 FROM source AS s WHERE s.id = t.id) RETURNING t.id",
    "DELETE FROM target AS t WHERE NOT EXISTS (SELECT 1 FROM source AS s WHERE s.id = t.id)",
    "DELETE FROM target ORDER BY id LIMIT 1",
    "UPDATE target PARTITION (p1) SET score = 40 WHERE id = 2",
    "UPDATE target AS t SET score = 40 RETURNING source.id",
    "DELETE FROM target WHERE id = 2 RETURNING *",
];

fn direct_catalog() -> RelationCatalog {
    RelationCatalog::from_schemas(&[TARGET, SOURCE].map(|name| {
        RelationSchema::new(
            name,
            ["id", "amount"]
                .into_iter()
                .map(|column| {
                    SchemaColumn::from_sql_type(
                        column,
                        if name == TARGET && column == "id" {
                            "BIGINT"
                        } else {
                            "INTEGER"
                        },
                        "postgresql",
                    )
                    .unwrap()
                })
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
id: task151parity
name: Task 151 equivalent mutation metadata
version: 1.0.0
status: active
schema:
  - name: stg_orders
    physicalName: warehouse.analytics.stg_orders
    properties:
      - name: id
        physicalType: BIGINT
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

fn sql(input: &str) -> String {
    input
        .replace("target", TARGET)
        .replace("source", SOURCE)
        .replace("score", "amount")
}

fn state(relation: &str, rows: &[(Option<i64>, i64)]) -> PhysicalWriteState {
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
                        LiteralValue::Number(amount.to_string()),
                    ),
                ]))
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
}

fn plan(input: &str, catalog: &RelationCatalog) -> PhysicalWriteStatePlan {
    let query = SqlInput::inline(sql(input));
    let dialect = dialect_from_name("postgresql").unwrap();
    let configured = ConfiguredSqlInput::new("task151", &query, "postgresql", dialect.as_ref());
    let bundle = analyze_configured_inputs_with_catalog(&[configured], catalog).unwrap();
    let rows = [
        state(
            TARGET,
            &[(Some(1), 10), (Some(2), 20), (None, 30), (Some(2), 20)],
        ),
        state(SOURCE, &[(Some(2), 7), (Some(2), 8), (None, 9)]),
    ];
    physical_ordered_write_state_plan(&bundle, &rows)
}

#[test]
fn source_independent_catalogs_agree_on_exact_write_effects() {
    for case in &CASES[..4] {
        let direct = plan(case, &direct_catalog());
        let dbt = plan(case, &dbt_catalog());
        assert!(
            matches!(direct, PhysicalWriteStatePlan::Feasible { .. }),
            "{case}: {direct:?}"
        );
        assert_eq!(direct, dbt, "{case}: dbt source metadata changes proof");
        #[cfg(feature = "odcs")]
        assert_eq!(
            direct,
            plan(case, &odcs_catalog()),
            "{case}: ODCS changes proof"
        );
    }
}

#[test]
fn every_supported_parser_preserves_mutations_and_unsupported_boundaries() {
    let mut observed = BTreeMap::<String, String>::new();
    for dialect in DIALECTS {
        let engine = dialect_from_name(dialect).unwrap();
        for (index, case) in CASES.iter().enumerate() {
            let disposition = match analyze_inputs(
                &[SqlInput::inline(*case)],
                dialect,
                engine.as_ref(),
            ) {
                Err(_) => "parser_boundary",
                Ok(bundle) => match &bundle.inputs()[0].statements()[0] {
                    ProtocolStatement::Unsupported(_) => "canonical_unsupported",
                    ProtocolStatement::Query(query) => {
                        let write = query.write().unwrap_or_else(|| {
                            panic!("{dialect}: {case} parsed but lost write identity")
                        });
                        assert!(
                            matches!(write.kind(), WriteKind::Update | WriteKind::Delete),
                            "{dialect}: {case}"
                        );
                        if index >= 4 {
                            panic!("{dialect}: unproved vendor or RETURNING shape claimed exact: {case}");
                        }
                        if index == 2 || index == 3 {
                            assert_eq!(query.sources().len(), 1, "{dialect}: {case}");
                            assert_eq!(query.dependencies(), &["source".to_string()]);
                        }
                        "parsed_canonical"
                    }
                    _ => panic!("{dialect}: {case} parsed as unrecognized statement"),
                },
            };
            observed.insert(format!("{index}::{dialect}"), disposition.to_string());
        }
    }
    assert_eq!(observed.len(), 104);
    let frozen: serde_json::Value =
        serde_json::from_str(include_str!("../docs/task-151-acceptance-freeze.json")).unwrap();
    let expected = frozen["dialect_expected"].as_object().unwrap_or_else(|| {
        panic!("TASK151_FREEZE_PARSER_CELLS:{}", serde_json::to_string(&observed).unwrap())
    });
    assert_eq!(expected.len(), observed.len(), "all 104 dialect cells must be frozen");
    for (key, value) in observed {
        assert_eq!(expected[&key].as_str(), Some(value.as_str()), "{key}");
    }
}

#[test]
fn ambiguous_or_missing_source_does_not_become_an_empty_source() {
    let case_sql = CASES[2];
    let query = SqlInput::inline(sql(case_sql));
    let dialect = dialect_from_name("postgresql").unwrap();
    let configured =
        ConfiguredSqlInput::new("task151-missing", &query, "postgresql", dialect.as_ref());
    let bundle = analyze_configured_inputs_with_catalog(&[configured], &direct_catalog()).unwrap();
    let plan = physical_ordered_write_state_plan(
        &bundle,
        &[state(TARGET, &[(Some(1), 10), (Some(2), 20)])],
    );
    assert!(
        matches!(plan, PhysicalWriteStatePlan::Residual { .. }),
        "{plan:?}"
    );
}
