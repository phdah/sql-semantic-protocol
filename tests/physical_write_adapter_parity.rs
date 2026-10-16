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
fn dbt_catalog_schema_and_direct_sql_share_canonical_write_states() {
    const SQL: &str = "INSERT INTO warehouse.analytics.stg_orders (id,amount) SELECT id,amount FROM warehouse.raw.orders WHERE id = 2";
    let manifest: Value =
        serde_json::from_str(include_str!("fixtures/dbt/manifest-v12.json")).unwrap();
    // A dbt model supplies canonical schema/graph evidence; a caller-owned
    // write statement consumes that same evidence, without pretending that
    // compiled dbt model SELECTs are executable DML statements.
    let manifest = parse_dbt_manifest(&manifest.to_string()).unwrap();
    let catalog = parse_dbt_catalog(include_str!("fixtures/dbt/catalog-v1.json")).unwrap();
    let dialect = PostgreSqlDialect {};
    let dbt = analyze_dbt_artifacts(&manifest, &catalog, "postgresql", &dialect).unwrap();

    let schemas = ["warehouse.raw.orders", "warehouse.analytics.stg_orders"]
        .into_iter()
        .map(|relation| {
            RelationSchema::new(
                relation,
                vec![
                    SchemaColumn::from_sql_type("id", "BIGINT", "postgresql").unwrap(),
                    SchemaColumn::from_sql_type("amount", "INTEGER", "postgresql").unwrap(),
                ],
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let direct_catalog = RelationCatalog::from_schemas(&schemas).unwrap();
    let dbt_catalog = RelationCatalog::from_schemas(dbt.source_schemas()).unwrap();
    let input = SqlInput::inline(SQL);
    let make_bundle = |catalog: &RelationCatalog| {
        analyze_configured_inputs_with_catalog(
            &[ConfiguredSqlInput::new(
                "direct",
                &input,
                "postgresql",
                &dialect,
            )],
            catalog,
        )
        .unwrap()
    };
    let direct = make_bundle(&direct_catalog);
    let from_dbt_metadata = make_bundle(&dbt_catalog);
    let rows = vec![
        PhysicalWriteRow::new(BTreeMap::from([
            ("id".to_string(), LiteralValue::Number("1".to_string())),
            ("amount".to_string(), LiteralValue::Number("4".to_string())),
        ]))
        .unwrap(),
        PhysicalWriteRow::new(BTreeMap::from([
            ("id".to_string(), LiteralValue::Number("2".to_string())),
            ("amount".to_string(), LiteralValue::Number("5".to_string())),
        ]))
        .unwrap(),
    ];
    let initial = vec![
        PhysicalWriteState::new("warehouse.raw.orders", rows).unwrap(),
        PhysicalWriteState::new("warehouse.analytics.stg_orders", vec![]).unwrap(),
    ];
    let plan = |bundle: &sql_semantic_protocol::AnalysisBundle| {
        physical_ordered_write_state_plan(bundle, &initial)
    };
    let direct = plan(&direct);
    let dbt = plan(&from_dbt_metadata);
    let PhysicalWriteStatePlan::Feasible {
        transitions: direct_steps,
        final_states: direct_states,
    } = direct
    else {
        panic!("direct write is not exact: {direct:?}");
    };
    let PhysicalWriteStatePlan::Feasible {
        transitions: dbt_steps,
        final_states: dbt_states,
    } = dbt
    else {
        panic!("dbt-provenanced schema DML is not exact: {dbt:?}");
    };
    assert_eq!(direct_steps.len(), 1);
    assert_eq!(dbt_steps.len(), 1);
    assert_eq!(direct_steps[0].after(), dbt_steps[0].after());
    assert_eq!(direct_states, dbt_states);
    assert_eq!(dbt_steps[0].affected().inserted(), 1);
}

#[test]
fn computed_ordered_writes_match_dbt_catalog_schema_evidence() {
    let manifest: Value =
        serde_json::from_str(include_str!("fixtures/dbt/manifest-v12.json")).unwrap();
    let manifest = parse_dbt_manifest(&manifest.to_string()).unwrap();
    let catalog = parse_dbt_catalog(include_str!("fixtures/dbt/catalog-v1.json")).unwrap();
    let dialect = PostgreSqlDialect {};
    let dbt = analyze_dbt_artifacts(&manifest, &catalog, "postgresql", &dialect).unwrap();
    let schemas = ["warehouse.raw.orders", "warehouse.analytics.stg_orders"]
        .into_iter()
        .map(|relation| {
            RelationSchema::new(
                relation,
                vec![
                    SchemaColumn::from_sql_type("id", "BIGINT", "postgresql").unwrap(),
                    SchemaColumn::from_sql_type("amount", "INTEGER", "postgresql").unwrap(),
                ],
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let direct_catalog = RelationCatalog::from_schemas(&schemas).unwrap();
    let dbt_catalog = RelationCatalog::from_schemas(dbt.source_schemas()).unwrap();
    let sql = [
        "INSERT INTO warehouse.analytics.stg_orders (id,amount) SELECT id,amount + 1 FROM warehouse.raw.orders WHERE id > 1",
        "UPDATE warehouse.analytics.stg_orders SET amount = amount + 2 WHERE id = 2",
        "MERGE INTO warehouse.analytics.stg_orders AS target USING warehouse.raw.orders AS source ON target.id = source.id
         WHEN MATCHED AND source.amount < 0 THEN DELETE
         WHEN MATCHED THEN UPDATE SET amount = source.amount + 4
         WHEN NOT MATCHED AND source.amount > 0 THEN
             INSERT (id,amount) VALUES (source.id,source.amount + 4)",
    ];
    let sources = sql
        .iter()
        .map(|statement| SqlInput::inline(*statement))
        .collect::<Vec<_>>();
    let ids = ["insert", "update", "merge"];
    let make_bundle = |catalog: &RelationCatalog| {
        let inputs = sources
            .iter()
            .zip(ids)
            .map(|(source, id)| ConfiguredSqlInput::new(id, source, "postgresql", &dialect))
            .collect::<Vec<_>>();
        analyze_configured_inputs_with_catalog(&inputs, catalog).unwrap()
    };
    let row = |id: i64, amount: Option<i64>| {
        PhysicalWriteRow::new(BTreeMap::from([
            ("id".to_string(), LiteralValue::Number(id.to_string())),
            (
                "amount".to_string(),
                amount.map_or(LiteralValue::Null, |value| {
                    LiteralValue::Number(value.to_string())
                }),
            ),
        ]))
        .unwrap()
    };
    let initial = vec![
        PhysicalWriteState::new("warehouse.analytics.stg_orders", vec![row(1, Some(4))]).unwrap(),
        PhysicalWriteState::new(
            "warehouse.raw.orders",
            vec![
                row(2, Some(9)),
                row(3, Some(-5)),
                row(0, Some(5)),
                row(-1, None),
            ],
        )
        .unwrap(),
    ];
    let direct = physical_ordered_write_state_plan(&make_bundle(&direct_catalog), &initial);
    let from_dbt = physical_ordered_write_state_plan(&make_bundle(&dbt_catalog), &initial);
    assert_eq!(
        direct, from_dbt,
        "adapter provenance cannot change write semantics"
    );
    let PhysicalWriteStatePlan::Feasible {
        transitions,
        final_states,
    } = from_dbt
    else {
        panic!("computed ordered writes must be feasible: {from_dbt:?}");
    };
    assert_eq!(transitions.len(), 3);
    assert_eq!(transitions[2].affected().deleted(), 1);
    assert_eq!(transitions[2].affected().updated(), 1);
    assert_eq!(transitions[2].affected().inserted(), 1);
    let target = final_states
        .iter()
        .find(|state| state.relation() == "warehouse.analytics.stg_orders")
        .unwrap();
    assert_eq!(
        target.rows(),
        &[row(1, Some(4)), row(2, Some(13)), row(0, Some(9))]
    );
}
