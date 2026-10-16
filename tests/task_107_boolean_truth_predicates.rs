//! TASK-107: ISO three-valued computed Boolean truth tests.
//! This exercises the analyzer public seam and an independent DuckDB oracle.

use duckdb::Connection;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, BooleanWitnessDirection,
    ComposedSemantics,
    ConfiguredSqlInput, RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
};

#[test]
fn computed_boolean_truth_tests_keep_unknown_rejected_with_typed_domains() {
    let relation = RelationSchema::new(
        "t",
        vec![SchemaColumn::from_sql_type("a", "INTEGER", "postgresql").unwrap()],
    )
    .unwrap();
    let catalog = RelationCatalog::from_schemas(&[relation]).unwrap();
    let dialect = dialect_from_name("postgresql").unwrap();
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE t (id INTEGER, a INTEGER);
         INSERT INTO t VALUES (1, NULL), (2, 0), (3, 1), (4, 2)",
    )
    .unwrap();

    for (predicate, expected_true, expected_not_true) in [
        ("(a = 1) IS TRUE", vec![3], vec![1, 2, 4]),
        ("(a = 1) IS FALSE", vec![2, 4], vec![1, 3]),
        ("(a = NULL) IS UNKNOWN", vec![1, 2, 3, 4], vec![]),
    ] {
        let sql = format!("SELECT a FROM t WHERE {predicate}");
        let source = SqlInput::inline(&sql);
        let bundle = analyze_configured_inputs_with_catalog(
            &[ConfiguredSqlInput::new(
                "task107",
                &source,
                "postgresql",
                dialect.as_ref(),
            )],
            &catalog,
        )
        .unwrap();
        let ComposedSemantics::Resolved(semantics) = bundle.layers()[0].composed_semantics() else {
            panic!("{predicate}: typed WHERE semantic resolution must succeed");
        };
        let exact_truth_directions = semantics.boolean_witnesses().iter().any(|item| {
            let witness = item.witness();
            matches!(
                witness.qualifying(),
                BooleanWitnessDirection::Exact(_) | BooleanWitnessDirection::Impossible
            ) && matches!(
                witness.rejected(),
                BooleanWitnessDirection::Exact(_) | BooleanWitnessDirection::Impossible
            )
        });
        assert!(
            semantics.condition_exactness().is_exact() || exact_truth_directions,
            "{predicate}: cannot certify computed truth test without exact domain or Boolean witness"
        );
        let sql_true = format!("SELECT id FROM t WHERE {predicate} ORDER BY id");
        let sql_rejected = format!("SELECT id FROM t WHERE ({predicate}) IS NOT TRUE ORDER BY id");
        let sql_ids = |sql: &str| {
            let mut stmt = db.prepare(sql).unwrap();
            stmt.query_map([], |row| row.get::<_, i64>(0))
                .unwrap()
                .map(|row| row.unwrap())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            sql_ids(&sql_true),
            expected_true,
            "{predicate}: positive DuckDB bag"
        );
        assert_eq!(
            sql_ids(&sql_rejected),
            expected_not_true,
            "{predicate}: NOT TRUE includes NULL expression rows"
        );
    }
}
