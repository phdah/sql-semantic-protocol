mod common;

use common::DIALECTS;
use sql_semantic_protocol::{
    analyze_sql, to_json, DiagnosticArea, Expression, Predicate, Protocol, ProtocolStatement,
    QueryStatement, WindowFrameBound, WindowFrameUnits,
};
use sqlparser::dialect::{dialect_from_str, GenericDialect, SnowflakeDialect};

fn first_query(protocol: &Protocol) -> &QueryStatement {
    match protocol.statements().first() {
        Some(ProtocolStatement::Query(query)) => query,
        other => panic!("expected query statement, got {other:?}"),
    }
}

fn first_window_expression(
    protocol: &Protocol,
) -> &sql_semantic_protocol::WindowFunctionExpression {
    match first_query(protocol).output().columns()[0].expression() {
        Expression::WindowFunction(window) => window,
        other => panic!("expected window function expression, got {other:?}"),
    }
}

#[test]
fn ranking_window_preserves_partition_order_and_lineage() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "SELECT ROW_NUMBER() OVER (PARTITION BY account_id ORDER BY created_at) AS rn FROM events",
        "generic",
        &dialect,
    )
    .expect("ranking window should analyze");

    let window = first_window_expression(&protocol);
    assert_eq!(window.function().name(), "ROW_NUMBER");
    assert!(window.function().arguments().is_empty());
    assert_eq!(window.window().partition_by().len(), 1);
    assert_eq!(window.window().order_by().len(), 1);

    let lineage = first_query(&protocol).output().columns()[0].lineage();
    assert_eq!(lineage.len(), 2);
    assert_eq!(lineage[0].relation(), "events");
    assert_eq!(lineage[0].column(), "account_id");
    assert_eq!(lineage[1].relation(), "events");
    assert_eq!(lineage[1].column(), "created_at");

    let json: serde_json::Value =
        serde_json::from_str(&to_json(&protocol)).expect("protocol JSON should parse");
    assert_eq!(
        json["inputs"][0]["statements"][0]["output"]["columns"][0]["expression"]["kind"],
        "window_function"
    );
}

#[test]
fn aggregate_window_arguments_and_partition_keys_contribute_lineage() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "SELECT SUM(amount) OVER (PARTITION BY account_id) AS total FROM events",
        "generic",
        &dialect,
    )
    .expect("aggregate window should analyze");

    let window = first_window_expression(&protocol);
    assert_eq!(window.function().name(), "SUM");
    assert_eq!(window.function().arguments().len(), 1);

    let lineage = first_query(&protocol).output().columns()[0].lineage();
    assert_eq!(
        lineage
            .iter()
            .map(|source| source.column())
            .collect::<Vec<_>>(),
        vec!["account_id", "amount"]
    );
}

#[test]
fn named_window_is_resolved_in_local_query_scope() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "SELECT ROW_NUMBER() OVER w AS rn FROM events WINDOW w AS (PARTITION BY account_id ORDER BY created_at)",
        "generic",
        &dialect,
    )
    .expect("named window should analyze");

    let window = first_window_expression(&protocol);
    assert_eq!(window.window().name(), Some("w"));
    assert_eq!(window.window().partition_by().len(), 1);
    assert_eq!(window.window().order_by().len(), 1);
    assert!(!first_query(&protocol)
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code() == "unsupported_named_window"));
}

#[test]
fn explicit_rows_range_and_groups_frames_are_typed() {
    let dialect = GenericDialect {};

    for (units_sql, expected) in [
        ("ROWS", WindowFrameUnits::Rows),
        ("RANGE", WindowFrameUnits::Range),
        ("GROUPS", WindowFrameUnits::Groups),
    ] {
        let sql = format!(
            "SELECT SUM(amount) OVER (ORDER BY created_at {units_sql} BETWEEN 2 PRECEDING AND CURRENT ROW) AS total FROM events"
        );
        let protocol =
            analyze_sql(&sql, "generic", &dialect).expect("explicit frame should analyze");
        let frame = first_window_expression(&protocol)
            .window()
            .frame()
            .expect("frame should be represented");

        assert_eq!(frame.units(), expected);
        assert!(matches!(
            frame.start_bound(),
            WindowFrameBound::Preceding(_)
        ));
        assert!(matches!(frame.end_bound(), WindowFrameBound::CurrentRow));
    }
}

#[test]
fn qualify_alias_resolves_to_window_expression_without_scalar_domain_claims() {
    let dialect = SnowflakeDialect {};
    let protocol = analyze_sql(
        "SELECT ROW_NUMBER() OVER (PARTITION BY account_id ORDER BY created_at) AS rn FROM events QUALIFY rn = 1",
        "snowflake",
        &dialect,
    )
    .expect("QUALIFY should analyze");

    let comparison = match first_query(&protocol)
        .predicates()
        .qualify_predicate()
        .expect("QUALIFY predicate should be present")
    {
        Predicate::Comparison(comparison) => comparison,
        other => panic!("expected comparison predicate, got {other:?}"),
    };
    assert!(matches!(comparison.left(), Expression::WindowFunction(_)));
    assert!(first_query(&protocol).column_domains().is_empty());
    assert_eq!(
        first_query(&protocol).output().columns()[0].lineage().len(),
        2
    );
}

#[test]
fn unsupported_window_options_remain_explicit() {
    let dialect = SnowflakeDialect {};
    let protocol = analyze_sql(
        "SELECT FIRST_VALUE(amount) IGNORE NULLS OVER (PARTITION BY account_id ORDER BY created_at) AS first_amount FROM events",
        "snowflake",
        &dialect,
    )
    .expect("unsupported window option should remain analyzable");

    assert!(matches!(
        first_query(&protocol).output().columns()[0].expression(),
        Expression::Unsupported(_)
    ));
    assert!(first_query(&protocol)
        .diagnostics()
        .iter()
        .any(|diagnostic| {
            diagnostic.area() == DiagnosticArea::Function
                && diagnostic.code() == "unsupported_function"
        }));
}

#[test]
fn shared_window_syntax_is_analyzed_across_all_exposed_dialects() {
    let sql =
        "SELECT ROW_NUMBER() OVER (PARTITION BY account_id ORDER BY created_at) AS rn FROM events";

    for dialect_name in DIALECTS {
        let dialect =
            dialect_from_str(dialect_name).expect("documented dialect should be recognized");
        let protocol = analyze_sql(sql, dialect_name, dialect.as_ref())
            .unwrap_or_else(|error| panic!("dialect {dialect_name} failed window syntax: {error}"));

        assert!(
            matches!(
                first_query(&protocol).output().columns()[0].expression(),
                Expression::WindowFunction(_)
            ),
            "dialect {dialect_name}"
        );
    }
}
