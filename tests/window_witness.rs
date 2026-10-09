mod common;

use common::DIALECTS;
use duckdb::Connection;
use sql_semantic_protocol::{
    analyze_sql, dialect_from_name, to_json, ConditionExactnessStatus, Protocol, ProtocolStatement,
    ValueDomain, WindowWitnessDirection,
};
use sqlparser::dialect::GenericDialect;

fn first(protocol: &Protocol) -> &sql_semantic_protocol::QueryStatement {
    match protocol.statements().first() {
        Some(ProtocolStatement::Query(query)) => query,
        other => panic!("expected query, got {other:?}"),
    }
}

fn analyze(sql: &str) -> Protocol {
    analyze_sql(sql, "generic", &GenericDialect {}).expect("parse window SQL")
}

#[test]
fn rank_one_exposes_both_source_membership_directions() {
    let protocol = analyze(
        "SELECT account_id, ROW_NUMBER() OVER (PARTITION BY account_id ORDER BY score ASC NULLS LAST) AS rn \
         FROM events QUALIFY rn = 1",
    );
    let query = first(&protocol);
    let witness = query.window_witness().expect("typed witness");
    assert_eq!(witness.boundary(), Some("events"));
    assert_eq!(witness.partition_by()[0].relation(), Some("events"));
    assert_eq!(witness.partition_by()[0].name(), "account_id");
    assert_eq!(witness.order_by()[0].column().name(), "score");
    assert!(witness.order_by()[0].ascending());
    assert!(!witness.order_by()[0].nulls_first());
    assert!(
        matches!(witness.qualifying(), WindowWitnessDirection::Exact(case)
        if case.min_preceding() == 0 && case.max_preceding() == Some(0))
    );
    assert!(
        matches!(witness.rejected(), WindowWitnessDirection::Exact(case)
        if case.min_preceding() == 1 && case.max_preceding().is_none())
    );
    assert_eq!(
        query.condition_exactness().status(),
        ConditionExactnessStatus::Exact
    );

    let json: serde_json::Value = serde_json::from_str(&to_json(&protocol)).unwrap();
    let statement = &json["inputs"][0]["statements"][0];
    assert_eq!(statement["window_witness"]["qualifying"]["status"], "exact");
    assert_eq!(
        statement["window_witness"]["order_by"][0]["strict_unique"],
        true
    );
    assert_eq!(
        statement["output"]["columns"][1]["domain"]["values"][0]["value"],
        1
    );
}

#[test]
fn upper_bound_and_impossible_rank_are_typed_and_bound_output() {
    let protocol = analyze(
        "SELECT ROW_NUMBER() OVER (ORDER BY score DESC NULLS FIRST) AS rn \
         FROM events QUALIFY rn <= 3",
    );
    let query = first(&protocol);
    let witness = query.window_witness().unwrap();
    assert!(
        matches!(witness.qualifying(), WindowWitnessDirection::Exact(case)
        if case.min_preceding() == 0 && case.max_preceding() == Some(2))
    );
    assert!(
        matches!(witness.rejected(), WindowWitnessDirection::Exact(case)
        if case.min_preceding() == 3 && case.max_preceding().is_none())
    );
    let json: serde_json::Value = serde_json::from_str(&to_json(&protocol)).unwrap();
    assert_eq!(
        json["inputs"][0]["statements"][0]["output"]["columns"][0]["domain"]["ranges"][0]["upper"]
            ["value"]["value"],
        3
    );

    let impossible = analyze(
        "SELECT ROW_NUMBER() OVER (ORDER BY score NULLS LAST) AS rn \
         FROM events QUALIFY rn <= 0",
    );
    let query = first(&impossible);
    assert!(matches!(
        query.window_witness().unwrap().qualifying(),
        WindowWitnessDirection::Impossible
    ));
    assert!(matches!(query.window_witness().unwrap().rejected(),
        WindowWitnessDirection::Exact(case) if case.min_preceding() == 0));
    assert!(matches!(
        query.output().columns()[0].domain(),
        ValueDomain::Empty
    ));
}

#[test]
fn rank_ties_implicit_null_ordering_and_computed_keys_remain_residual() {
    let patterns = [
        ("RANK() OVER (ORDER BY score NULLS LAST)", "unsupported_rank_function"),
        ("DENSE_RANK() OVER (ORDER BY score NULLS LAST)", "unsupported_rank_function"),
        ("ROW_NUMBER() OVER (ORDER BY score)", "implicit_null_ordering_or_unresolved_column"),
        ("ROW_NUMBER() OVER (ORDER BY score + 1 NULLS LAST)", "computed_order_key"),
        ("ROW_NUMBER() OVER (ORDER BY score NULLS LAST ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)", "unproven_window_frame_or_order"),
    ];
    for (window, reason) in patterns {
        let sql = format!("SELECT {window} AS rn FROM events QUALIFY rn = 1");
        let protocol = analyze(&sql);
        let witness = first(&protocol).window_witness().unwrap();
        assert!(
            matches!(witness.qualifying(), WindowWitnessDirection::Residual { reason: actual } if *actual == reason),
            "{sql}"
        );
        assert_eq!(
            first(&protocol).condition_exactness().status(),
            ConditionExactnessStatus::Residual
        );
    }
}

#[test]
fn duckdb_rank_oracle_confirms_matching_rejected_and_impossible_witnesses() {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE events(account_id INTEGER, score INTEGER);
         INSERT INTO events VALUES (1, 10), (1, 20), (1, 30), (2, 5), (2, 15);",
    )
    .unwrap();
    let query = "SELECT account_id, score, ROW_NUMBER() OVER (
         PARTITION BY account_id ORDER BY score NULLS LAST) AS rn FROM events";
    let count = |predicate: &str| -> i64 {
        db.query_row(
            &format!("SELECT COUNT(*) FROM ({query}) WHERE {predicate}"),
            [],
            |row| row.get(0),
        )
        .unwrap()
    };
    assert_eq!(count("rn = 1"), 2);
    assert_eq!(count("rn > 1"), 3);
    assert_eq!(count("rn <= 2"), 4);
    assert_eq!(count("rn > 2"), 1);
    assert_eq!(count("rn <= 0"), 0);
    for (predicate, expected) in [("rn <= 2", 4), ("rn = 1", 2), ("rn <= 0", 0)] {
        let direct = format!(
            "SELECT COUNT(*) FROM (SELECT account_id, score, ROW_NUMBER() OVER (PARTITION BY account_id ORDER BY score NULLS LAST) AS rn FROM events QUALIFY {predicate})"
        );
        let actual: i64 = db.query_row(&direct, [], |row| row.get(0)).unwrap();
        assert_eq!(actual, expected, "{predicate}");
    }
    // A non-unique ORDER BY produces an arbitrary tie winner; the witness
    // therefore explicitly requires strictly distinct order tuples.
}

#[test]
fn shared_qualify_syntax_is_checked_at_every_parser_boundary() {
    let sql = "SELECT ROW_NUMBER() OVER (PARTITION BY account_id ORDER BY score NULLS LAST) AS rn FROM events QUALIFY rn <= 2";
    let mut supported = 0;
    for name in DIALECTS {
        let dialect = dialect_from_name(name).expect("registered dialect");
        match analyze_sql(sql, name, dialect.as_ref()) {
            Ok(protocol) => {
                supported += 1;
                let witness = first(&protocol).window_witness().expect("QUALIFY witness");
                assert!(
                    matches!(witness.qualifying(), WindowWitnessDirection::Exact(_)),
                    "{name}"
                );
            }
            Err(sql_semantic_protocol::Error::Parse(_)) => {}
            Err(other) => panic!("{name}: unexpected analysis error: {other}"),
        }
    }
    assert!(supported > 0);
}

#[test]
fn projected_rank_filters_keep_the_original_partition_and_boundary() {
    for sql in [
        "SELECT ranked.rn FROM (SELECT ROW_NUMBER() OVER (
             PARTITION BY account_id ORDER BY score NULLS LAST) AS rn FROM events) ranked
         WHERE ranked.rn <= 2",
        "WITH ranked AS (SELECT ROW_NUMBER() OVER (
             PARTITION BY account_id ORDER BY score NULLS LAST) AS rn FROM events)
         SELECT rn FROM ranked WHERE rn = 1",
    ] {
        let protocol = analyze(sql);
        let query = first(&protocol);
        let witness = query
            .window_witness()
            .expect("nested ranked projection should retain witness");
        assert_eq!(witness.boundary(), Some("events"));
        assert_eq!(witness.partition_by()[0].name(), "account_id");
        assert!(matches!(
            witness.qualifying(),
            WindowWitnessDirection::Exact(_)
        ));
        assert!(matches!(
            witness.rejected(),
            WindowWitnessDirection::Exact(_)
        ));
    }
}
