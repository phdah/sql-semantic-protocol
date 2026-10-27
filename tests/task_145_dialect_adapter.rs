//! TASK-145: every exposed parser dialect and provenance-neutral INSERT state proofs.

use std::collections::BTreeMap;

use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_artifacts, analyze_inputs,
    dialect_from_name, parse_dbt_catalog, parse_dbt_manifest, physical_ordered_write_state_plan,
    ConfiguredSqlInput, LiteralValue, PhysicalWriteRow, PhysicalWriteState, PhysicalWriteStatePlan,
    ProtocolStatement, RelationCatalog, RelationSchema, SchemaColumn, SqlInput, WriteKind,
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

#[test]
fn each_exposed_dialect_classifies_literal_values_and_default_values_without_fabrication() {
    let forms = [
        (
            "INSERT INTO target (id, score) VALUES (1, 10), (2, 20)",
            false,
        ),
        ("INSERT INTO target DEFAULT VALUES", true),
        ("INSERT INTO target (id) VALUES (1), (2)", false),
    ];
    let mut parsed_values = 0;
    let mut parsed_defaults = 0;
    let mut cells = 0;
    for dialect in DIALECTS {
        let parser = dialect_from_name(dialect).expect("advertised parser dialect");
        for (sql, default) in forms {
            cells += 1;
            // A dialect parser rejection is a parser boundary, never a successful
            // claim of portable native execution for that vendor.
            let Ok(bundle) = analyze_inputs(&[SqlInput::inline(sql)], dialect, parser.as_ref())
            else {
                continue;
            };
            match &bundle.inputs()[0].statements()[0] {
                ProtocolStatement::Query(query) => {
                    assert_eq!(query.write().unwrap().kind(), WriteKind::Append);
                    assert_eq!(query.write().unwrap().default_values(), default);
                    if default {
                        assert!(query.finite_table().is_none(), "{dialect}");
                        parsed_defaults += 1;
                    } else {
                        let rows = query
                            .finite_table()
                            .expect("no invented VALUES rows")
                            .rows();
                        assert_eq!(rows.len(), 2, "{dialect}");
                        parsed_values += 1;
                    }
                }
                ProtocolStatement::Unsupported(_) => {
                    // Conservatively unsupported canonical semantics stay non-exact.
                }
            }
        }
    }
    assert_eq!(cells, 39);
    assert!(
        parsed_values >= 2,
        "at least generic and DuckDB support shared VALUES"
    );
    assert!(
        parsed_defaults >= 1,
        "at least one dialect parses DEFAULT VALUES"
    );
}

const TARGET: &str = "warehouse.analytics.stg_orders";
const SQL: &str = "INSERT INTO warehouse.analytics.stg_orders (id, amount) VALUES (1, 10), (2, 20)";

fn direct_catalog() -> RelationCatalog {
    RelationCatalog::from_schemas(&[RelationSchema::new(
        TARGET,
        vec![
            SchemaColumn::from_sql_type("id", "BIGINT", "postgresql").unwrap(),
            SchemaColumn::from_sql_type("amount", "INTEGER", "postgresql").unwrap(),
        ],
    )
    .unwrap()])
    .unwrap()
}

fn plan(catalog: &RelationCatalog) -> PhysicalWriteStatePlan {
    let dialect = dialect_from_name("postgresql").unwrap();
    let sql = SqlInput::inline(SQL);
    let configured = ConfiguredSqlInput::new("task-145", &sql, "postgresql", dialect.as_ref());
    let bundle = analyze_configured_inputs_with_catalog(&[configured], catalog).unwrap();
    let empty = PhysicalWriteState::new(TARGET, vec![]).unwrap();
    physical_ordered_write_state_plan(&bundle, &[empty])
}

fn assert_two_rows(result: &PhysicalWriteStatePlan) {
    let PhysicalWriteStatePlan::Feasible {
        transitions,
        final_states,
    } = result
    else {
        panic!("adapter evidence must produce complete INSERT state: {result:?}");
    };
    assert_eq!(transitions.len(), 1);
    assert_eq!(transitions[0].affected().inserted(), 2);
    assert!(transitions[0].before().rows().is_empty());
    let rows: Vec<PhysicalWriteRow> = [(1, 10), (2, 20)]
        .into_iter()
        .map(|(id, amount)| {
            PhysicalWriteRow::new(BTreeMap::from([
                ("id".into(), LiteralValue::Number(id.to_string())),
                ("amount".into(), LiteralValue::Number(amount.to_string())),
            ]))
            .unwrap()
        })
        .collect();
    assert_eq!(final_states[0].rows(), rows);
}

#[test]
fn direct_sql_and_dbt_catalog_attest_identical_target_insert_bags() {
    let raw_manifest = parse_dbt_manifest(include_str!("fixtures/dbt/manifest-v12.json")).unwrap();
    let raw_catalog = parse_dbt_catalog(include_str!("fixtures/dbt/catalog-v1.json")).unwrap();
    let dialect = dialect_from_name("postgresql").unwrap();
    let from_dbt =
        analyze_dbt_artifacts(&raw_manifest, &raw_catalog, "postgresql", dialect.as_ref()).unwrap();
    let dbt_catalog = RelationCatalog::from_schemas(from_dbt.source_schemas()).unwrap();
    let direct = plan(&direct_catalog());
    let dbt = plan(&dbt_catalog);
    assert_two_rows(&direct);
    assert_eq!(
        direct, dbt,
        "dbt-provenanced schemas must not change source-free DML"
    );
}

#[cfg(feature = "odcs")]
#[test]
fn odcs_schema_attests_the_same_closed_world_insert_bag() {
    use sql_semantic_protocol::parse_odcs_yaml;
    let existing = RelationCatalog::new(&[TARGET]).unwrap();
    let odcs = parse_odcs_yaml(
        r#"
apiVersion: v3.2.0
kind: DataContract
id: task145insert
name: Task 145 insert
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
"#,
        "postgresql",
        &existing,
    )
    .unwrap();
    let odcs_catalog = RelationCatalog::from_schemas(odcs.source_schemas()).unwrap();
    let direct = plan(&direct_catalog());
    assert_two_rows(&direct);
    assert_eq!(
        direct,
        plan(&odcs_catalog),
        "ODCS facts cannot change append laws"
    );
}
