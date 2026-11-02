//! TASK-160: ordered table lifecycle proofs against independent DuckDB snapshots.
//! A SELECT must not silently survive a known DROP in a write-only plan.

use duckdb::Connection;
use sql_semantic_protocol::{
    analyze_inputs, dialect_from_name, physical_ordered_write_state_plan, LiteralValue,
    PhysicalWriteStatePlan, SqlInput, WriteKind,
};

fn analyze(sql: &[&str]) -> sql_semantic_protocol::AnalysisBundle {
    let dialect = dialect_from_name("duckdb").unwrap();
    let inputs = sql
        .iter()
        .map(|statement| SqlInput::inline(*statement))
        .collect::<Vec<_>>();
    analyze_inputs(&inputs, "duckdb", dialect.as_ref()).unwrap()
}

#[test]
fn intermediate_read_matches_exact_pre_drop_bag_and_final_catalog_is_absent() {
    let sql = [
        "CREATE TABLE t (a INTEGER)",
        "INSERT INTO t (a) VALUES (1), (NULL), (1)",
        "ALTER TABLE t ADD COLUMN b INTEGER DEFAULT 2",
        "SELECT a, b FROM t",
        "DROP TABLE t",
    ];
    let bundle = analyze(&sql);
    let PhysicalWriteStatePlan::Feasible {
        transitions,
        final_states,
    } = physical_ordered_write_state_plan(&bundle, &[])
    else {
        panic!("bounded CREATE INSERT ALTER SELECT DROP should preserve ordered physical bags");
    };
    assert_eq!(transitions.len(), 4);
    assert_eq!(transitions[0].kind(), WriteKind::Definition);
    assert_eq!(transitions[1].kind(), WriteKind::Append);
    assert_eq!(transitions[2].kind(), WriteKind::SchemaAlter);
    assert_eq!(transitions[3].kind(), WriteKind::Lifecycle);
    assert!(final_states.is_empty(), "DROP must remove catalog identity");
    assert_eq!(transitions[3].before(), transitions[2].after());
    assert!(transitions[3].before_exists());
    assert!(!transitions[3].after_exists());
    assert_eq!(transitions[3].affected().deleted(), 3);
    let schema = transitions[3].before_schema().unwrap();
    let domains = transitions[3].before().exact_domains(schema).unwrap();
    assert_eq!(
        domains[0].values(),
        &[
            (LiteralValue::Number("1".into()), 2),
            (LiteralValue::Null, 1),
        ]
    );
    assert_eq!(
        domains[1].values(),
        &[(LiteralValue::Number("2".into()), 3)]
    );
    let db = Connection::open_in_memory().unwrap();
    for statement in &sql[..3] {
        db.execute_batch(statement).unwrap();
    }
    let mut native = db.prepare(sql[3]).unwrap();
    let rows = native
        .query_map([], |r| {
            Ok((r.get::<_, Option<i64>>(0)?, r.get::<_, i64>(1)?))
        })
        .unwrap()
        .collect::<duckdb::Result<Vec<_>>>()
        .unwrap();
    assert_eq!(rows, vec![(Some(1), 2), (None, 2), (Some(1), 2)]);
    db.execute_batch(sql[4]).unwrap();
    assert!(db.prepare("SELECT a, b FROM t").is_err());
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM information_schema.tables WHERE table_name='t'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .unwrap(),
        0
    );
}

#[test]
fn read_after_proven_drop_is_impossible_not_silently_skipped() {
    let sql = [
        "CREATE TABLE t (a INTEGER)",
        "INSERT INTO t (a) VALUES (1), (NULL), (1)",
        "DROP TABLE t",
        "SELECT a FROM t",
    ];
    let bundle = analyze(&sql);
    let PhysicalWriteStatePlan::Impossible { reason, .. } =
        physical_ordered_write_state_plan(&bundle, &[])
    else {
        panic!("SELECT from a known-dropped relation must never be reported Feasible");
    };
    assert!(reason.contains("dropped"), "{reason}");

    let db = Connection::open_in_memory().unwrap();
    for statement in &sql[..3] {
        db.execute_batch(statement).unwrap();
    }
    assert!(db.prepare(sql[3]).is_err());
}

#[test]
fn incomplete_prestate_is_not_proof_that_a_read_source_is_absent() {
    let bundle = analyze(&["SELECT a FROM t"]);
    // The write-only planner does not yet certify a standalone SELECT result;
    // absent caller input alone is not proof of a dropped relation.
    assert!(!matches!(
        physical_ordered_write_state_plan(&bundle, &[]),
        PhysicalWriteStatePlan::Impossible { .. }
    ));
}

#[test]
fn duplicate_column_is_impossible_before_later_read_or_drop() {
    let bundle = analyze(&[
        "CREATE TABLE t(a INTEGER)",
        "INSERT INTO t(a) VALUES (1)",
        "ALTER TABLE t ADD COLUMN a INTEGER",
        "SELECT a FROM t",
        "DROP TABLE t",
    ]);
    assert!(matches!(
        physical_ordered_write_state_plan(&bundle, &[]),
        PhysicalWriteStatePlan::Impossible { .. }
    ));
}

#[test]
fn task_160_scope_is_frozen_and_parent_acceptance_remains_open() {
    let freeze: serde_json::Value =
        serde_json::from_str(include_str!("../docs/task-160-acceptance-freeze.json")).unwrap();
    assert_eq!(freeze["owner"], "TASK-160");
    assert_eq!(freeze["parent"], "TASK-83");
    assert_eq!(freeze["version"], 1);
    assert_eq!(freeze["frozen_before_semantic_changes"], true);
    let ids = freeze["cases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|case| case["id"].as_str().unwrap())
        .collect::<std::collections::BTreeSet<_>>();
    assert!(ids.contains("83-C160-P-main"));
    assert!(ids.contains("83-C160-N-null-duplicates"));
    assert!(ids.contains("83-C160-N-drop-before-read"));
    assert!(freeze["closure_gates"]["parent_ac_independent_signoff"] == false);

    let manifest: serde_json::Value =
        serde_json::from_str(include_str!("../docs/coverage-manifest.json")).unwrap();
    assert_eq!(
        manifest["task_160_ordered_lifecycle_acceptance"]["freeze"],
        "docs/task-160-acceptance-freeze.json"
    );
    assert_eq!(
        manifest["task_160_ordered_lifecycle_acceptance"]["child_signed_off"],
        false
    );
}
