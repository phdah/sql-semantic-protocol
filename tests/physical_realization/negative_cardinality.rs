//! Closed-world cardinality and negative conjunction regression tests.

use super::*;

#[test]
fn several_zero_terminals_admit_one_nonempty_closed_physical_source() {
    for &dialect in DIALECTS {
        let b = bundle(
            &[
                "SELECT a FROM t WHERE a > 10",
                "SELECT a FROM t WHERE a < 0",
                "SELECT a FROM t WHERE a = 5",
            ],
            dialect,
        );
        let goals = b
            .layers()
            .iter()
            .map(|layer| (layer.id(), 0))
            .collect::<Vec<_>>();
        let plan = physical_joint_source_plan(&b, &goals);
        let WitnessDirection::Feasible(cases) = plan.outcome() else {
            panic!("{dialect}: expected simultaneous negative proof: {plan:?}");
        };
        assert_eq!(plan.sources(), &["t".to_string()]);
        assert!(cases.iter().any(|case| {
            case.obligations().iter().any(|obligation| {
                matches!(obligation,
                    WitnessObligation::Rows {
                        boundary,
                        bounds,
                        predicate: WitnessFormula::All(predicates),
                        closed_world: true,
                        ..
                    } if boundary.relation() == "t"
                        && bounds.minimum() == 1
                        && bounds.maximum() == Some(1)
                        && predicates.len() == 3
                        && predicates.iter().all(|predicate| matches!(
                            predicate,
                            WitnessFormula::RowTruth { truth: BooleanTruthCase::NotTrue, .. }
                        ))
                )
            }) && case.obligations().iter().filter(|obligation| {
                matches!(obligation, WitnessObligation::OutputRows { .. })
            }).count() == 3
                && case.obligations().iter().any(|obligation| matches!(
                    obligation, WitnessObligation::ClosedWorld { boundary, .. } if boundary.relation() == "t"
                ))
        }), "{dialect}: no complete nonempty joint rejection witness: {plan:?}");
        let reversed = goals.iter().rev().copied().collect::<Vec<_>>();
        assert_eq!(plan.outcome(), physical_joint_source_plan(&b, &reversed).outcome());
    }

    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE t(a INTEGER); INSERT INTO t VALUES (1);")
        .unwrap();
    for sql in [
        "SELECT COUNT(*) FROM t WHERE a > 10",
        "SELECT COUNT(*) FROM t WHERE a < 0",
        "SELECT COUNT(*) FROM t WHERE a = 5",
    ] {
        let count: i64 = db.query_row(sql, [], |row| row.get(0)).unwrap();
        assert_eq!(count, 0, "{sql}");
    }
}

#[test]
fn mutually_exclusive_nonempty_rejections_keep_valid_all_empty_solution() {
    let b = bundle(
        &[
            "SELECT a FROM t WHERE a IS NULL",
            "SELECT a FROM t WHERE a IS NOT NULL",
        ],
        "duckdb",
    );
    let goals = b.layers().iter().map(|layer| (layer.id(), 0)).collect::<Vec<_>>();
    let plan = physical_joint_source_plan(&b, &goals);
    let WitnessDirection::Feasible(cases) = plan.outcome() else {
        panic!("empty closed-world solution should remain possible: {plan:?}");
    };
    assert!(cases.iter().any(|case| case.obligations().iter().any(|obligation| {
        matches!(obligation,
            WitnessObligation::Rows { boundary, bounds, closed_world: true, .. }
                if boundary.relation() == "t" && bounds.minimum() == 0 && bounds.maximum() == Some(0))
    })));
    assert!(!cases.iter().any(|case| case.obligations().iter().any(|obligation| {
        matches!(obligation,
            WitnessObligation::Rows { boundary, bounds, closed_world: true, .. }
                if boundary.relation() == "t" && bounds.minimum() > 0)
    })));
}

#[test]
fn transparent_positive_and_different_rejected_filters_share_all_source_rows() {
    for &dialect in DIALECTS {
        let b = bundle(
            &[
                "SELECT a FROM t",
                "SELECT a FROM t WHERE a > 10",
                "SELECT a FROM t WHERE a < 0",
            ],
            dialect,
        );
        let goals = [(b.layers()[0].id(), 3), (b.layers()[1].id(), 0), (b.layers()[2].id(), 0)];
        let plan = physical_joint_source_plan(&b, &goals);
        let WitnessDirection::Feasible(cases) = plan.outcome() else {
            panic!("{dialect}: exact positive and two independent rejections: {plan:?}");
        };
        assert!(cases.iter().any(|case| case.obligations().iter().any(|obligation| {
            matches!(obligation,
                WitnessObligation::Rows {
                    boundary, bounds, predicate: WitnessFormula::All(parts),
                    closed_world: true, ..
                } if boundary.relation() == "t" && bounds.minimum() == 3
                  && bounds.maximum() == Some(3) && parts.len() == 2)
        })));
    }
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE t(a INTEGER); INSERT INTO t VALUES (1),(2),(3);").unwrap();
    for (sql, expected) in [
        ("SELECT COUNT(*) FROM t", 3_i64),
        ("SELECT COUNT(*) FROM t WHERE a > 10", 0),
        ("SELECT COUNT(*) FROM t WHERE a < 0", 0),
    ] {
        assert_eq!(db.query_row(sql, [], |row| row.get::<_, i64>(0)).unwrap(), expected);
    }
}
