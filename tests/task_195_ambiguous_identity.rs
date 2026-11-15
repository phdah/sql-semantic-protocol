//! TASK-195: complete physical source is not sufficient to certify the
//! exact source identity assigned to ROW_NUMBER when ORDER BY has peers.

use std::collections::BTreeMap;

use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_inputs, dialect_from_name,
    physical_finite_window_bag_plan, ConfiguredSqlInput, LiteralValue,
    PhysicalFiniteWindowProof, PhysicalWriteRow, PhysicalWriteState,
    RelationCatalog, SqlInput,
};

const FREEZE: &str = include_str!("../docs/task-195-acceptance-freeze.json");

fn complete_order_source(freeze: &Value) -> PhysicalWriteState {
    let rows = freeze["complete_source_bags"]["orders"]
        .as_array()
        .expect("all original six source occurrences")
        .iter()
        .map(|record| {
            let cells = record.as_array().expect("physical source tuple");
            let number = |cell: &Value| cell.as_i64().map_or(
                LiteralValue::Null,
                |value| LiteralValue::Number(value.to_string()),
            );
            PhysicalWriteRow::new(BTreeMap::from([
                ("id".to_string(), number(&cells[0])),
                ("amount".to_string(), number(&cells[1])),
            ]))
            .expect("one physical row with nullable id/amount")
        })
        .collect();
    PhysicalWriteState::new("orders", rows).expect("exact complete named source")
}

fn typed_source() -> RelationCatalog {
    let dialect = dialect_from_name("duckdb").expect("supported parser");
    let ddl = analyze_inputs(
        &[SqlInput::inline("CREATE TABLE orders(id INTEGER, amount INTEGER)")],
        "duckdb",
        dialect.as_ref(),
    )
    .expect("direct warehouse types");
    RelationCatalog::from_schemas(ddl.source_schemas()).expect("typed source catalog")
}

#[test]
fn peer_ordering_cannot_certify_individual_row_number_survival_for_original_sql() {
    let frozen: Value = serde_json::from_str(FREEZE).expect("36 original SQL");
    let source = complete_order_source(&frozen);
    let catalog = typed_source();
    let ids = [
        "88-C177-S106", "88-C177-S107", "88-C177-S108",
        "88-C177-S109", "88-C177-S110", "88-C177-S121",
        "88-C177-S122", "88-C177-S124",
    ];
    for id in ids {
        let original = frozen["cases"]
            .as_array()
            .expect("original inventory")
            .iter()
            .find(|case| case["id"] == id)
            .expect("stable original SQL identity");
        let sql = original["sql"].as_str().expect("frozen original SQL");
        for name in ["duckdb", "postgresql", "snowflake"] {
            let dialect = dialect_from_name(name).expect("named public dialect");
            let input = SqlInput::inline(sql);
            let configured = [ConfiguredSqlInput::new(
                id, &input, name, dialect.as_ref(),
            )];
            let analyzed = analyze_configured_inputs_with_catalog(&configured, &catalog)
                .unwrap_or_else(|error| panic!("{id}/{name}: public SQL: {error}"));
            let target = analyzed.layers().last().expect("original terminal").id();
            let proof = physical_finite_window_bag_plan(
                &analyzed, target, std::slice::from_ref(&source),
            );
            assert!(
                matches!(proof, PhysicalFiniteWindowProof::Residual { .. }),
                "{id}/{name}: identical source peers cannot prove who received which row_number: {proof:?}"
            );
        }
    }
}
