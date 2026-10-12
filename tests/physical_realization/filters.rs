//! filters regression tests.

use super::*;

#[test]
fn complete_rejected_source_rows_prove_zero_terminal_output_across_dialects() {
    for &dialect in DIALECTS {
        let b = bundle(&["SELECT a, b FROM t WHERE a > 2 OR b < 0"], dialect);
        let id = b.layers()[0].id();
        let proof = physical_rejected_row_count_plan(&b, id, 3);
        let WitnessDirection::Feasible(cases) = proof else {
            panic!("{dialect}: rejected physical rows must be constructive: {proof:?}");
        };
        assert_eq!(cases.len(), 1);
        assert!(cases[0].obligations().iter().any(|obligation| matches!(
            obligation,
            WitnessObligation::Rows {
                boundary,
                bounds,
                predicate: WitnessFormula::RowTruth {
                    truth: sql_semantic_protocol::BooleanTruthCase::NotTrue,
                    ..
                },
                closed_world: true,
                ..
            } if boundary.relation() == "t"
                && bounds.minimum() == 3
                && bounds.maximum() == Some(3)
        )));
        assert!(cases[0].obligations().iter().any(|obligation| matches!(
            obligation,
            WitnessObligation::OutputRows { layer_id, bounds }
                if layer_id == id && bounds.minimum() == 0 && bounds.maximum() == Some(0)
        )));
        assert!(cases[0].obligations().iter().any(|obligation| matches!(
            obligation,
            WitnessObligation::ClosedWorld { boundary, .. } if boundary.relation() == "t"
        )));
    }
    let conn = Connection::open_in_memory().expect("duckdb");
    conn.execute_batch(
        "CREATE TABLE t(a INTEGER, b INTEGER);
         INSERT INTO t VALUES (0,0), (NULL,NULL), (1,NULL);",
    )
    .expect("deliberate rejected source rows");
    let (input, output): (i64, i64) = conn
        .query_row(
            "SELECT (SELECT COUNT(*) FROM t),
                    (SELECT COUNT(*) FROM t WHERE a > 2 OR b < 0)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("whole-source oracle");
    assert_eq!((input, output), (3, 0));
}

#[test]
fn negative_closed_world_proof_requires_joint_filters_and_no_unproved_shaping() {
    let b = bundle(
        &[
            "CREATE TABLE stage AS SELECT a, b FROM t WHERE a > 2 OR b < 0",
            "SELECT a FROM stage WHERE a IS NULL OR b IS NOT NULL",
        ],
        "postgresql",
    );
    assert!(matches!(
        physical_rejected_row_count_plan(&b, b.layers()[1].id(), 4),
        WitnessDirection::Feasible(_)
    ));
    for query in [
        "SELECT a FROM t WHERE a > 2 OR b < 0 LIMIT 1",
        "SELECT a FROM t WHERE a * 2 > 5",
        "SELECT l.a FROM l JOIN r ON l.k = r.k",
    ] {
        let b = bundle(&[query], "postgresql");
        assert!(
            !matches!(
                physical_rejected_row_count_plan(&b, b.layers()[0].id(), 2),
                WitnessDirection::Feasible(_)
            ),
            "{query}: no unsupported closed-world absence claim"
        );
    }
}

#[test]
fn positive_filter_counts_require_closed_world_physical_qualifying_rows() {
    for &dialect in DIALECTS {
        let b = bundle(&["SELECT a, b FROM t WHERE a > 2 OR b < 0"], dialect);
        for rows in [1, 3, 8] {
            let target = b.layers()[0].id();
            let proof = physical_row_count_plan(&b, target, rows);
            let WitnessDirection::Feasible(cases) = proof else {
                panic!("{dialect}: expected proven filtered {rows}-row plan: {proof:?}");
            };
            assert_eq!(cases.len(), 1);
            assert!(cases[0].obligations().iter().any(|obligation| matches!(
                obligation,
                WitnessObligation::Rows {
                    boundary,
                    bounds,
                    predicate: WitnessFormula::RowTruth { .. },
                    closed_world: true,
                    ..
                } if boundary.relation() == "t"
                    && bounds.minimum() == rows
                    && bounds.maximum() == Some(rows)
            )));
            assert!(cases[0].obligations().iter().any(|obligation| matches!(
                obligation,
                WitnessObligation::ClosedWorld { boundary, .. }
                    if boundary.relation() == "t"
            )));
        }
    }
    let conn = Connection::open_in_memory().expect("duckdb");
    conn.execute_batch(
        "CREATE TABLE t(a INTEGER, b INTEGER);
         INSERT INTO t VALUES (3, NULL), (3, 3), (4, 4);
         CREATE TABLE stage AS SELECT a, b FROM t WHERE a > 2 OR b < 0;",
    )
    .expect("populate only qualifying rows");
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM stage", [], |row| row.get(0))
        .expect("filtered count");
    assert_eq!(count, 3);
    conn.execute_batch("INSERT INTO t VALUES (1, 9), (NULL, 2);")
        .expect("add deliberately rejected rows");
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM (SELECT a FROM t WHERE a > 2 OR b < 0)",
            [],
            |row| row.get(0),
        )
        .expect("rejected rows do not survive");
    assert_eq!(count, 3);
}

#[test]
fn nested_null_filters_prove_positive_counts_on_shared_source_rows() {
    for &dialect in DIALECTS {
        let b = bundle(
            &[
                "CREATE TABLE stage AS SELECT a, b FROM t WHERE a IS NOT NULL OR b IS NOT NULL",
                "SELECT a FROM stage WHERE a IS NULL OR b IS NOT NULL",
            ],
            dialect,
        );
        let proof = physical_row_count_plan(&b, b.layers()[1].id(), 3);
        assert!(
            matches!(proof, WitnessDirection::Feasible(_)),
            "{dialect}: {proof:?}"
        );
    }
    let conn = Connection::open_in_memory().expect("duckdb");
    conn.execute_batch(
        "CREATE TABLE t(a INTEGER, b INTEGER);
         INSERT INTO t VALUES (NULL, 1), (2, 2), (NULL, 3);
         CREATE TABLE stage AS SELECT a, b FROM t WHERE a IS NOT NULL OR b IS NOT NULL;",
    )
    .expect("populate matching physical rows");
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM (SELECT a FROM stage WHERE a IS NULL OR b IS NOT NULL)",
            [],
            |row| row.get(0),
        )
        .expect("nested filter count");
    assert_eq!(count, 3);
}

#[test]
fn joint_counts_keep_filter_truth_and_fail_closed_on_distinct_conditions() {
    for &dialect in DIALECTS {
        let b = bundle(
            &[
                "SELECT a FROM t",
                "SELECT a FROM t WHERE a IS NOT NULL OR b IS NOT NULL",
                "SELECT b FROM r WHERE a IS NULL OR b IS NULL",
                "SELECT a FROM t WHERE a IS NULL OR b IS NULL",
            ],
            dialect,
        );
        let raw = b.layers()[0].id();
        let positive = b.layers()[1].id();
        let independent = b.layers()[2].id();
        let incompatible = b.layers()[3].id();
        let proof = physical_joint_row_count_plan(&b, &[(raw, 3), (positive, 3), (independent, 2)]);
        let WitnessDirection::Feasible(cases) = proof else {
            panic!("{dialect}: independently constructible filters: {proof:?}");
        };
        assert_eq!(cases.len(), 1);
        assert_eq!(
            cases[0]
                .obligations()
                .iter()
                .filter(|obligation| matches!(
                    obligation,
                    WitnessObligation::Rows {
                        predicate: WitnessFormula::RowTruth { .. },
                        ..
                    }
                ))
                .count(),
            2,
            "{dialect}: do not discard physical row truth while merging sources"
        );
        assert!(matches!(
            physical_joint_row_count_plan(&b, &[(positive, 3), (incompatible, 3)]),
            WitnessDirection::Feasible(_)
        ));
        assert!(matches!(
            physical_joint_row_count_plan(&b, &[(raw, 4), (positive, 3)]),
            WitnessDirection::Residual { .. }
        ));
    }
}

#[test]
fn distinct_compatible_filters_require_one_shared_source_row_truth() {
    for &dialect in DIALECTS {
        let b = bundle(
            &[
                "SELECT a FROM t",
                "SELECT a FROM t WHERE a > 1",
                "SELECT a FROM t WHERE a < 5",
                "SELECT b FROM r WHERE b IS NULL",
            ],
            dialect,
        );
        let goals = [
            (b.layers()[0].id(), 3),
            (b.layers()[1].id(), 3),
            (b.layers()[2].id(), 3),
            (b.layers()[3].id(), 1),
        ];
        let witness = physical_joint_row_count_plan(&b, &goals);
        let WitnessDirection::Feasible(cases) = witness else {
            panic!("{dialect}: compatible shared-source truth: {witness:?}");
        };
        let [case] = cases.as_slice() else {
            panic!("{dialect}: expected one joint case");
        };
        assert_eq!(
            case.obligations()
                .iter()
                .filter(|o| matches!(o, WitnessObligation::ClosedWorld { .. }))
                .count(),
            2,
        );
        assert!(case.obligations().iter().any(|o| matches!(
            o,
            WitnessObligation::Rows {
                boundary,
                predicate: WitnessFormula::RowTruth {
                    predicate: BooleanRowConstraint::All(children),
                    truth: BooleanTruthCase::True,
                    ..
                },
                ..
            } if boundary.relation() == "t" && children.len() == 2
        )));
    }
    let conn = Connection::open_in_memory().expect("duckdb");
    conn.execute_batch(
        "CREATE TABLE t(a INTEGER, b INTEGER);
         CREATE TABLE r(a INTEGER, b INTEGER);
         INSERT INTO t VALUES (2, NULL), (3, 4), (4, NULL);
         INSERT INTO r VALUES (0, NULL);
        ",
    )
    .expect("physical assignments");
    for (sql, expected) in [
        ("SELECT COUNT(*) FROM t", 3),
        ("SELECT COUNT(*) FROM t WHERE a > 1", 3),
        ("SELECT COUNT(*) FROM t WHERE a < 5", 3),
        ("SELECT COUNT(*) FROM r WHERE b IS NULL", 1),
    ] {
        let count: i64 = conn.query_row(sql, [], |row| row.get(0)).expect("count");
        assert_eq!(count, expected, "{sql}");
    }
}

#[test]
fn disjoint_shared_filters_require_necessary_source_count_to_prove_impossible() {
    for &dialect in DIALECTS {
        let b = bundle(
            &[
                "SELECT a FROM t",
                "SELECT a FROM t WHERE a > 10",
                "SELECT a FROM t WHERE a < 0",
            ],
            dialect,
        );
        assert!(
            matches!(
                physical_joint_row_count_plan(
                    &b,
                    &[
                        (b.layers()[0].id(), 2),
                        (b.layers()[1].id(), 2),
                        (b.layers()[2].id(), 2),
                    ]
                ),
                WitnessDirection::Impossible
            ),
            "{dialect}: disjoint predicates cannot cover the same fixed two source rows"
        );
        assert!(
            matches!(
                physical_joint_row_count_plan(
                    &b,
                    &[(b.layers()[1].id(), 2), (b.layers()[2].id(), 2)]
                ),
                WitnessDirection::Residual { .. }
            ),
            "{dialect}: extra rows could satisfy each filter separately"
        );
    }
    let conn = Connection::open_in_memory().expect("duckdb");
    conn.execute_batch(
        "CREATE TABLE t(a INTEGER);
         INSERT INTO t VALUES (11), (12), (-1), (-2);",
    )
    .expect("split source rows");
    let counts: (i64, i64, i64) = conn
        .query_row(
            "SELECT (SELECT COUNT(*) FROM t),
                    (SELECT COUNT(*) FROM t WHERE a > 10),
                    (SELECT COUNT(*) FROM t WHERE a < 0)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("split counts");
    assert_eq!(counts, (4, 2, 2));
}

#[test]
fn sql_null_truth_is_solved_on_shared_physical_rows() {
    for &dialect in DIALECTS {
        let b = bundle(
            &[
                "SELECT a FROM t",
                "SELECT a FROM t WHERE a IS NULL",
                "SELECT b FROM t WHERE b IS NOT NULL",
            ],
            dialect,
        );
        assert!(
            matches!(
                physical_joint_row_count_plan(
                    &b,
                    &[
                        (b.layers()[0].id(), 2),
                        (b.layers()[1].id(), 2),
                        (b.layers()[2].id(), 2),
                    ],
                ),
                WitnessDirection::Feasible(_)
            ),
            "{dialect}: both NULL-sensitive filters admit the same row assignment"
        );
    }
    let conn = Connection::open_in_memory().expect("duckdb");
    conn.execute_batch(
        "CREATE TABLE t(a INTEGER,b INTEGER);
         INSERT INTO t VALUES (NULL,1),(NULL,2);",
    )
    .expect("nullable source");
    let counts: (i64, i64) = conn
        .query_row(
            "SELECT (SELECT COUNT(*) FROM t WHERE a IS NULL),
                    (SELECT COUNT(*) FROM t WHERE b IS NOT NULL)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("NULL-sensitive counts");
    assert_eq!(counts, (2, 2));
}

#[test]
fn physical_single_comparison_and_null_filters_construct_closed_world_counts() {
    for &dialect in DIALECTS {
        for query in [
            "SELECT a FROM t WHERE a > 2",
            "SELECT a FROM t WHERE a IS NULL",
            "SELECT a FROM t WHERE a IS NOT NULL",
        ] {
            let b = bundle(&[query], dialect);
            for requested in [1, 3] {
                let id = b.layers()[0].id();
                let positive = physical_row_count_plan(&b, id, requested);
                assert!(
                    matches!(positive, WitnessDirection::Feasible(_)),
                    "{dialect}: {query}: positive {requested}: {positive:?}"
                );
                let negative = physical_rejected_row_count_plan(&b, id, requested);
                assert!(
                    matches!(negative, WitnessDirection::Feasible(_)),
                    "{dialect}: {query}: negative {requested}: {negative:?}"
                );
            }
        }
        let b = bundle(
            &[
                "CREATE TABLE stage AS SELECT a FROM t WHERE a > 2",
                "SELECT a FROM stage",
            ],
            dialect,
        );
        assert!(matches!(
            physical_row_count_plan(&b, b.layers()[1].id(), 3),
            WitnessDirection::Feasible(_)
        ));
    }
    let db = Connection::open_in_memory().expect("duckdb");
    db.execute_batch(
        "CREATE TABLE t(a INTEGER,b INTEGER,k INTEGER);
         INSERT INTO t VALUES (3,NULL,NULL),(4,2,NULL),(5,2,NULL);",
    )
    .expect("source rows");
    let positive: i64 = db
        .query_row("SELECT COUNT(*) FROM t WHERE a > 2", [], |row| row.get(0))
        .expect("positive");
    db.execute_batch("DELETE FROM t; INSERT INTO t VALUES (0,0,0),(NULL,0,0),(2,0,0);")
        .expect("rejected rows");
    let negative: i64 = db
        .query_row("SELECT COUNT(*) FROM t WHERE a > 2", [], |row| row.get(0))
        .expect("negative");
    assert_eq!((positive, negative), (3, 0));
}

#[test]
fn physical_scalar_count_rejects_opaque_or_noninvertible_filters() {
    for query in [
        "SELECT a FROM t WHERE a + 1 > 2 LIMIT 1",
        "SELECT a FROM t WHERE a IS NOT NULL LIMIT 1",
        "SELECT a FROM t WHERE a * 2 > 5",
        "SELECT a FROM t WHERE a > 2147483647",
    ] {
        let b = bundle(&[query], "postgresql");
        let id = b.layers()[0].id();
        assert!(
            !matches!(
                physical_row_count_plan(&b, id, 3),
                WitnessDirection::Feasible(_)
            ),
            "{query}: unsupported source count must remain residual"
        );
    }
}
