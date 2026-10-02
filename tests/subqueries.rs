mod common;

use common::DIALECTS;
use sql_semantic_protocol::{
    analyze_sql, DiagnosticArea, Expression, Predicate, Protocol, ProtocolStatement,
    QueryStatement,
};
use sqlparser::dialect::{
    dialect_from_str, BigQueryDialect, GenericDialect, PostgreSqlDialect,
};

fn first_query(protocol: &Protocol) -> &QueryStatement {
    match protocol.statements().first() {
        Some(ProtocolStatement::Query(query)) => query,
        other => panic!("expected query statement, got {other:?}"),
    }
}

#[test]
fn scalar_subquery_preserves_lineage_dependencies_and_correlation() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "SELECT (SELECT p.price FROM prices p WHERE p.id = o.id) AS price FROM orders o",
        "generic",
        &dialect,
    )
    .expect("scalar subquery should analyze");

    let query = first_query(&protocol);
    assert_eq!(query.dependencies(), ["orders", "prices"]);

    let subquery = match query.output().columns()[0].expression() {
        Expression::ScalarSubquery(expression) => expression.subquery(),
        other => panic!("expected scalar subquery expression, got {other:?}"),
    };

    assert_eq!(subquery.dependencies(), ["prices"]);
    assert_eq!(subquery.correlations().len(), 1);
    assert_eq!(subquery.correlations()[0].relation(), "orders");
    assert_eq!(subquery.correlations()[0].column(), "id");

    let lineage = query.output().columns()[0].lineage();
    assert_eq!(lineage.len(), 1);
    assert_eq!(lineage[0].relation(), "prices");
    assert_eq!(lineage[0].column(), "price");
}

#[test]
fn exists_subquery_preserves_predicate_structure_and_correlation() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "SELECT o.id FROM orders o WHERE EXISTS (SELECT 1 FROM line_items li WHERE li.order_id = o.id)",
        "generic",
        &dialect,
    )
    .expect("EXISTS subquery should analyze");

    let query = first_query(&protocol);
    assert_eq!(query.dependencies(), ["line_items", "orders"]);

    let exists = match query
        .predicates()
        .where_predicate()
        .expect("WHERE predicate should exist")
    {
        Predicate::Exists(exists) => exists,
        other => panic!("expected EXISTS predicate, got {other:?}"),
    };

    assert!(!exists.negated());
    assert_eq!(exists.subquery().dependencies(), ["line_items"]);
    assert_eq!(exists.subquery().correlations().len(), 1);
    assert_eq!(exists.subquery().correlations()[0].relation(), "orders");
    assert_eq!(exists.subquery().correlations()[0].column(), "id");
}

#[test]
fn in_and_not_in_subqueries_preserve_membership_semantics() {
    let dialect = GenericDialect {};

    for (keyword, expected_negated) in [("IN", false), ("NOT IN", true)] {
        let sql = format!(
            "SELECT o.id FROM orders o WHERE o.customer_id {keyword} (SELECT c.id FROM customers c)"
        );
        let protocol =
            analyze_sql(&sql, "generic", &dialect).expect("IN subquery should analyze");

        let predicate = first_query(&protocol)
            .predicates()
            .where_predicate()
            .expect("WHERE predicate should exist");
        let membership = match predicate {
            Predicate::InSubquery(membership) => membership,
            other => panic!("expected IN-subquery predicate, got {other:?}"),
        };

        assert_eq!(membership.negated(), expected_negated);
        assert!(matches!(membership.expression(), Expression::Column(_)));
        assert_eq!(membership.subquery().dependencies(), ["customers"]);
    }
}

#[test]
fn derived_table_exposes_only_projected_columns() {
    let dialect = GenericDialect {};

    let resolved = analyze_sql(
        "SELECT d.id FROM (SELECT t.id FROM raw_table t) d",
        "generic",
        &dialect,
    )
    .expect("derived table should analyze");
    let lineage = first_query(&resolved).output().columns()[0].lineage();
    assert_eq!(lineage.len(), 1);
    assert_eq!(lineage[0].relation(), "raw_table");
    assert_eq!(lineage[0].column(), "id");

    let hidden = analyze_sql(
        "SELECT d.hidden FROM (SELECT t.id FROM raw_table t) d",
        "generic",
        &dialect,
    )
    .expect("unresolved derived column should remain analyzable");
    let query = first_query(&hidden);
    assert!(query.output().columns()[0].lineage().is_empty());
    assert!(query.diagnostics().iter().any(|diagnostic| {
        diagnostic.code() == "unresolved_output_lineage"
            && diagnostic.area() == DiagnosticArea::Output
    }));
}

#[test]
fn lateral_derived_table_can_reference_preceding_source() {
    let dialect = PostgreSqlDialect {};
    let protocol = analyze_sql(
        "SELECT d.order_id FROM orders o CROSS JOIN LATERAL (SELECT o.id AS order_id) d",
        "postgresql",
        &dialect,
    )
    .expect("LATERAL derived table should analyze");

    let lineage = first_query(&protocol).output().columns()[0].lineage();
    assert_eq!(lineage.len(), 1);
    assert_eq!(lineage[0].relation(), "orders");
    assert_eq!(lineage[0].column(), "id");
}

#[test]
fn local_alias_shadows_outer_scope_in_correlation_analysis() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "SELECT o.id FROM orders o WHERE EXISTS (SELECT 1 FROM archived_orders o WHERE o.id = 1)",
        "generic",
        &dialect,
    )
    .expect("shadowed subquery should analyze");

    let exists = match first_query(&protocol)
        .predicates()
        .where_predicate()
        .expect("WHERE predicate should exist")
    {
        Predicate::Exists(exists) => exists,
        other => panic!("expected EXISTS predicate, got {other:?}"),
    };

    assert!(exists.subquery().correlations().is_empty());
    assert_eq!(exists.subquery().dependencies(), ["archived_orders"]);
}

#[test]
fn unnest_and_table_functions_remain_explicit_when_schema_is_unresolved() {
    let bigquery = BigQueryDialect {};
    let unnest = analyze_sql(
        "SELECT * FROM UNNEST([1, 2, 3]) AS value",
        "bigquery",
        &bigquery,
    )
    .expect("UNNEST should remain analyzable");
    assert!(first_query(&unnest).diagnostics().iter().any(|diagnostic| {
        diagnostic.code() == "unsupported_table_factor"
            && diagnostic.area() == DiagnosticArea::Source
    }));

    let postgres = PostgreSqlDialect {};
    let table_function = analyze_sql(
        "SELECT * FROM generate_series(1, 3) AS g(value)",
        "postgresql",
        &postgres,
    )
    .expect("table function should remain analyzable");
    assert!(first_query(&table_function)
        .diagnostics()
        .iter()
        .any(|diagnostic| {
            diagnostic.code() == "unsupported_table_factor"
                && diagnostic.area() == DiagnosticArea::Source
        }));
}

#[test]
fn unsupported_nested_expression_stays_visible_inside_subquery() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "SELECT (SELECT CASE WHEN p.price > 0 THEN 1 ELSE 0 END FROM prices p) AS flag FROM orders",
        "generic",
        &dialect,
    )
    .expect("unsupported nested expression should remain analyzable");

    let subquery = match first_query(&protocol).output().columns()[0].expression() {
        Expression::ScalarSubquery(expression) => expression.subquery(),
        other => panic!("expected scalar subquery expression, got {other:?}"),
    };

    assert!(subquery
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code() == "unsupported_expression"));
}

#[test]
fn shared_in_subquery_syntax_is_analyzed_across_all_exposed_dialects() {
    let sql = "SELECT id FROM orders WHERE id IN (SELECT order_id FROM line_items)";

    for dialect_name in DIALECTS {
        let dialect =
            dialect_from_str(dialect_name).expect("documented dialect should be recognized");
        let protocol = analyze_sql(sql, dialect_name, dialect.as_ref())
            .unwrap_or_else(|error| panic!("dialect {dialect_name} failed subquery syntax: {error}"));

        assert!(
            matches!(
                first_query(&protocol)
                    .predicates()
                    .where_predicate()
                    .expect("WHERE predicate should exist"),
                Predicate::InSubquery(_)
            ),
            "dialect {dialect_name}"
        );
    }
}
