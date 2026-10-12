//! lineage regression tests.

use super::*;

#[test]
fn typed_materialization_preserves_source_truth_only_with_matching_schema() {
    for &dialect in DIALECTS {
        let b = bundle(
            &[
                "CREATE TABLE stage AS SELECT a, b FROM t",
                "SELECT a FROM stage WHERE a > 2",
                "SELECT a FROM t",
            ],
            dialect,
        );
        let plan = physical_joint_source_plan(
            &b,
            &[
                (b.layers()[0].id(), 2),
                (b.layers()[1].id(), 2),
                (b.layers()[2].id(), 2),
            ],
        );
        assert!(
            matches!(plan.outcome(), WitnessDirection::Feasible(_)),
            "{dialect}: typed source copy should preserve filter truth: {plan:?}"
        );
    }

    // A physically narrowed stage type could truncate/coerce values during
    // materialization; the producer-to-physical column proof must not guess.
    let source = RelationSchema::new(
        "t",
        vec![
            SchemaColumn::from_sql_type("a", "INTEGER", "postgresql").expect("source a"),
            SchemaColumn::from_sql_type("b", "INTEGER", "postgresql").expect("source b"),
        ],
    )
    .expect("source schema");
    let narrowed = RelationSchema::new(
        "stage",
        vec![
            SchemaColumn::from_sql_type("a", "SMALLINT", "postgresql").expect("stage a"),
            SchemaColumn::from_sql_type("b", "INTEGER", "postgresql").expect("stage b"),
        ],
    )
    .expect("stage schema");
    let b = bundle_with_schemas(
        &[
            "CREATE TABLE stage AS SELECT a, b FROM t",
            "SELECT a FROM stage WHERE a > 2",
        ],
        "postgresql",
        &[source, narrowed],
    );
    let proof = physical_joint_source_plan(&b, &[(b.layers()[1].id(), 2)]);
    assert!(matches!(proof.outcome(), WitnessDirection::Residual { .. }));
}

#[test]
fn typed_multistage_renamed_projections_resolve_each_physical_column_edge() {
    let schema = |relation: &str, columns: &[&str], narrowed: bool| {
        RelationSchema::new(
            relation,
            columns
                .iter()
                .map(|name| {
                    let sql_type = if narrowed && *name == "a" {
                        "SMALLINT"
                    } else {
                        "INTEGER"
                    };
                    SchemaColumn::from_sql_type(*name, sql_type, "postgresql")
                        .expect("typed column")
                })
                .collect(),
        )
        .expect("relation schema")
    };
    let sql = [
        "CREATE TABLE stage AS SELECT a, b FROM t",
        "CREATE TABLE mart AS SELECT a AS x, b FROM stage",
        "SELECT x FROM mart WHERE x > 2",
        "SELECT a FROM t",
    ];

    for &dialect in DIALECTS {
        let schemas = [
            schema("t", &["a", "b"], false),
            schema("stage", &["a", "b"], false),
            schema("mart", &["x", "b"], false),
        ];
        let b = bundle_with_schemas(&sql, dialect, &schemas);
        let witness =
            physical_joint_source_plan(&b, &[(b.layers()[2].id(), 3), (b.layers()[3].id(), 3)]);
        assert!(
            matches!(witness.outcome(), WitnessDirection::Feasible(_)),
            "{dialect}: every producer copy has a certified type and identity: {witness:?}"
        );
        assert_eq!(witness.sources(), &["t".to_string()]);
        assert_eq!(witness.nodes().len(), 5);
    }

    let schemas = [
        schema("t", &["a", "b"], false),
        schema("stage", &["a", "b"], true),
        schema("mart", &["x", "b"], false),
    ];
    let b = bundle_with_schemas(&sql, "postgresql", &schemas);
    let result = physical_joint_source_plan(&b, &[(b.layers()[2].id(), 3)]);
    assert!(
        matches!(result.outcome(), WitnessDirection::Residual { .. }),
        "an intermediate width mismatch must not be silently inverted"
    );

    let conn = Connection::open_in_memory().expect("duckdb");
    conn.execute_batch(
        "CREATE TABLE t(a INTEGER, b INTEGER);
         INSERT INTO t VALUES (3,1), (4,1), (5,2);
         CREATE TABLE stage AS SELECT a, b FROM t;
         CREATE TABLE mart AS SELECT a AS x, b FROM stage;",
    )
    .expect("two copied producer tables");
    for sql in [
        "SELECT COUNT(*) FROM mart WHERE x > 2",
        "SELECT COUNT(*) FROM t",
    ] {
        let count: i64 = conn.query_row(sql, [], |row| row.get(0)).expect("oracle");
        assert_eq!(count, 3);
    }
}

#[test]
fn mixed_truth_goals_reject_incompatible_row_membership_without_overclaiming() {
    for &dialect in DIALECTS {
        let b = bundle(
            &[
                "SELECT a FROM t WHERE a IS NULL",
                "SELECT a FROM t WHERE a IS NULL",
                "SELECT a FROM t",
            ],
            dialect,
        );
        assert!(
            matches!(
                physical_joint_row_count_plan(
                    &b,
                    &[(b.layers()[0].id(), 2), (b.layers()[1].id(), 0)],
                ),
                WitnessDirection::Impossible
            ),
            "{dialect}: one positive candidate cannot be universally rejected"
        );

        let disjoint_positive = bundle(
            &[
                "SELECT a FROM t WHERE a < 0",
                "SELECT a FROM t WHERE a > 10",
                "SELECT a FROM t WHERE b IS NULL",
            ],
            dialect,
        );
        assert!(
            matches!(
                physical_joint_row_count_plan(
                    &disjoint_positive,
                    &[
                        (disjoint_positive.layers()[0].id(), 2),
                        (disjoint_positive.layers()[1].id(), 2),
                        (disjoint_positive.layers()[2].id(), 0),
                    ],
                ),
                WitnessDirection::Residual { .. }
            ),
            "{dialect}: two disjoint positive groups could use extra distinct rows"
        );
    }
}

#[test]
fn joint_terminal_zero_from_filter_is_proved_by_nonempty_rejected_source_rows() {
    let b = bundle(
        &["SELECT a FROM t WHERE a > 1", "SELECT a FROM t"],
        "postgresql",
    );
    let witness =
        physical_joint_row_count_plan(&b, &[(b.layers()[0].id(), 0), (b.layers()[1].id(), 2)]);
    let WitnessDirection::Feasible(cases) = witness else {
        panic!("two fully rejected physical rows prove filtered output zero: {witness:?}");
    };
    assert!(cases[0].obligations().iter().any(|obligation| matches!(
        obligation,
        WitnessObligation::Rows {
            predicate: WitnessFormula::RowTruth {
                truth: sql_semantic_protocol::BooleanTruthCase::NotTrue,
                ..
            },
            bounds,
            ..
        } if bounds.minimum() == 2 && bounds.maximum() == Some(2)
    )));
}

#[test]
fn simultaneous_empty_filtered_and_joined_terminals_reuse_sources_once() {
    for &dialect in DIALECTS {
        let b = bundle(
            &[
                "CREATE TABLE filtered AS SELECT a FROM t WHERE a > 10",
                "CREATE TABLE joined AS SELECT l.a FROM l JOIN r ON l.k = r.k",
                "SELECT a FROM filtered WHERE a < 0",
                "SELECT a FROM joined WHERE a > 0",
            ],
            dialect,
        );
        let targets = [(b.layers()[2].id(), 0), (b.layers()[3].id(), 0)];
        let WitnessDirection::Feasible(cases) = physical_joint_row_count_plan(&b, &targets) else {
            panic!("{dialect}: all-empty physical leaves jointly prove both zero outputs");
        };
        assert_eq!(cases.len(), 1);
        let obligations = cases[0].obligations();
        assert_eq!(
            obligations
                .iter()
                .filter(|o| matches!(o, WitnessObligation::ClosedWorld { .. }))
                .count(),
            3
        );
        assert_eq!(
            obligations
                .iter()
                .filter(|o| matches!(o, WitnessObligation::OutputRows { .. }))
                .count(),
            2
        );
    }
    let conn = Connection::open_in_memory().expect("duckdb");
    conn.execute_batch(
        "CREATE TABLE t(a INTEGER); CREATE TABLE l(a INTEGER, k INTEGER);
         CREATE TABLE r(a INTEGER, k INTEGER);
         CREATE TABLE filtered AS SELECT a FROM t WHERE a > 10;
         CREATE TABLE joined AS SELECT l.a FROM l JOIN r ON l.k = r.k;",
    )
    .expect("empty source materialization");
    for sql in [
        "SELECT COUNT(*) FROM filtered WHERE a < 0",
        "SELECT COUNT(*) FROM joined WHERE a > 0",
    ] {
        let count: i64 = conn
            .query_row(sql, [], |row| row.get(0))
            .expect("terminal row count");
        assert_eq!(count, 0);
    }
}

#[test]
fn compatible_filtered_zero_and_positive_transparent_path_is_constructive() {
    let b = bundle(
        &[
            "CREATE TABLE filtered AS SELECT a FROM t WHERE a > 10",
            "CREATE TABLE other AS SELECT a FROM t",
        ],
        "postgresql",
    );
    let actual =
        physical_joint_row_count_plan(&b, &[(b.layers()[0].id(), 0), (b.layers()[1].id(), 2)]);
    assert!(
        matches!(actual, WitnessDirection::Feasible(_)),
        "two rows at t can both fail a > 10 while satisfying other: {actual:?}"
    );
    let conn = Connection::open_in_memory().expect("duckdb");
    conn.execute_batch(
        "CREATE TABLE t(a INTEGER);
         INSERT INTO t VALUES (NULL), (2);
         CREATE TABLE filtered AS SELECT a FROM t WHERE a > 10;
         CREATE TABLE other AS SELECT a FROM t;",
    )
    .expect("nullable filtered source");
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM filtered", [], |row| row.get(0))
        .expect("filtered");
    let other: i64 = conn
        .query_row("SELECT COUNT(*) FROM other", [], |row| row.get(0))
        .expect("other");
    assert_eq!((count, other), (0, 2));
}

#[test]
fn shared_transparent_zero_and_positive_targets_are_impossible() {
    let b = bundle(
        &[
            "CREATE TABLE first AS SELECT a FROM t",
            "CREATE TABLE second AS SELECT a FROM t",
        ],
        "postgresql",
    );
    assert!(matches!(
        physical_joint_row_count_plan(&b, &[(b.layers()[0].id(), 0), (b.layers()[1].id(), 1)]),
        WitnessDirection::Impossible
    ));
}

#[test]
fn zero_rows_prove_complete_empty_histograms_and_group_count() {
    let mut b = bundle(
        &[
            "CREATE TABLE stage AS SELECT a FROM t WHERE a IS NOT NULL",
            "SELECT a, COUNT(*) AS n FROM stage GROUP BY a HAVING COUNT(*) > 0",
        ],
        "postgresql",
    );
    let target = b.layers()[1].id().to_string();
    let histogram = OutputDistribution::new(
        "a",
        vec![OutputValueCount::new(
            sql_semantic_protocol::ConstraintValue::Integer(5),
            0,
        )],
    )
    .expect("empty frequency");
    b.set_outcome_goals(
        &[OutcomeGoal::new(&target, Some(0), Some(0), vec![histogram])
            .expect("complete empty output")],
    )
    .expect("goal");
    assert_eq!(b.outcome_goals()[0].status(), OutcomeGoalStatus::Feasible);
    let json: serde_json::Value =
        serde_json::from_str(&sql_semantic_protocol::to_bundle_json(&b)).expect("JSON");
    assert_eq!(json["outcome_goals"][0]["witness"]["kind"], "empty_sources");
    assert_eq!(
        json["outcome_goals"][0]["witness"]["relations"],
        serde_json::json!(["t"])
    );

    let mut groups_only = bundle(&["SELECT a, COUNT(*) AS n FROM t GROUP BY a"], "postgresql");
    let id = groups_only.layers()[0].id().to_string();
    groups_only
        .set_outcome_goals(&[
            OutcomeGoal::new(&id, None, Some(0), vec![]).expect("zero surviving groups")
        ])
        .expect("group goal");
    assert_eq!(
        groups_only.outcome_goals()[0].status(),
        OutcomeGoalStatus::Feasible
    );

    let conn = Connection::open_in_memory().expect("duckdb");
    conn.execute_batch("CREATE TABLE t(a INTEGER)")
        .expect("empty source");
    let rows: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM (SELECT a, COUNT(*) FROM t GROUP BY a)",
            [],
            |row| row.get(0),
        )
        .expect("grouped query");
    assert_eq!(rows, 0);
}
