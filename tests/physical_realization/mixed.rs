//! mixed regression tests.

use super::*;

#[test]
fn common_matched_join_rows_make_one_multi_contributor_having_group() {
    let schema = |name: &str| {
        RelationSchema::new(
            name,
            vec![SchemaColumn::from_sql_type("k", "INTEGER", "postgresql").unwrap()],
        )
        .unwrap()
    };
    let schemas = [schema("l"), schema("r"), schema("joined")];
    let b = bundle_with_schemas(
        &[
            "CREATE TABLE joined AS SELECT l.k AS k FROM l INNER JOIN r ON l.k = r.k",
            "SELECT k, COUNT(*) AS n FROM joined GROUP BY k HAVING COUNT(*) >= 2",
        ],
        "postgresql",
        &schemas,
    );
    let plan = physical_joint_source_plan(&b, &[(b.layers()[1].id(), 1)]);
    let WitnessDirection::Feasible(cases) = plan.outcome() else {
        panic!("nontrivial join contribution should be constructible: {plan:?}");
    };
    assert!(cases.iter().any(|case| {
        case.obligations().iter().any(|o| {
            matches!(
                o,
                WitnessObligation::JoinPopulation {
                    pattern: JoinPopulationPattern::CommonMatched,
                    output_rows: 2,
                    ..
                }
            )
        }) && case
            .obligations()
            .iter()
            .any(|o| matches!(o, WitnessObligation::Group { .. }))
    }));

    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE l(k INTEGER); CREATE TABLE r(k INTEGER);
         INSERT INTO l VALUES (0),(0);
         INSERT INTO r VALUES (0);
         CREATE TABLE joined AS SELECT l.k AS k FROM l INNER JOIN r ON l.k = r.k;",
    )
    .unwrap();
    let count: i64 = db.query_row(
        "SELECT COUNT(*) FROM (SELECT k, COUNT(*) AS n FROM joined GROUP BY k HAVING COUNT(*) >= 2)",
        [],
        |row| row.get(0),
    ).unwrap();
    assert_eq!(count, 1);
}

#[test]
fn mixed_joined_group_and_source_terminals_share_join_key_assignments() {
    let schema = |name: &str| {
        RelationSchema::new(
            name,
            vec![SchemaColumn::from_sql_type("k", "INTEGER", "postgresql").unwrap()],
        )
        .unwrap()
    };
    let schemas = [schema("l"), schema("r"), schema("joined")];
    let b = bundle_with_schemas(
        &[
            "CREATE TABLE joined AS SELECT l.k AS k FROM l JOIN r ON l.k = r.k",
            "SELECT k, COUNT(*) AS n FROM joined GROUP BY k HAVING COUNT(*) >= 2",
            "SELECT k FROM l",
            "SELECT k FROM r",
        ],
        "postgresql",
        &schemas,
    );
    let ids = b
        .layers()
        .iter()
        .map(|layer| layer.id())
        .collect::<Vec<_>>();
    let compatible = physical_joint_source_plan(&b, &[(ids[1], 1), (ids[2], 2), (ids[3], 1)]);
    let WitnessDirection::Feasible(cases) = compatible.outcome() else {
        panic!("one complete join source assignment expected: {compatible:?}");
    };
    assert!(cases
        .iter()
        .any(|case| case.obligations().iter().any(|o| matches!(
            o,
            WitnessObligation::JoinPopulation {
                pattern: JoinPopulationPattern::CommonMatched,
                left_rows: 2,
                right_rows: 1,
                ..
            }
        ))));
    let unproven = physical_joint_source_plan(&b, &[(ids[1], 1), (ids[2], 3), (ids[3], 1)]);
    assert!(matches!(
        unproven.outcome(),
        WitnessDirection::Residual { .. }
    ));
}

#[test]
fn grouped_count_output_filter_proves_complete_positive_and_rejected_sources() {
    let schema = |name: &str, columns: &[(&str, &str)]| {
        RelationSchema::new(
            name,
            columns
                .iter()
                .map(|(column, ty)| SchemaColumn::from_sql_type(*column, ty, "postgresql").unwrap())
                .collect(),
        )
        .unwrap()
    };
    let schemas = [
        schema("t", &[("a", "INTEGER")]),
        schema("grouped", &[("a", "INTEGER"), ("n", "BIGINT")]),
    ];
    let producer =
        "CREATE TABLE grouped AS SELECT a, COUNT(*) AS n FROM t GROUP BY a HAVING COUNT(*) >= 2";
    let positive = "SELECT a FROM grouped WHERE n >= 2";
    for &dialect in DIALECTS {
        let b = bundle_with_schemas(&[producer, positive], dialect, &schemas);
        let id = b.layers()[1].id();
        let plan = physical_joint_source_plan(&b, &[(id, 2)]);
        let WitnessDirection::Feasible(cases) = plan.outcome() else {
            let query = b
                .inputs()
                .iter()
                .flat_map(|input| input.statements())
                .nth(1);
            panic!(
                "{dialect}: physical aggregate count filter not proven: {plan:?}; query: {query:?}"
            );
        };
        assert!(
            cases.iter().any(|case| case.obligations().iter().any(|o| {
                matches!(o, WitnessObligation::OperatorPopulation {
                    witness: OutcomeWitness::Groups {
                        relation, groups: 2, rows_per_group: 2, ..
                    }
                } if relation == "t")
            })),
            "{dialect}: closed-world group values must originate at t"
        );
    }
    let b = bundle_with_schemas(
        &[producer, "SELECT a FROM grouped WHERE n < 2"],
        "postgresql",
        &schemas,
    );
    let id = b.layers()[1].id();
    let rejected = physical_joint_source_plan(&b, &[(id, 0)]);
    let WitnessDirection::Feasible(cases) = rejected.outcome() else {
        panic!("nonempty rejected grouping must be provable: {rejected:?}");
    };
    assert!(cases.iter().any(|case| case.obligations().iter().any(|o| {
        matches!(o, WitnessObligation::OperatorPopulation {
            witness: OutcomeWitness::Groups {
                relation, groups: 1, rows_per_group: 2, ..
            }
        } if relation == "t")
    })));
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE t(a INTEGER); INSERT INTO t VALUES (0),(0),(1),(1);
         CREATE TABLE grouped AS SELECT a, COUNT(*) AS n FROM t GROUP BY a HAVING COUNT(*) >= 2;",
    )
    .unwrap();
    let pos: i64 = db
        .query_row("SELECT COUNT(*) FROM grouped WHERE n >= 2", [], |row| {
            row.get(0)
        })
        .unwrap();
    let neg: i64 = db
        .query_row("SELECT COUNT(*) FROM grouped WHERE n < 2", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!((pos, neg), (2, 0));
    let b = bundle_with_schemas(
        &[producer, "SELECT a FROM grouped WHERE n + 1 >= 2"],
        "postgresql",
        &schemas,
    );
    assert!(matches!(
        physical_joint_source_plan(&b, &[(b.layers()[1].id(), 2)]).outcome(),
        WitnessDirection::Residual { .. }
    ));
}

#[test]
fn empty_physical_subquery_producers_prove_correlated_positive_and_negative_membership() {
    let schema = |relation: &str, column: &str| {
        RelationSchema::new(
            relation,
            vec![SchemaColumn::from_sql_type(column, "INTEGER", "postgresql").unwrap()],
        )
        .unwrap()
    };
    let schemas = [
        schema("orders", "id"),
        schema("lines", "order_id"),
        schema("stage_orders", "id"),
        schema("stage_lines", "order_id"),
    ];
    let producers = [
        "CREATE TABLE stage_orders AS SELECT id FROM orders",
        "CREATE TABLE stage_lines AS SELECT order_id FROM lines",
    ];
    for (keyword, count) in [
        ("EXISTS", 0_u64),
        ("NOT EXISTS", 3),
        ("IN", 0),
        ("NOT IN", 3),
    ] {
        let query = match keyword {
            "EXISTS" | "NOT EXISTS" => format!(
                "SELECT o.id FROM stage_orders o WHERE {keyword} (SELECT 1 FROM stage_lines l WHERE l.order_id = o.id)"
            ),
            _ => format!(
                "SELECT o.id FROM stage_orders o WHERE o.id {keyword} (SELECT l.order_id FROM stage_lines l)"
            ),
        };
        let statements = [producers[0], producers[1], &query];
        let b = bundle_with_schemas(&statements, "postgresql", &schemas);
        let target = b.layers()[2].id();
        let plan = physical_joint_source_plan(&b, &[(target, count)]);
        let WitnessDirection::Feasible(cases) = plan.outcome() else {
            panic!("{keyword} physical emptiness was not proved: {plan:?}");
        };
        assert!(cases
            .iter()
            .any(|case| case.obligations().iter().any(|o| matches!(
                o,
                WitnessObligation::Membership {
                    case: sql_semantic_protocol::SubqueryMembershipCase::NoCandidates,
                    closed_world: true,
                    ..
                }
            ))));
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(
            "CREATE TABLE orders(id INTEGER);
             CREATE TABLE lines(order_id INTEGER);
             INSERT INTO orders VALUES (0),(1),(NULL);
             CREATE TABLE stage_orders AS SELECT id FROM orders;
             CREATE TABLE stage_lines AS SELECT order_id FROM lines;",
        )
        .unwrap();
        let found: i64 = db
            .query_row(&format!("SELECT COUNT(*) FROM ({query})"), [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(found as u64, count, "{keyword}");
    }

    let b = bundle_with_schemas(
        &[
            "CREATE TABLE stage_orders AS SELECT id FROM orders",
            "CREATE TABLE stage_lines AS SELECT id AS order_id FROM orders",
            "SELECT o.id FROM stage_orders o WHERE NOT EXISTS (SELECT 1 FROM stage_lines l WHERE l.order_id = o.id)",
        ],
        "postgresql",
        &schemas,
    );
    let same = physical_joint_source_plan(&b, &[(b.layers()[2].id(), 3)]);
    assert!(matches!(same.outcome(), WitnessDirection::Residual { .. }));
}

#[test]
fn joint_empty_subquery_and_outer_source_reads_share_single_closed_population() {
    let schema = |relation: &str, column: &str| {
        RelationSchema::new(
            relation,
            vec![SchemaColumn::from_sql_type(column, "INTEGER", "postgresql").unwrap()],
        )
        .unwrap()
    };
    let schemas = [
        schema("orders", "id"),
        schema("lines", "order_id"),
        schema("stage_orders", "id"),
        schema("stage_lines", "order_id"),
    ];
    let b = bundle_with_schemas(
        &[
            "CREATE TABLE stage_orders AS SELECT id FROM orders",
            "CREATE TABLE stage_lines AS SELECT order_id FROM lines",
            "SELECT o.id FROM stage_orders o WHERE NOT EXISTS (SELECT 1 FROM stage_lines l WHERE l.order_id = o.id)",
            "SELECT id FROM orders",
            "SELECT order_id FROM lines",
        ],
        "postgresql",
        &schemas,
    );
    let ids = b
        .layers()
        .iter()
        .map(|layer| layer.id())
        .collect::<Vec<_>>();
    let complete = physical_joint_source_plan(&b, &[(ids[2], 3), (ids[3], 3), (ids[4], 0)]);
    let WitnessDirection::Feasible(cases) = complete.outcome() else {
        panic!("joint closed-world membership should reuse outer rows: {complete:?}");
    };
    assert!(cases.iter().any(|case| {
        case.obligations()
            .iter()
            .filter(|o| matches!(o, WitnessObligation::OutputRows { .. }))
            .count()
            >= 3
    }));
    let incompatible = physical_joint_source_plan(&b, &[(ids[2], 3), (ids[3], 4), (ids[4], 0)]);
    assert!(matches!(
        incompatible.outcome(),
        WitnessDirection::Residual { .. }
    ));
}

#[test]
fn grouped_count_rejection_and_transparent_source_are_jointly_closed() {
    let schema = |relation: &str, columns: &[(&str, &str)]| {
        RelationSchema::new(
            relation,
            columns
                .iter()
                .map(|(name, ty)| SchemaColumn::from_sql_type(*name, ty, "postgresql").unwrap())
                .collect(),
        )
        .unwrap()
    };
    let schemas = [
        schema("t", &[("a", "INTEGER")]),
        schema("grouped", &[("a", "INTEGER"), ("n", "BIGINT")]),
    ];
    let b = bundle_with_schemas(
        &[
            "CREATE TABLE grouped AS SELECT a, COUNT(*) AS n FROM t GROUP BY a HAVING COUNT(*) >= 2",
            "SELECT a FROM grouped WHERE n < 2",
            "SELECT a FROM t",
        ],
        "postgresql",
        &schemas,
    );
    let ids = b
        .layers()
        .iter()
        .map(|layer| layer.id())
        .collect::<Vec<_>>();
    let plan = physical_joint_source_plan(&b, &[(ids[1], 0), (ids[2], 2)]);
    assert!(
        matches!(plan.outcome(), WitnessDirection::Feasible(_)),
        "{plan:?}"
    );
    let mismatch = physical_joint_source_plan(&b, &[(ids[1], 0), (ids[2], 3)]);
    assert!(matches!(
        mismatch.outcome(),
        WitnessDirection::Residual { .. }
    ));
}

#[test]
fn identical_set_producer_branches_share_one_complete_physical_population() {
    let schema = |relation: &str, columns: &[&str]| {
        RelationSchema::new(
            relation,
            columns
                .iter()
                .map(|column| {
                    SchemaColumn::from_sql_type(*column, "INTEGER", "postgresql").unwrap()
                })
                .collect(),
        )
        .unwrap()
    };
    let schemas = [
        schema("t", &["a", "b"]),
        schema("left_stage", &["id"]),
        schema("right_stage", &["id"]),
    ];
    for dialect in ["duckdb", "postgresql", "generic"] {
        for operator in ["UNION", "INTERSECT", "INTERSECT ALL"] {
            let statements = [
                "CREATE TABLE left_stage AS SELECT a AS id FROM t".to_string(),
                "CREATE TABLE right_stage AS SELECT a AS id FROM t".to_string(),
                format!("SELECT id FROM left_stage {operator} SELECT id FROM right_stage"),
            ];
            let queries = statements.iter().map(String::as_str).collect::<Vec<_>>();
            let mut b = bundle_with_schemas(&queries, dialect, &schemas);
            let target = b.layers()[2].id().to_string();
            b.set_outcome_goals(&[OutcomeGoal::new(&target, Some(2), None, vec![]).unwrap()])
                .unwrap();
            let local = b.outcome_goals()[0].witness();
            let Some(OutcomeWitness::SetTuples { case, .. }) = local else {
                panic!("{dialect} {operator}: missing shared-source set proof: {local:?}");
            };
            let obligations = case.obligations();
            assert_eq!(obligations.len(), 2);
            assert!(obligations.iter().all(|o| {
                o.boundary().relation() == "t"
                    && o.boundary().tuple_columns() == ["a"]
                    && o.matching_tuple_count() == obligations[0].matching_tuple_count()
            }));
            let plan = physical_row_count_plan(&b, &target, 2);
            let WitnessDirection::Feasible(cases) = &plan else {
                panic!("{dialect} {operator}: expected physical case: {plan:?}");
            };
            assert!(cases.iter().any(|case| {
                case.obligations()
                    .iter()
                    .filter(|o| {
                        matches!(
                            o,
                            WitnessObligation::Rows { boundary, closed_world: true, .. }
                                if boundary.relation() == "t"
                        )
                    })
                    .count()
                    == 1
            }));
            // One source is populated once, and both materializations read
            // those same rows. Emitting independent branch populations here
            // would silently double the real input.
            let db = Connection::open_in_memory().unwrap();
            db.execute_batch("CREATE TABLE t(a INTEGER, b INTEGER);")
                .unwrap();
            for key in 0..2 {
                for _ in 0..obligations[0].matching_tuple_count() {
                    db.execute_batch(&format!("INSERT INTO t VALUES ({key}, 99);"))
                        .unwrap();
                }
            }
            db.execute_batch(
                "CREATE TABLE left_stage AS SELECT a AS id FROM t;
                 CREATE TABLE right_stage AS SELECT a AS id FROM t;",
            )
            .unwrap();
            let count: i64 = db
                .query_row(
                    &format!(
                        "SELECT COUNT(*) FROM (SELECT id FROM left_stage {operator} SELECT id FROM right_stage)"
                    ),
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(count, 2, "{dialect} {operator}");
        }
    }
}

#[test]
fn shared_union_all_producers_reuse_one_physical_bag_with_even_output_counts() {
    let schema = |name: &str, column: &str| {
        RelationSchema::new(
            name,
            vec![SchemaColumn::from_sql_type(column, "INTEGER", "postgresql").unwrap()],
        )
        .unwrap()
    };
    let schemas = [
        schema("t", "a"),
        schema("left_stage", "id"),
        schema("right_stage", "id"),
    ];
    let statements = [
        "CREATE TABLE left_stage AS SELECT a AS id FROM t",
        "CREATE TABLE right_stage AS SELECT a AS id FROM t",
        "SELECT id FROM left_stage UNION ALL SELECT id FROM right_stage",
    ];
    for dialect in ["duckdb", "postgresql", "generic"] {
        let mut b = bundle_with_schemas(&statements, dialect, &schemas);
        let target = b.layers()[2].id().to_string();
        for rows in [2_u64, 4, 6] {
            b.set_outcome_goals(&[OutcomeGoal::new(&target, Some(rows), None, vec![]).unwrap()])
                .unwrap();
            let Some(OutcomeWitness::SetTuples { tuples, case, values, .. }) =
                b.outcome_goals()[0].witness()
            else {
                panic!("{dialect} {rows}: missing constructive shared bag witness");
            };
            assert_eq!(*tuples, rows / 2);
            assert!(values.is_empty());
            assert_eq!(case.output_tuple_count(), 2);
            assert_eq!(case.obligations().len(), 2);
            assert!(case.obligations().iter().all(|o| {
                o.boundary().relation() == "t"
                    && o.boundary().tuple_columns() == ["a"]
                    && o.matching_tuple_count() == 1
            }));
            let plan = physical_row_count_plan(&b, &target, rows);
            let WitnessDirection::Feasible(cases) = plan else {
                panic!("{dialect} {rows}: expected complete physical plan: {plan:?}");
            };
            assert!(cases.iter().any(|case| {
                case.obligations().iter().filter(|o| {
                    matches!(
                        o,
                        WitnessObligation::Rows {
                            boundary,
                            bounds,
                            closed_world: true,
                            ..
                        } if boundary.relation() == "t"
                            && bounds.minimum() == rows / 2
                            && bounds.maximum() == Some(rows / 2)
                    )
                }).count() == 1
            }));
            let db = Connection::open_in_memory().unwrap();
            db.execute_batch("CREATE TABLE t(a INTEGER);").unwrap();
            for key in 0..rows / 2 {
                db.execute_batch(&format!("INSERT INTO t VALUES ({key});")).unwrap();
            }
            db.execute_batch(
                "CREATE TABLE left_stage AS SELECT a AS id FROM t;
                 CREATE TABLE right_stage AS SELECT a AS id FROM t;",
            ).unwrap();
            let (total, distinct): (i64, i64) = db.query_row(
                "SELECT COUNT(*), COUNT(DISTINCT id) FROM
                 (SELECT id FROM left_stage UNION ALL SELECT id FROM right_stage)",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            ).unwrap();
            assert_eq!((total, distinct), (rows as i64, (rows / 2) as i64));
        }
    }
}

#[test]
fn shared_set_sources_with_different_columns_or_bag_laws_remain_residual() {
    let schema = |relation: &str, columns: &[&str]| {
        RelationSchema::new(
            relation,
            columns
                .iter()
                .map(|column| {
                    SchemaColumn::from_sql_type(*column, "INTEGER", "postgresql").unwrap()
                })
                .collect(),
        )
        .unwrap()
    };
    let schemas = [
        schema("t", &["a", "b"]),
        schema("left_stage", &["id"]),
        schema("right_stage", &["id"]),
    ];
    for (right_projection, operator, rows) in [("b", "INTERSECT", 2), ("a", "UNION ALL", 3), ("b", "UNION", 2), ("a", "EXCEPT ALL", 2)] {
        let statements = [
            "CREATE TABLE left_stage AS SELECT a AS id FROM t".to_string(),
            format!("CREATE TABLE right_stage AS SELECT {right_projection} AS id FROM t"),
            format!("SELECT id FROM left_stage {operator} SELECT id FROM right_stage"),
        ];
        let queries = statements.iter().map(String::as_str).collect::<Vec<_>>();
        let mut b = bundle_with_schemas(&queries, "duckdb", &schemas);
        let target = b.layers()[2].id().to_string();
        b.set_outcome_goals(&[OutcomeGoal::new(&target, Some(rows), None, vec![]).unwrap()])
            .unwrap();
        assert_eq!(
            b.outcome_goals()[0].status(),
            OutcomeGoalStatus::Residual,
            "{right_projection} {operator}"
        );
        assert!(
            matches!(
                physical_row_count_plan(&b, &target, rows),
                WitnessDirection::Residual { .. }
            ),
            "{right_projection} {operator}"
        );
    }
}

#[test]
fn independent_join_group_dags_compose_with_transparent_terminal_counts() {
    let schema = |name: &str| {
        RelationSchema::new(
            name,
            vec![SchemaColumn::from_sql_type("k", "INTEGER", "postgresql").unwrap()],
        )
        .unwrap()
    };
    let schemas = [
        schema("l1"),
        schema("r1"),
        schema("l2"),
        schema("r2"),
        schema("joined1"),
        schema("joined2"),
    ];
    let statements = [
        "CREATE TABLE joined1 AS SELECT l1.k AS k FROM l1 JOIN r1 ON l1.k = r1.k",
        "SELECT k, COUNT(*) AS n FROM joined1 GROUP BY k HAVING COUNT(*) >= 2",
        "CREATE TABLE joined2 AS SELECT l2.k AS k FROM l2 JOIN r2 ON l2.k = r2.k",
        "SELECT k, COUNT(*) AS n FROM joined2 GROUP BY k HAVING COUNT(*) >= 2",
        "SELECT k FROM l1",
    ];
    let b = bundle_with_schemas(&statements, "duckdb", &schemas);
    let ids = b
        .layers()
        .iter()
        .map(|layer| layer.id())
        .collect::<Vec<_>>();
    let plan = physical_joint_source_plan(&b, &[(ids[1], 1), (ids[3], 1), (ids[4], 2)]);
    let WitnessDirection::Feasible(cases) = plan.outcome() else {
        panic!("disjoint mixed operator DAGs should compose: {plan:?}");
    };
    assert_eq!(plan.sources().len(), 4);
    assert!(cases.iter().any(|case| {
        case.obligations()
            .iter()
            .filter(|o| matches!(o, WitnessObligation::JoinPopulation { .. }))
            .count()
            == 2
            && case
                .obligations()
                .iter()
                .filter(|o| matches!(o, WitnessObligation::OutputRows { .. }))
                .count()
                >= 3
    }));

    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE l1(k INTEGER); CREATE TABLE r1(k INTEGER);
         CREATE TABLE l2(k INTEGER); CREATE TABLE r2(k INTEGER);
         INSERT INTO l1 VALUES (0), (0); INSERT INTO r1 VALUES (0);
         INSERT INTO l2 VALUES (2), (2); INSERT INTO r2 VALUES (2);
         CREATE TABLE joined1 AS SELECT l1.k AS k FROM l1 JOIN r1 ON l1.k = r1.k;
         CREATE TABLE joined2 AS SELECT l2.k AS k FROM l2 JOIN r2 ON l2.k = r2.k;",
    )
    .unwrap();
    for table in ["joined1", "joined2"] {
        let count: i64 = db
            .query_row(
                &format!(
                    "SELECT COUNT(*) FROM (SELECT k, COUNT(*) AS n FROM {table} GROUP BY k HAVING COUNT(*) >= 2)"
                ),
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "{table}");
    }
    let source_rows: i64 = db
        .query_row("SELECT COUNT(*) FROM l1", [], |row| row.get(0))
        .unwrap();
    assert_eq!(source_rows, 2);

    let overlapping = [
        statements[0],
        statements[1],
        "CREATE TABLE joined2 AS SELECT l1.k AS k FROM l1 JOIN r2 ON l1.k = r2.k",
        statements[3],
        statements[4],
    ];
    let shared = bundle_with_schemas(&overlapping, "duckdb", &schemas);
    let ids = shared
        .layers()
        .iter()
        .map(|layer| layer.id())
        .collect::<Vec<_>>();
    let unresolved = physical_joint_source_plan(&shared, &[(ids[1], 1), (ids[3], 1)]);
    assert!(matches!(
        unresolved.outcome(),
        WitnessDirection::Residual { .. }
    ));
}
