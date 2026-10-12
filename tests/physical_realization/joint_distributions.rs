//! Joint, source-identity-aware distribution proof regressions.

use super::*;
use sql_semantic_protocol::physical_joint_distribution_plan;

fn buckets(column: &str, entries: &[(ConstraintValue, u64)]) -> OutputDistribution {
    OutputDistribution::new(
        column,
        entries
            .iter()
            .map(|(value, rows)| OutputValueCount::new(value.clone(), *rows))
            .collect(),
    )
    .expect("typed histogram")
}

type HistogramTarget<'a> = (usize, &'a str, Vec<(ConstraintValue, u64)>);

fn targets(bundle: &AnalysisBundle, rows: u64, input: &[HistogramTarget<'_>]) -> Vec<OutcomeGoal> {
    input
        .iter()
        .map(|(layer, column, values)| {
            OutcomeGoal::new(
                bundle.layers()[*layer].id(),
                Some(rows),
                None,
                vec![buckets(column, values)],
            )
            .expect("requested output")
        })
        .collect()
}

#[test]
fn three_correlated_terminals_share_one_closed_world_histogram() {
    for &dialect in DIALECTS {
        let typed = |relation: &str, column: &str| {
            RelationSchema::new(
                relation,
                vec![SchemaColumn::from_sql_type(column, "INTEGER", "postgresql").unwrap()],
            ).unwrap()
        };
        let b = bundle_with_schemas(
            &[
                "CREATE TABLE stage AS SELECT a AS x FROM t",
                "SELECT x AS first FROM stage",
                "SELECT x AS second FROM stage",
                "SELECT a AS third FROM t",
            ],
            dialect,
            &[typed("t", "a"), typed("stage", "x")],
        );
        let values = vec![(ConstraintValue::Null, 1), (ConstraintValue::Integer(4), 2)];
        let goals = targets(
            &b,
            3,
            &[
                (1, "first", values.clone()),
                (2, "second", values.clone()),
                (3, "third", values),
            ],
        );
        let proof = physical_joint_distribution_plan(&b, &goals);
        let WitnessDirection::Feasible(cases) = proof else {
            panic!("{dialect}: complete shared histogram must compose: {proof:?}");
        };
        assert_eq!(cases.len(), 1);
        let obligations = cases[0].obligations();
        let source_bags = obligations
            .iter()
            .filter_map(|item| {
                if let WitnessObligation::OperatorPopulation {
                    witness:
                        OutcomeWitness::SourceRows {
                            relation,
                            rows,
                            columns,
                        },
                } = item
                {
                    Some((relation, rows, columns))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(source_bags.len(), 1);
        assert_eq!((source_bags[0].0.as_str(), *source_bags[0].1), ("t", 3));
        assert_eq!(source_bags[0].2.len(), 1);
        assert_eq!(source_bags[0].2[0].column(), "a");
        assert_eq!(
            obligations
                .iter()
                .filter(|item| matches!(item, WitnessObligation::OutputRows { .. }))
                .count(),
            3
        );
        assert_eq!(
            obligations
                .iter()
                .filter(|item| matches!(item, WitnessObligation::ClosedWorld { .. }))
                .count(),
            1
        );
    }
    let duckdb = Connection::open_in_memory().expect("duckdb");
    duckdb
        .execute_batch(
            "CREATE TABLE t(a INTEGER);
         INSERT INTO t VALUES (4),(NULL),(4);
         CREATE TABLE stage AS SELECT a AS x FROM t;",
        )
        .expect("physical bag");
    for sql in [
        "SELECT x AS v FROM stage",
        "SELECT x AS v FROM stage",
        "SELECT a AS v FROM t",
    ] {
        let query = format!(
            "SELECT COUNT(*), COUNT(*) FILTER (WHERE v=4),
                    COUNT(*) FILTER (WHERE v IS NULL) FROM ({sql})"
        );
        let frequencies: (i64, i64, i64) = duckdb
            .query_row(&query, [], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })
            .expect("complete correlated output histogram");
        assert_eq!(frequencies, (3, 2, 1));
    }
}

#[test]
fn conflicting_shared_counts_and_buckets_are_impossible() {
    let b = bundle(
        &["SELECT a AS x FROM t", "SELECT a AS y FROM t"],
        "postgresql",
    );
    let values = vec![(ConstraintValue::Integer(5), 2)];
    let goals = targets(&b, 2, &[(0, "x", values.clone()), (1, "y", values.clone())]);
    assert!(matches!(
        physical_joint_distribution_plan(&b, &goals),
        WitnessDirection::Feasible(_)
    ));

    let contradictory = targets(
        &b,
        2,
        &[
            (0, "x", values),
            (1, "y", vec![(ConstraintValue::Integer(6), 2)]),
        ],
    );
    assert_eq!(
        physical_joint_distribution_plan(&b, &contradictory),
        WitnessDirection::Impossible
    );

    let different_counts = vec![
        OutcomeGoal::new(
            b.layers()[0].id(),
            Some(2),
            None,
            vec![buckets("x", &[(ConstraintValue::Integer(5), 2)])],
        )
        .unwrap(),
        OutcomeGoal::new(
            b.layers()[1].id(),
            Some(1),
            None,
            vec![buckets("y", &[(ConstraintValue::Integer(5), 1)])],
        )
        .unwrap(),
    ];
    assert_eq!(
        physical_joint_distribution_plan(&b, &different_counts),
        WitnessDirection::Impossible
    );
}

#[test]
fn disjoint_source_bags_and_columns_are_independently_reconciled() {
    for &dialect in DIALECTS {
        let b = bundle(
            &[
                "SELECT a AS x FROM t",
                "SELECT b AS y FROM t",
                "SELECT a AS z FROM l",
            ],
            dialect,
        );
        let goals = targets(
            &b,
            2,
            &[
                (0, "x", vec![(ConstraintValue::Integer(1), 2)]),
                (
                    1,
                    "y",
                    vec![
                        (ConstraintValue::Integer(10), 1),
                        (ConstraintValue::Null, 1),
                    ],
                ),
                (2, "z", vec![(ConstraintValue::Integer(20), 2)]),
            ],
        );
        let WitnessDirection::Feasible(cases) = physical_joint_distribution_plan(&b, &goals) else {
            panic!("{dialect}: independent complete physical populations must compose");
        };
        let populations = cases[0]
            .obligations()
            .iter()
            .filter_map(|item| {
                if let WitnessObligation::OperatorPopulation {
                    witness:
                        OutcomeWitness::SourceRows {
                            relation, columns, ..
                        },
                } = item
                {
                    Some((relation.as_str(), columns.len()))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(populations, vec![("l", 1), ("t", 2)]);
    }
}

#[test]
fn unsupported_filter_and_missing_schema_remain_residual() {
    let source = bundle(
        &["SELECT a AS x FROM t WHERE a > 0", "SELECT a AS y FROM t"],
        "postgresql",
    );
    let goals = targets(
        &source,
        1,
        &[
            (0, "x", vec![(ConstraintValue::Integer(2), 1)]),
            (1, "y", vec![(ConstraintValue::Integer(2), 1)]),
        ],
    );
    assert!(matches!(
        physical_joint_distribution_plan(&source, &goals),
        WitnessDirection::Residual { .. }
    ));

    let no_schema = {
        let dialect = dialect_from_name("postgresql").unwrap();
        let inputs = [SqlInput::inline("SELECT a AS x FROM t")];
        let configured = [ConfiguredSqlInput::new(
            "q0",
            &inputs[0],
            "postgresql",
            dialect.as_ref(),
        )];
        analyze_configured_inputs_with_catalog(
            &configured,
            &RelationCatalog::from_schemas(&[]).unwrap(),
        )
        .unwrap()
    };
    let goal = OutcomeGoal::new(
        no_schema.layers()[0].id(),
        Some(1),
        None,
        vec![buckets("x", &[(ConstraintValue::Integer(2), 1)])],
    )
    .unwrap();
    assert!(matches!(
        physical_joint_distribution_plan(&no_schema, &[goal]),
        WitnessDirection::Residual { .. }
    ));
}

#[test]
fn joint_histograms_reconcile_transparent_count_only_and_empty_goals() {
    let b = bundle(
        &["SELECT a AS x FROM t", "SELECT b AS y FROM t"],
        "postgresql",
    );
    let histogram_goal = OutcomeGoal::new(
        b.layers()[0].id(),
        Some(3),
        None,
        vec![buckets("x", &[(ConstraintValue::Null, 1), (ConstraintValue::Integer(3), 2)])],
    ).unwrap();
    let count = OutcomeGoal::new(b.layers()[1].id(), Some(3), None, Vec::new()).unwrap();
    let proof = physical_joint_distribution_plan(&b, &[histogram_goal.clone(), count]);
    let WitnessDirection::Feasible(cases) = proof else {
        panic!("transparent count must reuse complete histogram source: {proof:?}");
    };
    assert_eq!(
        cases[0].obligations().iter().filter(|obligation| {
            matches!(obligation, WitnessObligation::OperatorPopulation { .. })
        }).count(),
        1,
    );
    let conflict = OutcomeGoal::new(b.layers()[1].id(), Some(2), None, Vec::new()).unwrap();
    assert_eq!(
        physical_joint_distribution_plan(&b, &[histogram_goal, conflict]),
        WitnessDirection::Impossible,
    );

    let empty = [
        OutcomeGoal::new(
            b.layers()[0].id(),
            Some(0),
            None,
            vec![buckets("x", &[(ConstraintValue::Integer(1), 0)])],
        ).unwrap(),
        OutcomeGoal::new(
            b.layers()[1].id(),
            Some(0),
            None,
            vec![buckets("y", &[(ConstraintValue::Null, 0)])],
        ).unwrap(),
    ];
    assert!(matches!(
        physical_joint_distribution_plan(&b, &empty),
        WitnessDirection::Feasible(_),
    ));
}
