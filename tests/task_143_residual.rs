//! TASK-143 fail-closed boundaries for unproved computed producer semantics.
use std::collections::BTreeMap;

use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_finite_cte_bag_plan,
    ConfiguredSqlInput, LiteralValue, PhysicalCteProof, PhysicalWriteRow, PhysicalWriteState,
    RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
};

#[test]
fn frozen_overflow_and_opaque_computation_never_claim_exact_cte_bags() {
    let freeze: Value =
        serde_json::from_str(include_str!("../docs/task-143-acceptance-freeze.json")).unwrap();
    let catalog = RelationCatalog::from_schemas(&[
        RelationSchema::new(
            "raw".to_string(),
            vec![SchemaColumn::from_sql_type("k", "INTEGER", "duckdb").unwrap()],
        )
        .unwrap(),
    ])
    .unwrap();
    let dialect = dialect_from_name("duckdb").unwrap();
    let mut verified = Vec::new();
    for case in freeze["cases"].as_array().unwrap() {
        if case["classification"] != "conditional_residual" {
            continue;
        }
        let id = case["id"].as_str().unwrap();
        let source = SqlInput::inline(case["sql"].as_str().unwrap());
        let configured = [ConfiguredSqlInput::new("cte", &source, "duckdb", dialect.as_ref())];
        let bundle = analyze_configured_inputs_with_catalog(&configured, &catalog).unwrap();
        let rows = case["initial_bags"]["raw"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| {
                let k = row["k"].as_i64().unwrap();
                PhysicalWriteRow::new(BTreeMap::from([(
                    "k".to_string(),
                    LiteralValue::Number(k.to_string()),
                )]))
                .unwrap()
            })
            .collect();
        let snapshot = [PhysicalWriteState::new("raw".to_string(), rows).unwrap()];
        let target = bundle.layers().last().unwrap();
        assert!(
            matches!(
                physical_finite_cte_bag_plan(&bundle, target.id(), &snapshot, None),
                PhysicalCteProof::Residual { .. }
            ),
            "{id}: unsupported precision/function semantics must fail closed"
        );
        verified.push(id.to_string());
    }
    verified.sort();
    assert_eq!(verified, ["78-C143-N-OPAQUE", "78-C143-N-OVERFLOW"]);
}
