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
fn complete_rank_partitions_prove_exact_selected_counts_across_producers() {
    let schema = |relation: &str, names: &[&str]| {
        RelationSchema::new(
            relation,
            names.iter()
                .map(|name| SchemaColumn::from_sql_type(*name, "INTEGER", "postgresql").unwrap())
                .collect()
        ).unwrap()
    };
    let schemas = [
        schema("t", &["a", "b"]),
        schema("stage", &["grp", "score"]),
        schema("mart", &["group_id", "sort_value"]),
    ];
    let prefix = [
        "CREATE TABLE stage AS SELECT a AS grp, b AS score FROM t",
        "CREATE TABLE mart AS SELECT grp AS group_id, score AS sort_value FROM stage",
    ];
    for dialect in DIALECTS {
        for (limit, wanted, partitions, per_partition) in [
            (1_u64, 2_u64, 2_u64, 2_u64),
            (2, 4, 2, 3),
            (0, 0, 1, 1),
        ] {
            let query = format!(
                "SELECT group_id, ROW_NUMBER() OVER \
                 (PARTITION BY group_id ORDER BY sort_value ASC NULLS LAST) AS rn \
                 FROM mart QUALIFY rn <= {limit}"
            );
            let b = bundle_with_schemas(&[prefix[0], prefix[1], &query], dialect, &schemas);
            let target = b.layers()[2].id();
            let plan = physical_joint_source_plan(&b, &[(target, wanted)]);
            let WitnessDirection::Feasible(cases) = plan.outcome() else {
                panic!("{dialect} limit={limit}: expected complete rank proof: {plan:?}");
            };
            assert!(cases.iter().any(|case| case.obligations().iter().any(|o| {
                matches!(
                    o,
                    WitnessObligation::RankPopulation {
                        boundary,
                        partitions: p,
                        rows_per_partition: n,
                        rank_limit: cap,
                        closed_world: true,
                        ..
                    } if boundary.relation() == "t"
                        && *p == partitions && *n == per_partition && *cap == limit
                )
            })), "{dialect} limit={limit}: missing complete rank law: {plan:?}");

            let db = Connection::open_in_memory().unwrap();
            db.execute_batch("CREATE TABLE t(a INTEGER, b INTEGER);").unwrap();
            for partition in 0..partitions {
                for order in 0..per_partition {
                    db.execute_batch(&format!(
                        "INSERT INTO t VALUES ({partition},{order});"
                    )).unwrap();
                }
            }
            db.execute_batch(&format!("{};{};", prefix[0], prefix[1])).unwrap();
            let mut stmt = db.prepare(&format!(
                "SELECT group_id, rn FROM ({query}) ORDER BY group_id, rn"
            )).unwrap();
            let actual = stmt.query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
            }).unwrap().collect::<Result<Vec<_>, _>>().unwrap();
            let expected = (0..partitions).flat_map(|partition| {
                (1..=limit.min(per_partition))
                    .map(move |rank| (partition as i64, rank as i64))
            }).collect::<Vec<_>>();
            assert_eq!(actual, expected, "{dialect} limit={limit}");
        }
    }
}

#[test]
fn grouped_join_key_populations_preserve_contributor_bags_through_having() {
    let schema = |relation: &str| {
        RelationSchema::new(
            relation,
            vec![SchemaColumn::from_sql_type("k", "INTEGER", "postgresql").unwrap()],
        )
        .unwrap()
    };
    let schemas = [schema("l"), schema("r"), schema("joined")];
    for dialect in DIALECTS {
        let b = bundle_with_schemas(
            &[
                "CREATE TABLE joined AS SELECT l.k AS k FROM l INNER JOIN r ON l.k = r.k",
                "SELECT k, COUNT(*) AS n FROM joined GROUP BY k HAVING COUNT(*) >= 2",
            ],
            dialect,
            &schemas,
        );
        let goal = b.layers()[1].id();
        let plan = physical_joint_source_plan(&b, &[(goal, 2)]);
        let WitnessDirection::Feasible(cases) = plan.outcome() else {
            panic!("{dialect}: missing grouped-contributor population: {plan:?}");
        };
        assert!(
            cases.iter().any(|case| {
                case.obligations().iter().any(|o| {
                    matches!(
                        o,
                        WitnessObligation::JoinPopulation {
                            pattern: JoinPopulationPattern::GroupedMatched { groups: 2 },
                            left_rows: 4,
                            right_rows: 2,
                            output_rows: 4,
                            closed_world: true,
                            ..
                        }
                    )
                }) && case.obligations().iter().any(|o| {
                    matches!(o, WitnessObligation::Group {
                    rows,
                    ..
                } if rows.minimum() == 2 && rows.maximum() == Some(2))
                })
            }),
            "{dialect}: source contributors and exact group counts must agree"
        );

        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(
            "CREATE TABLE l(k INTEGER); CREATE TABLE r(k INTEGER);
             INSERT INTO l VALUES (0),(0),(1),(1);
             INSERT INTO r VALUES (0),(1);
             CREATE TABLE joined AS SELECT l.k AS k FROM l JOIN r ON l.k=r.k;",
        )
        .unwrap();
        let mut stmt = db
            .prepare(
                "SELECT k, n FROM
             (SELECT k, COUNT(*) AS n FROM joined GROUP BY k HAVING COUNT(*) >= 2)
             ORDER BY k",
            )
            .unwrap();
        let rows = stmt
            .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(rows, vec![(0, 2), (1, 2)], "{dialect}");
    }
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
fn aggregate_count_composed_filters_use_three_valued_boolean_expressions() {
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
    let producer =
        "CREATE TABLE grouped AS SELECT a, COUNT(*) AS n FROM t GROUP BY a HAVING COUNT(*) >= 2";
    for dialect in DIALECTS {
        for (filter, wanted) in [
            ("n >= 2 AND n < 3", 2_u64),
            ("n < 2 OR n >= 3", 0),
            ("NOT (n < 2)", 2),
            ("2 <= n", 2),
        ] {
            let query = format!("SELECT a FROM grouped WHERE {filter}");
            let b = bundle_with_schemas(&[producer, &query], dialect, &schemas);
            let plan = physical_joint_source_plan(&b, &[(b.layers()[1].id(), wanted)]);
            assert!(
                matches!(plan.outcome(), WitnessDirection::Feasible(_)),
                "{dialect} {filter}: expected complete aggregate filter proof: {plan:?}"
            );
            let db = Connection::open_in_memory().unwrap();
            db.execute_batch(
                "CREATE TABLE t(a INTEGER);
                 INSERT INTO t VALUES (0),(0),(1),(1);
                 CREATE TABLE grouped AS SELECT a, COUNT(*) AS n FROM t GROUP BY a HAVING COUNT(*) >= 2;"
            ).unwrap();
            let actual: i64 = db
                .query_row(&format!("SELECT COUNT(*) FROM ({query})"), [], |row| {
                    row.get(0)
                })
                .unwrap();
            assert_eq!(actual as u64, wanted, "{dialect} {filter}");
        }
    }
    // No other expression is assumed to be COUNT(*) or an exact Boolean
    // combination of it, even if DuckDB could evaluate that expression.
    let b = bundle_with_schemas(
        &[producer, "SELECT a FROM grouped WHERE n + 1 >= 3"],
        "postgresql",
        &schemas,
    );
    let plan = physical_joint_source_plan(&b, &[(b.layers()[1].id(), 2)]);
    assert!(matches!(plan.outcome(), WitnessDirection::Residual { .. }));
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
            let Some(OutcomeWitness::SetTuples {
                tuples,
                case,
                values,
                ..
            }) = b.outcome_goals()[0].witness()
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
                case.obligations()
                    .iter()
                    .filter(|o| {
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
                    })
                    .count()
                    == 1
            }));
            let db = Connection::open_in_memory().unwrap();
            db.execute_batch("CREATE TABLE t(a INTEGER);").unwrap();
            for key in 0..rows / 2 {
                db.execute_batch(&format!("INSERT INTO t VALUES ({key});"))
                    .unwrap();
            }
            db.execute_batch(
                "CREATE TABLE left_stage AS SELECT a AS id FROM t;
                 CREATE TABLE right_stage AS SELECT a AS id FROM t;",
            )
            .unwrap();
            let (total, distinct): (i64, i64) = db
                .query_row(
                    "SELECT COUNT(*), COUNT(DISTINCT id) FROM
                 (SELECT id FROM left_stage UNION ALL SELECT id FROM right_stage)",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap();
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
    for (right_projection, operator, rows) in [
        ("b", "INTERSECT", 2),
        ("a", "UNION ALL", 3),
        ("b", "UNION", 2),
        ("a", "EXCEPT ALL", 2),
    ] {
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

#[test]
fn nonempty_uncorrelated_exists_and_not_exists_prove_complete_physical_populations() {
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
    for &dialect in DIALECTS {
        for (keyword, wanted) in [("EXISTS", 3_u64), ("NOT EXISTS", 0)] {
            let query = format!(
                "SELECT o.id FROM stage_orders o WHERE {keyword} (SELECT 1 FROM stage_lines l)"
            );
            let b = bundle_with_schemas(
                &[
                    "CREATE TABLE stage_orders AS SELECT id FROM orders",
                    "CREATE TABLE stage_lines AS SELECT order_id FROM lines",
                    &query,
                ],
                dialect,
                &schemas,
            );
            let target = b.layers()[2].id();
            let plan = physical_joint_source_plan(&b, &[(target, wanted)]);
            let WitnessDirection::Feasible(cases) = plan.outcome() else {
                panic!("{dialect} {keyword}: expected physical proof: {plan:?}");
            };
            let complete_membership = cases.iter().any(|case| {
                let matched = case.obligations().iter().any(|obligation| {
                    matches!(
                        obligation,
                        WitnessObligation::Membership {
                            case: sql_semantic_protocol::SubqueryMembershipCase::MatchingRow,
                            closed_world: true,
                            ..
                        }
                    )
                });
                let source_counts = case
                    .obligations()
                    .iter()
                    .filter_map(|obligation| match obligation {
                        WitnessObligation::Rows {
                            boundary,
                            bounds,
                            closed_world: true,
                            ..
                        } => Some((boundary.relation(), bounds.minimum(), bounds.maximum())),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                let outer_rows = wanted.max(1);
                let complete_counts =
                    source_counts.contains(&("orders", outer_rows, Some(outer_rows)))
                        && source_counts.contains(&("lines", 1, Some(1)));
                matched && complete_counts
            });
            assert!(
                complete_membership,
                "{dialect} {keyword}: expected complete membership case: {plan:?}"
            );

            // The proof controls every physical row. Duplicated and NULL
            // outer values still survive EXISTS; one NULL inner row suffices.
            let db = Connection::open_in_memory().unwrap();
            let outer_values = if wanted > 0 {
                "(0), (0), (NULL)"
            } else {
                "(NULL)"
            };
            db.execute_batch(&format!(
                "CREATE TABLE orders(id INTEGER);
                 CREATE TABLE lines(order_id INTEGER);
                 INSERT INTO orders VALUES {outer_values};
                 INSERT INTO lines VALUES (NULL);
                 CREATE TABLE stage_orders AS SELECT id FROM orders;
                 CREATE TABLE stage_lines AS SELECT order_id FROM lines;"
            ))
            .unwrap();
            let ordered = format!("SELECT id FROM ({query}) ORDER BY id NULLS LAST");
            let mut statement = db.prepare(&ordered).unwrap();
            let values = statement
                .query_map([], |row| row.get::<_, Option<i32>>(0))
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            let expected = if wanted == 3 {
                vec![Some(0), Some(0), None]
            } else {
                Vec::new()
            };
            assert_eq!(values, expected, "{dialect} {keyword}");
        }
    }
}

#[test]
fn membership_with_unproved_inner_filters_stays_residual() {
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
    for query in [
        "SELECT o.id FROM stage_orders o WHERE EXISTS (SELECT 1 FROM stage_lines l WHERE l.order_id > 5)",
        "SELECT o.id FROM stage_orders o WHERE o.id IN (SELECT l.order_id FROM stage_lines l WHERE l.order_id > 5)",
    ] {
        let b = bundle_with_schemas(&[producers[0], producers[1], query], "duckdb", &schemas);
        let outcome = physical_joint_source_plan(&b, &[(b.layers()[2].id(), 3)]);
        assert!(
            matches!(outcome.outcome(), WitnessDirection::Residual { .. }),
            "unsupported candidate proof must not be exact: {query}: {outcome:?}"
        );
    }
}

#[test]
fn correlated_exists_and_nonnull_in_are_realized_as_full_key_populations() {
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
    for dialect in DIALECTS {
        for (operator, wanted, outer_key) in [
            ("EXISTS", 3_u64, 0_i32),
            ("EXISTS", 0, 1),
            ("NOT EXISTS", 0, 0),
            ("NOT EXISTS", 3, 1),
            ("IN", 3, 0),
            ("IN", 0, 1),
            ("NOT IN", 0, 0),
            ("NOT IN", 3, 1),
        ] {
            let query = if operator.contains("EXISTS") {
                format!(
                    "SELECT o.id FROM stage_orders o WHERE {operator} \
                     (SELECT 1 FROM stage_lines l WHERE l.order_id = o.id)"
                )
            } else {
                format!(
                    "SELECT o.id FROM stage_orders o WHERE o.id {operator} \
                     (SELECT l.order_id FROM stage_lines l)"
                )
            };
            let b = bundle_with_schemas(&[producers[0], producers[1], &query], dialect, &schemas);
            let target = b.layers()[2].id();
            let proof = physical_joint_source_plan(&b, &[(target, wanted)]);
            let WitnessDirection::Feasible(cases) = proof.outcome() else {
                panic!("{dialect} {operator} rows={wanted}: expected proof: {proof:?}");
            };
            let complete = cases.iter().any(|case| {
                case.obligations().iter().any(|o| {
                    matches!(
                        o,
                        WitnessObligation::Membership {
                            closed_world: true,
                            ..
                        }
                    )
                }) && case.obligations().iter().any(|o| {
                    matches!(
                        o, WitnessObligation::Rows {
                            boundary, bounds, closed_world: true, ..
                        } if boundary.relation() == "orders"
                            && bounds.minimum() == wanted.max(1)
                    )
                }) && case.obligations().iter().any(|o| {
                    matches!(
                        o, WitnessObligation::Rows {
                            boundary, bounds, closed_world: true, ..
                        } if boundary.relation() == "lines"
                            && bounds.minimum() == 1
                    )
                })
            });
            assert!(
                complete,
                "{dialect} {operator}: missing closed source proof"
            );

            let db = Connection::open_in_memory().unwrap();
            let outer_values =
                std::iter::repeat_n(format!("({outer_key})"), wanted.max(1) as usize)
                    .collect::<Vec<_>>()
                    .join(",");
            db.execute_batch(&format!(
                "CREATE TABLE orders(id INTEGER);
                 CREATE TABLE lines(order_id INTEGER);
                 INSERT INTO orders VALUES {outer_values};
                 INSERT INTO lines VALUES (0);
                 {}; {};",
                producers[0], producers[1]
            ))
            .unwrap();
            let result = format!("SELECT id FROM ({query}) ORDER BY id");
            let mut stmt = db.prepare(&result).unwrap();
            let values = stmt
                .query_map([], |row| row.get::<_, i32>(0))
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            assert_eq!(
                values,
                vec![outer_key; wanted as usize],
                "{dialect} {operator} rows={wanted}"
            );
        }
    }
}

#[test]
fn nonempty_exists_jointly_reconciles_outer_and_inner_source_goals() {
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
            "SELECT o.id FROM stage_orders o WHERE EXISTS (SELECT 1 FROM stage_lines l)",
            "SELECT id FROM orders",
            "SELECT order_id FROM lines",
        ],
        "duckdb",
        &schemas,
    );
    let ids = b
        .layers()
        .iter()
        .map(|layer| layer.id())
        .collect::<Vec<_>>();
    let valid = physical_joint_source_plan(&b, &[(ids[2], 3), (ids[3], 3), (ids[4], 1)]);
    assert!(
        matches!(valid.outcome(), WitnessDirection::Feasible(_)),
        "a complete membership population should satisfy transparent readers: {valid:?}"
    );
    let other_count = physical_joint_source_plan(&b, &[(ids[2], 3), (ids[3], 3), (ids[4], 2)]);
    assert!(
        matches!(other_count.outcome(), WitnessDirection::Residual { .. }),
        "one sufficient nonempty inner example does not imply that every alternative is impossible"
    );

    let same_source = bundle_with_schemas(
        &[
            "CREATE TABLE stage_orders AS SELECT id FROM orders",
            "CREATE TABLE stage_lines AS SELECT id AS order_id FROM orders",
            "SELECT o.id FROM stage_orders o WHERE EXISTS (SELECT 1 FROM stage_lines l)",
        ],
        "duckdb",
        &schemas,
    );
    let repeated = physical_joint_source_plan(&same_source, &[(same_source.layers()[2].id(), 3)]);
    assert!(
        matches!(repeated.outcome(), WitnessDirection::Residual { .. }),
        "two producer aliases of one physical population must not be counted independently"
    );
}
