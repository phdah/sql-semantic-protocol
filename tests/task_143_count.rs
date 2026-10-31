//! COUNT(*) on filtered/reused physical CTE bags, independently checked by DuckDB.
use std::collections::BTreeMap;

use duckdb::{params, Connection};
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_finite_cte_bag_plan,
    ConfiguredSqlInput, LiteralValue, PhysicalCteProof, PhysicalWriteRow, PhysicalWriteState,
    RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
};

const SQL: &str = "WITH stage AS (SELECT k, k + 10 AS shifted FROM raw WHERE k > 1) \
SELECT COUNT(*) AS total FROM stage a JOIN stage b ON a.k = b.k";

fn assert_count_bags(values: &[Option<i64>], expected_stage: usize, expected_count: i64) {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE raw (k INTEGER)").unwrap();
    for value in values {
        db.execute("INSERT INTO raw VALUES (?1)", params![value]).unwrap();
    }
    let observed: i64 = db.query_row(SQL, [], |row| row.get(0)).unwrap();
    assert_eq!(observed, expected_count);

    let schema = RelationSchema::new(
        "raw".to_string(),
        vec![SchemaColumn::from_sql_type("k", "INTEGER", "duckdb").unwrap()],
    )
    .unwrap();
    let catalog = RelationCatalog::from_schemas(&[schema]).unwrap();
    let dialect = dialect_from_name("duckdb").unwrap();
    let input = SqlInput::inline(SQL);
    let configured = [ConfiguredSqlInput::new("cte", &input, "duckdb", dialect.as_ref())];
    let bundle = analyze_configured_inputs_with_catalog(&configured, &catalog).unwrap();
    let rows = values
        .iter()
        .map(|value| {
            PhysicalWriteRow::new(BTreeMap::from([(
                "k".to_string(),
                value.map_or(LiteralValue::Null, |value| LiteralValue::Number(value.to_string())),
            )]))
            .unwrap()
        })
        .collect();
    let states = [PhysicalWriteState::new("raw".to_string(), rows).unwrap()];
    let target = bundle.layers().last().unwrap();
    let PhysicalCteProof::Exact(proof) =
        physical_finite_cte_bag_plan(&bundle, target.id(), &states, None)
    else {
        panic!("TASK-143 count has no canonical exact physical proof");
    };
    assert_eq!(proof.sources, vec!["raw"]);
    assert_eq!(proof.source_bags["raw"].len(), values.len());
    assert_eq!(proof.stage_bags["terminal/cte:0:stage"].len(), expected_stage);
    assert_eq!(proof.terminal.len(), 1);
    assert_eq!(proof.terminal[0].values["total"], LiteralValue::Number(expected_count.to_string()));
    assert!(!proof.terminal_domains["total"].null_allowed);
    let missing = BTreeMap::from([(
        "total".to_string(),
        LiteralValue::Number((expected_count + 1).to_string()),
    )]);
    assert!(matches!(
        physical_finite_cte_bag_plan(&bundle, target.id(), &states, Some(&missing)),
        PhysicalCteProof::Impossible { .. }
    ));
}

#[test]
fn count_preserves_one_shared_filtered_stage() {
    assert_count_bags(&[Some(1), Some(2), Some(3)], 2, 2);
}

#[test]
fn count_preserves_duplicate_join_multiplicity_and_null_rejection() {
    assert_count_bags(&[Some(1), Some(2), Some(2), None], 2, 4);
}

#[test]
fn count_zero_over_empty_filtered_producer_is_one_row() {
    assert_count_bags(&[Some(1), None], 0, 0);
}
