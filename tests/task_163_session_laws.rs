//! TASK-163: verify native DuckDB DDL rollback and fail closed on
//! transaction/session laws that cannot be established by parser support.

use duckdb::Connection;
use sql_semantic_protocol::{
    physical_duckdb_sql_transaction_plan_with_laws, PhysicalScriptCatalog,
    PhysicalTransactionScriptProof, PhysicalTransactionSessionLaw,
};

fn empty_catalog() -> PhysicalScriptCatalog {
    PhysicalScriptCatalog {
        states: Vec::new(),
        schemas: Vec::new(),
    }
}

fn assert_rows(db: &Connection, expected: &[(i64, Option<i64>)]) {
    let mut stmt = db.prepare("SELECT id, v FROM t ORDER BY rowid").unwrap();
    let observed = stmt
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, Option<i64>>(1)?))
        })
        .unwrap()
        .collect::<duckdb::Result<Vec<_>>>()
        .unwrap();
    assert_eq!(observed, expected);
}

#[test]
fn create_inside_transaction_is_rolled_back_without_implicit_commit() {
    let sql = [
        "BEGIN",
        "CREATE TABLE t (id INTEGER, v INTEGER)",
        "INSERT INTO t VALUES (1, 10), (1, NULL)",
        "ROLLBACK",
    ];
    let proof = physical_duckdb_sql_transaction_plan_with_laws(
        &sql,
        &empty_catalog(),
        &[PhysicalTransactionSessionLaw::DuckDbTransactionalDdlRollback],
    );
    let PhysicalTransactionScriptProof::Exact(exact) = proof else {
        panic!("expected a native DuckDB DDL rollback: {proof:?}");
    };
    assert_eq!(exact.checkpoints.len(), sql.len());
    assert!(exact.checkpoints[0].after.states.is_empty());
    assert_eq!(exact.checkpoints[2].after.states[0].rows().len(), 2);
    let inserted = exact.checkpoints[2]
        .statement
        .as_ref()
        .unwrap()
        .write
        .as_ref()
        .unwrap();
    assert_eq!(inserted.affected().inserted(), 2);
    let domains = exact.checkpoints[2].after.exact_domains().unwrap();
    assert_eq!(domains.len(), 1);
    assert_eq!(
        domains[0].1[0].values(),
        &[(sql_semantic_protocol::LiteralValue::Number("1".into()), 2)]
    );
    assert_eq!(
        domains[0].1[1].values(),
        &[
            (sql_semantic_protocol::LiteralValue::Number("10".into()), 1),
            (sql_semantic_protocol::LiteralValue::Null, 1),
        ]
    );
    assert!(exact.checkpoints[3].after.states.is_empty());
    assert!(exact.final_catalog.schemas.is_empty());

    let db = Connection::open_in_memory().unwrap();
    for (index, statement) in sql.iter().enumerate() {
        db.execute_batch(statement).unwrap();
        let exists: i64 = db
            .query_row(
                "SELECT count(*) FROM information_schema.tables WHERE table_name='t'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(exists, if index == 1 || index == 2 { 1 } else { 0 });
        if index == 1 {
            assert_rows(&db, &[]);
        }
        if index == 2 {
            assert_rows(&db, &[(1, Some(10)), (1, None)]);
        }
    }
}

#[test]
fn rollback_restores_dropped_relation_duplicate_nulls_and_rejected_read() {
    let sql = [
        "CREATE TABLE t (id INTEGER, v INTEGER)",
        "INSERT INTO t VALUES (1, 10), (1, NULL)",
        "BEGIN",
        "DROP TABLE t",
        "ROLLBACK",
        "SELECT id, v FROM t WHERE v >= 10",
    ];
    let proof = physical_duckdb_sql_transaction_plan_with_laws(
        &sql,
        &empty_catalog(),
        &[PhysicalTransactionSessionLaw::DuckDbTransactionalDdlRollback],
    );
    let PhysicalTransactionScriptProof::Exact(exact) = proof else {
        panic!("DROP TABLE rollback must preserve complete source bag: {proof:?}");
    };
    assert!(exact.checkpoints[3].after.states.is_empty());
    let drop = exact.checkpoints[3]
        .statement
        .as_ref()
        .unwrap()
        .write
        .as_ref()
        .unwrap();
    assert_eq!(drop.affected().deleted(), 2);
    assert_eq!(drop.affected_target_ordinals(), &[0, 1]);
    assert_eq!(exact.checkpoints[4].after, exact.checkpoints[2].after);
    assert_eq!(exact.final_catalog.states[0].rows().len(), 2);
    let restored_domains = exact.final_catalog.exact_domains().unwrap();
    assert_eq!(restored_domains[0].1[0].values().len(), 1);
    assert_eq!(restored_domains[0].1[1].values().len(), 2);
    assert_eq!(
        restored_domains[0].1[1].values()[1].0,
        sql_semantic_protocol::LiteralValue::Null
    );
    let read = exact.checkpoints[5]
        .statement
        .as_ref()
        .unwrap()
        .read
        .as_ref()
        .unwrap();
    assert_eq!(read.surviving_source_ordinals, vec![0]);
    assert_eq!(read.rejected_source_ordinals, vec![1]);
    assert_eq!(read.output.rows().len(), 1);

    let db = Connection::open_in_memory().unwrap();
    for (index, statement) in sql.iter().enumerate() {
        db.execute_batch(statement).unwrap();
        if index == 3 {
            assert!(db.prepare("SELECT * FROM t").is_err());
            assert!(exact.checkpoints[index].after.states.is_empty());
        } else {
            let expected: &[(i64, Option<i64>)] = if index == 0 {
                &[]
            } else {
                &[(1, Some(10)), (1, None)]
            };
            assert_rows(&db, expected);
            assert_eq!(
                exact.checkpoints[index].after.states[0].rows().len(),
                expected.len()
            );
        }
    }
}

#[test]
fn native_ddl_scope_does_not_prove_a_script_without_ddl_rollback() {
    let proof = physical_duckdb_sql_transaction_plan_with_laws(
        &["BEGIN", "COMMIT"],
        &empty_catalog(),
        &[PhysicalTransactionSessionLaw::DuckDbTransactionalDdlRollback],
    );
    assert!(matches!(
        proof,
        PhysicalTransactionScriptProof::Residual { step_index: 0, .. }
    ));
}

#[test]
fn vendor_and_cross_session_assumptions_never_inherit_native_duckdb_exactness() {
    for law in [
        PhysicalTransactionSessionLaw::TemporaryTableLifetime,
        PhysicalTransactionSessionLaw::DeferredForeignKeyChecks,
        PhysicalTransactionSessionLaw::ConcurrentIsolation,
        PhysicalTransactionSessionLaw::VendorImplicitDdlCommit,
        PhysicalTransactionSessionLaw::VendorTransactionOptions,
    ] {
        let proof = physical_duckdb_sql_transaction_plan_with_laws(
            &["BEGIN", "COMMIT"],
            &empty_catalog(),
            &[law],
        );
        match proof {
            PhysicalTransactionScriptProof::Residual {
                step_index: 0,
                reason,
            } => {
                assert!(reason.contains("TASK-163"), "{reason}");
                assert!(reason.contains(law.as_str()), "{reason}");
            }
            other => panic!("{law:?} must be Residual, found {other:?}"),
        }
    }
}

#[test]
fn unproved_sql_and_invalid_controls_are_not_upgraded_by_context() {
    let impossible = physical_duckdb_sql_transaction_plan_with_laws(
        &["COMMIT"],
        &empty_catalog(),
        &[PhysicalTransactionSessionLaw::VendorImplicitDdlCommit],
    );
    assert!(matches!(
        impossible,
        PhysicalTransactionScriptProof::Impossible { step_index: 0, .. }
    ));
    assert!(Connection::open_in_memory()
        .unwrap()
        .execute_batch("COMMIT")
        .is_err());
    let residual = physical_duckdb_sql_transaction_plan_with_laws(
        &["BEGIN", "SELECT 1 / 0", "ROLLBACK"],
        &empty_catalog(),
        &[PhysicalTransactionSessionLaw::DuckDbTransactionalDdlRollback],
    );
    match residual {
        PhysicalTransactionScriptProof::Residual {
            step_index: 1,
            reason,
        } => {
            assert!(reason.contains("TASK-163"), "{reason}");
            assert!(
                reason.contains("duckdb_transactional_ddl_rollback"),
                "{reason}"
            );
            assert!(reason
                .split(';')
                .next()
                .is_some_and(|part| !part.is_empty()));
        }
        other => panic!("earlier SQL uncertainty cannot be upgraded: {other:?}"),
    }
}

#[test]
fn task_163_active_matrix_and_immutable_preimplementation_freeze_are_versioned() {
    let freeze: serde_json::Value =
        serde_json::from_str(include_str!("../docs/task-163-acceptance-freeze.json")).unwrap();
    assert_eq!(freeze["version"], 2);
    let frozen_v1: serde_json::Value =
        serde_json::from_str(include_str!("../docs/task-163-acceptance-freeze-v1.json")).unwrap();
    assert_eq!(frozen_v1["version"], 1);
    assert_eq!(
        frozen_v1["status"],
        "frozen_before_implementation_unverified"
    );
    assert_eq!(freeze["owner"], "TASK-163");
    assert_eq!(freeze["dialects"].as_array().unwrap().len(), 13);
    assert_eq!(freeze["cases"].as_array().unwrap().len(), 8);
    assert_eq!(
        freeze["cases"][0]["id"],
        "84-C163-P-transactional-ddl-rollback"
    );
}

#[test]
fn ddl_committed_outside_transaction_survives_later_dml_rollback() {
    let sql = [
        "CREATE TABLE t (id INTEGER, v INTEGER)",
        "BEGIN",
        "INSERT INTO t VALUES (1, 10), (2, NULL)",
        "ROLLBACK",
    ];
    let proof = physical_duckdb_sql_transaction_plan_with_laws(&sql, &empty_catalog(), &[]);
    let PhysicalTransactionScriptProof::Exact(exact) = proof else {
        panic!("DDL before BEGIN must survive DML rollback: {proof:?}");
    };
    assert_eq!(exact.checkpoints[0].after.schemas.len(), 1);
    assert!(exact.checkpoints[0].after.states[0].rows().is_empty());
    assert_eq!(exact.checkpoints[2].after.states[0].rows().len(), 2);
    assert_eq!(exact.checkpoints[3].after, exact.checkpoints[0].after);
    assert!(exact.final_catalog.states[0].rows().is_empty());
    let domains = exact.final_catalog.exact_domains().unwrap();
    assert_eq!(domains[0].1.len(), 2);
    assert!(domains[0].1.iter().all(|domain| domain.values().is_empty()));

    let db = Connection::open_in_memory().unwrap();
    for (index, statement) in sql.iter().enumerate() {
        db.execute_batch(statement).unwrap();
        if index == 0 || index == 1 || index == 3 {
            assert_rows(&db, &[]);
        } else {
            assert_rows(&db, &[(1, Some(10)), (2, None)]);
        }
    }
}

#[test]
fn external_catalog_without_in_script_create_does_not_certify_drop() {
    let created = physical_duckdb_sql_transaction_plan_with_laws(
        &["CREATE TABLE t (id INTEGER, v INTEGER)"],
        &empty_catalog(),
        &[],
    );
    let PhysicalTransactionScriptProof::Exact(created) = created else {
        panic!("expected canonical CREATE: {created:?}");
    };
    let proof = physical_duckdb_sql_transaction_plan_with_laws(
        &["DROP TABLE t"],
        &created.final_catalog,
        &[],
    );
    assert!(
        matches!(
            proof,
            PhysicalTransactionScriptProof::Residual { step_index: 0, .. }
        ),
        "a caller-supplied bag is not trusted CREATE provenance: {proof:?}"
    );
}

#[test]
fn conditional_sql_dialect_and_engine_laws_are_never_promoted_by_parser_only() {
    let sql = ["CREATE TEMP TABLE temp_t (id INTEGER)"];
    let temp = physical_duckdb_sql_transaction_plan_with_laws(
        &sql,
        &empty_catalog(),
        &[PhysicalTransactionSessionLaw::TemporaryTableLifetime],
    );
    match temp {
        PhysicalTransactionScriptProof::Residual {
            step_index: 0,
            reason,
        } => {
            assert!(reason.contains("TASK-163"), "{reason}");
            assert!(reason.contains("temporary_table_lifetime"), "{reason}");
        }
        other => panic!("temporary table lifetime must be unproved: {other:?}"),
    }
    for sql in [
        "SET CONSTRAINTS ALL DEFERRED",
        "BEGIN TRANSACTION ISOLATION LEVEL SERIALIZABLE",
        "COMMIT AND CHAIN",
    ] {
        let proof = physical_duckdb_sql_transaction_plan_with_laws(
            &[sql],
            &empty_catalog(),
            &[PhysicalTransactionSessionLaw::VendorTransactionOptions],
        );
        assert!(
            matches!(proof, PhysicalTransactionScriptProof::Residual { .. }),
            "{sql} cannot establish native execution from parser support: {proof:?}"
        );
    }
}
