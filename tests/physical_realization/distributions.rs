//! TASK-96 physical projection inversion and distribution regression tests.

use super::*;

fn schema(relation: &str, columns: &[(&str, &str)]) -> RelationSchema {
    RelationSchema::new(
        relation,
        columns
            .iter()
            .map(|(name, data_type)| {
                SchemaColumn::from_sql_type(*name, data_type, "postgresql")
                    .expect("typed source column")
            })
            .collect(),
    )
    .expect("typed relation")
}

fn histogram(column: &str, values: &[(ConstraintValue, u64)]) -> OutputDistribution {
    OutputDistribution::new(
        column,
        values
            .iter()
            .map(|(value, count)| OutputValueCount::new(value.clone(), *count))
            .collect(),
    )
    .expect("complete histogram")
}

fn goal(
    bundle: &mut AnalysisBundle,
    target: usize,
    rows: u64,
    distributions: Vec<OutputDistribution>,
) {
    let target = bundle.layers()[target].id().to_string();
    bundle
        .set_outcome_goals(&[
            OutcomeGoal::new(target, Some(rows), None, distributions).expect("valid goal")
        ])
        .expect("attach goal");
}

#[test]
fn selective_renamed_and_reordered_histograms_map_to_one_physical_source() {
    for &dialect in DIALECTS {
        let mut bundle = bundle_with_schemas(
            &[
                "CREATE TABLE stage AS SELECT b, a AS x FROM t",
                "SELECT b AS value_b, x AS alias_a, x AS repeat_a FROM stage",
            ],
            dialect,
            &[
                schema("t", &[("a", "INTEGER"), ("b", "INTEGER")]),
                schema("stage", &[("b", "INTEGER"), ("x", "INTEGER")]),
            ],
        );
        let a = vec![(ConstraintValue::Null, 1), (ConstraintValue::Integer(7), 2)];
        goal(
            &mut bundle,
            1,
            3,
            vec![
                histogram("alias_a", &a),
                histogram("repeat_a", &a),
                histogram(
                    "value_b",
                    &[
                        (ConstraintValue::Integer(9), 1),
                        (ConstraintValue::Integer(10), 2),
                    ],
                ),
            ],
        );
        let Some(OutcomeWitness::SourceRows {
            relation,
            rows,
            columns,
        }) = bundle.outcome_goals()[0].witness()
        else {
            panic!(
                "{dialect}: expected one canonical physical-source histogram: {:?}",
                bundle.outcome_goals()
            );
        };
        assert_eq!((relation.as_str(), *rows), ("t", 3));
        assert_eq!(columns.len(), 2, "{dialect}: duplicate alias maps once");
        assert_eq!((columns[0].column(), columns[1].column()), ("a", "b"));
        assert_eq!(
            columns[0].values(),
            &a.iter()
                .map(|(v, n)| OutputValueCount::new(v.clone(), *n))
                .collect::<Vec<_>>()
        );
    }
    let conn = Connection::open_in_memory().expect("duckdb");
    conn.execute_batch(
        "CREATE TABLE t(a INTEGER, b INTEGER);
         INSERT INTO t VALUES (7,9), (NULL,10), (7,10);
         CREATE TABLE stage AS SELECT b, a AS x FROM t;",
    )
    .expect("source rows");
    let counts: (i64, i64, i64, i64) = conn
        .query_row(
            "SELECT COUNT(*), COUNT(*) FILTER (WHERE alias_a=7 AND repeat_a=7),
          COUNT(*) FILTER (WHERE alias_a IS NULL AND repeat_a IS NULL),
          COUNT(*) FILTER (WHERE value_b=10)
          FROM (SELECT b AS value_b, x AS alias_a, x AS repeat_a FROM stage)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("full physical output");
    assert_eq!(counts, (3, 2, 1, 2));
}

#[test]
fn guarded_bijective_integer_projections_invert_complete_null_and_duplicate_bags() {
    for &dialect in DIALECTS {
        let mut bundle = bundle_with_schemas(
            &[
                "CREATE TABLE stage AS SELECT a AS x FROM t",
                "SELECT x + 1 AS inc, 10 - x AS reverse FROM stage",
            ],
            dialect,
            &[
                schema("t", &[("a", "INTEGER")]),
                schema("stage", &[("x", "INTEGER")]),
            ],
        );
        goal(
            &mut bundle,
            1,
            3,
            vec![
                histogram(
                    "inc",
                    &[(ConstraintValue::Null, 1), (ConstraintValue::Integer(4), 2)],
                ),
                histogram(
                    "reverse",
                    &[(ConstraintValue::Null, 1), (ConstraintValue::Integer(7), 2)],
                ),
            ],
        );
        let Some(OutcomeWitness::SourceRows {
            relation,
            rows,
            columns,
        }) = bundle.outcome_goals()[0].witness()
        else {
            panic!(
                "{dialect}: expected inverted physical histogram: {:?}",
                bundle.outcome_goals()
            );
        };
        assert_eq!(relation, "t");
        assert_eq!(*rows, 3);
        assert_eq!(columns.len(), 1);
        assert_eq!(columns[0].column(), "a");
        assert_eq!(
            columns[0].values(),
            &[
                OutputValueCount::new(ConstraintValue::Null, 1),
                OutputValueCount::new(ConstraintValue::Integer(3), 2),
            ],
        );
    }
    let conn = Connection::open_in_memory().expect("duckdb");
    conn.execute_batch(
        "CREATE TABLE t(a INTEGER); INSERT INTO t VALUES (3),(NULL),(3);
         CREATE TABLE stage AS SELECT a AS x FROM t;",
    )
    .expect("source rows");
    let actual: (i64, i64, i64) = conn
        .query_row(
            "SELECT COUNT(*), COUNT(*) FILTER (WHERE inc=4 AND reverse=7),
            COUNT(*) FILTER (WHERE inc IS NULL AND reverse IS NULL)
          FROM (SELECT x + 1 AS inc, 10 - x AS reverse FROM stage)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("full histogram");
    assert_eq!(actual, (3, 2, 1));
}

#[test]
fn nested_materialized_arithmetic_inverts_complete_physical_bags() {
    for &dialect in DIALECTS {
        let schemas = [
            schema("t", &[("a", "INTEGER")]),
            schema("stage", &[("x", "INTEGER")]),
            schema("mart", &[("y", "INTEGER")]),
        ];
        let mut b = bundle_with_schemas(
            &[
                "CREATE TABLE stage AS SELECT a + 1 AS x FROM t",
                "CREATE TABLE mart AS SELECT x + 2 AS y FROM stage",
                "SELECT y AS total FROM mart",
            ],
            dialect,
            &schemas,
        );
        goal(
            &mut b,
            2,
            3,
            vec![histogram(
                "total",
                &[(ConstraintValue::Null, 1), (ConstraintValue::Integer(7), 2)],
            )],
        );
        let Some(OutcomeWitness::SourceRows {
            relation,
            rows,
            columns,
        }) = b.outcome_goals()[0].witness()
        else {
            panic!(
                "{dialect}: transitive inverse must reach the true physical source: {:?}",
                b.outcome_goals()
            );
        };
        assert_eq!((relation.as_str(), *rows), ("t", 3));
        assert_eq!(columns.len(), 1);
        assert_eq!(columns[0].column(), "a");
        assert_eq!(
            columns[0].values(),
            &[
                OutputValueCount::new(ConstraintValue::Null, 1),
                OutputValueCount::new(ConstraintValue::Integer(4), 2),
            ],
        );
    }
    let conn = Connection::open_in_memory().expect("duckdb");
    conn.execute_batch(
        "CREATE TABLE t(a INTEGER); INSERT INTO t VALUES (4), (NULL), (4);
         CREATE TABLE stage AS SELECT a + 1 AS x FROM t;
         CREATE TABLE mart AS SELECT x + 2 AS y FROM stage;",
    )
    .expect("complete materialized arithmetic");
    let actual: (i64, i64, i64) = conn
        .query_row(
            "SELECT COUNT(*), COUNT(*) FILTER (WHERE total = 7),
                COUNT(*) FILTER (WHERE total IS NULL)
         FROM (SELECT y AS total FROM mart)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("full end-to-end output bag");
    assert_eq!(actual, (3, 2, 1));
}

#[test]
fn computed_producer_outputs_without_complete_assignments_remain_residual() {
    let mut b = bundle_with_schemas(
        &[
            "CREATE TABLE stage AS SELECT a + 1 AS x, b + 1 AS y FROM t",
            "SELECT x AS value FROM stage",
        ],
        "postgresql",
        &[
            schema("t", &[("a", "INTEGER"), ("b", "INTEGER")]),
            schema("stage", &[("x", "INTEGER"), ("y", "INTEGER")]),
        ],
    );
    goal(
        &mut b,
        1,
        1,
        vec![histogram("value", &[(ConstraintValue::Integer(5), 1)])],
    );
    assert_eq!(b.outcome_goals()[0].status(), OutcomeGoalStatus::Residual);
}

#[test]
fn equal_source_aliases_with_different_histograms_are_impossible() {
    let mut bundle = bundle_with_schemas(
        &["SELECT a AS x, a AS y FROM t"],
        "postgresql",
        &[schema("t", &[("a", "INTEGER")])],
    );
    goal(
        &mut bundle,
        0,
        2,
        vec![
            histogram("x", &[(ConstraintValue::Integer(2), 2)]),
            histogram("y", &[(ConstraintValue::Integer(3), 2)]),
        ],
    );
    assert_eq!(
        bundle.outcome_goals()[0].status(),
        OutcomeGoalStatus::Unsatisfiable
    );
}

#[test]
fn computed_histograms_fail_closed_for_lossy_overflow_and_intermediate_coercion() {
    for &dialect in DIALECTS {
        let schemas = [
            schema("t", &[("a", "INTEGER")]),
            schema("stage", &[("x", "INTEGER")]),
        ];
        for (expression, bucket) in [
            ("x * 2", ConstraintValue::Integer(6)),
            ("x + 1", ConstraintValue::Integer(2_147_483_648)),
        ] {
            let sql = format!("SELECT {expression} AS v FROM stage");
            let mut bundle = bundle_with_schemas(
                &["CREATE TABLE stage AS SELECT a AS x FROM t", &sql],
                dialect,
                &schemas,
            );
            goal(&mut bundle, 1, 1, vec![histogram("v", &[(bucket, 1)])]);
            assert_eq!(
                bundle.outcome_goals()[0].status(),
                OutcomeGoalStatus::Residual,
                "{dialect}: unproved {expression} must not become writable"
            );
        }
    }

    // A NULL-only goal must not skip expression-structure validation:
    // COALESCE does not propagate source NULL.
    let mut null_bundle = bundle_with_schemas(
        &[
            "CREATE TABLE stage AS SELECT a AS x FROM t",
            "SELECT COALESCE(x, 0) AS v FROM stage",
        ],
        "postgresql",
        &[
            schema("t", &[("a", "INTEGER")]),
            schema("stage", &[("x", "INTEGER")]),
        ],
    );
    goal(
        &mut null_bundle,
        1,
        1,
        vec![histogram("v", &[(ConstraintValue::Null, 1)])],
    );
    assert_eq!(
        null_bundle.outcome_goals()[0].status(),
        OutcomeGoalStatus::Residual
    );

    let mut bundle = bundle_with_schemas(
        &[
            "CREATE TABLE stage AS SELECT a AS x FROM t",
            "SELECT x + 1 AS v FROM stage",
        ],
        "postgresql",
        &[
            schema("t", &[("a", "INTEGER")]),
            schema("stage", &[("x", "SMALLINT")]),
        ],
    );
    goal(
        &mut bundle,
        1,
        1,
        vec![histogram("v", &[(ConstraintValue::Integer(4), 1)])],
    );
    assert_eq!(
        bundle.outcome_goals()[0].status(),
        OutcomeGoalStatus::Residual
    );
}

#[test]
fn unrelated_computation_is_not_declared_total_from_another_column_histogram() {
    let mut bundle = bundle_with_schemas(
        &[
            "CREATE TABLE stage AS SELECT a FROM t",
            "SELECT a AS x, a * 2 AS y FROM stage",
        ],
        "postgresql",
        &[
            schema("t", &[("a", "INTEGER")]),
            schema("stage", &[("a", "INTEGER")]),
        ],
    );
    goal(
        &mut bundle,
        1,
        1,
        vec![histogram("x", &[(ConstraintValue::Integer(1), 1)])],
    );
    assert_eq!(
        bundle.outcome_goals()[0].status(),
        OutcomeGoalStatus::Residual
    );
}

#[test]
fn complete_bool_text_and_null_histograms_preserve_declared_column_types() {
    let source = [schema("t", &[("flag", "BOOLEAN"), ("label", "VARCHAR(8)")])];
    let mut b = bundle_with_schemas(
        &["SELECT flag AS out_flag, label AS out_text FROM t"],
        "postgresql",
        &source,
    );
    goal(
        &mut b,
        0,
        3,
        vec![
            histogram(
                "out_flag",
                &[
                    (ConstraintValue::Boolean(true), 2),
                    (ConstraintValue::Null, 1),
                ],
            ),
            histogram(
                "out_text",
                &[
                    (ConstraintValue::String("abc".to_string()), 2),
                    (ConstraintValue::Null, 1),
                ],
            ),
        ],
    );
    let Some(OutcomeWitness::SourceRows { columns, .. }) = b.outcome_goals()[0].witness() else {
        panic!("complete typed boolean and text source assignments must be constructive");
    };
    assert_eq!(columns.len(), 2);
    assert_eq!(
        (columns[0].column(), columns[1].column()),
        ("flag", "label")
    );

    let target = b.layers()[0].id().to_string();
    b.set_outcome_goals(&[OutcomeGoal::new(
        target,
        Some(1),
        None,
        vec![histogram(
            "out_text",
            &[(ConstraintValue::String("over-eight".to_string()), 1)],
        )],
    )
    .unwrap()])
        .unwrap();
    assert_eq!(b.outcome_goals()[0].status(), OutcomeGoalStatus::Residual);

    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE t(flag BOOLEAN, label VARCHAR(8));
         INSERT INTO t VALUES (TRUE, 'abc'), (NULL, NULL), (TRUE, 'abc');",
    )
    .unwrap();
    let actual: (i64, i64, i64) = conn
        .query_row(
            "SELECT COUNT(*), COUNT(*) FILTER (WHERE out_flag AND out_text='abc'),
             COUNT(*) FILTER (WHERE out_flag IS NULL AND out_text IS NULL)
         FROM (SELECT flag AS out_flag, label AS out_text FROM t)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(actual, (3, 2, 1));
}

#[test]
fn reversed_computed_aliases_reconcile_and_detect_necessary_conflicts() {
    for &dialect in DIALECTS {
        let mut b = bundle_with_schemas(
            &["SELECT a + 1 AS x, 5 - a AS y FROM t"],
            dialect,
            &[schema("t", &[("a", "INTEGER")])],
        );
        goal(
            &mut b,
            0,
            2,
            vec![
                histogram("x", &[(ConstraintValue::Integer(4), 2)]),
                histogram("y", &[(ConstraintValue::Integer(2), 2)]),
            ],
        );
        let Some(OutcomeWitness::SourceRows { columns, .. }) = b.outcome_goals()[0].witness()
        else {
            panic!("{dialect}: both inverses refer to two rows of a=3");
        };
        assert_eq!(columns.len(), 1);
        assert_eq!(columns[0].column(), "a");
        assert_eq!(
            columns[0].values(),
            &[OutputValueCount::new(ConstraintValue::Integer(3), 2)],
        );

        goal(
            &mut b,
            0,
            2,
            vec![
                histogram("x", &[(ConstraintValue::Integer(4), 2)]),
                histogram("y", &[(ConstraintValue::Integer(7), 2)]),
            ],
        );
        assert_eq!(
            b.outcome_goals()[0].status(),
            OutcomeGoalStatus::Unsatisfiable,
            "{dialect}: a=3 and a=-2 cannot both be the complete same physical bag",
        );
    }
}

#[test]
fn reversible_terminal_histogram_lifts_complete_materialized_set_law() {
    for dialect in ["duckdb", "postgresql"] {
        let schemas = [
            schema("l", &[("a", "INTEGER")]),
            schema("r", &[("a", "INTEGER")]),
            schema("left_stage", &[("id", "INTEGER")]),
            schema("right_stage", &[("id", "INTEGER")]),
            schema("set_stage", &[("id", "INTEGER")]),
        ];
        let mut b = bundle_with_schemas(
            &[
                "CREATE TABLE left_stage AS SELECT a AS id FROM l",
                "CREATE TABLE right_stage AS SELECT a AS id FROM r",
                "CREATE TABLE set_stage AS SELECT id FROM left_stage UNION SELECT id FROM right_stage",
                "SELECT id + 1 AS shifted FROM set_stage",
            ],
            dialect,
            &schemas,
        );
        goal(
            &mut b,
            3,
            2,
            vec![histogram(
                "shifted",
                &[(ConstraintValue::Null, 1), (ConstraintValue::Integer(4), 1)],
            )],
        );
        assert!(
            matches!(b.outcome_goals()[0].witness(), Some(OutcomeWitness::SetTuples { .. })),
            "{dialect}: complete DISTINCT set membership should survive a reversible final projection: {:?}",
            b.outcome_goals()
        );
    }
    let db = Connection::open_in_memory().expect("duckdb");
    db.execute_batch(
        "CREATE TABLE l(a INTEGER); CREATE TABLE r(a INTEGER);
         INSERT INTO l VALUES (3),(NULL);
         INSERT INTO r VALUES (NULL),(3);
         CREATE TABLE left_stage AS SELECT a AS id FROM l;
         CREATE TABLE right_stage AS SELECT a AS id FROM r;
         CREATE TABLE set_stage AS SELECT id FROM left_stage UNION SELECT id FROM right_stage;",
    ).expect("complete distinct physical branches");
    let actual: (i64, i64, i64) = db.query_row(
        "SELECT COUNT(*), COUNT(*) FILTER (WHERE shifted = 4),
                COUNT(*) FILTER (WHERE shifted IS NULL)
         FROM (SELECT id + 1 AS shifted FROM set_stage)",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).unwrap();
    assert_eq!(actual, (2,1,1));
}
