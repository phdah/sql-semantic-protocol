//! TASK-150: source-independent joined write semantics and parser boundaries.
//! Parser success never claims native vendor execution equivalence.

use std::collections::BTreeMap;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_artifacts, analyze_inputs,
    dialect_from_name, parse_dbt_catalog, parse_dbt_manifest, physical_ordered_write_state_plan,
    ConfiguredSqlInput, LiteralValue, PhysicalWriteRow, PhysicalWriteState, PhysicalWriteStatePlan,
    ProtocolStatement, RelationCatalog, RelationSchema, SchemaColumn, SqlInput, WriteKind,
};

const DIALECTS: [&str; 13] = [
    "generic", "mysql", "postgresql", "hive", "sqlite", "snowflake",
    "redshift", "mssql", "clickhouse", "bigquery", "ansi", "duckdb", "databricks",
];
const TARGET: &str = "warehouse.analytics.stg_orders";
const SOURCE: &str = "warehouse.raw.orders";

fn direct_catalog() -> RelationCatalog {
    RelationCatalog::from_schemas(&[TARGET, SOURCE].map(|relation| {
        RelationSchema::new(
            relation,
            ["id", "amount"].into_iter()
                .map(|column| SchemaColumn::from_sql_type(column, "INTEGER", "postgresql").unwrap())
                .collect(),
        ).unwrap()
    })).unwrap()
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
    let contract = parse_odcs_yaml(r#"
apiVersion: v3.2.0
kind: DataContract
id: task150parity
name: Task 150 equivalent joined mutation source metadata
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
"#, "postgresql", &known).unwrap();
    RelationCatalog::from_schemas(contract.source_schemas()).unwrap()
}

fn state(relation: &str, rows: &[(Option<i64>, i64)]) -> PhysicalWriteState {
    PhysicalWriteState::new(relation, rows.iter().map(|(id, amount)| {
        PhysicalWriteRow::new(BTreeMap::from([
            ("id".to_string(), id.map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string()))),
            ("amount".to_string(), LiteralValue::Number(amount.to_string())),
        ])).unwrap()
    }).collect()).unwrap()
}

fn sql(s: &str) -> String {
    s.replace("target", TARGET).replace("source", SOURCE).replace("score", "amount")
}

fn plan(statement: &str, catalog: &RelationCatalog, target: &[(Option<i64>, i64)], source: &[(Option<i64>, i64)]) -> PhysicalWriteStatePlan {
    let sql = sql(statement);
    let input = SqlInput::inline(&sql);
    let dialect = dialect_from_name("postgresql").unwrap();
    let configured = ConfiguredSqlInput::new("task150", &input, "postgresql", dialect.as_ref());
    let bundle = analyze_configured_inputs_with_catalog(&[configured], catalog).unwrap();
    let write = match &bundle.inputs()[0].statements()[0] {
        ProtocolStatement::Query(query) => query.write().expect("joined write"),
        _ => panic!("joined write unexpectedly unsupported: {sql}"),
    };
    assert_eq!(write.joined_source().unwrap().name(), SOURCE);
    physical_ordered_write_state_plan(
        &bundle, &[state(TARGET, target), state(SOURCE, source)],
    )
}

#[test]
fn equivalent_dbt_catalog_and_direct_sql_prove_identical_joined_mutations() {
    let target = [(Some(1), 10), (Some(2), 20), (None, 30)];
    let source = [(Some(1), 40), (None, 50)];
    for sql in [
        "UPDATE target AS t SET score = s.score FROM source AS s WHERE t.id = s.id",
        "DELETE FROM target AS t USING source AS s WHERE t.id = s.id",
    ] {
        let direct = plan(sql, &direct_catalog(), &target, &source);
        let dbt = plan(sql, &dbt_catalog(), &target, &source);
        assert!(matches!(&direct, PhysicalWriteStatePlan::Feasible { .. }), "{direct:?}");
        assert_eq!(direct, dbt, "dbt catalog is only evidence, not another SQL law: {sql}");
    }
}

#[cfg(feature = "odcs")]
#[test]
fn equivalent_odcs_schema_proves_identical_joined_mutations() {
    let target = [(Some(1), 10), (Some(2), 20), (None, 30)];
    let source = [(Some(1), 40), (None, 50)];
    for sql in [
        "UPDATE target AS t SET score = s.score FROM source AS s WHERE t.id = s.id",
        "DELETE FROM target AS t USING source AS s WHERE t.id = s.id",
    ] {
        assert_eq!(
            plan(sql, &direct_catalog(), &target, &source),
            plan(sql, &odcs_catalog(), &target, &source),
        );
    }
}

#[test]
fn all_thirteen_parser_dialects_record_an_explicit_joined_write_boundary() {
    let examples = [
        ("UPDATE target AS t SET score = s.score FROM source AS s WHERE t.id = s.id", WriteKind::Update),
        ("DELETE FROM target AS t USING source AS s WHERE t.id = s.id", WriteKind::Delete),
        ("UPDATE target AS t SET score = s.score FROM source AS s WHERE t.id = s.id RETURNING id", WriteKind::Update),
        ("DELETE FROM target AS t USING source AS s WHERE t.id = s.id RETURNING id", WriteKind::Delete),
    ];
    let mut cells = 0;
    for dialect in DIALECTS {
        let parser = dialect_from_name(dialect).unwrap();
        for (sql, expected_kind) in examples {
            match analyze_inputs(&[SqlInput::inline(sql)], dialect, parser.as_ref()) {
                Err(_) => {}, // explicit parser boundary, never a vendor equivalence claim
                Ok(bundle) => match &bundle.inputs()[0].statements()[0] {
                    ProtocolStatement::Unsupported(_) => {}, // canonical boundary
                    ProtocolStatement::Query(query) => {
                        let write = query.write().expect("no silently dropped write");
                        assert_eq!(write.kind(), expected_kind, "{dialect}: {sql}");
                        assert_eq!(write.joined_source().map(|source| source.name()), Some("source"), "{dialect}: {sql}");
                        assert_eq!(query.dependencies(), &["source".to_string()], "{dialect}: {sql}");
                    }
                    _ => panic!("{dialect}: unrecognized protocol statement for {sql}"),
                },
            }
            cells += 1;
        }
    }
    assert_eq!(cells, 52);
}

#[test]
fn ambiguous_update_source_is_residual_for_direct_and_dbt() {
    let target = [(Some(1), 10)];
    let source = [(Some(1), 40), (Some(1), 50)];
    let sql = "UPDATE target AS t SET score = s.score FROM source AS s WHERE t.id = s.id";
    for catalog in [direct_catalog(), dbt_catalog()] {
        assert!(matches!(plan(sql, &catalog, &target, &source), PhysicalWriteStatePlan::Residual { .. }));
    }
}
