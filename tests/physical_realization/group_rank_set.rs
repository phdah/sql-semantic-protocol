//! group rank set regression tests.

use super::*;

#[test]
fn grouped_having_populations_reach_physical_sources_through_renamed_producers() {
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
    let sources = [
        schema("t", &["a", "b"]),
        schema("stage", &["category", "score"]),
        schema("mart", &["bucket", "rank_score"]),
    ];
    let sql = [
        "CREATE TABLE stage AS SELECT a AS category, b AS score FROM t",
        "CREATE TABLE mart AS SELECT category AS bucket, score AS rank_score FROM stage",
        "SELECT bucket, COUNT(*) AS n FROM mart GROUP BY bucket HAVING COUNT(*) >= 2",
    ];
    for &dialect in DIALECTS {
        let mut b = bundle_with_schemas(&sql, dialect, &sources);
        let layer = b.layers()[2].id().to_string();
        b.set_outcome_goals(&[OutcomeGoal::new(&layer, Some(2), Some(2), vec![]).unwrap()])
            .unwrap();
        assert!(
            matches!(
                b.outcome_goals()[0].witness(),
                Some(OutcomeWitness::Groups { relation, key, groups: 2, rows_per_group: 2 })
                    if relation == "t" && key.relation() == Some("t") && key.name() == "a"
            ),
            "{dialect}: {:?}",
            b.outcome_goals()
        );
        let json: serde_json::Value =
            serde_json::from_str(&sql_semantic_protocol::to_bundle_json(&b)).unwrap();
        assert_eq!(json["outcome_goals"][0]["witness"]["relation"], "t");
    }

    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE t(a INTEGER, b INTEGER);
         INSERT INTO t VALUES (0,10),(0,11),(1,20),(1,21);
         CREATE TABLE stage AS SELECT a AS category, b AS score FROM t;
         CREATE TABLE mart AS SELECT category AS bucket, score AS rank_score FROM stage;",
    )
    .unwrap();
    let actual: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM (SELECT bucket, COUNT(*) AS n FROM mart GROUP BY bucket HAVING COUNT(*) >= 2)",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(actual, 2);
}

#[test]
fn ranked_partitions_and_order_map_to_one_physical_leaf() {
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
    let schemas = [
        schema("t", &["a", "b"]),
        schema("stage", &["grp", "score"]),
        schema("mart", &["group_id", "sort_value"]),
    ];
    let sql = [
        "CREATE TABLE stage AS SELECT a AS grp, b AS score FROM t",
        "CREATE TABLE mart AS SELECT grp AS group_id, score AS sort_value FROM stage",
        "SELECT group_id, ROW_NUMBER() OVER (PARTITION BY group_id ORDER BY sort_value ASC NULLS LAST) AS rn FROM mart QUALIFY rn = 1",
    ];
    let mut b = bundle_with_schemas(&sql, "duckdb", &schemas);
    let layer = b.layers()[2].id().to_string();
    b.set_outcome_goals(&[OutcomeGoal::new(&layer, Some(2), None, vec![]).unwrap()])
        .unwrap();
    assert!(
        matches!(
            b.outcome_goals()[0].witness(),
            Some(OutcomeWitness::Ranked { relation, partition_key: Some(key), order_by, rows: 2 })
                if relation == "t"
                && key.relation() == Some("t") && key.name() == "a"
                && order_by.len() == 1
                && order_by[0].column().relation() == Some("t")
                && order_by[0].column().name() == "b"
        ),
        "{:?}",
        b.outcome_goals()
    );

    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE t(a INTEGER, b INTEGER);
         INSERT INTO t VALUES (0,10),(0,11),(1,20),(1,21);
         CREATE TABLE stage AS SELECT a AS grp, b AS score FROM t;
         CREATE TABLE mart AS SELECT grp AS group_id, score AS sort_value FROM stage;",
    )
    .unwrap();
    let actual: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM (SELECT group_id, ROW_NUMBER() OVER (PARTITION BY group_id ORDER BY sort_value ASC NULLS LAST) AS rn FROM mart QUALIFY rn = 1)",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(actual, 2);
}

#[test]
fn computed_or_filtered_parents_do_not_create_physical_group_proofs() {
    for &producer in &[
        "CREATE TABLE stage AS SELECT a + 1 AS a, b FROM t",
        "CREATE TABLE stage AS SELECT a, b FROM t WHERE b > 0",
    ] {
        let mut b = bundle(
            &[
                producer,
                "SELECT a, COUNT(*) AS n FROM stage GROUP BY a HAVING COUNT(*) >= 2",
            ],
            "postgresql",
        );
        let layer = b.layers()[1].id().to_string();
        b.set_outcome_goals(&[OutcomeGoal::new(&layer, Some(2), Some(2), vec![]).unwrap()])
            .unwrap();
        assert_eq!(
            b.outcome_goals()[0].status(),
            OutcomeGoalStatus::Residual,
            "{producer}: {:?}",
            b.outcome_goals()
        );
    }
}

#[test]
fn set_tuple_cases_prove_bag_counts_through_two_materialized_producers() {
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
    let schemas = [
        schema("l", &["a"]),
        schema("r", &["a"]),
        schema("left_stage", &["id"]),
        schema("right_stage", &["id"]),
    ];
    for operator in ["UNION ALL", "UNION", "INTERSECT", "EXCEPT"] {
        let statements = [
            "CREATE TABLE left_stage AS SELECT a AS id FROM l".to_string(),
            "CREATE TABLE right_stage AS SELECT a AS id FROM r".to_string(),
            format!("SELECT id FROM left_stage {operator} SELECT id FROM right_stage"),
        ];
        let queries = statements.iter().map(String::as_str).collect::<Vec<_>>();
        let mut b = bundle_with_schemas(&queries, "duckdb", &schemas);
        let id = b.layers()[2].id().to_string();
        b.set_outcome_goals(&[OutcomeGoal::new(&id, Some(2), None, vec![]).unwrap()])
            .unwrap();
        let case = match b.outcome_goals()[0].witness() {
            Some(OutcomeWitness::SetTuples {
                tuples: 2, case, ..
            }) => case,
            other => panic!("{operator}: expected mapped physical case, got {other:?}"),
        };
        assert!(
            case.obligations().iter().all(|obligation| {
                !obligation.boundary().is_intermediate()
                    && ["l", "r"].contains(&obligation.boundary().relation())
                    && obligation.boundary().tuple_columns() == ["a"]
            }),
            "{operator}: {case:?}"
        );
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE l(a INTEGER); CREATE TABLE r(a INTEGER);")
            .unwrap();
        for obligation in case.obligations() {
            for key in 0..2 {
                for _ in 0..obligation.matching_tuple_count() {
                    db.execute_batch(&format!(
                        "INSERT INTO {} VALUES ({key});",
                        obligation.boundary().relation()
                    ))
                    .unwrap();
                }
            }
        }
        db.execute_batch(
            "CREATE TABLE left_stage AS SELECT a AS id FROM l;
             CREATE TABLE right_stage AS SELECT a AS id FROM r;",
        )
        .unwrap();
        let result: i64 = db.query_row(
            &format!("SELECT COUNT(*) FROM (SELECT id FROM left_stage {operator} SELECT id FROM right_stage)"),
            [],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(result, 2, "{operator}");
    }
}

#[test]
fn set_tuple_mapping_preserves_null_frequencies_and_rejects_shared_sources() {
    let schema = |relation: &str, name: &str| {
        RelationSchema::new(
            relation,
            vec![SchemaColumn::from_sql_type(name, "INTEGER", "postgresql").unwrap()],
        )
        .unwrap()
    };
    let schemas = [
        schema("l", "a"),
        schema("r", "a"),
        schema("left_stage", "id"),
        schema("right_stage", "id"),
    ];
    let statements = [
        "CREATE TABLE left_stage AS SELECT a AS id FROM l",
        "CREATE TABLE right_stage AS SELECT a AS id FROM r",
        "SELECT id FROM left_stage UNION ALL SELECT id FROM right_stage",
    ];
    let mut b = bundle_with_schemas(&statements, "duckdb", &schemas);
    let id = b.layers()[2].id().to_string();
    let distribution =
        OutputDistribution::new("id", vec![OutputValueCount::new(ConstraintValue::Null, 3)])
            .unwrap();
    b.set_outcome_goals(&[OutcomeGoal::new(&id, Some(3), None, vec![distribution]).unwrap()])
        .unwrap();
    let case = match b.outcome_goals()[0].witness() {
        Some(OutcomeWitness::SetTuples {
            tuples: 1,
            case,
            scale_by_value_rows: true,
            ..
        }) => case,
        other => panic!("NULL bag witness missing: {other:?}"),
    };
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("CREATE TABLE l(a INTEGER); CREATE TABLE r(a INTEGER);")
        .unwrap();
    for obligation in case.obligations() {
        for _ in 0..(obligation.matching_tuple_count() * 3) {
            conn.execute_batch(&format!(
                "INSERT INTO {} VALUES (NULL);",
                obligation.boundary().relation()
            ))
            .unwrap();
        }
    }
    conn.execute_batch(
        "CREATE TABLE left_stage AS SELECT a AS id FROM l;
         CREATE TABLE right_stage AS SELECT a AS id FROM r;",
    )
    .unwrap();
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM (SELECT id FROM left_stage UNION ALL SELECT id FROM right_stage) WHERE id IS NULL",
        [],
        |row| row.get(0),
    ).unwrap();
    assert_eq!(count, 3);

    let mut same = bundle_with_schemas(
        &[
            statements[0],
            "CREATE TABLE right_stage AS SELECT a AS id FROM l",
            statements[2],
        ],
        "postgresql",
        &schemas,
    );
    let id = same.layers()[2].id().to_string();
    same.set_outcome_goals(&[OutcomeGoal::new(&id, Some(3), None, vec![]).unwrap()])
        .unwrap();
    assert_eq!(
        same.outcome_goals()[0].status(),
        OutcomeGoalStatus::Residual
    );
}
