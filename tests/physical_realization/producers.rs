//! producers regression tests.

use super::*;

#[test]
fn equijoins_over_two_materialized_producer_branches_have_physical_pair_witnesses() {
    for &dialect in DIALECTS {
        let mut b = bundle(
            &[
                "CREATE TABLE stage AS SELECT a, k FROM l",
                "CREATE TABLE mart AS SELECT a, k FROM r",
                "SELECT stage.a FROM stage JOIN mart ON stage.k = mart.k",
            ],
            dialect,
        );
        let id = b.layers()[2].id().to_string();
        b.set_outcome_goals(&[OutcomeGoal::new(&id, Some(3), None, vec![]).expect("goal")])
            .expect("attach");
        let Some(OutcomeWitness::JoinPairs { left, right, pairs }) = b.outcome_goals()[0].witness()
        else {
            panic!(
                "{dialect}: independent materialized join: {:?}",
                b.outcome_goals()
            );
        };
        assert_eq!(*pairs, 3);
        assert_eq!(left.relation(), "l");
        assert_eq!(right.relation(), "r");
        assert_eq!((left.column(), right.column()), ("k", "k"));
    }

    let schema = |relation: &str, columns: &[&str]| {
        RelationSchema::new(
            relation,
            columns
                .iter()
                .map(|column| {
                    SchemaColumn::from_sql_type(*column, "INTEGER", "postgresql")
                        .expect("typed column")
                })
                .collect(),
        )
        .expect("schema")
    };
    let schemas = [
        schema("l", &["a", "k"]),
        schema("r", &["a", "k"]),
        schema("stage", &["a", "k"]),
        schema("mart", &["a", "k"]),
        schema("joined", &["a"]),
    ];
    let mut b = bundle_with_schemas(
        &[
            "CREATE TABLE stage AS SELECT a, k FROM l",
            "CREATE TABLE mart AS SELECT a, k FROM r",
            "CREATE TABLE joined AS SELECT stage.a AS a FROM stage JOIN mart ON stage.k = mart.k",
            "SELECT a FROM joined",
        ],
        "postgresql",
        &schemas,
    );
    let downstream = b.layers()[3].id().to_string();
    b.set_outcome_goals(&[OutcomeGoal::new(&downstream, Some(3), None, vec![]).expect("goal")])
        .expect("attach");
    assert!(
        matches!(
            b.outcome_goals()[0].witness(),
            Some(OutcomeWitness::JoinPairs { pairs: 3, .. })
        ),
        "joined materialization must not discard complete physical key proof: {:?}",
        b.outcome_goals()
    );

    let conn = Connection::open_in_memory().expect("duckdb");
    conn.execute_batch(
        "CREATE TABLE l(a INTEGER, k INTEGER);
         CREATE TABLE r(a INTEGER, k INTEGER);
         INSERT INTO l VALUES (10,0),(11,1),(12,2);
         INSERT INTO r VALUES (20,0),(21,1),(22,2);
         CREATE TABLE stage AS SELECT a,k FROM l;
         CREATE TABLE mart AS SELECT a,k FROM r;",
    )
    .expect("independent physical rows");
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM (SELECT stage.a FROM stage JOIN mart ON stage.k = mart.k)",
            [],
            |row| row.get(0),
        )
        .expect("join cardinality");
    assert_eq!(count, 3);
}

#[test]
fn materialized_join_rejects_filtered_upstream_but_proves_shared_key_population() {
    let scenarios = [
        (
            [
                "CREATE TABLE stage AS SELECT a, k FROM l WHERE a > 2 OR k IS NULL",
                "CREATE TABLE mart AS SELECT a, k FROM r",
                "SELECT stage.a FROM stage JOIN mart ON stage.k = mart.k",
            ],
            OutcomeGoalStatus::Residual,
        ),
        (
            [
                "CREATE TABLE stage AS SELECT a, k FROM l",
                "CREATE TABLE mart AS SELECT a, k FROM l",
                "SELECT stage.a FROM stage JOIN mart ON stage.k = mart.k",
            ],
            OutcomeGoalStatus::Feasible,
        ),
    ];
    for (queries, expected) in scenarios {
        let mut b = bundle(&queries, "postgresql");
        let id = b.layers()[2].id().to_string();
        b.set_outcome_goals(&[OutcomeGoal::new(&id, Some(3), None, vec![]).expect("goal")])
            .expect("attach");
        assert_eq!(b.outcome_goals()[0].status(), expected);
    }
}

#[test]
fn direct_join_group_and_set_counts_survive_materialized_copy_chains() {
    for &dialect in DIALECTS {
        for (producer, expected) in [
            (
                "CREATE TABLE stage AS SELECT l.a AS a FROM l INNER JOIN r ON l.k = r.k",
                "join_pairs",
            ),
            (
                "CREATE TABLE stage AS SELECT k, COUNT(*) AS n FROM t GROUP BY k HAVING COUNT(*) >= 2",
                "groups",
            ),
            (
                "CREATE TABLE stage AS SELECT a FROM l UNION ALL SELECT a FROM r",
                "set_tuples",
            ),
        ] {
            let mut b = bundle(
                &[producer, "CREATE TABLE mart AS SELECT a FROM stage", "SELECT a FROM mart"],
                dialect,
            );
            // A group produces k/n rather than a, so use its actual
            // preserved grouping key in the downstream projections.
            if expected == "groups" {
                b = bundle(
                    &[
                        producer,
                        "CREATE TABLE mart AS SELECT k FROM stage",
                        "SELECT k FROM mart",
                    ],
                    dialect,
                );
            }
            let id = b.layers()[2].id().to_string();
            b.set_outcome_goals(&[
                OutcomeGoal::new(&id, Some(2), None, vec![]).expect("goal")
            ])
            .expect("attach");
            assert_eq!(
                b.outcome_goals()[0].status(),
                OutcomeGoalStatus::Feasible,
                "{dialect}: {producer}: {:?}",
                b.outcome_goals()
            );
            let kind = match b.outcome_goals()[0].witness() {
                Some(OutcomeWitness::JoinPairs { .. }) => "join_pairs",
                Some(OutcomeWitness::Groups { .. }) => "groups",
                Some(OutcomeWitness::SetTuples { .. }) => "set_tuples",
                other => panic!("{dialect}: unexpected witness {other:?}"),
            };
            assert_eq!(kind, expected);
        }
    }
    let conn = Connection::open_in_memory().expect("duckdb");
    conn.execute_batch(
        "CREATE TABLE l(a INTEGER, k INTEGER);
         CREATE TABLE r(a INTEGER, k INTEGER);
         INSERT INTO l VALUES (10,1),(20,2);
         INSERT INTO r VALUES (30,1),(40,2);
         CREATE TABLE stage AS SELECT l.a AS a FROM l INNER JOIN r ON l.k = r.k;
         CREATE TABLE mart AS SELECT a FROM stage;",
    )
    .expect("joined stage");
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM (SELECT a FROM mart)", [], |row| {
            row.get(0)
        })
        .expect("joined output");
    assert_eq!(count, 2);
}

#[test]
fn ranked_operator_count_survives_transparent_materialization() {
    let mut b = bundle(
        &[
            "CREATE TABLE stage AS SELECT a, ROW_NUMBER() OVER (PARTITION BY k ORDER BY b ASC NULLS LAST) AS rn FROM t QUALIFY rn = 1",
            "SELECT a FROM stage",
        ],
        "duckdb",
    );
    let id = b.layers()[1].id().to_string();
    b.set_outcome_goals(&[OutcomeGoal::new(&id, Some(2), None, vec![]).expect("goal")])
        .expect("attach");
    assert!(matches!(
        b.outcome_goals()[0].witness(),
        Some(OutcomeWitness::Ranked { rows: 2, .. })
    ));
    let conn = Connection::open_in_memory().expect("duckdb");
    conn.execute_batch(
        "CREATE TABLE t(a INTEGER, b INTEGER, k INTEGER);
         INSERT INTO t VALUES (10,1,1),(11,2,1),(20,1,2);
         CREATE TABLE stage AS SELECT a,
           ROW_NUMBER() OVER (PARTITION BY k ORDER BY b ASC NULLS LAST) AS rn
           FROM t QUALIFY rn = 1;",
    )
    .expect("ranked rows");
    let rows: i64 = conn
        .query_row("SELECT COUNT(*) FROM (SELECT a FROM stage)", [], |row| {
            row.get(0)
        })
        .expect("ranked cardinality");
    assert_eq!(rows, 2);
}

#[test]
fn downstream_filters_do_not_inherit_unqualified_join_constructions() {
    let b = bundle(
        &[
            "CREATE TABLE stage AS SELECT l.a AS a FROM l INNER JOIN r ON l.k = r.k",
            "SELECT a FROM stage WHERE a > 100 OR a IS NULL",
        ],
        "postgresql",
    );
    let mut b = b;
    let id = b.layers()[1].id().to_string();
    b.set_outcome_goals(&[OutcomeGoal::new(&id, Some(2), None, vec![]).expect("goal")])
        .expect("attach");
    assert_eq!(b.outcome_goals()[0].status(), OutcomeGoalStatus::Residual);
}

#[test]
fn transitive_value_histograms_preserve_renamed_source_columns() {
    for &dialect in DIALECTS {
        let schema = |relation: &str, columns: &[&str]| {
            RelationSchema::new(
                relation,
                columns
                    .iter()
                    .map(|column| {
                        SchemaColumn::from_sql_type(*column, "INTEGER", "postgresql")
                            .expect("typed column")
                    })
                    .collect(),
            )
            .expect("typed schema")
        };
        // Each materialized producer has its *actual* named output schema.
        // Inventing an unrelated catalog would correctly make lineage
        // unresolved, not allow a physical histogram proof.
        let schemas = [
            schema("t", &["a", "b", "k"]),
            schema("stage", &["v", "b"]),
            schema("mart", &["final_a", "b"]),
        ];
        let mut b = bundle_with_schemas(
            &[
                "CREATE TABLE stage AS SELECT a AS v, b FROM t",
                "CREATE TABLE mart AS SELECT v AS final_a, b FROM stage",
                "SELECT final_a AS total FROM mart",
            ],
            dialect,
            &schemas,
        );
        let id = b.layers()[2].id().to_string();
        let histogram = OutputDistribution::new(
            "total",
            vec![
                OutputValueCount::new(ConstraintValue::Integer(4), 2),
                OutputValueCount::new(ConstraintValue::Null, 1),
            ],
        )
        .expect("valid histogram");
        b.set_outcome_goals(
            &[OutcomeGoal::new(&id, Some(3), None, vec![histogram]).expect("goal")],
        )
        .expect("attach");
        assert_eq!(
            b.outcome_goals()[0].status(),
            OutcomeGoalStatus::Feasible,
            "{dialect}: {:?}; row count {:?}; physical {:?}; inputs {:?}",
            b.outcome_goals(),
            physical_row_count_plan(&b, &id, 3),
            physical_source_plan(&b, &id),
            b.inputs()
        );
        let json: serde_json::Value =
            serde_json::from_str(&sql_semantic_protocol::to_bundle_json(&b)).expect("wire");
        assert_eq!(json["outcome_goals"][0]["witness"]["kind"], "source_rows");
        assert_eq!(json["outcome_goals"][0]["witness"]["relation"], "t");
        assert_eq!(
            json["outcome_goals"][0]["witness"]["columns"][0]["column"],
            "a"
        );
    }
    let conn = Connection::open_in_memory().expect("duckdb");
    conn.execute_batch(
        "CREATE TABLE t(a INTEGER, b INTEGER);
         INSERT INTO t VALUES (4, 1), (NULL, 2), (4, 3);
         CREATE TABLE stage AS SELECT a AS v, b FROM t;
         CREATE TABLE mart AS SELECT v AS final_a, b FROM stage;",
    )
    .expect("physical fixture");
    let (total, fours, nulls): (i64, i64, i64) = conn
        .query_row(
            "SELECT COUNT(*), COUNT(*) FILTER (WHERE total = 4),
             COUNT(*) FILTER (WHERE total IS NULL)
             FROM (SELECT final_a AS total FROM mart)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("histogram oracle");
    assert_eq!((total, fours, nulls), (3, 2, 1));
}

#[test]
fn transitive_histograms_fail_closed_on_noninvertible_or_incompatible_values() {
    for &dialect in DIALECTS {
        let queries = [
            "CREATE TABLE stage AS SELECT a + 1 AS v FROM t",
            "SELECT v AS total FROM stage",
        ];
        let mut b = bundle(&queries, dialect);
        let id = b.layers()[1].id().to_string();
        let histogram = OutputDistribution::new(
            "total",
            vec![OutputValueCount::new(ConstraintValue::Integer(4), 3)],
        )
        .expect("histogram");
        b.set_outcome_goals(
            &[OutcomeGoal::new(&id, Some(3), None, vec![histogram]).expect("goal")],
        )
        .expect("attach");
        assert_eq!(
            b.outcome_goals()[0].status(),
            OutcomeGoalStatus::Residual,
            "{dialect}: computed source values cannot be inverted"
        );
    }
    let mut b = bundle(
        &[
            "CREATE TABLE stage AS SELECT a AS v FROM t",
            "SELECT v AS total FROM stage",
        ],
        "postgresql",
    );
    let id = b.layers()[1].id().to_string();
    let invalid_type = OutputDistribution::new(
        "total",
        vec![OutputValueCount::new(
            ConstraintValue::Integer(3_000_000_000),
            1,
        )],
    )
    .expect("histogram");
    b.set_outcome_goals(&[OutcomeGoal::new(&id, Some(1), None, vec![invalid_type]).expect("goal")])
        .expect("attach");
    assert_eq!(b.outcome_goals()[0].status(), OutcomeGoalStatus::Residual);
}

#[test]
fn outcome_goal_adapter_emits_derived_physical_source_count_proofs() {
    let mut b = bundle(
        &[
            "CREATE TABLE stage AS SELECT a, b FROM t",
            "CREATE TABLE mart AS SELECT a FROM stage",
        ],
        "postgresql",
    );
    let terminal_id = b.layers()[1].id().to_string();
    b.set_outcome_goals(&[
        OutcomeGoal::new(&terminal_id, Some(4), None, vec![]).expect("valid terminal row count")
    ])
    .expect("goal attachment");
    assert_eq!(b.outcome_goals()[0].status(), OutcomeGoalStatus::Feasible);
    let json: serde_json::Value =
        serde_json::from_str(&sql_semantic_protocol::to_bundle_json(&b)).expect("outcomes JSON");
    assert_eq!(json["outcome_goals"][0]["assessment"]["status"], "feasible");
    assert_eq!(json["outcome_goals"][0]["witness"]["kind"], "source_rows");
    assert_eq!(json["outcome_goals"][0]["witness"]["relation"], "t");
    assert_eq!(json["outcome_goals"][0]["witness"]["rows"], 4);

    let mut filtered = bundle(&["SELECT a FROM t WHERE a > 2"], "postgresql");
    let filtered_id = filtered.layers()[0].id().to_string();
    filtered
        .set_outcome_goals(&[
            OutcomeGoal::new(&filtered_id, Some(0), None, vec![]).expect("zero count goal")
        ])
        .expect("goal attachment");
    assert_eq!(
        filtered.outcome_goals()[0].status(),
        OutcomeGoalStatus::Feasible,
    );
    let data: serde_json::Value =
        serde_json::from_str(&sql_semantic_protocol::to_bundle_json(&filtered))
            .expect("filtered JSON");
    assert_eq!(data["outcome_goals"][0]["witness"]["kind"], "empty_sources");
    assert_eq!(
        data["outcome_goals"][0]["witness"]["relations"],
        serde_json::json!(["t"])
    );

    // The legacy SourceRows witness cannot express positive RowTruth
    // obligations. A typed physical count must not become unfiltered data.
    let mut positive = bundle(&["SELECT a, b FROM t WHERE a > 2 OR b < 0"], "postgresql");
    let positive_id = positive.layers()[0].id().to_string();
    assert!(matches!(
        physical_row_count_plan(&positive, &positive_id, 3),
        WitnessDirection::Feasible(_)
    ));
    positive
        .set_outcome_goals(&[
            OutcomeGoal::new(&positive_id, Some(3), None, vec![]).expect("positive filter goal")
        ])
        .expect("goal attachment");
    assert_eq!(
        positive.outcome_goals()[0].status(),
        OutcomeGoalStatus::Residual
    );
}
