//! TASK-165: standalone terminal row and complete typed histogram goals.
//! Fixed public API seams: analyze -> set goals -> witness/protocol -> DuckDB.

use duckdb::Connection;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, to_bundle_json, AnalysisBundle,
    ConfiguredSqlInput, ConstraintValue, OutcomeGoal, OutcomeGoalStatus, OutcomeWitness,
    OutputDistribution, OutputValueCount, RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
};

const DIALECTS: &[&str] = &[
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

fn bundle(sql: &str, dialect: &str) -> AnalysisBundle {
    let schema = RelationSchema::new(
        "t",
        vec![SchemaColumn::from_sql_type("k", "INTEGER", "postgresql").expect("catalog integer")],
    )
    .expect("relation");
    let catalog = RelationCatalog::from_schemas(&[schema]).expect("catalog");
    let source = SqlInput::inline(sql);
    let dialect_impl = dialect_from_name(dialect).expect("supported dialect");
    let input = ConfiguredSqlInput::new("task-165", &source, dialect, dialect_impl.as_ref());
    analyze_configured_inputs_with_catalog(&[input], &catalog).expect("analyze")
}

fn histogram(values: &[(ConstraintValue, u64)]) -> OutputDistribution {
    OutputDistribution::new(
        "k",
        values
            .iter()
            .map(|(value, count)| OutputValueCount::new(value.clone(), *count))
            .collect(),
    )
    .expect("valid histogram")
}

fn request(
    bundle: &mut AnalysisBundle,
    rows: u64,
    groups: Option<u64>,
    distributions: Vec<OutputDistribution>,
) {
    let layer = bundle.layers()[0].id().to_string();
    bundle
        .set_outcome_goals(&[
            OutcomeGoal::new(layer, Some(rows), groups, distributions).expect("valid goal")
        ])
        .expect("attach goal");
}

#[test]
fn complete_typed_histogram_proves_rows_nulls_and_duplicates_across_dialects() {
    let distribution = histogram(&[(ConstraintValue::Integer(1), 2), (ConstraintValue::Null, 1)]);
    let statistics = distribution
        .statistics()
        .expect("bounded complete histogram");
    assert_eq!(statistics.rows(), 3);
    assert_eq!(statistics.distinct_non_null(), 1);
    assert_eq!(statistics.null_rows(), 1);
    assert_eq!(statistics.duplicate_rows(), 1);
    // NULL share is exactly 1/3, never floating point or rounded.
    assert_eq!((statistics.null_rows(), statistics.rows()), (1, 3));

    for dialect in DIALECTS {
        let mut b = bundle("SELECT k FROM t", dialect);
        request(&mut b, 3, None, vec![distribution.clone()]);
        assert_eq!(
            b.outcome_goals()[0].status(),
            OutcomeGoalStatus::Feasible,
            "{dialect}"
        );
        let Some(OutcomeWitness::SourceRows {
            relation,
            rows,
            columns,
        }) = b.outcome_goals()[0].witness()
        else {
            panic!("{dialect}: expected complete source-row witness");
        };
        assert_eq!(relation, "t");
        assert_eq!(*rows, 3);
        assert_eq!(columns.len(), 1);
        assert_eq!(columns[0].column(), "k");
        assert_eq!(columns[0].values(), distribution.values());

        let emitted: serde_json::Value =
            serde_json::from_str(&to_bundle_json(&b)).expect("canonical protocol");
        assert_eq!(
            emitted["outcome_goals"][0]["assessment"]["status"],
            "feasible"
        );
        assert_eq!(
            emitted["outcome_goals"][0]["witness"]["kind"],
            "source_rows"
        );
    }
}

#[test]
fn mismatched_and_duplicate_distinct_goals_are_impossible() {
    for dialect in DIALECTS {
        let mut b = bundle("SELECT k FROM t", dialect);
        request(
            &mut b,
            2,
            None,
            vec![histogram(&[
                (ConstraintValue::Integer(1), 2),
                (ConstraintValue::Null, 1),
            ])],
        );
        assert_eq!(
            b.outcome_goals()[0].status(),
            OutcomeGoalStatus::Unsatisfiable,
            "{dialect}: a complete three-row histogram cannot describe two rows"
        );

        let mut distinct = bundle("SELECT DISTINCT k FROM t", dialect);
        request(
            &mut distinct,
            2,
            None,
            vec![histogram(&[(ConstraintValue::Integer(1), 2)])],
        );
        assert_eq!(
            distinct.outcome_goals()[0].status(),
            OutcomeGoalStatus::Unsatisfiable,
            "{dialect}: DISTINCT cannot emit two copies of one non-NULL scalar"
        );
    }
}

#[test]
fn grouped_row_goal_is_bound_to_surviving_groups_not_input_occurrences() {
    for dialect in DIALECTS {
        let mut b = bundle("SELECT k, COUNT(*) AS n FROM t GROUP BY k", dialect);
        request(&mut b, 2, Some(3), vec![]);
        assert_eq!(
            b.outcome_goals()[0].status(),
            OutcomeGoalStatus::Unsatisfiable,
            "{dialect}: two output rows cannot contain three ordinary groups"
        );
    }
}

#[test]
fn duckdb_replays_the_complete_positive_and_rejected_bags() {
    let db = Connection::open_in_memory().expect("DuckDB");
    db.execute_batch("CREATE TABLE t(k INTEGER); INSERT INTO t VALUES (1), (1), (NULL);")
        .expect("physical source");
    let mut query = db
        .prepare("SELECT k FROM t ORDER BY k NULLS LAST")
        .expect("terminal SQL");
    let bag: Vec<Option<i64>> = query
        .query_map([], |row| row.get(0))
        .expect("terminal rows")
        .map(|row| row.expect("value"))
        .collect();
    assert_eq!(bag, vec![Some(1), Some(1), None]);

    let (rows, distinct, nulls): (i64, i64, i64) = db
        .query_row(
            "SELECT COUNT(*), COUNT(DISTINCT k), COUNT(*) FILTER (WHERE k IS NULL) FROM t",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("independent aggregate oracle");
    assert_eq!((rows, distinct, nulls), (3, 1, 1));
    let mut distinct_stmt = db
        .prepare("SELECT DISTINCT k FROM t ORDER BY k NULLS LAST")
        .expect("distinct SQL");
    let distinct_bag: Vec<Option<i64>> = distinct_stmt
        .query_map([], |row| row.get(0))
        .expect("distinct rows")
        .map(|row| row.expect("distinct value"))
        .collect();
    assert_eq!(distinct_bag, vec![Some(1), None]);
    // Three occurrences, two SQL DISTINCT equivalence classes: one duplicate.
    assert_eq!(rows - distinct - i64::from(nulls > 0), 1);

    let mut group = db
        .prepare("SELECT k, COUNT(*) AS n FROM t GROUP BY k ORDER BY k NULLS LAST")
        .expect("group SQL");
    let groups: Vec<(Option<i64>, i64)> = group
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .expect("group rows")
        .map(|row| row.expect("group"))
        .collect();
    assert_eq!(groups, vec![(Some(1), 2), (None, 1)]);
}

#[test]
fn zero_and_overflow_histograms_never_invent_undefined_fractions() {
    let empty = histogram(&[(ConstraintValue::Null, 0)]);
    let zero = empty.statistics().expect("empty histogram");
    assert_eq!(zero.rows(), 0);
    assert_eq!(zero.null_rows(), 0);
    assert_eq!(zero.duplicate_rows(), 0);

    let overflow = histogram(&[
        (ConstraintValue::Integer(1), u64::MAX),
        (ConstraintValue::Null, 1),
    ]);
    assert!(overflow.statistics().is_none());
}
