//! TASK-162: public transaction proof checked against independent DuckDB
//! complete physical catalogs at every statement boundary.

use std::collections::BTreeMap;

use duckdb::Connection;
use sql_semantic_protocol::{
    analyze_inputs, dialect_from_name, physical_duckdb_transaction_plan, AnalysisBundle,
    LiteralValue, PhysicalScriptCatalog, PhysicalTransactionAction, PhysicalTransactionScriptProof,
    PhysicalTransactionStep, PhysicalWriteRow, SqlInput,
};

fn analyze(sql: &str) -> AnalysisBundle {
    let dialect = dialect_from_name("duckdb").unwrap();
    analyze_inputs(&[SqlInput::inline(sql)], "duckdb", dialect.as_ref()).unwrap()
}

fn empty() -> PhysicalScriptCatalog {
    PhysicalScriptCatalog {
        states: Vec::new(),
        schemas: Vec::new(),
    }
}

fn row(id: i64, value: Option<i64>) -> PhysicalWriteRow {
    PhysicalWriteRow::new(BTreeMap::from([
        ("id".to_string(), LiteralValue::Number(id.to_string())),
        (
            "v".to_string(),
            value.map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string())),
        ),
    ]))
    .unwrap()
}

fn sorted_rows(rows: &[PhysicalWriteRow]) -> Vec<String> {
    let mut values = rows
        .iter()
        .map(|row| format!("{:?}", row.values()))
        .collect::<Vec<_>>();
    values.sort();
    values
}

fn native_rows(db: &Connection) -> Vec<PhysicalWriteRow> {
    let mut query = db.prepare("SELECT id, v FROM t ORDER BY rowid").unwrap();
    query
        .query_map([], |row| {
            Ok(PhysicalWriteRow::new(BTreeMap::from([
                (
                    "id".to_string(),
                    row.get::<_, Option<i64>>(0)?
                        .map_or(LiteralValue::Null, |v| LiteralValue::Number(v.to_string())),
                ),
                (
                    "v".to_string(),
                    row.get::<_, Option<i64>>(1)?
                        .map_or(LiteralValue::Null, |v| LiteralValue::Number(v.to_string())),
                ),
            ]))
            .unwrap())
        })
        .unwrap()
        .collect::<duckdb::Result<Vec<_>>>()
        .unwrap()
}

#[test]
fn rollback_discards_all_duplicate_and_null_occurrences_then_commit_retains_exact_read() {
    let create = analyze("CREATE TABLE t (id INTEGER, v INTEGER)");
    let discarded = analyze("INSERT INTO t (id, v) VALUES (1, 10), (2, NULL), (2, NULL)");
    let kept = analyze("INSERT INTO t (id, v) VALUES (2, 20), (3, NULL)");
    let select = analyze("SELECT id, v FROM t WHERE v >= 20");
    let steps = [
        PhysicalTransactionStep::Sql(&create),
        PhysicalTransactionStep::Begin,
        PhysicalTransactionStep::Sql(&discarded),
        PhysicalTransactionStep::Rollback,
        PhysicalTransactionStep::Begin,
        PhysicalTransactionStep::Sql(&kept),
        PhysicalTransactionStep::Commit,
        PhysicalTransactionStep::Sql(&select),
    ];
    let proof = physical_duckdb_transaction_plan(&steps, &empty());
    let PhysicalTransactionScriptProof::Exact(exact) = proof else {
        panic!("expected source-complete transaction proof: {proof:?}");
    };
    assert_eq!(exact.checkpoints.len(), steps.len());
    assert_eq!(exact.checkpoints[2].after.states[0].rows().len(), 3);
    assert_eq!(
        exact.checkpoints[2]
            .statement
            .as_ref()
            .unwrap()
            .write
            .as_ref()
            .unwrap()
            .affected()
            .inserted(),
        3
    );
    assert_eq!(
        exact.checkpoints[3].action,
        PhysicalTransactionAction::Rollback
    );
    assert_eq!(exact.checkpoints[3].after.states[0].rows().len(), 0);
    assert_eq!(
        exact.checkpoints[5]
            .statement
            .as_ref()
            .unwrap()
            .write
            .as_ref()
            .unwrap()
            .affected()
            .inserted(),
        2
    );
    assert_eq!(
        exact.final_catalog.states[0].rows(),
        &[row(2, Some(20)), row(3, None)]
    );
    let read = exact.checkpoints[7]
        .statement
        .as_ref()
        .unwrap()
        .read
        .as_ref()
        .unwrap();
    assert_eq!(read.output.rows(), &[row(2, Some(20))]);
    assert_eq!(read.surviving_source_ordinals, vec![0]);
    assert_eq!(read.rejected_source_ordinals, vec![1]);
    let domains = exact.final_catalog.exact_domains().unwrap();
    assert_eq!(
        domains[0].1[0].values(),
        &[
            (LiteralValue::Number("2".into()), 1),
            (LiteralValue::Number("3".into()), 1)
        ]
    );
    assert_eq!(
        domains[0].1[1].values(),
        &[
            (LiteralValue::Number("20".into()), 1),
            (LiteralValue::Null, 1)
        ]
    );

    let sql = [
        "CREATE TABLE t (id INTEGER, v INTEGER)",
        "BEGIN TRANSACTION",
        "INSERT INTO t (id, v) VALUES (1, 10), (2, NULL), (2, NULL)",
        "ROLLBACK",
        "BEGIN TRANSACTION",
        "INSERT INTO t (id, v) VALUES (2, 20), (3, NULL)",
        "COMMIT",
        "SELECT id, v FROM t WHERE v >= 20",
    ];
    let db = Connection::open_in_memory().unwrap();
    for (index, statement) in sql.iter().enumerate() {
        db.execute_batch(statement).unwrap();
        let expected = sorted_rows(&native_rows(&db));
        let actual = sorted_rows(exact.checkpoints[index].after.states[0].rows());
        assert_eq!(
            actual, expected,
            "complete DuckDB catalog mismatch at step {index}"
        );
        assert!(exact.checkpoints[index].after.exact_domains().is_some());
    }
    assert_eq!(
        sorted_rows(&native_rows(&db)),
        sorted_rows(exact.final_catalog.states[0].rows())
    );
}

#[test]
fn invalid_commit_and_rollback_are_not_exact_proofs() {
    for (step, sql) in [
        (PhysicalTransactionStep::Commit, "COMMIT"),
        (PhysicalTransactionStep::Rollback, "ROLLBACK"),
    ] {
        let proof = physical_duckdb_transaction_plan(&[step], &empty());
        assert!(
            matches!(
                proof,
                PhysicalTransactionScriptProof::Impossible { step_index: 0, .. }
            ),
            "{proof:?}"
        );
        let db = Connection::open_in_memory().unwrap();
        assert!(db.execute_batch(sql).is_err());
    }
}

#[test]
fn nested_begin_and_open_transaction_fail_closed() {
    let proof = physical_duckdb_transaction_plan(
        &[
            PhysicalTransactionStep::Begin,
            PhysicalTransactionStep::Begin,
        ],
        &empty(),
    );
    assert!(
        matches!(
            proof,
            PhysicalTransactionScriptProof::Impossible { step_index: 1, .. }
        ),
        "{proof:?}"
    );
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("BEGIN").unwrap();
    assert!(db.execute_batch("BEGIN").is_err());

    let open = physical_duckdb_transaction_plan(&[PhysicalTransactionStep::Begin], &empty());
    assert!(matches!(
        open,
        PhysicalTransactionScriptProof::Residual { step_index: 1, .. }
    ));
}

#[test]
fn savepoint_operators_are_not_inferred_from_parser_support() {
    for control in [
        PhysicalTransactionStep::Savepoint("sp"),
        PhysicalTransactionStep::RollbackTo("sp"),
        PhysicalTransactionStep::Release("sp"),
    ] {
        let proof =
            physical_duckdb_transaction_plan(&[PhysicalTransactionStep::Begin, control], &empty());
        assert!(
            matches!(
                proof,
                PhysicalTransactionScriptProof::Residual { step_index: 1, .. }
            ),
            "{proof:?}"
        );
    }
}

#[test]
fn unproved_sql_does_not_turn_into_an_exact_transaction() {
    let unsupported = analyze("SELECT 1 / 0");
    let proof = physical_duckdb_transaction_plan(
        &[
            PhysicalTransactionStep::Begin,
            PhysicalTransactionStep::Sql(&unsupported),
            PhysicalTransactionStep::Rollback,
        ],
        &empty(),
    );
    assert!(
        matches!(
            proof,
            PhysicalTransactionScriptProof::Residual { step_index: 1, .. }
        ),
        "{proof:?}"
    );
}
