//! TASK-141 lexical source identity regressions and independent SQL bag oracles.
//! Exact physical-plan certification of repeated producers remains a separate
//! acceptance gate; parser/lineage and a DuckDB output are not that proof.

mod common;

use common::DIALECTS;
use duckdb::Connection;
use sql_semantic_protocol::{analyze_sql, dialect_from_name, ProtocolStatement, QueryStatement};
use sqlparser::dialect::GenericDialect;

fn analyzed_query(sql: &str) -> QueryStatement {
    let protocol = analyze_sql(sql, "generic", &GenericDialect {}).unwrap();
    match protocol.statements().first() {
        Some(ProtocolStatement::Query(query)) => query.clone(),
        other => panic!("expected query statement: {other:?}"),
    }
}

fn duckdb_rows(db: &Connection, sql: &str) -> Vec<Option<i64>> {
    let mut statement = db.prepare(sql).unwrap();
    statement
        .query_map([], |row| row.get::<_, Option<i64>>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
}

#[test]
fn ordinary_with_does_not_bind_earlier_cte_to_later_sibling() {
    // The later definition is not visible in the earlier definition's scope.
    // It remains an external dependency instead of disappearing as a
    // prematurely declared but unresolved local CTE.
    let query = analyzed_query(
        "WITH early AS (SELECT k FROM later), later AS (SELECT k FROM raw) \
         SELECT k FROM early",
    );
    assert_eq!(query.dependencies(), ["later"]);
    assert!(query
        .diagnostics()
        .iter()
        .all(|diagnostic| diagnostic.code() != "unresolved_local_relation_analysis"));
}

#[test]
fn nested_cte_shadowing_keeps_the_inner_physical_source() {
    let sql = "WITH shared AS (SELECT k FROM outer_src), \
               chosen AS (WITH shared AS (SELECT k FROM inner_src) \
               SELECT k FROM shared) \
               SELECT k FROM chosen ORDER BY k";
    let query = analyzed_query(sql);
    assert_eq!(query.dependencies(), ["inner_src"]);

    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE outer_src(k INTEGER); \
         INSERT INTO outer_src VALUES (100); \
         CREATE TABLE inner_src(k INTEGER); \
         INSERT INTO inner_src VALUES (2),(3);",
    )
    .unwrap();
    assert_eq!(duckdb_rows(&db, "SELECT k FROM outer_src"), [Some(100)]);
    assert_eq!(duckdb_rows(&db, "SELECT k FROM inner_src"), [Some(2), Some(3)]);
    assert_eq!(duckdb_rows(&db, sql), [Some(2), Some(3)]);
}

#[test]
fn reused_cte_aliases_share_one_duplicate_and_null_aware_source_bag() {
    let sql = "WITH shared AS (SELECT k FROM s) \
               SELECT a.k, b.k FROM shared AS a \
               JOIN shared AS b ON a.k = b.k \
               ORDER BY a.k, b.k";
    let query = analyzed_query(sql);
    assert_eq!(query.dependencies(), ["s"]);
    assert_eq!(query.sources().len(), 2);
    assert!(query.sources().iter().all(|source| source.name() == "shared"));
    assert_eq!(query.sources()[0].alias(), Some("a"));
    assert_eq!(query.sources()[1].alias(), Some("b"));

    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE s(k INTEGER); INSERT INTO s VALUES (1),(2),(2),(NULL);")
        .unwrap();
    assert_eq!(
        duckdb_rows(&db, "SELECT k FROM s ORDER BY k NULLS LAST"),
        [Some(1), Some(2), Some(2), None]
    );
    let mut statement = db.prepare(sql).unwrap();
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, Option<i64>>(0)?,
                row.get::<_, Option<i64>>(1)?,
            ))
        })
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(
        rows,
        [
            (Some(1), Some(1)),
            (Some(2), Some(2)),
            (Some(2), Some(2)),
            (Some(2), Some(2)),
            (Some(2), Some(2)),
        ]
    );
    assert!(!rows.contains(&(None, None)));
    assert!(!rows.contains(&(Some(1), Some(2))));
}

#[test]
fn shared_ordinary_cte_resolves_to_one_physical_relation_across_all_dialects() {
    let sql = "WITH shared AS (SELECT k FROM s) \
               SELECT a.k FROM shared AS a JOIN shared AS b ON a.k = b.k";
    for name in DIALECTS {
        let dialect = dialect_from_name(name).unwrap();
        let protocol = analyze_sql(sql, name, dialect.as_ref())
            .unwrap_or_else(|error| panic!("{name}: shared WITH syntax: {error}"));
        let Some(ProtocolStatement::Query(query)) = protocol.statements().first() else {
            panic!("{name}: expected query");
        };
        assert_eq!(query.dependencies(), ["s"], "{name}: physical identity");
        assert_eq!(query.sources().len(), 2, "{name}: two SQL source instances");
        assert_eq!(query.sources()[0].name(), "shared", "{name}");
        assert_eq!(query.sources()[1].name(), "shared", "{name}");
        assert_eq!(query.sources()[0].alias(), Some("a"), "{name}");
        assert_eq!(query.sources()[1].alias(), Some("b"), "{name}");
    }
}
