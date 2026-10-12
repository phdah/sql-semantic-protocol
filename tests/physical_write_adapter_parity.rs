//! Physical before/after proof must not depend on direct SQL versus dbt adapter origin.

use std::collections::BTreeMap;

use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_artifacts, parse_dbt_catalog,
    parse_dbt_manifest, physical_ordered_write_state_plan, ConfiguredSqlInput, LiteralValue,
    PhysicalWriteRow, PhysicalWriteState, PhysicalWriteStatePlan, RelationCatalog, RelationSchema,
    SchemaColumn, SqlInput,
};
use sqlparser::dialect::PostgreSqlDialect;

#[test]
fn dbt_compiled_sql_and_direct_sql_share_canonical_write_states() {
    const SQL: &str = "UPDATE warehouse.raw.orders SET amount=amount+3 WHERE id = 2";
    let mut manifest: Value =
        serde_json::from_str(include_str!("fixtures/dbt/manifest-v12.json")).unwrap();
    manifest["nodes"]["model.demo.stg_orders"]["compiled_code"] = SQL.into();
    let manifest = parse_dbt_manifest(&manifest.to_string()).unwrap();
    let catalog = parse_dbt_catalog(include_str!("fixtures/dbt/catalog-v1.json")).unwrap();
    let dialect = PostgreSqlDialect {};
    let dbt = analyze_dbt_artifacts(&manifest, &catalog, "postgresql", &dialect).unwrap();

    let schema = RelationSchema::new("warehouse.raw.orders", vec![
        SchemaColumn::from_sql_type("id", "BIGINT", "postgresql").unwrap(),
        SchemaColumn::from_sql_type("amount", "INTEGER", "postgresql").unwrap(),
    ]).unwrap();
    let direct_catalog = RelationCatalog::from_schemas(&[schema]).unwrap();
    let input = SqlInput::inline(SQL);
    let direct = analyze_configured_inputs_with_catalog(
        &[ConfiguredSqlInput::new("direct", &input, "postgresql", &dialect)],
        &direct_catalog,
    ).unwrap();
    let rows = vec![
        PhysicalWriteRow::new(BTreeMap::from([
            ("id".to_string(), LiteralValue::Number("1".to_string())),
            ("amount".to_string(), LiteralValue::Number("4".to_string())),
        ])).unwrap(),
        PhysicalWriteRow::new(BTreeMap::from([
            ("id".to_string(), LiteralValue::Number("2".to_string())),
            ("amount".to_string(), LiteralValue::Number("5".to_string())),
        ])).unwrap(),
    ];
    let initial = PhysicalWriteState::new("warehouse.raw.orders", rows).unwrap();
    let plan = |bundle: &sql_semantic_protocol::AnalysisBundle| {
        physical_ordered_write_state_plan(bundle, std::slice::from_ref(&initial))
    };
    let direct = plan(&direct);
    let dbt = plan(&dbt);
    let PhysicalWriteStatePlan::Feasible { transitions: direct_steps, final_states: direct_states } = direct else {
        panic!("direct write is not exact: {direct:?}");
    };
    let PhysicalWriteStatePlan::Feasible { transitions: dbt_steps, final_states: dbt_states } = dbt else {
        panic!("compiled dbt write is not exact: {dbt:?}");
    };
    assert_eq!(direct_steps.len(), 1);
    assert_eq!(dbt_steps.len(), 1);
    assert_eq!(direct_steps[0].after(), dbt_steps[0].after());
    assert_eq!(direct_states, dbt_states);
    assert_eq!(dbt_steps[0].affected().updated(), 1);
}
