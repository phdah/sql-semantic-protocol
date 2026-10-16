//! joins regression tests.

use super::*;

#[test]
fn complete_equi_join_populations_certify_shared_and_independent_sources() {
    for &dialect in DIALECTS {
        let mut independent = bundle(
            &[
                "CREATE TABLE stage AS SELECT a, k FROM l",
                "CREATE TABLE mart AS SELECT a, k FROM r",
                "SELECT s.a FROM stage AS s JOIN mart AS m ON s.k = m.k",
            ],
            dialect,
        );
        let plan = physical_joint_source_plan(&independent, &[(independent.layers()[2].id(), 3)]);
        let WitnessDirection::Feasible(cases) = plan.outcome() else {
            panic!("{dialect}: independent producer join unproved: {plan:?}");
        };
        assert_eq!(plan.sources(), &["l".to_string(), "r".to_string()]);
        assert!(
            cases
                .iter()
                .any(|case| case.obligations().iter().any(|obligation| matches!(
                    obligation,
                    WitnessObligation::JoinPopulation {
                        pattern: JoinPopulationPattern::DistinctMatched,
                        left_rows: 3,
                        right_rows: 3,
                        output_rows: 3,
                        closed_world: true,
                        ..
                    }
                ))),
            "{dialect}: requires one complete matching-key population"
        );
        assert!(cases.iter().any(|case| case
            .obligations()
            .iter()
            .filter(|o| matches!(o, WitnessObligation::ClosedWorld { .. }))
            .count()
            == 2));
        assert!(
            !cases.iter().any(|case| case
                .obligations()
                .iter()
                .any(|o| matches!(o, WitnessObligation::Producer { .. }))),
            "{dialect}: only physical source rows can be assigned"
        );

        let goal = OutcomeGoal::new(independent.layers()[2].id(), Some(3), None, vec![])
            .expect("join row goal");
        independent
            .set_outcome_goals(&[goal])
            .expect("evaluate goal");
        let wire: serde_json::Value =
            serde_json::from_str(&sql_semantic_protocol::to_bundle_json(&independent))
                .expect("canonical JSON");
        let emitted = &wire["graph"]["physical_joint_count_plan"];
        assert_eq!(emitted["outcome"]["status"], "feasible", "{dialect}");
        assert!(emitted["outcome"]["cases"]
            .as_array()
            .expect("cases")
            .iter()
            .any(|case| case["obligations"]
                .as_array()
                .expect("obligations")
                .iter()
                .any(|obligation| obligation["kind"] == "join_population"
                    && obligation["join_kind"] == "inner"
                    && obligation["pattern"] == "distinct_matched"
                    && obligation["output_rows"] == 3
                    && obligation["closed_world"] == true)));
        let schema: serde_json::Value =
            serde_json::from_str(include_str!("../../schema/protocol.schema.json"))
                .expect("schema JSON");
        assert!(schema["$defs"]["constructiveObligation"]["oneOf"]
            .as_array()
            .expect("variants")
            .iter()
            .any(|variant| variant["properties"]["kind"]["const"] == "join_population"));

        let self_join = bundle(
            &[
                "CREATE TABLE stage AS SELECT a, k FROM t",
                "CREATE TABLE mart AS SELECT a, k FROM t",
                "SELECT x.a FROM stage x JOIN mart y ON x.k = y.k",
            ],
            dialect,
        );
        let plan = physical_joint_source_plan(&self_join, &[(self_join.layers()[2].id(), 4)]);
        let WitnessDirection::Feasible(cases) = plan.outcome() else {
            panic!("{dialect}: shared physical self-join unproved: {plan:?}");
        };
        assert_eq!(plan.sources(), &["t".to_string()]);
        assert!(
            cases
                .iter()
                .any(|case| case.obligations().iter().any(|o| matches!(
                    o,
                    WitnessObligation::JoinPopulation {
                        pattern: JoinPopulationPattern::CommonMatched,
                        left_rows: 2,
                        right_rows: 2,
                        output_rows: 4,
                        ..
                    }
                ))),
            "{dialect}: one two-row source produces four self-join matches"
        );
        assert!(cases.iter().all(|case| case
            .obligations()
            .iter()
            .filter(|o| matches!(o, WitnessObligation::ClosedWorld { .. }))
            .count()
            == 1));
    }

    let db = Connection::open_in_memory().expect("duckdb");
    db.execute_batch(
        "CREATE TABLE t (a INTEGER,k INTEGER);
         INSERT INTO t VALUES (1,0), (1,0);
         CREATE TABLE stage AS SELECT a,k FROM t;
         CREATE TABLE mart AS SELECT a,k FROM t;",
    )
    .expect("materialized shared producers");
    let count: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM stage x JOIN mart y ON x.k=y.k",
            [],
            |row| row.get(0),
        )
        .expect("count");
    assert_eq!(count, 4);
}

#[test]
fn join_population_reconciles_joint_terminal_counts_without_duplicate_source_rows() {
    for &dialect in DIALECTS {
        let b = bundle(
            &[
                "CREATE TABLE stage AS SELECT a,k FROM l",
                "CREATE TABLE mart AS SELECT a,k FROM r",
                "SELECT s.a FROM stage s JOIN mart m ON s.k=m.k",
                "SELECT a FROM l",
                "SELECT a FROM r",
            ],
            dialect,
        );
        let conflicting_duplicate =
            physical_joint_source_plan(&b, &[(b.layers()[2].id(), 2), (b.layers()[2].id(), 3)]);
        assert!(
            matches!(
                conflicting_duplicate.outcome(),
                WitnessDirection::Impossible
            ),
            "{dialect}: a single output cannot have two exact cardinalities"
        );

        let targets = [
            (b.layers()[2].id(), 3),
            (b.layers()[3].id(), 3),
            (b.layers()[4].id(), 3),
        ];
        let proof = physical_joint_source_plan(&b, &targets);
        let WitnessDirection::Feasible(cases) = proof.outcome() else {
            panic!("{dialect}: one consistent physical join and terminal population: {proof:?}");
        };
        assert!(cases.iter().all(|case| case
            .obligations()
            .iter()
            .filter(|o| matches!(o, WitnessObligation::ClosedWorld { .. }))
            .count()
            == 2));
        assert!(cases.iter().all(|case| case
            .obligations()
            .iter()
            .filter(|o| matches!(o, WitnessObligation::OutputRows { .. }))
            .count()
            == 3));
        assert!(cases
            .iter()
            .any(|case| case.obligations().iter().any(|o| matches!(
                o,
                WitnessObligation::JoinPopulation {
                    pattern: JoinPopulationPattern::DistinctMatched,
                    left_rows: 3,
                    right_rows: 3,
                    ..
                }
            ))));

        let conflicting = physical_joint_source_plan(
            &b,
            &[
                (b.layers()[2].id(), 3),
                (b.layers()[3].id(), 2),
                (b.layers()[4].id(), 2),
            ],
        );
        assert!(
            matches!(conflicting.outcome(), WitnessDirection::Residual { .. }),
            "{dialect}: different keys and match multiplicity may remain feasible"
        );

        let necessarily_impossible =
            physical_joint_source_plan(&b, &[(b.layers()[2].id(), 3), (b.layers()[4].id(), 0)]);
        assert!(
            matches!(
                necessarily_impossible.outcome(),
                WitnessDirection::Impossible
            ),
            "{dialect}: positive inner join cannot read a fully empty right source"
        );

        let shared = bundle(
            &[
                "CREATE TABLE stage AS SELECT a,k FROM t",
                "CREATE TABLE mart AS SELECT a,k FROM t",
                "SELECT x.a FROM stage x JOIN mart y ON x.k=y.k",
                "SELECT a FROM t",
            ],
            dialect,
        );
        let proof = physical_joint_source_plan(
            &shared,
            &[(shared.layers()[2].id(), 4), (shared.layers()[3].id(), 2)],
        );
        let WitnessDirection::Feasible(cases) = proof.outcome() else {
            panic!("{dialect}: same two physical rows yield four self-join pairs: {proof:?}");
        };
        assert!(cases.iter().all(|case| case
            .obligations()
            .iter()
            .filter(|o| matches!(o, WitnessObligation::ClosedWorld { .. }))
            .count()
            == 1));
    }
}

#[test]
fn task_106_join_and_two_negative_terminals_share_full_physical_sources() {
    let b = bundle(
        &[
            "CREATE TABLE stage AS SELECT a,k FROM l",
            "CREATE TABLE mart AS SELECT a,k FROM r",
            "SELECT s.a FROM stage s JOIN mart m ON s.k=m.k",
            "SELECT a FROM l WHERE a < 0",
            "SELECT a FROM r WHERE a > 10",
        ],
        "postgresql",
    );
    let goals = [
        (b.layers()[2].id(), 2),
        (b.layers()[3].id(), 0),
        (b.layers()[4].id(), 0),
    ];
    let result = physical_joint_source_plan(&b, &goals);
    let WitnessDirection::Feasible(cases) = result.outcome() else {
        panic!("join and NOT TRUE terminals must share controlled source rows: {result:?}");
    };
    assert_eq!(result.sources(), &["l".to_string(), "r".to_string()]);
    assert!(cases.iter().any(|case| {
        case.obligations()
            .iter()
            .filter(|obligation| matches!(obligation, WitnessObligation::ClosedWorld { .. }))
            .count()
            == 2
            && case
                .obligations()
                .iter()
                .filter(|obligation| {
                    matches!(
                        obligation,
                        WitnessObligation::Rows {
                            predicate: WitnessFormula::RowTruth {
                                truth: BooleanTruthCase::NotTrue,
                                ..
                            },
                            ..
                        }
                    )
                })
                .count()
                == 2
            && case
                .obligations()
                .iter()
                .filter(|obligation| matches!(obligation, WitnessObligation::OutputRows { .. }))
                .count()
                == 3
    }));

    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE l(a INTEGER,k INTEGER);
         CREATE TABLE r(a INTEGER,k INTEGER);
         INSERT INTO l VALUES (NULL,0),(1,1);
         INSERT INTO r VALUES (2,0),(3,1);
         CREATE TABLE stage AS SELECT a,k FROM l;
         CREATE TABLE mart AS SELECT a,k FROM r;",
    )
    .unwrap();
    let joined: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM stage s JOIN mart m ON s.k=m.k",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let rejected_left: i64 = db
        .query_row("SELECT COUNT(*) FROM l WHERE a < 0", [], |row| row.get(0))
        .unwrap();
    let rejected_right: i64 = db
        .query_row("SELECT COUNT(*) FROM r WHERE a > 10", [], |row| row.get(0))
        .unwrap();
    assert_eq!((joined, rejected_left, rejected_right), (2, 0, 0));
}

#[test]
fn task_106_positive_inner_join_cannot_reject_both_null_complements() {
    let b = bundle(
        &[
            "CREATE TABLE stage AS SELECT a,k FROM l",
            "CREATE TABLE mart AS SELECT a,k FROM r",
            "SELECT s.a FROM stage s JOIN mart m ON s.k=m.k",
            "SELECT a FROM l WHERE a IS NULL",
            "SELECT a FROM l WHERE a IS NOT NULL",
        ],
        "postgresql",
    );
    let goals = [
        (b.layers()[2].id(), 1),
        (b.layers()[3].id(), 0),
        (b.layers()[4].id(), 0),
    ];
    let joint = physical_joint_source_plan(&b, &goals);
    assert!(
        matches!(joint.outcome(), WitnessDirection::Impossible),
        "every nonempty left-side row must pass one of the complementary filters: {joint:?}"
    );

    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE l(a INTEGER,k INTEGER);
         CREATE TABLE r(a INTEGER,k INTEGER);
         INSERT INTO l VALUES (NULL,1),(4,2);
         INSERT INTO r VALUES (1,1),(2,2);
         CREATE TABLE stage AS SELECT a,k FROM l;
         CREATE TABLE mart AS SELECT a,k FROM r;",
    )
    .unwrap();
    let actual: (i64, i64, i64) = db
        .query_row(
            "SELECT
                (SELECT COUNT(*) FROM stage s JOIN mart m ON s.k=m.k),
                (SELECT COUNT(*) FROM l WHERE a IS NULL),
                (SELECT COUNT(*) FROM l WHERE a IS NOT NULL)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(actual, (2, 1, 1));
}

#[test]
fn physical_semijoin_and_antijoin_use_complete_nonempty_or_absent_partners() {
    for sql in [
        "SELECT l.a FROM l LEFT SEMI JOIN r ON l.k = r.k",
        "SELECT l.a FROM l LEFT ANTI JOIN r ON l.k = r.k",
        "SELECT r.a FROM l RIGHT SEMI JOIN r ON l.k = r.k",
        "SELECT r.a FROM l RIGHT ANTI JOIN r ON l.k = r.k",
    ] {
        let dialect = "duckdb";
        let b = bundle(&[sql], dialect);
        let proof = physical_joint_source_plan(&b, &[(b.layers()[0].id(), 3)]);
        let WitnessDirection::Feasible(cases) = proof.outcome() else {
            panic!("expected complete source law for {sql}: {proof:?}");
        };
        assert!(
            cases
                .iter()
                .any(|case| case.obligations().iter().any(|o| matches!(
                    o,
                    WitnessObligation::JoinPopulation {
                        output_rows: 3,
                        closed_world: true,
                        ..
                    }
                ))),
            "{sql}"
        );
    }

    let b = bundle(&["SELECT l.a FROM l JOIN r ON l.k=r.k"], "duckdb");
    let proof = physical_joint_source_plan(&b, &[(b.layers()[0].id(), 0)]);
    assert!(
        matches!(proof.outcome(), WitnessDirection::Feasible(_)),
        "a completely empty source population guarantees an empty join"
    );
}

#[test]
fn complete_join_population_preserves_outer_absence_and_duplicate_bags() {
    let cases = [
        (
            "SELECT l.a FROM l LEFT JOIN r ON l.k=r.k",
            3,
            JoinPopulationPattern::EmptyRight,
        ),
        (
            "SELECT r.a FROM l RIGHT JOIN r ON l.k=r.k",
            3,
            JoinPopulationPattern::EmptyLeft,
        ),
        (
            "SELECT l.a FROM l FULL JOIN r ON l.k=r.k",
            3,
            JoinPopulationPattern::EmptyRight,
        ),
        (
            "SELECT l.a FROM l JOIN r ON l.k=r.k",
            4,
            JoinPopulationPattern::CommonMatched,
        ),
    ];
    for &(sql, expected_rows, expected_pattern) in &cases {
        let b = bundle(&[sql], "postgresql");
        let p = physical_joint_source_plan(&b, &[(b.layers()[0].id(), expected_rows)]);
        let WitnessDirection::Feasible(options) = p.outcome() else {
            panic!("join physical population unproved: {sql}: {p:?}");
        };
        assert!(
            options.iter().any(|case| case.obligations().iter().any(
                |o| matches!(o, WitnessObligation::JoinPopulation { pattern, .. }
                if *pattern == expected_pattern)
            )),
            "{sql}"
        );
    }

    // A matched key assignment must also retain a local pair witness;
    // complete absence has explicit closed-world no-partner evidence.
    let matched = bundle(&["SELECT l.a FROM l JOIN r ON l.k=r.k"], "postgresql");
    let proof = physical_joint_source_plan(&matched, &[(matched.layers()[0].id(), 3)]);
    let WitnessDirection::Feasible(matched_cases) = proof.outcome() else {
        panic!("matched joined sources must be realizable");
    };
    assert!(matched_cases.iter().all(|case| case
        .obligations()
        .iter()
        .any(|o| matches!(o, WitnessObligation::JoinPair { .. }))));

    let unmatched = bundle(&["SELECT l.a FROM l LEFT JOIN r ON l.k=r.k"], "postgresql");
    let proof = physical_joint_source_plan(&unmatched, &[(unmatched.layers()[0].id(), 3)]);
    let WitnessDirection::Feasible(unmatched_cases) = proof.outcome() else {
        panic!("left-outer unmatched source rows must be realizable");
    };
    assert!(unmatched_cases
        .iter()
        .any(|case| case.obligations().iter().any(|o| matches!(
            o,
            WitnessObligation::NoMatchingPartner {
                null_extended: Some(sql_semantic_protocol::JoinSide::Right),
                closed_world: true,
                ..
            }
        ))));

    let many = bundle(&["SELECT l.a FROM l JOIN r ON l.k=r.k"], "postgresql");
    let plan = physical_joint_source_plan(&many, &[(many.layers()[0].id(), 12)]);
    let WitnessDirection::Feasible(cases) = plan.outcome() else {
        panic!("bounded factors should prove all 3x4 join matches: {plan:?}");
    };
    assert!(cases
        .iter()
        .any(|case| case.obligations().iter().any(|o| matches!(
            o,
            WitnessObligation::JoinPopulation {
                pattern: JoinPopulationPattern::CommonMatched,
                left_rows: 3,
                right_rows: 4,
                output_rows: 12,
                ..
            }
        ))));

    let filtered = bundle(
        &["SELECT l.a FROM l JOIN r ON l.k=r.k AND l.a > 0"],
        "postgresql",
    );
    let proof = physical_joint_source_plan(&filtered, &[(filtered.layers()[0].id(), 4)]);
    assert!(
        matches!(proof.outcome(), WitnessDirection::Feasible(_)),
        "typed non-key ON filters are carried by every complete source row"
    );

    let distinct = bundle(
        &["SELECT DISTINCT l.a FROM l JOIN r ON l.k=r.k"],
        "postgresql",
    );
    let proof = physical_joint_source_plan(&distinct, &[(distinct.layers()[0].id(), 4)]);
    assert!(
        !matches!(proof.outcome(), WitnessDirection::Feasible(_)),
        "DISTINCT row shaping may not be discarded"
    );

    let b = bundle(&["SELECT l.a FROM l CROSS JOIN r"], "postgresql");
    let p = physical_joint_source_plan(&b, &[(b.layers()[0].id(), 4)]);
    assert!(
        matches!(p.outcome(), WitnessDirection::Residual { .. }),
        "unsupported cross join-local evidence must not become feasible"
    );

    let conn = Connection::open_in_memory().expect("duckdb");
    conn.execute_batch(
        "CREATE TABLE l(a INTEGER,k INTEGER); CREATE TABLE r(a INTEGER,k INTEGER);
         INSERT INTO l VALUES (1,NULL),(2,NULL),(3,4);",
    )
    .expect("left-only input including SQL NULL");
    let left: i64 = conn
        .query_row("SELECT COUNT(*) FROM l LEFT JOIN r ON l.k=r.k", [], |row| {
            row.get(0)
        })
        .expect("left outer");
    let inner: i64 = conn
        .query_row("SELECT COUNT(*) FROM l JOIN r ON l.k=r.k", [], |row| {
            row.get(0)
        })
        .expect("inner");
    assert_eq!((left, inner), (3, 0));
    conn.execute_batch(
        "INSERT INTO r VALUES (7,0),(8,0);
         DELETE FROM l; INSERT INTO l VALUES (1,0),(2,0);",
    )
    .expect("duplicate-bag rows");
    let product: i64 = conn
        .query_row("SELECT COUNT(*) FROM l JOIN r ON l.k=r.k", [], |row| {
            row.get(0)
        })
        .expect("many-to-many count");
    assert_eq!(product, 4);
}
