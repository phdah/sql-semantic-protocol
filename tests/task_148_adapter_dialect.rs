//! TASK-148: shared parser dialects and provenance-independent ordered INSERT states.

use std::collections::BTreeMap;

use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_artifacts, analyze_inputs,
    dialect_from_name, parse_dbt_catalog, parse_dbt_manifest,
    physical_ordered_write_state_plan, ConfiguredSqlInput, LiteralValue,
    PhysicalWriteRow, PhysicalWriteState, PhysicalWriteStatePlan,
    ProtocolStatement, RelationCatalog, RelationSchema, SchemaColumn,
    SqlInput, WriteKind,
};

const DIALECTS: [&str; 13] = [
    "generic", "mysql", "postgresql", "hive", "sqlite", "snowflake",
    "redshift", "mssql", "clickhouse", "bigquery", "ansi", "duckdb", "databricks",
];
const TARGET: &str = "warehouse.analytics.stg_orders";
const SOURCE: &str = "warehouse.raw.orders";
const SCRIPT: [&str; 2] = [
    "INSERT INTO warehouse.analytics.stg_orders(id,amount) SELECT id,amount FROM warehouse.raw.orders WHERE id > 1",
    "INSERT INTO warehouse.analytics.stg_orders(id,amount) VALUES (4,40)",
];

fn direct_catalog() -> RelationCatalog {
    RelationCatalog::from_schemas(&[TARGET, SOURCE].map(|relation| {
        RelationSchema::new(relation, ["id", "amount"].into_iter().map(|column| {
            SchemaColumn::from_sql_type(column, "INTEGER", "postgresql").unwrap()
        }).collect()).unwrap()
    })).unwrap()
}

fn dbt_catalog() -> RelationCatalog {
    let manifest = parse_dbt_manifest(include_str!("fixtures/dbt/manifest-v12.json")).unwrap();
    let catalog = parse_dbt_catalog(include_str!("fixtures/dbt/catalog-v1.json")).unwrap();
    let dialect = dialect_from_name("postgresql").unwrap();
    let analyzed = analyze_dbt_artifacts(&manifest, &catalog, "postgresql", dialect.as_ref()).unwrap();
    RelationCatalog::from_schemas(analyzed.source_schemas()).unwrap()
}

#[cfg(feature = "odcs")]
fn odcs_catalog() -> RelationCatalog {
    use sql_semantic_protocol::parse_odcs_yaml;
    let known = RelationCatalog::new(&[TARGET, SOURCE]).unwrap();
    let contract = parse_odcs_yaml(r#"
apiVersion: v3.2.0
kind: DataContract
id: task148parity
name: Equivalent ordered INSERT metadata
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

fn state(relation: &str, values: &[(i64, i64)]) -> PhysicalWriteState {
    PhysicalWriteState::new(relation, values.iter().map(|(id, amount)| {
        PhysicalWriteRow::new(BTreeMap::from([
            ("id".to_string(), LiteralValue::Number(id.to_string())),
            ("amount".to_string(), LiteralValue::Number(amount.to_string())),
        ])).unwrap()
    }).collect()).unwrap()
}

fn execute(catalog: &RelationCatalog) -> PhysicalWriteStatePlan {
    let sqls = SCRIPT.iter().map(|sql| SqlInput::inline(*sql)).collect::<Vec<_>>();
    let names = ["task148-parity-1", "task148-parity-2"];
    let dialect = dialect_from_name("postgresql").unwrap();
    let configured = sqls.iter().zip(names).map(|(sql, name)| {
        ConfiguredSqlInput::new(name, sql, "postgresql", dialect.as_ref())
    }).collect::<Vec<_>>();
    let bundle = analyze_configured_inputs_with_catalog(&configured, catalog).unwrap();
    physical_ordered_write_state_plan(
        &bundle,
        &[state(SOURCE, &[(1,10), (2,20)]), state(TARGET, &[(0,0)])],
    )
}

#[test]
fn ordered_script_is_source_adapter_independent() {
    let direct = execute(&direct_catalog());
    let dbt = execute(&dbt_catalog());
    let PhysicalWriteStatePlan::Feasible { transitions, final_states } = &direct else {
        panic!("expected exact two-append script: {direct:?}");
    };
    assert_eq!(transitions.len(), 2);
    assert_eq!(transitions[0].before().rows(), state(TARGET, &[(0,0)]).rows());
    assert_eq!(transitions[0].after().rows(), state(TARGET, &[(0,0),(2,20)]).rows());
    assert_eq!(transitions[1].before(), transitions[0].after());
    assert_eq!(transitions[1].after().rows(), state(TARGET, &[(0,0),(2,20),(4,40)]).rows());
    assert_eq!(transitions[0].affected().inserted(), 1);
    assert_eq!(transitions[1].affected().inserted(), 1);
    assert_eq!(final_states.iter().find(|s| s.relation() == SOURCE),
        Some(&state(SOURCE, &[(1,10),(2,20)])));
    assert_eq!(direct, dbt, "dbt-introspected types cannot change exact rows");
}

#[cfg(feature = "odcs")]
#[test]
fn ordered_script_is_odcs_equivalent_when_metadata_is_equivalent() {
    assert_eq!(execute(&direct_catalog()), execute(&odcs_catalog()));
}

#[test]
fn all_thirteen_dialects_have_explicit_parser_or_canonical_outcomes() {
    const FAMILIES: [(&str, bool); 4] = [
        ("INSERT INTO target(id,score) VALUES (1,10),(2,20)", false),
        ("INSERT INTO target DEFAULT VALUES", false),
        ("INSERT INTO target(id,score) SELECT id,score FROM source WHERE id > 1", false),
        ("INSERT INTO target(id,score) WITH filtered AS (SELECT id,score FROM source WHERE id > 1) SELECT id,score FROM filtered", true),
    ];
    let mut classifications = 0_usize;
    let mut supported = [0_usize; 4];
    for dialect in DIALECTS {
        let parser = dialect_from_name(dialect).unwrap();
        for (index, (sql, cte)) in FAMILIES.iter().enumerate() {
            classifications += 1;
            match analyze_inputs(&[SqlInput::inline(sql)], dialect, parser.as_ref()) {
                Err(_) => {} // Parser boundary; not a native engine capability claim.
                Ok(bundle) => match &bundle.inputs()[0].statements()[0] {
                    ProtocolStatement::Unsupported(_) => {} // Explicit fail-closed canonical boundary.
                    ProtocolStatement::Query(query) => {
                        let write = query.write().expect("INSERT cannot silently become SELECT");
                        assert_eq!(write.kind(), WriteKind::Append, "{dialect}: {sql}");
                        if *cte {
                            assert!(write.insert_cte_source().is_some(), "{dialect}: {sql}");
                        }
                        supported[index] += 1;
                    }
                }
            }
        }
    }
    assert_eq!(classifications, 52);
    assert!(supported.iter().all(|count| *count > 0),
        "all four families must have at least one certified parser path: {supported:?}");
}
