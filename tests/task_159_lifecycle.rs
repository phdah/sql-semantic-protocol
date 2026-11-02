//! TASK-159: canonical table lifecycle and complete DuckDB physical oracles.
//!
//! A vanished table is different from an empty live table, and a missing
//! caller-supplied snapshot is never proof of table absence.

use std::collections::BTreeMap;

use duckdb::Connection;
use sql_semantic_protocol::{
    analyze_inputs, dialect_from_name, physical_ordered_write_state_plan, to_bundle_json,
    LiteralValue, PhysicalWriteStatePlan, ProtocolStatement, SqlInput, WriteKind,
    WriteLifecycleAction,
};

fn analyze(sql: &[&str]) -> sql_semantic_protocol::AnalysisBundle {
    let dialect = dialect_from_name("duckdb").unwrap();
    let inputs = sql.iter().map(|sql| SqlInput::inline(*sql)).collect::<Vec<_>>();
    analyze_inputs(&inputs, "duckdb", dialect.as_ref()).unwrap()
}

#[test]
fn truncate_removes_every_duplicate_and_null_row_but_keeps_typed_relation() {
    let statements = [
        "CREATE TABLE t (a INTEGER)",
        "INSERT INTO t (a) VALUES (1), (NULL), (1)",
        "TRUNCATE TABLE t",
    ];
    let bundle = analyze(&statements);
    let ProtocolStatement::Query(query) = &bundle.inputs()[2].statements()[0] else {
        panic!("TRUNCATE must be a normalized canonical write");
    };
    assert_eq!(query.write().unwrap().kind(), WriteKind::Lifecycle);
    assert_eq!(query.write().unwrap().lifecycle_action(), Some(&WriteLifecycleAction::Truncate));

    let PhysicalWriteStatePlan::Feasible { transitions, final_states } =
        physical_ordered_write_state_plan(&bundle, &[])
    else {
        panic!("known, constraint-free table TRUNCATE must have an exact closed bag");
    };
    assert_eq!(transitions.len(), 3);
    let truncate = &transitions[2];
    assert!(truncate.before_exists() && truncate.after_exists());
    assert_eq!(truncate.before().rows().len(), 3);
    assert!(truncate.after().rows().is_empty());
    assert_eq!(truncate.affected().deleted(), 3);
    let schema = truncate.after_schema().unwrap();
    assert_eq!(schema.columns()[0].name(), "a");
    assert_eq!(truncate.after().exact_domains(schema).unwrap()[0].values(), &[]);
    assert_eq!(final_states.len(), 1);
    assert_eq!(final_states[0].relation(), "t");

    let native = Connection::open_in_memory().unwrap();
    for sql in statements {
        native.execute_batch(sql).unwrap();
    }
    assert_eq!(native.query_row("SELECT COUNT(*) FROM t", [], |r| r.get::<_, i64>(0)).unwrap(), 0);
    assert_eq!(
        native.query_row(
            "SELECT column_name FROM information_schema.columns WHERE table_name='t'",
            [],
            |r| r.get::<_, String>(0),
        ).unwrap(),
        "a"
    );
}

#[test]
fn drop_removes_relation_identity_not_merely_rows() {
    let statements = [
        "CREATE TABLE t (a INTEGER)",
        "INSERT INTO t (a) VALUES (1), (2)",
        "DROP TABLE t",
    ];
    let bundle = analyze(&statements);
    let PhysicalWriteStatePlan::Feasible { transitions, final_states } =
        physical_ordered_write_state_plan(&bundle, &[])
    else {
        panic!("full closed-world DROP requires an exact absent relation");
    };
    let drop = &transitions[2];
    assert_eq!(drop.kind(), WriteKind::Lifecycle);
    assert!(drop.before_exists());
    assert!(!drop.after_exists());
    assert_eq!(drop.before().rows().len(), 2);
    assert!(drop.after().rows().is_empty());
    assert_eq!(drop.affected().deleted(), 2);
    assert_eq!(drop.before_schema().unwrap().columns()[0].name(), "a");
    assert!(drop.after_schema().is_none());
    assert!(final_states.is_empty());

    let native = Connection::open_in_memory().unwrap();
    for sql in statements {
        native.execute_batch(sql).unwrap();
    }
    assert!(native.prepare("SELECT * FROM t").is_err());
}

#[test]
fn if_exists_requires_proven_absence_and_never_mistakes_missing_snapshot_for_absence() {
    let statements = [
        "CREATE TABLE t (a INTEGER)",
        "DROP TABLE t",
        "DROP TABLE IF EXISTS t",
    ];
    let bundle = analyze(&statements);
    let PhysicalWriteStatePlan::Feasible { transitions, final_states } =
        physical_ordered_write_state_plan(&bundle, &[])
    else {
        panic!("second DROP IF EXISTS must be an exact no-op after a proved DROP");
    };
    assert_eq!(transitions.len(), 3);
    assert!(!transitions[2].before_exists());
    assert!(!transitions[2].after_exists());
    assert_eq!(transitions[2].affected().deleted(), 0);
    assert!(final_states.is_empty());

    let native = Connection::open_in_memory().unwrap();
    for sql in statements {
        native.execute_batch(sql).unwrap();
    }

    assert!(matches!(
        physical_ordered_write_state_plan(&analyze(&["DROP TABLE IF EXISTS t"]), &[]),
        PhysicalWriteStatePlan::Residual { .. }
    ));
    assert!(matches!(
        physical_ordered_write_state_plan(
            &analyze(&["CREATE TABLE t(a INTEGER)", "DROP TABLE t", "DROP TABLE t"]),
            &[]
        ),
        PhysicalWriteStatePlan::Impossible { .. }
    ));
    assert!(native.execute_batch("DROP TABLE t").is_err());
}

#[test]
fn rename_preserves_row_occurrences_nulls_and_integer_domains() {
    let statements = [
        "CREATE TABLE t (a INTEGER)",
        "INSERT INTO t (a) VALUES (1), (NULL), (1)",
        "ALTER TABLE t RENAME TO u",
    ];
    let bundle = analyze(&statements);
    let PhysicalWriteStatePlan::Feasible { transitions, final_states } =
        physical_ordered_write_state_plan(&bundle, &[])
    else {
        panic!("constraint-free table RENAME preserves the full physical bag");
    };
    assert_eq!(transitions.len(), 3);
    let renamed = &transitions[2];
    assert!(renamed.before_exists() && renamed.after_exists());
    assert_eq!(renamed.before().relation(), "t");
    assert_eq!(renamed.after().relation(), "u");
    assert_eq!(renamed.before().rows(), renamed.after().rows());
    assert_eq!(renamed.affected().deleted(), 0);
    assert_eq!(renamed.after_schema().unwrap().relation(), "u");
    assert_eq!(final_states.len(), 1);
    assert_eq!(final_states[0].relation(), "u");
    assert_eq!(
        final_states[0].exact_domains(renamed.after_schema().unwrap()).unwrap()[0].values(),
        &[(LiteralValue::Number("1".into()), 2), (LiteralValue::Null, 1)]
    );

    let native = Connection::open_in_memory().unwrap();
    for sql in statements {
        native.execute_batch(sql).unwrap();
    }
    let mut query = native.prepare("SELECT a FROM u ORDER BY a NULLS LAST").unwrap();
    let rows: Vec<Option<i64>> = query
        .query_map([], |r| r.get::<_, Option<i64>>(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(rows, vec![Some(1), Some(1), None]);
    assert!(native.prepare("SELECT * FROM t").is_err());
}

#[test]
fn rename_existing_target_is_impossible_and_nonexistent_source_is_residual() {
    let collision = analyze(&[
        "CREATE TABLE t(a INTEGER)",
        "CREATE TABLE u(a INTEGER)",
        "ALTER TABLE t RENAME TO u",
    ]);
    assert!(matches!(
        physical_ordered_write_state_plan(&collision, &[]),
        PhysicalWriteStatePlan::Impossible { .. }
    ));
    assert!(matches!(
        physical_ordered_write_state_plan(&analyze(&["ALTER TABLE t RENAME TO u"]), &[]),
        PhysicalWriteStatePlan::Residual { .. }
    ));
}

#[test]
fn foreign_keys_or_unattested_lifecycle_options_fail_closed() {
    let constrained = analyze(&[
        "CREATE TABLE t (a INTEGER PRIMARY KEY)",
        "INSERT INTO t(a) VALUES (1)",
        "TRUNCATE TABLE t",
    ]);
    assert!(matches!(
        physical_ordered_write_state_plan(&constrained, &[]),
        PhysicalWriteStatePlan::Residual { .. }
    ));

    for sql in [
        "DROP MATERIALIZED VIEW mv",
        "DROP TABLE t CASCADE",
        "TRUNCATE TABLE t RESTART IDENTITY",
        "CREATE TEMPORARY TABLE t(a INTEGER)",
        "CREATE TABLE IF NOT EXISTS t(a INTEGER)",
        "CREATE TABLE copy LIKE t",
    ] {
        let dialect = dialect_from_name("duckdb").unwrap();
        if sqlparser::parser::Parser::parse_sql(dialect.as_ref(), sql).is_err() {
            continue;
        }
        let result = analyze(&[sql]);
        assert!(matches!(
            result.inputs()[0].statements()[0],
            ProtocolStatement::Unsupported(_)
        ), "unproved conditional variant must not masquerade as Exact: {sql}");
        assert!(matches!(
            physical_ordered_write_state_plan(&result, &[]),
            PhysicalWriteStatePlan::Residual { .. }
        ));
    }
}

#[test]
fn canonical_wire_records_lifecycle_kind_and_action_deterministically() {
    let bundle = analyze(&["DROP TABLE IF EXISTS t"]);
    let json: serde_json::Value = serde_json::from_str(&to_bundle_json(&bundle)).unwrap();
    let action = &json["inputs"][0]["statements"][0]["write"];
    assert_eq!(action["kind"], "relation_lifecycle");
    assert_eq!(action["lifecycle_action"]["kind"], "drop");
    assert_eq!(action["lifecycle_action"]["if_exists"], true);
    assert_eq!(to_bundle_json(&bundle), to_bundle_json(&bundle));

    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../schema/protocol.schema.json")).unwrap();
    assert!(schema["$defs"]["writeOperation"]["properties"]["kind"]["enum"]
        .as_array().unwrap()
        .contains(&serde_json::json!("relation_lifecycle")));
    assert_eq!(
        schema["$defs"]["writeOperation"]["allOf"][2]["then"]["required"],
        serde_json::json!(["lifecycle_action"])
    );
}

#[test]
fn source_free_snapshot_cannot_be_invented_even_with_typed_metadata() {
    let bundle = analyze(&["TRUNCATE TABLE t"]);
    let incomplete = sql_semantic_protocol::PhysicalWriteState::new("t", vec![]).unwrap();
    assert!(matches!(
        physical_ordered_write_state_plan(&bundle, &[incomplete]),
        PhysicalWriteStatePlan::Residual { .. }
    ));
}

#[test]
fn drop_does_not_apply_row_selection_or_invent_new_occurrences() {
    let statements = [
        "CREATE TABLE t(a INTEGER)",
        "INSERT INTO t(a) VALUES (1), (1), (NULL)",
        "DROP TABLE t",
    ];
    let b = analyze(&statements);
    let PhysicalWriteStatePlan::Feasible { transitions, .. } =
        physical_ordered_write_state_plan(&b, &[])
    else {
        panic!("complete row bag must drop all occurrences");
    };
    let before = &transitions[2].before();
    let values = before.rows().iter().map(|row| {
        row.values().get("a").cloned().unwrap()
    }).collect::<Vec<_>>();
    assert_eq!(
        values,
        vec![
            LiteralValue::Number("1".into()),
            LiteralValue::Number("1".into()),
            LiteralValue::Null,
        ]
    );
    assert_eq!(transitions[2].affected().deleted(), 3);
    assert!(transitions[2].after().rows().is_empty());

    // Complete physical rows are compared directly, not generated from SQL.
    let constructed = sql_semantic_protocol::PhysicalWriteRow::new(BTreeMap::from([
        ("a".to_owned(), LiteralValue::Number("1".to_owned())),
    ])).unwrap();
    assert_eq!(before.rows()[0], constructed);
    assert_eq!(before.rows()[1], constructed);
}
