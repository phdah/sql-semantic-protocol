//! DuckDB execution checks for complete source-row cardinality witnesses.

mod common;

use common::DIALECTS;
use duckdb::Connection;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, to_bundle_json, ConfiguredSqlInput,
    ConstraintEnforcement, ConstraintEvidence, ConstraintProvenance, ConstraintSourceKind,
    ConstraintValue, OutcomeGoal, OutcomeGoalStatus, OutcomeWitness, OutputDistribution,
    OutputValueCount, RelationCatalog, RelationConstraint, RelationConstraintSet,
    RelationSchema, SchemaColumn, SqlInput,
};

fn typed(sql: &str, sources: &[(&str, &[&str])]) -> sql_semantic_protocol::AnalysisBundle {
    typed_for_dialect(sql, sources, "generic")
}

fn typed_for_dialect(
    sql: &str,
    sources: &[(&str, &[&str])],
    dialect_name: &str,
) -> sql_semantic_protocol::AnalysisBundle {
    let schemas = sources
        .iter()
        .map(|(relation, columns)| {
            RelationSchema::new(
                *relation,
                columns
                    .iter()
                    .map(|column| {
                        SchemaColumn::from_sql_type(*column, "BIGINT", "postgresql").expect("type")
                    })
                    .collect(),
            )
            .expect("schema")
        })
        .collect::<Vec<_>>();
    let catalog = RelationCatalog::from_schemas(&schemas).expect("catalog");
    let dialect = dialect_from_name(dialect_name).expect("dialect");
    let input = SqlInput::inline(sql);
    analyze_configured_inputs_with_catalog(
        &[ConfiguredSqlInput::new(
            "sql",
            &input,
            dialect_name,
            dialect.as_ref(),
        )],
        &catalog,
    )
    .expect("analyze")
}

fn assess(
    bundle: &mut sql_semantic_protocol::AnalysisBundle,
    rows: u64,
    groups: Option<u64>,
    distributions: Vec<OutputDistribution>,
) {
    let id = bundle.layers()[0].id().to_string();
    bundle
        .set_outcome_goals(
            &[OutcomeGoal::new(id, Some(rows), groups, distributions).expect("goal")],
        )
        .expect("assessment");
}

fn count(db: &Connection, sql: &str) -> u64 {
    db.query_row(
        &format!("SELECT COUNT(*) FROM ({sql}) AS result"),
        [],
        |row| row.get::<_, i64>(0),
    )
    .expect("oracle count") as u64
}

#[test]
fn typed_direct_projection_has_complete_histogram_proof() {
    let sql = "SELECT id FROM source_data";
    let mut bundle = typed(sql, &[("source_data", &["id"])]);
    let distribution = OutputDistribution::new(
        "id",
        vec![
            OutputValueCount::new(ConstraintValue::Integer(4), 2),
            OutputValueCount::new(ConstraintValue::Null, 1),
        ],
    )
    .expect("histogram");
    assess(&mut bundle, 3, None, vec![distribution]);
    assert_eq!(
        bundle.outcome_goals()[0].status(),
        OutcomeGoalStatus::Feasible
    );
    assert!(matches!(bundle.outcome_goals()[0].witness(),
        Some(OutcomeWitness::SourceRows { rows: 3, columns, .. }) if columns.len() == 1));
    let json: serde_json::Value = serde_json::from_str(&to_bundle_json(&bundle)).unwrap();
    assert_eq!(json["outcome_goals"][0]["witness"]["kind"], "source_rows");
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE source_data(id BIGINT); INSERT INTO source_data VALUES (4),(4),(NULL);",
    )
    .unwrap();
    assert_eq!(count(&db, sql), 3);
    let frequency: i64 = db
        .query_row("SELECT COUNT(*) FROM source_data WHERE id = 4", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(frequency, 2);
}

#[test]
fn two_source_join_has_constructive_matching_pair_counts() {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE l(id BIGINT); CREATE TABLE r(id BIGINT); INSERT INTO l VALUES (0),(1); INSERT INTO r VALUES (0),(1);").unwrap();
    for keyword in ["JOIN", "LEFT JOIN", "RIGHT JOIN", "FULL JOIN"] {
        let sql = format!("SELECT l.id FROM l {keyword} r ON l.id = r.id");
        let mut bundle = typed(&sql, &[("l", &["id"]), ("r", &["id"])]);
        assess(&mut bundle, 2, None, Vec::new());
        assert_eq!(
            bundle.outcome_goals()[0].status(),
            OutcomeGoalStatus::Feasible,
            "{keyword}"
        );
        assert!(
            matches!(
                bundle.outcome_goals()[0].witness(),
                Some(OutcomeWitness::JoinPairs { pairs: 2, .. })
            ),
            "{keyword}"
        );
        assert_eq!(count(&db, &sql), 2);
    }
}

#[test]
fn grouped_having_witness_creates_exact_surviving_groups() {
    let sql = "SELECT category, COUNT(*) AS n FROM sales GROUP BY category HAVING COUNT(*) >= 2";
    let mut bundle = typed(sql, &[("sales", &["category"])]);
    assess(&mut bundle, 2, Some(2), Vec::new());
    assert_eq!(
        bundle.outcome_goals()[0].status(),
        OutcomeGoalStatus::Feasible
    );
    assert!(matches!(
        bundle.outcome_goals()[0].witness(),
        Some(OutcomeWitness::Groups {
            groups: 2,
            rows_per_group: 2,
            ..
        })
    ));
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE sales(category BIGINT); INSERT INTO sales VALUES (0),(0),(1),(1);",
    )
    .unwrap();
    assert_eq!(count(&db, sql), 2);

    assess(&mut bundle, 1, Some(2), Vec::new());
    assert_eq!(
        bundle.outcome_goals()[0].status(),
        OutcomeGoalStatus::Unsatisfiable
    );

    let output_id = bundle.layers()[0].id().to_string();
    let goal = OutcomeGoal::new(output_id, None, Some(2), Vec::new()).unwrap();
    bundle.set_outcome_goals(&[goal]).unwrap();
    assert_eq!(
        bundle.outcome_goals()[0].status(),
        OutcomeGoalStatus::Feasible
    );
    assert!(matches!(
        bundle.outcome_goals()[0].witness(),
        Some(OutcomeWitness::Groups { groups: 2, .. })
    ));
}

#[test]
fn rank_witness_controls_partitions_and_global_upper_bounds() {
    let partitioned = "SELECT account_id, ROW_NUMBER() OVER (PARTITION BY account_id ORDER BY score ASC NULLS LAST) AS rn FROM events QUALIFY rn = 1";
    let unpartitioned = "SELECT ROW_NUMBER() OVER (ORDER BY score ASC NULLS LAST) AS rn FROM events QUALIFY rn <= 3";
    let mut bundle = typed(partitioned, &[("events", &["account_id", "score"])]);
    assess(&mut bundle, 3, None, Vec::new());
    assert_eq!(
        bundle.outcome_goals()[0].status(),
        OutcomeGoalStatus::Feasible
    );
    assert!(matches!(
        bundle.outcome_goals()[0].witness(),
        Some(OutcomeWitness::Ranked {
            rows: 3,
            partition_key: Some(_),
            ..
        })
    ));
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE events(account_id BIGINT, score BIGINT);
         INSERT INTO events VALUES (0,0),(1,0),(2,0);",
    )
    .unwrap();
    assert_eq!(count(&db, partitioned), 3);

    let mut bundle = typed(unpartitioned, &[("events", &["account_id", "score"])]);
    assess(&mut bundle, 2, None, Vec::new());
    assert_eq!(
        bundle.outcome_goals()[0].status(),
        OutcomeGoalStatus::Feasible
    );
    assert_eq!(bundle.outcome_goals()[0].max_rows(), Some(3));
    assert!(matches!(
        bundle.outcome_goals()[0].witness(),
        Some(OutcomeWitness::Ranked {
            rows: 2,
            partition_key: None,
            ..
        })
    ));
    assess(&mut bundle, 4, None, Vec::new());
    assert_eq!(
        bundle.outcome_goals()[0].status(),
        OutcomeGoalStatus::Unsatisfiable
    );
    db.execute_batch("DELETE FROM events; INSERT INTO events VALUES (0,0),(0,1),(0,2),(0,3);")
        .unwrap();
    assert_eq!(count(&db, unpartitioned), 3);
}

#[test]
fn set_witnesses_reproduce_actual_final_tuple_multiplicity() {
    for operator in ["UNION ALL", "UNION", "INTERSECT", "EXCEPT"] {
        let sql = format!("SELECT id FROM l {operator} SELECT id FROM r");
        let mut bundle = typed(&sql, &[("l", &["id"]), ("r", &["id"])]);
        assess(&mut bundle, 2, None, Vec::new());
        assert_eq!(
            bundle.outcome_goals()[0].status(),
            OutcomeGoalStatus::Feasible,
            "{operator}"
        );
        let case = match bundle.outcome_goals()[0].witness() {
            Some(OutcomeWitness::SetTuples {
                tuples: 2, case, ..
            }) => case,
            other => panic!("missing set obligations for {operator}: {other:?}"),
        };
        assert_eq!(case.output_tuple_count(), 1);
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE l(id BIGINT); CREATE TABLE r(id BIGINT);")
            .unwrap();
        for obligation in case.obligations() {
            let relation = obligation.boundary().relation();
            for key in 0..2 {
                for _ in 0..obligation.matching_tuple_count() {
                    db.execute_batch(&format!("INSERT INTO {relation} VALUES ({key});"))
                        .unwrap();
                }
            }
        }
        assert_eq!(count(&db, &sql), 2, "{operator}");
    }
}

#[test]
fn multiple_identical_values_collapse_under_distinct_set_rules() {
    let sql = "SELECT id FROM l UNION SELECT id FROM r";
    let mut bundle = typed(sql, &[("l", &["id"]), ("r", &["id"])]);
    let distribution = OutputDistribution::new(
        "id",
        vec![OutputValueCount::new(ConstraintValue::Integer(7), 2)],
    )
    .unwrap();
    assess(&mut bundle, 2, None, vec![distribution]);
    assert_eq!(
        bundle.outcome_goals()[0].status(),
        OutcomeGoalStatus::Unsatisfiable
    );
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE l(id BIGINT); CREATE TABLE r(id BIGINT); INSERT INTO l VALUES(7),(7); INSERT INTO r VALUES (7);").unwrap();
    assert_eq!(count(&db, sql), 1);
}

#[test]
fn untyped_integer_join_does_not_claim_constructive_key_feasibility() {
    let sql = "SELECT l.id FROM l JOIN r ON l.id = r.id";
    let mut bundle = sql_semantic_protocol::analyze_inputs(
        &[SqlInput::inline(sql)],
        "generic",
        &sqlparser::dialect::GenericDialect {},
    )
    .expect("analysis");
    assess(&mut bundle, 2, None, Vec::new());
    assert_eq!(
        bundle.outcome_goals()[0].status(),
        OutcomeGoalStatus::Residual
    );
}

#[test]
fn all_set_histogram_scales_branch_counts() {
    let sql = "SELECT id FROM l UNION ALL SELECT id FROM r";
    let mut bundle = typed(sql, &[("l", &["id"]), ("r", &["id"])]);
    let values = vec![OutputValueCount::new(ConstraintValue::Integer(7), 3)];
    assess(
        &mut bundle,
        3,
        None,
        vec![OutputDistribution::new("id", values).unwrap()],
    );
    assert_eq!(
        bundle.outcome_goals()[0].status(),
        OutcomeGoalStatus::Feasible
    );
    let case = match bundle.outcome_goals()[0].witness() {
        Some(OutcomeWitness::SetTuples {
            tuples: 1,
            case,
            scale_by_value_rows: true,
            ..
        }) => case,
        other => panic!("unexpected set proof: {other:?}"),
    };
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE l(id BIGINT); CREATE TABLE r(id BIGINT);")
        .unwrap();
    for obligation in case.obligations() {
        for _ in 0..(obligation.matching_tuple_count() * 3) {
            db.execute_batch(&format!(
                "INSERT INTO {} VALUES (7);",
                obligation.boundary().relation()
            ))
            .unwrap();
        }
    }
    assert_eq!(count(&db, sql), 3);
}

#[test]
fn enrichment_reassesses_existing_constructive_goals() {
    let sql = "SELECT id FROM source_data";
    let mut bundle = typed(sql, &[("source_data", &["id"])]);
    assess(&mut bundle, 2, None, Vec::new());
    assert_eq!(
        bundle.outcome_goals()[0].status(),
        OutcomeGoalStatus::Feasible
    );
    assert!(matches!(
        bundle.outcome_goals()[0].witness(),
        Some(OutcomeWitness::SourceRows { rows: 2, .. })
    ));

    let evidence = ConstraintEvidence::new(
        ConstraintProvenance::new(ConstraintSourceKind::ExternalMetadata, "test-constraint")
            .expect("source"),
        ConstraintEnforcement::Unknown,
    );
    let constraint = RelationConstraint::not_null("id", vec![evidence]).expect("constraint");
    let set = RelationConstraintSet::new("source_data", vec![constraint]).expect("constraint set");
    bundle.enrich_relation_constraints(&[set]);

    let assessed = &bundle.outcome_goals()[0];
    assert_eq!(assessed.goal().rows(), Some(2), "retain the caller's goal");
    assert_eq!(assessed.status(), OutcomeGoalStatus::Residual);
    assert!(assessed.witness().is_none());
    let json: serde_json::Value = serde_json::from_str(&to_bundle_json(&bundle)).unwrap();
    assert_eq!(json["outcome_goals"][0]["assessment"]["status"], "residual");
    assert!(json["outcome_goals"][0].get("witness").is_none());
}

#[test]
fn branch_local_limits_are_not_physical_set_witnesses() {
    // Parenthesized branches apply LIMIT before UNION ALL. Loading any number
    // of source rows can never make either branch emit a row.
    let sql = "(SELECT id FROM l LIMIT 0) UNION ALL (SELECT id FROM r LIMIT 0)";
    let mut bundle = typed_for_dialect(sql, &[("l", &["id"]), ("r", &["id"])], "duckdb");
    assess(&mut bundle, 1, None, Vec::new());
    assert_eq!(bundle.outcome_goals()[0].status(), OutcomeGoalStatus::Residual);
    assert!(bundle.outcome_goals()[0].witness().is_none());

    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE l(id BIGINT); CREATE TABLE r(id BIGINT);
         INSERT INTO l VALUES (1), (2); INSERT INTO r VALUES (3), (4);",
    )
    .unwrap();
    assert_eq!(count(&db, sql), 0);
}

#[test]
fn shared_constructive_goal_classes_are_dialect_independent() {
    let sources: &[(&str, &[&str])] = &[
        ("source_data", &["id"]),
        ("l", &["id"]),
        ("r", &["id"]),
        ("sales", &["category"]),
    ];
    let classes = [
        ("SELECT id FROM source_data", 3, None, OutcomeGoalStatus::Feasible),
        (
            "SELECT l.id FROM l JOIN r ON l.id = r.id",
            2,
            None,
            OutcomeGoalStatus::Feasible,
        ),
        (
            "SELECT category, COUNT(*) AS n FROM sales GROUP BY category HAVING COUNT(*) >= 2",
            2,
            Some(2),
            OutcomeGoalStatus::Feasible,
        ),
        (
            "SELECT id FROM l UNION ALL SELECT id FROM r",
            2,
            None,
            OutcomeGoalStatus::Feasible,
        ),
        (
            "SELECT category, COUNT(*) AS n FROM sales GROUP BY category",
            1,
            Some(2),
            OutcomeGoalStatus::Unsatisfiable,
        ),
    ];
    for dialect_name in DIALECTS {
        let dialect = dialect_from_name(dialect_name).expect("registered dialect");
        let mut parsed = 0;
        for (sql, rows, groups, status) in classes {
            // Treat rejected syntax as a parser-boundary limitation, not an
            // excuse to infer a different semantic outcome.
            match sqlparser::parser::Parser::parse_sql(dialect.as_ref(), sql) {
                Ok(_) => {
                    parsed += 1;
                    let mut bundle = typed_for_dialect(sql, sources, dialect_name);
                    assess(&mut bundle, rows, groups, Vec::new());
                    assert_eq!(
                        bundle.outcome_goals()[0].status(),
                        status,
                        "dialect {dialect_name}: {sql}"
                    );
                }
                Err(error) => eprintln!("{dialect_name} parser does not accept {sql}: {error}"),
            }
        }
        assert!(parsed > 0, "{dialect_name} must support some shared SQL");
    }
}

#[test]
fn qualify_goal_bounds_follow_every_dialect_that_parses_qualify() {
    let sql = "SELECT ROW_NUMBER() OVER (ORDER BY score ASC NULLS LAST) AS rn FROM events QUALIFY rn <= 3";
    let mut supported = 0;
    for dialect_name in DIALECTS {
        let dialect = dialect_from_name(dialect_name).expect("registered dialect");
        match sqlparser::parser::Parser::parse_sql(dialect.as_ref(), sql) {
            Ok(_) => {
                supported += 1;
                let mut bundle = typed_for_dialect(sql, &[("events", &["score"])], dialect_name);
                assess(&mut bundle, 4, None, Vec::new());
                let goal = &bundle.outcome_goals()[0];
                assert_eq!(goal.status(), OutcomeGoalStatus::Unsatisfiable, "{dialect_name}");
                assert_eq!(goal.max_rows(), Some(3), "{dialect_name}");
            }
            Err(error) => eprintln!("{dialect_name} QUALIFY parser boundary: {error}"),
        }
    }
    assert!(supported > 0, "at least one parser must accept QUALIFY");
}
