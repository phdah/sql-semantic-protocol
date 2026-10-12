//! joint filters regression tests.

use super::*;

#[test]
fn shared_source_many_terminal_positive_and_negative_goals_are_jointly_constructed() {
    for &dialect in DIALECTS {
        let b = bundle(
            &[
                "SELECT a FROM t",
                "SELECT b FROM t",
                "SELECT a FROM t WHERE a > 10",
                "SELECT b FROM t WHERE a > 10",
                "SELECT a FROM t WHERE b < 0",
            ],
            dialect,
        );
        let outputs = [
            (b.layers()[0].id(), 3),
            (b.layers()[1].id(), 3),
            (b.layers()[2].id(), 0),
            (b.layers()[3].id(), 0),
        ];
        let proof = physical_joint_row_count_plan(&b, &outputs);
        let WitnessDirection::Feasible(cases) = proof else {
            panic!("{dialect}: shared positive and rejected row goals: {proof:?}");
        };
        assert_eq!(cases.len(), 1);
        let obligations = cases[0].obligations();
        assert_eq!(
            obligations
                .iter()
                .filter(|o| matches!(o, WitnessObligation::Rows { .. }))
                .count(),
            1,
        );
        assert_eq!(
            obligations
                .iter()
                .filter(|o| matches!(o, WitnessObligation::ClosedWorld { .. }))
                .count(),
            1,
        );
        assert_eq!(
            obligations
                .iter()
                .filter(|o| matches!(o, WitnessObligation::OutputRows { .. }))
                .count(),
            4,
        );
        assert!(obligations.iter().any(|o| matches!(
            o,
            WitnessObligation::Rows {
                predicate: WitnessFormula::RowTruth {
                    truth: sql_semantic_protocol::BooleanTruthCase::NotTrue,
                    ..
                },
                bounds,
                ..
            } if bounds.minimum() == 3 && bounds.maximum() == Some(3)
        )));
        // Different negative predicates must be jointly rejected by each
        // physical source row, not satisfied by independent examples.
        let shared_rejections = [
            (b.layers()[0].id(), 3),
            (b.layers()[2].id(), 0),
            (b.layers()[4].id(), 0),
        ];
        let witness = physical_joint_row_count_plan(&b, &shared_rejections);
        let WitnessDirection::Feasible(cases) = witness else {
            panic!("{dialect}: different predicates can reject the same rows: {witness:?}");
        };
        assert!(cases[0].obligations().iter().any(|obligation| matches!(
            obligation,
            WitnessObligation::Rows {
                predicate: WitnessFormula::All(items),
                ..
            } if items.len() == 2 && items.iter().all(|item| matches!(
                item, WitnessFormula::RowTruth {
                    truth: BooleanTruthCase::NotTrue,
                    ..
                }
            ))
        )));
        assert!(matches!(
            physical_joint_row_count_plan(
                &b,
                &[
                    (b.layers()[0].id(), 3),
                    (b.layers()[1].id(), 4),
                    (b.layers()[2].id(), 0),
                ],
            ),
            WitnessDirection::Impossible
        ));
    }

    let conn = Connection::open_in_memory().expect("duckdb");
    conn.execute_batch(
        "CREATE TABLE t(a INTEGER,b INTEGER,k INTEGER);
         INSERT INTO t VALUES (0,1,NULL),(NULL,2,NULL),(5,3,NULL);",
    )
    .expect("three controlled source rows");
    for sql in [
        "SELECT COUNT(*) FROM t",
        "SELECT COUNT(b) FROM t",
        "SELECT COUNT(*) FROM t WHERE a > 10",
        "SELECT COUNT(*) FROM t WHERE a > 10 AND b IS NOT NULL",
    ] {
        let count: i64 = conn.query_row(sql, [], |row| row.get(0)).expect("oracle");
        assert_eq!(count, if sql.contains("WHERE") { 0 } else { 3 }, "{sql}");
    }
}

#[test]
fn joint_negative_filters_prove_sql_not_true_without_disconnected_examples() {
    for &dialect in DIALECTS {
        let b = bundle(
            &[
                "SELECT a FROM t",
                "SELECT a FROM t WHERE a IS NULL",
                "SELECT b FROM t WHERE b > 5",
            ],
            dialect,
        );
        assert!(
            matches!(
                physical_joint_row_count_plan(
                    &b,
                    &[
                        (b.layers()[0].id(), 2),
                        (b.layers()[1].id(), 0),
                        (b.layers()[2].id(), 0),
                    ]
                ),
                WitnessDirection::Feasible(_)
            ),
            "{dialect}: nonnull a and UNKNOWN/FALSE b reject both filters"
        );

        let contradiction = bundle(
            &[
                "SELECT a FROM t",
                "SELECT a FROM t WHERE a IS NULL",
                "SELECT a FROM t WHERE a IS NOT NULL",
            ],
            dialect,
        );
        assert!(
            matches!(
                physical_joint_row_count_plan(
                    &contradiction,
                    &[
                        (contradiction.layers()[0].id(), 2),
                        (contradiction.layers()[1].id(), 0),
                        (contradiction.layers()[2].id(), 0),
                    ]
                ),
                WitnessDirection::Impossible
            ),
            "{dialect}: every nonempty row satisfies one NULL-complement filter"
        );
    }

    let conn = Connection::open_in_memory().expect("duckdb");
    conn.execute_batch(
        "CREATE TABLE t(a INTEGER, b INTEGER);
         INSERT INTO t VALUES (1,NULL),(2,3);",
    )
    .expect("two physical rows");
    for (sql, expected) in [
        ("SELECT COUNT(*) FROM t", 2),
        ("SELECT COUNT(*) FROM t WHERE a IS NULL", 0),
        ("SELECT COUNT(*) FROM t WHERE b > 5", 0),
    ] {
        let actual: i64 = conn.query_row(sql, [], |row| row.get(0)).expect("count");
        assert_eq!(actual, expected, "{sql}");
    }
}

#[test]
fn mixed_positive_and_negative_goals_share_one_complete_physical_assignment() {
    for &dialect in DIALECTS {
        let b = bundle(
            &[
                "CREATE TABLE stage AS SELECT a, b FROM t",
                "SELECT a FROM stage WHERE a > 0",
                "SELECT b FROM t WHERE a < 10",
                "SELECT a FROM t WHERE b IS NULL",
            ],
            dialect,
        );
        let goals = [
            (b.layers()[0].id(), 3),
            (b.layers()[1].id(), 3),
            (b.layers()[2].id(), 3),
            (b.layers()[3].id(), 0),
        ];
        let proof = physical_joint_row_count_plan(&b, &goals);
        let WitnessDirection::Feasible(cases) = proof else {
            panic!("{dialect}: all terminals must share the same source rows: {proof:?}");
        };
        let [case] = cases.as_slice() else {
            panic!("{dialect}: exactly one joint construction required");
        };
        assert_eq!(
            case.obligations()
                .iter()
                .filter(|item| matches!(item, WitnessObligation::ClosedWorld { .. }))
                .count(),
            1
        );
        assert!(case.obligations().iter().any(|item| matches!(
            item,
            WitnessObligation::Rows {
                predicate: WitnessFormula::All(items),
                ..
            } if items.len() == 3
        )));
        assert_eq!(
            case.obligations()
                .iter()
                .filter(|item| matches!(item, WitnessObligation::OutputRows { .. }))
                .count(),
            4
        );
    }

    let conn = Connection::open_in_memory().expect("duckdb");
    conn.execute_batch(
        "CREATE TABLE t(a INTEGER, b INTEGER);
         INSERT INTO t VALUES (1,0), (2,2), (3,3);
         CREATE TABLE stage AS SELECT a, b FROM t;",
    )
    .expect("shared physical rows and materialization");
    for (sql, expected) in [
        ("SELECT COUNT(*) FROM stage", 3),
        ("SELECT COUNT(*) FROM stage WHERE a > 0", 3),
        ("SELECT COUNT(*) FROM t WHERE a < 10", 3),
        ("SELECT COUNT(*) FROM t WHERE b IS NULL", 0),
    ] {
        let actual: i64 = conn.query_row(sql, [], |row| row.get(0)).expect("oracle");
        assert_eq!(actual, expected, "{sql}");
    }
}
