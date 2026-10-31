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
    assert_eq!(
        duckdb_rows(&db, "SELECT k FROM inner_src"),
        [Some(2), Some(3)]
    );
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
    assert!(query
        .sources()
        .iter()
        .all(|source| source.name() == "shared"));
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
            Ok((row.get::<_, Option<i64>>(0)?, row.get::<_, Option<i64>>(1)?))
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

#[test]
fn recursive_local_name_does_not_borrow_inherited_producer_semantics() {
    // Recursive execution and termination belong to TASK-142. This assertion
    // only protects the binding: the inner recursive CTE must not silently
    // borrow the complete physical bag of the same-named outer CTE.
    let sql = "WITH shared AS (SELECT k FROM outer_src), \
               chosen AS (WITH RECURSIVE shared(k) AS ( \
                   SELECT 1 UNION ALL \
                   SELECT k + 1 FROM shared WHERE k < 3 \
               ) SELECT k FROM shared) \
               SELECT k FROM chosen";
    let query = analyzed_query(sql);
    assert!(
        !query
            .dependencies()
            .iter()
            .any(|source| source == "outer_src"),
        "recursive binding must not borrow an inherited physical source"
    );
    assert!(
        query
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "unresolved_local_relation_analysis"),
        "unproven recursive source requires an explicit residual diagnostic"
    );
}

#[test]
fn recursive_shadow_does_not_inherit_outer_output_domain() {
    // The recursive self-reference is not the already-complete outer CTE.
    // Without a recursive producer proof it cannot inherit its scalar domain.
    let outer = analyzed_query("SELECT 999 AS k");
    let inner = analyzed_query(
        "WITH shared AS (SELECT 999 AS k), \
         chosen AS (WITH RECURSIVE shared(k) AS (SELECT k FROM shared) \
         SELECT k FROM shared) SELECT k FROM chosen",
    );
    let outer_domain = outer.output().columns()[0].domain();
    let inner_domain = inner.output().columns()[0].domain();
    assert_ne!(
        inner_domain, outer_domain,
        "recursive self-reference borrowed a same-named outer producer domain"
    );
    assert!(
        inner
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "unresolved_local_relation_analysis"),
        "recursive physical producer must remain an explicit residual"
    );
}

#[test]
fn every_mandatory_frozen_cte_case_is_analyzed_across_exposed_dialects() {
    let freeze: serde_json::Value =
        serde_json::from_str(include_str!("../docs/task-141-acceptance-freeze.json")).unwrap();
    let mut checked = 0;
    for case in freeze["cases"].as_array().unwrap() {
        if case["classification"] != "mandatory_exact" {
            continue;
        }
        let id = case["id"].as_str().unwrap();
        let sql = case["sql"].as_str().unwrap();
        let expected_source = if id.ends_with("SHADOW") {
            "inner_src"
        } else {
            "s"
        };
        for dialect_name in DIALECTS {
            let dialect = dialect_from_name(dialect_name).unwrap();
            let protocol = analyze_sql(sql, dialect_name, dialect.as_ref())
                .unwrap_or_else(|error| panic!("{id}/{dialect_name}: {error}"));
            let Some(ProtocolStatement::Query(query)) = protocol.statements().first() else {
                panic!("{id}/{dialect_name}: missing analyzed query");
            };
            assert_eq!(
                query.dependencies(),
                [expected_source],
                "{id}/{dialect_name}: lexical physical source"
            );
            assert!(
                !query.output().columns().is_empty(),
                "{id}/{dialect_name}: missing output columns"
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 4 * DIALECTS.len());
}
