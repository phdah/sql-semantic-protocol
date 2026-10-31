//! TASK-143: physical source and shared CTE producer truth oracles.
//! These use the public API and fixed source snapshots, not evaluator internals.

use std::collections::BTreeMap;

use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_finite_cte_bag_plan,
    ConfiguredSqlInput, LiteralValue, PhysicalCteProof, PhysicalWriteRow, PhysicalWriteState,
    RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
};

const SQL: &str = "WITH stage AS (SELECT k, k + 10 AS shifted FROM raw WHERE k > 1) \
SELECT a.shifted AS l, b.shifted AS r FROM stage a \
JOIN stage b ON a.k = b.k ORDER BY l, r";

fn number(value: i64) -> LiteralValue {
    LiteralValue::Number(value.to_string())
}

fn analyzed(
    values: &[Option<i64>],
) -> (
    sql_semantic_protocol::AnalysisBundle,
    Vec<PhysicalWriteState>,
) {
    let schema = RelationSchema::new(
        "raw".to_string(),
        vec![SchemaColumn::from_sql_type("k", "INTEGER", "duckdb").unwrap()],
    )
    .unwrap();
    let catalog = RelationCatalog::from_schemas(&[schema]).unwrap();
    let dialect = dialect_from_name("duckdb").unwrap();
    let source = SqlInput::inline(SQL);
    let configured = [ConfiguredSqlInput::new(
        "cte",
        &source,
        "duckdb",
        dialect.as_ref(),
    )];
    let bundle = analyze_configured_inputs_with_catalog(&configured, &catalog).unwrap();
    let rows = values
        .iter()
        .map(|value| {
            PhysicalWriteRow::new(BTreeMap::from([(
                "k".to_string(),
                value.map_or(LiteralValue::Null, number),
            )]))
            .unwrap()
        })
        .collect();
    (
        bundle,
        vec![PhysicalWriteState::new("raw".to_string(), rows).unwrap()],
    )
}

#[test]
fn filtering_and_computed_aliases_share_one_physical_producer_bag() {
    let (bundle, snapshots) = analyzed(&[Some(1), Some(2), Some(3)]);
    let target = bundle.layers().last().unwrap();
    let PhysicalCteProof::Exact(exact) =
        physical_finite_cte_bag_plan(&bundle, target.id(), &snapshots, None)
    else {
        panic!("78-C143-P: no complete physical proof");
    };

    assert_eq!(exact.sources, vec!["raw"]);
    assert_eq!(exact.source_bags["raw"].len(), 3);
    let stage = &exact.stage_bags["terminal/cte:0:stage"];
    assert_eq!(stage.len(), 2);
    assert_eq!(stage[0].values["shifted"], number(12));
    assert_eq!(stage[1].values["shifted"], number(13));
    assert_eq!(stage[0].source_row_ids, vec!["raw#1"]);
    assert_eq!(stage[1].source_row_ids, vec!["raw#2"]);
    assert_eq!(
        exact.false_pairs["terminal/cte:0:stage"],
        vec![vec!["raw#0"]]
    );
    assert!(exact.unknown_pairs["terminal/cte:0:stage"].is_empty());
    let rows = exact
        .terminal
        .iter()
        .map(|row| {
            (
                row.values["l"].clone(),
                row.values["r"].clone(),
                row.source_row_ids.clone(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        rows,
        vec![
            (
                number(12),
                number(12),
                vec!["raw#1".to_string(), "raw#1".to_string()]
            ),
            (
                number(13),
                number(13),
                vec!["raw#2".to_string(), "raw#2".to_string()]
            ),
        ],
    );
    assert!(!exact.terminal_domains["l"].null_allowed);
    assert!(!exact.terminal_domains["r"].null_allowed);
}

#[test]
fn rejected_nulls_and_duplicate_source_occurrences_do_not_create_free_producer_rows() {
    let (bundle, snapshots) = analyzed(&[Some(1), Some(2), Some(2), None]);
    let target = bundle.layers().last().unwrap();
    let PhysicalCteProof::Exact(exact) =
        physical_finite_cte_bag_plan(&bundle, target.id(), &snapshots, None)
    else {
        panic!("78-C143-N: no complete physical proof");
    };

    assert_eq!(exact.stage_bags["terminal/cte:0:stage"].len(), 2);
    assert_eq!(exact.terminal.len(), 4);
    assert_eq!(
        exact.false_pairs["terminal/cte:0:stage"],
        vec![vec!["raw#0"]]
    );
    assert_eq!(
        exact.unknown_pairs["terminal/cte:0:stage"],
        vec![vec!["raw#3"]]
    );
    for row in &exact.terminal {
        assert_eq!(row.values["l"], number(12));
        assert_eq!(row.values["r"], number(12));
        assert_eq!(row.source_row_ids.len(), 2);
        assert!(row
            .source_row_ids
            .iter()
            .all(|id| id == "raw#1" || id == "raw#2"));
    }
    let absent = BTreeMap::from([("l".to_string(), number(11)), ("r".to_string(), number(11))]);
    assert!(matches!(
        physical_finite_cte_bag_plan(&bundle, target.id(), &snapshots, Some(&absent)),
        PhysicalCteProof::Impossible { .. }
    ));
}
