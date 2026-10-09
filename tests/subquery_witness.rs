mod common;

use common::DIALECTS;
use duckdb::Connection;
use sql_semantic_protocol::{
    analyze_inputs, analyze_sql, to_json, ComposedSemantics, ProtocolStatement, SqlInput,
    SubqueryMembershipCase, SubqueryMembershipDirection, SubqueryMembershipKind,
};
use sqlparser::dialect::{dialect_from_str, GenericDialect};

fn analyze(sql: &str) -> sql_semantic_protocol::Protocol {
    analyze_sql(sql, "generic", &GenericDialect {}).expect("valid SQL")
}

fn witness(sql: &str) -> sql_semantic_protocol::SubqueryMembershipWitness {
    let protocol = analyze(sql);
    let Some(ProtocolStatement::Query(query)) = protocol.statements().first() else {
        panic!("expected query statement");
    };
    query.subquery_witnesses().first().expect("witness").clone()
}

fn count(db: &Connection, sql: &str) -> i64 {
    db.query_row(sql, [], |row| row.get(0)).expect("DuckDB oracle")
}

#[test]
fn correlated_exists_and_not_exists_have_exact_opposing_cases() {
    for (keyword, qualifying, rejected) in [
        ("EXISTS", SubqueryMembershipCase::MatchingRow, SubqueryMembershipCase::NoCandidates),
        ("NOT EXISTS", SubqueryMembershipCase::NoCandidates, SubqueryMembershipCase::MatchingRow),
    ] {
        let sql = format!(
            "SELECT o.id FROM orders o WHERE {keyword} (SELECT 1 FROM lines l WHERE l.order_id = o.id)"
        );
        let item = witness(&sql);
        assert_eq!(item.correlations().len(), 1, "{keyword}");
        assert_eq!(item.correlations()[0].outer().relation(), Some("orders"));
        assert_eq!(item.correlations()[0].inner().relation(), Some("lines"));
        assert!(matches!(item.qualifying(), SubqueryMembershipDirection::Exact(cases) if cases.contains(&qualifying)));
        assert!(matches!(item.rejected(), SubqueryMembershipDirection::Exact(cases) if cases.contains(&rejected)));
    }

    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE orders(id INTEGER);
         CREATE TABLE lines(order_id INTEGER);
         INSERT INTO orders VALUES (1), (2), (3), (NULL);
         INSERT INTO lines VALUES (1), (1), (NULL);",
    ).unwrap();

    assert_eq!(count(&db, "SELECT COUNT(*) FROM orders o WHERE EXISTS (SELECT 1 FROM lines l WHERE l.order_id = o.id)"), 1);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM orders o WHERE NOT EXISTS (SELECT 1 FROM lines l WHERE l.order_id = o.id)"), 3);
    db.execute_batch("DELETE FROM lines;").unwrap();
    assert_eq!(count(&db, "SELECT COUNT(*) FROM orders o WHERE EXISTS (SELECT 1 FROM lines l WHERE l.order_id = o.id)"), 0);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM orders o WHERE NOT EXISTS (SELECT 1 FROM lines l WHERE l.order_id = o.id)"), 4);
}

#[test]
fn in_and_not_in_distinguish_empty_duplicates_and_null_poison() {
    let in_item = witness("SELECT o.id FROM orders o WHERE o.id IN (SELECT l.order_id FROM lines l)");
    assert_eq!(in_item.kind(), SubqueryMembershipKind::In);
    assert!(matches!(in_item.qualifying(), SubqueryMembershipDirection::Exact(cases) if cases == &[SubqueryMembershipCase::MatchingNonNullKey]));
    let not_in = witness("SELECT o.id FROM orders o WHERE o.id NOT IN (SELECT l.order_id FROM lines l)");
    assert_eq!(not_in.kind(), SubqueryMembershipKind::NotIn);
    assert!(matches!(not_in.qualifying(), SubqueryMembershipDirection::Exact(cases)
        if cases.contains(&SubqueryMembershipCase::NoCandidates)
        && cases.contains(&SubqueryMembershipCase::NoMatchNoNull)));
    assert!(matches!(not_in.rejected(), SubqueryMembershipDirection::Exact(cases)
        if cases.contains(&SubqueryMembershipCase::NoMatchNullCandidate)
        && cases.contains(&SubqueryMembershipCase::OuterNullNonempty)));

    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE orders(id INTEGER);
         CREATE TABLE lines(order_id INTEGER);
         INSERT INTO orders VALUES (1), (2), (3), (NULL);
         INSERT INTO lines VALUES (1), (1), (NULL);",
    ).unwrap();
    assert_eq!(count(&db, "SELECT COUNT(*) FROM orders o WHERE o.id IN (SELECT l.order_id FROM lines l)"), 1);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM orders o WHERE o.id NOT IN (SELECT l.order_id FROM lines l)"), 0);
    db.execute_batch("DELETE FROM lines WHERE order_id IS NULL;").unwrap();
    assert_eq!(count(&db, "SELECT COUNT(*) FROM orders o WHERE o.id NOT IN (SELECT l.order_id FROM lines l)"), 2);
    db.execute_batch("DELETE FROM lines;").unwrap();
    assert_eq!(count(&db, "SELECT COUNT(*) FROM orders o WHERE o.id IN (SELECT l.order_id FROM lines l)"), 0);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM orders o WHERE o.id NOT IN (SELECT l.order_id FROM lines l)"), 4);
}

#[test]
fn unsupported_subquery_shapes_retain_residual_directions() {
    for sql in [
        "SELECT o.id FROM orders o WHERE EXISTS (SELECT COUNT(*) FROM lines l)",
        "SELECT o.id FROM orders o WHERE EXISTS (SELECT 1 FROM lines l WHERE l.order_id > o.id)",
        "SELECT o.id FROM orders o WHERE o.id IN (SELECT l.order_id + 1 FROM lines l)",
        "SELECT o.id FROM orders o WHERE EXISTS (SELECT 1 FROM lines l LIMIT 0)",
    ] {
        let item = witness(sql);
        assert!(matches!(item.qualifying(), SubqueryMembershipDirection::Residual { .. }), "{sql}");
        assert!(matches!(item.rejected(), SubqueryMembershipDirection::Residual { .. }), "{sql}");
    }
}

#[test]
fn membership_witnesses_are_emitted_in_local_and_composed_contract() {
    let sql = "CREATE TABLE current_orders AS SELECT o.id FROM orders o WHERE o.id IN (SELECT l.order_id FROM lines l)";
    let bundle = analyze_inputs(&[SqlInput::inline(sql)], "generic", &GenericDialect {}).unwrap();
    let layer = bundle.layers().first().expect("layer");
    let ComposedSemantics::Resolved(composed) = layer.composed_semantics() else {
        panic!("composed semantics should resolve");
    };
    assert_eq!(composed.subquery_witnesses().len(), 1);
    assert_eq!(composed.subquery_witnesses()[0].origin_layer_id(), layer.id());

    let protocol = analyze(sql);
    let emitted: serde_json::Value = serde_json::from_str(&to_json(&protocol)).unwrap();
    assert_eq!(emitted["inputs"][0]["statements"][0]["subquery_witnesses"][0]["qualifying"]["status"], "exact");
    assert_eq!(emitted["inputs"][0]["statements"][0]["subquery_witnesses"][0]["operator"], "in");
}

#[test]
fn shared_exists_and_in_syntax_is_consistent_across_dialects() {
    for dialect_name in DIALECTS {
        let dialect = dialect_from_str(dialect_name).expect("supported dialect");
        for sql in [
            "SELECT o.id FROM orders o WHERE EXISTS (SELECT 1 FROM lines l WHERE l.order_id = o.id)",
            "SELECT o.id FROM orders o WHERE o.id IN (SELECT l.order_id FROM lines l)",
        ] {
            let protocol = analyze_sql(sql, dialect_name, dialect.as_ref())
                .unwrap_or_else(|error| panic!("{dialect_name} failed: {error}"));
            let Some(ProtocolStatement::Query(query)) = protocol.statements().first() else {
                panic!("{dialect_name} did not produce a query");
            };
            assert_eq!(query.subquery_witnesses().len(), 1, "{dialect_name}");
            assert!(matches!(query.subquery_witnesses()[0].qualifying(), SubqueryMembershipDirection::Exact(_)), "{dialect_name}");
        }
    }
}
