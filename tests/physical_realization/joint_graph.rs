//! joint graph regression tests.

use super::*;

#[test]
fn joint_plan_deduplicates_physical_dag_and_is_order_invariant() {
    for &dialect in DIALECTS {
        let mut b = bundle(
            &[
                "CREATE TABLE stage AS SELECT a, b FROM t",
                "CREATE TABLE mart AS SELECT a, b FROM stage",
                "SELECT a FROM mart",
                "SELECT b FROM stage",
            ],
            dialect,
        );
        let requests = [(b.layers()[2].id(), 3), (b.layers()[3].id(), 3)];
        let original = physical_joint_source_plan(&b, &requests);
        let reversed = physical_joint_source_plan(&b, &[requests[1], requests[0]]);
        assert_eq!(
            original, reversed,
            "{dialect}: input order cannot change the proof"
        );
        assert!(matches!(original.outcome(), WitnessDirection::Feasible(_)));
        assert_eq!(original.sources(), &["t".to_string()]);
        assert_eq!(
            original.nodes().len(),
            5,
            "{dialect}: source and four unique layers"
        );
        assert_eq!(
            original.nodes()[0].id(),
            &PhysicalPlanRef::Source("t".to_string())
        );
        assert_eq!(original.gap(), None);

        let goals = requests
            .iter()
            .map(|(layer_id, rows)| {
                OutcomeGoal::new(*layer_id, Some(*rows), None, vec![])
                    .expect("row-only requested goal")
            })
            .collect::<Vec<_>>();
        b.set_outcome_goals(&goals).expect("valid goals");
        let json: serde_json::Value =
            serde_json::from_str(&sql_semantic_protocol::to_bundle_json(&b))
                .expect("emitted protocol");
        let joint = &json["graph"]["physical_joint_count_plan"];
        assert_eq!(joint["outcome"]["status"], "feasible");
        assert_eq!(joint["physical_sources"], serde_json::json!(["t"]));
        assert_eq!(joint["node_refs"].as_array().map(Vec::len), Some(5));
        assert_eq!(joint["targets"].as_array().map(Vec::len), Some(2));
        assert!(joint["gap"].is_null());
    }
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../schema/protocol.schema.json"))
            .expect("active protocol schema");
    assert_eq!(
        schema["$defs"]["graph"]["properties"]["physical_joint_count_plan"]["$ref"],
        "#/$defs/physicalJointCountPlan"
    );
}

#[test]
fn joint_plan_keeps_cycles_and_ambiguous_writers_typed_and_residual() {
    let cyclic = bundle(
        &[
            "CREATE TABLE first AS SELECT a FROM second",
            "CREATE TABLE second AS SELECT a FROM first",
        ],
        "postgresql",
    );
    let plan = physical_joint_source_plan(&cyclic, &[(cyclic.layers()[0].id(), 2)]);
    assert_eq!(plan.gap(), Some(PhysicalProofGap::Cycle));
    assert!(matches!(plan.outcome(), WitnessDirection::Residual { .. }));

    let ambiguous = bundle(
        &[
            "CREATE TABLE stage AS SELECT a FROM t",
            "CREATE TABLE stage AS SELECT a FROM r",
            "SELECT a FROM stage",
        ],
        "postgresql",
    );
    let plan = physical_joint_source_plan(&ambiguous, &[(ambiguous.layers()[2].id(), 2)]);
    assert_eq!(plan.gap(), Some(PhysicalProofGap::AmbiguousProducer));
    assert!(matches!(plan.outcome(), WitnessDirection::Residual { .. }));
}

#[test]
fn joint_plan_handles_repeated_source_aliases_and_duplicate_physical_rows() {
    for &dialect in DIALECTS {
        let b = bundle(
            &[
                "SELECT x.a FROM t AS x",
                "SELECT y.a FROM t AS y",
                "SELECT z.a FROM r AS z",
            ],
            dialect,
        );
        let requested = [
            (b.layers()[0].id(), 3),
            (b.layers()[1].id(), 3),
            (b.layers()[2].id(), 2),
        ];
        let plan = physical_joint_source_plan(&b, &requested);
        assert_eq!(plan.sources(), &["r".to_string(), "t".to_string()]);
        assert!(
            matches!(plan.outcome(), WitnessDirection::Feasible(_)),
            "{dialect}: aliases preserve one shared source identity: {plan:?}"
        );
        let WitnessDirection::Feasible(cases) = plan.outcome() else {
            unreachable!("asserted feasible");
        };
        assert_eq!(
            cases[0]
                .obligations()
                .iter()
                .filter(|item| matches!(item, WitnessObligation::ClosedWorld { .. }))
                .count(),
            2
        );
    }
    let conn = Connection::open_in_memory().expect("duckdb");
    conn.execute_batch(
        "CREATE TABLE t(a INTEGER); CREATE TABLE r(a INTEGER);
         INSERT INTO t VALUES (1), (1), (NULL);
         INSERT INTO r VALUES (2), (2);",
    )
    .expect("duplicate and NULL inputs");
    for (sql, expected) in [
        ("SELECT COUNT(*) FROM t AS x", 3),
        ("SELECT COUNT(*) FROM t AS y", 3),
        ("SELECT COUNT(*) FROM r AS z", 2),
    ] {
        let actual: i64 = conn.query_row(sql, [], |row| row.get(0)).expect("oracle");
        assert_eq!(actual, expected);
    }
}

#[test]
fn correlated_self_join_witnesses_are_retained_but_not_jointly_assumed() {
    for &dialect in DIALECTS {
        let b = bundle(
            &[
                "SELECT l.a FROM t AS l JOIN t AS r ON l.k = r.k",
                "SELECT a FROM t",
            ],
            dialect,
        );
        let plan =
            physical_joint_source_plan(&b, &[(b.layers()[0].id(), 4), (b.layers()[1].id(), 3)]);
        assert_eq!(plan.sources(), &["t".to_string()]);
        assert_eq!(
            plan.gap(),
            Some(PhysicalProofGap::UnprovedCrossRowCorrelation)
        );
        assert!(
            matches!(
                plan.outcome(),
                WitnessDirection::Residual { reason } if reason == "unproved_cross_row_correlation"
            ),
            "{dialect}: matching pairs need an explicit shared-row/multiplicity proof"
        );
        assert!(
            plan.nodes()
                .iter()
                .any(|node| !node.operator_witnesses().is_empty()),
            "{dialect}: original local witness should not be discarded"
        );
    }

    let conn = Connection::open_in_memory().expect("duckdb");
    conn.execute_batch(
        "CREATE TABLE t(a INTEGER,k INTEGER);
         INSERT INTO t VALUES (1,1),(1,1),(NULL,NULL);",
    )
    .expect("duplicate and SQL NULL key rows");
    let (source_rows, pairs): (i64, i64) = conn
        .query_row(
            "SELECT (SELECT COUNT(*) FROM t),
                    (SELECT COUNT(*) FROM t AS l JOIN t AS r ON l.k = r.k)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("join cardinality");
    assert_eq!((source_rows, pairs), (3, 4));
}

#[test]
fn joint_plan_does_not_upgrade_missing_schema_or_partial_producers() {
    let unknown = bundle_with_schemas(&["SELECT a FROM t", "SELECT b FROM t"], "postgresql", &[]);
    let unknown_plan = physical_joint_source_plan(
        &unknown,
        &[(unknown.layers()[0].id(), 2), (unknown.layers()[1].id(), 2)],
    );
    assert!(matches!(
        unknown_plan.outcome(),
        WitnessDirection::Residual { .. }
    ));
    assert_eq!(unknown_plan.sources(), &["t".to_string()]);

    let partial = bundle(
        &[
            "INSERT INTO stage SELECT a, b FROM t",
            "SELECT a FROM stage",
        ],
        "postgresql",
    );
    let plan = physical_joint_source_plan(&partial, &[(partial.layers()[1].id(), 2)]);
    assert_eq!(plan.gap(), Some(PhysicalProofGap::PartialProducer));
    assert!(matches!(plan.outcome(), WitnessDirection::Residual { .. }));
    assert!(plan.nodes().iter().any(|node| node.write_kind().is_some()));
}

#[test]
fn joint_terminal_goals_share_physical_rows_once_and_detect_conflicting_counts() {
    for &dialect in DIALECTS {
        let b = bundle(
            &[
                "CREATE TABLE stage AS SELECT a, b FROM t",
                "CREATE TABLE mart AS SELECT a, b FROM stage",
                "SELECT a FROM stage",
                "SELECT b FROM mart",
            ],
            dialect,
        );
        let outputs = [(b.layers()[2].id(), 3), (b.layers()[3].id(), 3)];
        let witness = physical_joint_row_count_plan(&b, &outputs);
        let WitnessDirection::Feasible(cases) = witness else {
            panic!("{dialect}: jointly consistent physical source goals: {witness:?}");
        };
        assert_eq!(cases.len(), 1);
        assert_eq!(
            cases[0]
                .obligations()
                .iter()
                .filter(|o| matches!(
                    o, WitnessObligation::Rows { boundary, .. } if boundary.relation() == "t"
                ))
                .count(),
            1,
            "shared physical input should be constructed once",
        );
        assert_eq!(
            cases[0]
                .obligations()
                .iter()
                .filter(|o| matches!(o, WitnessObligation::OutputRows { .. }))
                .count(),
            2,
        );
        assert!(matches!(
            physical_joint_row_count_plan(&b, &[(b.layers()[2].id(), 3), (b.layers()[3].id(), 4),]),
            WitnessDirection::Impossible,
        ));
    }
}

#[test]
fn independent_physical_sources_can_satisfy_joint_row_targets() {
    let b = bundle(&["SELECT a FROM t", "SELECT a FROM r"], "postgresql");
    let WitnessDirection::Feasible(cases) =
        physical_joint_row_count_plan(&b, &[(b.layers()[0].id(), 2), (b.layers()[1].id(), 7)])
    else {
        panic!("two independent, unconstrained physical source counts should be satisfiable");
    };
    assert_eq!(cases.len(), 1);
    assert_eq!(
        cases[0]
            .obligations()
            .iter()
            .filter(|obligation| matches!(obligation, WitnessObligation::ClosedWorld { .. }))
            .count(),
        2,
    );
}

#[test]
fn positive_unfiltered_and_zero_filtered_targets_share_nonempty_rejected_source() {
    for &dialect in DIALECTS {
        let b = bundle(
            &["SELECT a FROM t WHERE a > 2 OR b < 0", "SELECT a FROM t"],
            dialect,
        );
        let zero = b.layers()[0].id();
        let positive = b.layers()[1].id();
        for targets in [[(zero, 0), (positive, 3)], [(positive, 3), (zero, 0)]] {
            let proof = physical_joint_row_count_plan(&b, &targets);
            let WitnessDirection::Feasible(cases) = proof else {
                panic!("{dialect}: expected shared physical rejection: {proof:?}");
            };
            assert_eq!(cases.len(), 1);
            assert_eq!(
                cases[0]
                    .obligations()
                    .iter()
                    .filter(|o| matches!(
                        o,
                        WitnessObligation::Rows {
                            predicate: WitnessFormula::RowTruth {
                                truth: sql_semantic_protocol::BooleanTruthCase::NotTrue,
                                ..
                            },
                            bounds,
                            ..
                        } if bounds.minimum() == 3 && bounds.maximum() == Some(3)
                    ))
                    .count(),
                1
            );
            assert_eq!(
                cases[0]
                    .obligations()
                    .iter()
                    .filter(|o| matches!(o, WitnessObligation::OutputRows { .. }))
                    .count(),
                2
            );
        }
    }
    let conn = Connection::open_in_memory().expect("duckdb");
    conn.execute_batch(
        "CREATE TABLE t(a INTEGER,b INTEGER);
         INSERT INTO t VALUES (0,0),(NULL,NULL),(1,1);",
    )
    .expect("three rejected rows");
    let (zero, positive): (i64, i64) = conn
        .query_row(
            "SELECT (SELECT COUNT(*) FROM t WHERE a > 2 OR b < 0),
                    (SELECT COUNT(*) FROM t)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("joint physical oracle");
    assert_eq!((zero, positive), (0, 3));
}
