//! TASK-162: transaction control SQL must cross the parser boundary once.
//! These tests compare entire typed catalogs to independent native DuckDB bags.

use std::collections::BTreeMap;

use duckdb::Connection;
use sql_semantic_protocol::{
    physical_duckdb_sql_transaction_plan, LiteralValue, PhysicalScriptCatalog,
    PhysicalTransactionScriptProof, PhysicalWriteRow,
};

fn empty() -> PhysicalScriptCatalog {
    PhysicalScriptCatalog {
        states: vec![],
        schemas: vec![],
    }
}

fn row(id: i64, v: Option<i64>) -> PhysicalWriteRow {
    PhysicalWriteRow::new(BTreeMap::from([
        ("id".to_string(), LiteralValue::Number(id.to_string())),
        (
            "v".to_string(),
            v.map_or(LiteralValue::Null, |x| LiteralValue::Number(x.to_string())),
        ),
    ]))
    .unwrap()
}

fn sorted_rows(rows: &[PhysicalWriteRow]) -> Vec<String> {
    let mut rows = rows
        .iter()
        .map(|r| format!("{:?}", r.values()))
        .collect::<Vec<_>>();
    rows.sort();
    rows
}

#[test]
fn full_sql_script_rolls_back_a_created_relation_and_commits_later_rows() {
    let sql = [
        "BEGIN TRANSACTION",
        "CREATE TABLE t (id INTEGER, v INTEGER)",
        "INSERT INTO t (id, v) VALUES (1, 10), (1, NULL)",
        "ROLLBACK",
        "CREATE TABLE t (id INTEGER, v INTEGER)",
        "BEGIN TRANSACTION",
        "INSERT INTO t (id, v) VALUES (2, 20), (3, NULL)",
        "COMMIT",
        "SELECT id, v FROM t WHERE v >= 20",
    ];
    let proof = physical_duckdb_sql_transaction_plan(&sql, &empty());
    let PhysicalTransactionScriptProof::Exact(exact) = proof else {
        panic!("transaction SQL frontend failed: {proof:?}");
    };
    assert_eq!(exact.checkpoints.len(), sql.len());
    assert!(exact.checkpoints[3].after.states.is_empty());
    assert!(exact.checkpoints[3].after.schemas.is_empty());
    assert_eq!(
        exact.final_catalog.states[0].rows(),
        &[row(2, Some(20)), row(3, None)]
    );
    let read = exact.checkpoints[8]
        .statement
        .as_ref()
        .unwrap()
        .read
        .as_ref()
        .unwrap();
    assert_eq!(read.output.rows(), &[row(2, Some(20))]);
    assert_eq!(read.rejected_source_ordinals, vec![1]);

    let db = Connection::open_in_memory().unwrap();
    for (index, statement) in sql.iter().enumerate() {
        if index == 8 {
            break;
        }
        db.execute_batch(statement).unwrap();
        if index == 3 {
            let absent: i64 = db
                .query_row(
                    "SELECT count(*) FROM information_schema.tables WHERE table_name='t'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(absent, 0, "DuckDB ROLLBACK must remove CREATE TABLE");
        }
    }
    let actual = db
        .prepare("SELECT id, v FROM t ORDER BY rowid")
        .unwrap()
        .query_map([], |r| {
            Ok(row(r.get::<_, i64>(0)?, r.get::<_, Option<i64>>(1)?))
        })
        .unwrap()
        .collect::<duckdb::Result<Vec<_>>>()
        .unwrap();
    assert_eq!(
        sorted_rows(exact.final_catalog.states[0].rows()),
        sorted_rows(&actual)
    );
}

#[test]
fn single_sql_transaction_boundaries_reject_invalid_controls_and_batching() {
    for sql in [
        ["COMMIT"].as_slice(),
        ["ROLLBACK"].as_slice(),
        ["BEGIN", "BEGIN"].as_slice(),
    ] {
        let proof = physical_duckdb_sql_transaction_plan(sql, &empty());
        assert!(
            matches!(proof, PhysicalTransactionScriptProof::Impossible { .. }),
            "{proof:?}"
        );
    }
    for sql in [
        "BEGIN; COMMIT",
        "BEGIN TRANSACTION READ ONLY",
        "COMMIT AND CHAIN",
        "ROLLBACK AND CHAIN",
    ] {
        let proof = physical_duckdb_sql_transaction_plan(&[sql], &empty());
        assert!(
            matches!(proof, PhysicalTransactionScriptProof::Residual { .. }),
            "{sql}: {proof:?}"
        );
    }
}

#[test]
fn sql_savepoint_forms_are_conditional_and_never_exact() {
    for sql in [
        "SAVEPOINT s",
        "ROLLBACK TO SAVEPOINT s",
        "RELEASE SAVEPOINT s",
    ] {
        let proof = physical_duckdb_sql_transaction_plan(&["BEGIN", sql], &empty());
        assert!(
            matches!(proof, PhysicalTransactionScriptProof::Residual { .. }),
            "{sql}: {proof:?}"
        );
        // DuckDB 1.10506 is pinned in Cargo; do not claim engine support
        // from sqlparser accepting a vendor's syntax.
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch("BEGIN").unwrap();
        assert!(
            db.execute_batch(sql).is_err(),
            "native DuckDB unexpectedly accepted {sql}"
        );
    }
}

#[test]
fn false_exactness_is_prevented_for_unproved_and_mixed_sql_inputs() {
    for sql in [
        ["BEGIN", "SELECT 1 / 0", "ROLLBACK"].as_slice(),
        ["CREATE TABLE t (id INTEGER); BEGIN"].as_slice(),
        ["CREATE TABLE t (id INTEGER); INVALID SQL"].as_slice(),
    ] {
        let proof = physical_duckdb_sql_transaction_plan(sql, &empty());
        assert!(
            matches!(proof, PhysicalTransactionScriptProof::Residual { .. }),
            "{proof:?}"
        );
    }
}

#[test]
fn rollback_restores_updated_deleted_and_untouched_rows_on_same_physical_bag() {
    let sql = [
        "CREATE TABLE t (id INTEGER, v INTEGER)",
        "INSERT INTO t (id, v) VALUES (1, 10), (2, NULL), (2, NULL)",
        "BEGIN TRANSACTION",
        "UPDATE t SET v = 99 WHERE id = 1",
        "DELETE FROM t WHERE id = 2",
        "ROLLBACK",
        "SELECT id, v FROM t",
    ];
    let proof = physical_duckdb_sql_transaction_plan(&sql, &empty());
    let PhysicalTransactionScriptProof::Exact(exact) = proof else {
        panic!("expected exact rollback of the full DML bag: {proof:?}");
    };
    let original = &exact.checkpoints[1].after;
    assert_eq!(exact.checkpoints[5].after, *original);
    assert_eq!(
        exact.final_catalog.states[0].rows(),
        &[row(1, Some(10)), row(2, None), row(2, None)]
    );
    let read = exact.checkpoints[6]
        .statement
        .as_ref()
        .unwrap()
        .read
        .as_ref()
        .unwrap();
    assert_eq!(read.surviving_source_ordinals, vec![0, 1, 2]);
    assert!(read.rejected_source_ordinals.is_empty());
    assert_eq!(read.output.rows().len(), 3);

    let db = Connection::open_in_memory().unwrap();
    for (step, statement) in sql.iter().enumerate() {
        if step != 6 {
            db.execute_batch(statement).unwrap();
        }
        let observed = db
            .prepare("SELECT id, v FROM t ORDER BY rowid")
            .unwrap()
            .query_map([], |r| {
                Ok(row(r.get::<_, i64>(0)?, r.get::<_, Option<i64>>(1)?))
            })
            .unwrap()
            .collect::<duckdb::Result<Vec<_>>>()
            .unwrap();
        assert_eq!(
            sorted_rows(&observed),
            sorted_rows(exact.checkpoints[step].after.states[0].rows()),
            "independent complete physical bag mismatch at SQL step {step}"
        );
    }
}
