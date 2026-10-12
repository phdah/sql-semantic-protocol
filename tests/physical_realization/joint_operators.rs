//! joint operators regression tests.

use super::*;

#[test]
fn joint_grouped_and_transparent_outputs_share_one_complete_physical_population() {
    let schema = |relation: &str, columns: &[&str]| {
        RelationSchema::new(
            relation,
            columns
                .iter()
                .map(|name| SchemaColumn::from_sql_type(*name, "INTEGER", "postgresql").unwrap())
                .collect(),
        )
        .unwrap()
    };
    let schemas = [
        schema("t", &["a", "b"]),
        schema("stage", &["category", "b"]),
    ];
    let b = bundle_with_schemas(
        &[
            "CREATE TABLE stage AS SELECT a AS category, b FROM t",
            "SELECT category, COUNT(*) AS n FROM stage GROUP BY category HAVING COUNT(*) >= 2",
            "SELECT a FROM t",
        ],
        "postgresql",
        &schemas,
    );
    let grouped = b.layers()[1].id().to_string();
    let source = b.layers()[2].id().to_string();
    // The GROUP BY population contains exactly two keys, each duplicated
    // twice, and the direct query must observe those same four source rows.
    let plan = physical_joint_source_plan(&b, &[(&grouped, 2), (&source, 4)]);
    let WitnessDirection::Feasible(cases) = plan.outcome() else {
        panic!("expected one shared physical grouped population: {plan:?}");
    };
    assert_eq!(plan.sources(), &["t".to_string()]);
    assert!(cases.iter().any(|case| {
        case.obligations().iter().any(|o| {
            matches!(
                o,
                WitnessObligation::OperatorPopulation {
                    witness: OutcomeWitness::Groups {
                        relation,
                        groups: 2,
                        rows_per_group: 2,
                        ..
                    }
                } if relation == "t"
            )
        })
    }));
    assert_eq!(
        physical_joint_source_plan(&b, &[(&source, 4), (&grouped, 2)]).outcome(),
        plan.outcome()
    );
    let doc: serde_json::Value =
        serde_json::from_str(&sql_semantic_protocol::to_bundle_json(&b)).unwrap();
    assert!(doc["graph"]["physical_joint_count_plan"].is_null());
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE t(a INTEGER,b INTEGER);
         INSERT INTO t VALUES (0,4),(0,5),(1,6),(1,7);
         CREATE TABLE stage AS SELECT a AS category, b FROM t;",
    )
    .unwrap();
    let grouped_rows: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM (SELECT category, COUNT(*) AS n FROM stage GROUP BY category HAVING COUNT(*) >= 2)",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let source_rows: i64 = db
        .query_row("SELECT COUNT(*) FROM t", [], |row| row.get(0))
        .unwrap();
    assert_eq!((grouped_rows, source_rows), (2, 4));

    // A sufficient example with a different count cannot be declared
    // contradictory: a larger source may have additional rejected groups.
    let conflicting = physical_joint_source_plan(&b, &[(&grouped, 2), (&source, 5)]);
    assert!(matches!(
        conflicting.outcome(),
        WitnessDirection::Residual { .. }
    ));
}

#[test]
fn independent_group_rank_and_set_goals_compose_without_reusing_sources() {
    let typed_schema = |relation: &str, column: &str| {
        RelationSchema::new(
            relation,
            vec![SchemaColumn::from_sql_type(column, "INTEGER", "postgresql").unwrap()],
        )
        .unwrap()
    };
    let schemas = [
        typed_schema("l", "a"),
        typed_schema("r", "a"),
        typed_schema("t", "a"),
        typed_schema("stage_l", "id"),
        typed_schema("stage_r", "id"),
        typed_schema("stage_t", "a"),
    ];
    let statements = [
        "CREATE TABLE stage_l AS SELECT a AS id FROM l",
        "CREATE TABLE stage_r AS SELECT a AS id FROM r",
        "CREATE TABLE stage_t AS SELECT a FROM t",
        "SELECT id FROM stage_l UNION ALL SELECT id FROM stage_r",
        "SELECT a, COUNT(*) AS n FROM stage_t GROUP BY a HAVING COUNT(*) >= 2",
    ];
    let b = bundle_with_schemas(&statements, "postgresql", &schemas);
    let set = b.layers()[3].id();
    let grouped = b.layers()[4].id();
    let plan = physical_joint_source_plan(&b, &[(set, 2), (grouped, 2)]);
    let WitnessDirection::Feasible(cases) = plan.outcome() else {
        panic!("disjoint operator sources should compose: {plan:?}");
    };
    assert!(cases.iter().any(|case| case
        .obligations()
        .iter()
        .filter(|o| matches!(o, WitnessObligation::OperatorPopulation { .. }))
        .count()
        == 2));
    assert_eq!(
        plan.sources(),
        &["l".to_string(), "r".to_string(), "t".to_string()]
    );

    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE l(a INTEGER);
         CREATE TABLE r(a INTEGER);
         CREATE TABLE t(a INTEGER);
         INSERT INTO l VALUES(0),(1);
         INSERT INTO t VALUES (0),(0),(1),(1);
         CREATE TABLE stage_l AS SELECT a AS id FROM l;
         CREATE TABLE stage_r AS SELECT a AS id FROM r;
         CREATE TABLE stage_t AS SELECT a FROM t;",
    )
    .unwrap();
    let set_count: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM (SELECT id FROM stage_l UNION ALL SELECT id FROM stage_r)",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let group_count: i64 = db.query_row(
        "SELECT COUNT(*) FROM (SELECT a, COUNT(*) AS n FROM stage_t GROUP BY a HAVING COUNT(*) >= 2)",
        [],
        |row| row.get(0),
    ).unwrap();
    assert_eq!((set_count, group_count), (2, 2));
}

#[test]
fn shared_operator_source_requires_joint_key_and_row_proof() {
    let mut b = bundle(
        &[
            "CREATE TABLE stage AS SELECT a, b FROM t",
            "SELECT a, COUNT(*) AS n FROM stage GROUP BY a HAVING COUNT(*) >= 2",
            "SELECT a, ROW_NUMBER() OVER (PARTITION BY a ORDER BY b ASC NULLS LAST) AS rn FROM stage QUALIFY rn = 1",
        ],
        "duckdb",
    );
    let grouped = b.layers()[1].id();
    let ranked = b.layers()[2].id();
    let plan = physical_joint_source_plan(&b, &[(grouped, 2), (ranked, 2)]);
    assert!(matches!(plan.outcome(), WitnessDirection::Residual { .. }));
    b.set_outcome_goals(&[
        OutcomeGoal::new(grouped, Some(2), None, vec![]).unwrap(),
        OutcomeGoal::new(ranked, Some(2), None, vec![]).unwrap(),
    ])
    .unwrap();
    // Each terminal is independently realizable, but that does not license
    // combining their distinct physical row assignments without a proof.
    assert!(b
        .outcome_goals()
        .iter()
        .all(|goal| goal.status() == OutcomeGoalStatus::Feasible));
}

#[test]
fn matched_join_population_proves_materialized_distinct_key_grouping() {
    let schema = |name: &str, columns: &[&str]| {
        RelationSchema::new(
            name,
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
        schema("l", &["k"]),
        schema("r", &["k"]),
        schema("joined", &["k"]),
    ];
    for &dialect in DIALECTS {
        let b = bundle_with_schemas(
            &[
                "CREATE TABLE joined AS SELECT l.k AS k FROM l JOIN r ON l.k = r.k",
                "SELECT k, COUNT(*) AS n FROM joined GROUP BY k HAVING COUNT(*) >= 1",
            ],
            dialect,
            &schemas,
        );
        let result = physical_joint_source_plan(&b, &[(b.layers()[1].id(), 2)]);
        let WitnessDirection::Feasible(cases) = result.outcome() else {
            panic!("{dialect}: composed join grouping unresolved: {result:?}");
        };
        assert!(
            cases.iter().any(|case| {
                case.obligations().iter().any(|obligation| {
                    matches!(
                        obligation,
                        WitnessObligation::JoinPopulation {
                            pattern: JoinPopulationPattern::DistinctMatched,
                            output_rows: 2,
                            ..
                        }
                    )
                }) && case.obligations().iter().any(|obligation| {
                    matches!(
                        obligation,
                        WitnessObligation::OperatorPopulation {
                            witness: OutcomeWitness::Groups {
                                groups: 2,
                                rows_per_group: 1,
                                ..
                            }
                        }
                    )
                })
            }),
            "{dialect}: expected jointly proven join and groups"
        );
    }
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE l(k INTEGER); CREATE TABLE r(k INTEGER);
         INSERT INTO l VALUES (0),(1);
         INSERT INTO r VALUES (0),(1);
         CREATE TABLE joined AS SELECT l.k AS k FROM l JOIN r ON l.k = r.k;",
    )
    .unwrap();
    let actual: i64 = db.query_row(
        "SELECT COUNT(*) FROM (SELECT k, COUNT(*) AS n FROM joined GROUP BY k HAVING COUNT(*) >= 1)",
        [],
        |row| row.get(0),
    ).unwrap();
    assert_eq!(actual, 2);

    let b = bundle_with_schemas(
        &[
            "CREATE TABLE joined AS SELECT l.k AS k FROM l JOIN r ON l.k = r.k",
            "SELECT k, COUNT(*) AS n FROM joined GROUP BY k HAVING COUNT(*) >= 2",
        ],
        "postgresql",
        &schemas,
    );
    let unproven = physical_joint_source_plan(&b, &[(b.layers()[1].id(), 2)]);
    assert!(matches!(
        unproven.outcome(),
        WitnessDirection::Residual { .. }
    ));
}

#[test]
fn grouped_materialization_feeds_one_rank_per_complete_partition() {
    let schema = |relation: &str, names: &[&str]| {
        RelationSchema::new(
            relation,
            names
                .iter()
                .map(|name| SchemaColumn::from_sql_type(*name, "INTEGER", "postgresql").unwrap())
                .collect(),
        )
        .unwrap()
    };
    let schemas = [schema("t", &["a"]), schema("grouped", &["a", "n"])];
    let input = [
        "CREATE TABLE grouped AS SELECT a, COUNT(*) AS n FROM t GROUP BY a HAVING COUNT(*) >= 2",
        "SELECT a, ROW_NUMBER() OVER (PARTITION BY a ORDER BY a ASC NULLS LAST) AS rn FROM grouped QUALIFY rn = 1",
    ];
    let b = bundle_with_schemas(&input, "duckdb", &schemas);
    let plan = physical_joint_source_plan(&b, &[(b.layers()[1].id(), 2)]);
    let WitnessDirection::Feasible(cases) = plan.outcome() else {
        panic!("rank must compose from complete grouped physical population: {plan:?}");
    };
    assert!(cases.iter().any(|case| case.obligations().iter().any(
        |o| matches!(o, WitnessObligation::OperatorPopulation {
            witness: OutcomeWitness::Groups {
                relation,
                groups: 2,
                rows_per_group: 2,
                ..
            }
        } if relation == "t")
    )));
    assert!(cases.iter().any(|case| case
        .obligations()
        .iter()
        .any(|o| matches!(o, WitnessObligation::Producer { .. }))));
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE t(a INTEGER);
         INSERT INTO t VALUES (0),(0),(1),(1);
         CREATE TABLE grouped AS SELECT a, COUNT(*) AS n FROM t GROUP BY a HAVING COUNT(*) >= 2;",
    )
    .unwrap();
    let actual: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM (
           SELECT a, ROW_NUMBER() OVER (PARTITION BY a ORDER BY a ASC NULLS LAST) AS rn
           FROM grouped QUALIFY rn = 1
         )",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(actual, 2);

    // Switching to a rank requiring a predecessor invalidates the proof.
    let b = bundle_with_schemas(
        &[
            input[0],
            "SELECT a, ROW_NUMBER() OVER (PARTITION BY a ORDER BY a ASC NULLS LAST) AS rn FROM grouped QUALIFY rn > 1",
        ],
        "duckdb",
        &schemas,
    );
    let residual = physical_joint_source_plan(&b, &[(b.layers()[1].id(), 2)]);
    assert!(matches!(
        residual.outcome(),
        WitnessDirection::Residual { .. }
    ));
}
