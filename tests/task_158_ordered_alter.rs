//! TASK-158: canonical SQL ALTER statements and ordered complete-bag execution.
//! Native DuckDB supplies an independent before/after row and schema oracle.

use std::collections::BTreeMap;

use duckdb::Connection;
use sql_semantic_protocol::{
    analyze_inputs, dialect_from_name, physical_ordered_write_state_plan, LiteralValue,
    PhysicalWriteRow, PhysicalWriteStatePlan, ProtocolStatement, SqlInput,
    WriteKind,
};

fn bundle(statements: &[&str]) -> sql_semantic_protocol::AnalysisBundle {
    let dialect = dialect_from_name("duckdb").unwrap();
    let inputs = statements
        .iter()
        .map(|sql| SqlInput::inline(*sql))
        .collect::<Vec<_>>();
    analyze_inputs(&inputs, "duckdb", dialect.as_ref()).unwrap()
}

fn row(a: Option<i64>, b: Option<i64>, columns: (&str, &str)) -> PhysicalWriteRow {
    PhysicalWriteRow::new(BTreeMap::from([
        (
            columns.0.to_string(),
            a.map_or(LiteralValue::Null, |value| {
                LiteralValue::Number(value.to_string())
            }),
        ),
        (
            columns.1.to_string(),
            b.map_or(LiteralValue::Null, |value| {
                LiteralValue::Number(value.to_string())
            }),
        ),
    ]))
    .unwrap()
}

fn expected_bag(rows: &[PhysicalWriteRow]) -> Vec<String> {
    let mut result = rows
        .iter()
        .map(|r| format!("{:?}", r.values()))
        .collect::<Vec<_>>();
    result.sort();
    result
}

#[test]
fn create_insert_add_default_rename_executes_in_statement_order() {
    let sql = [
        "CREATE TABLE t (a INTEGER)",
        "INSERT INTO t (a) VALUES (1), (NULL), (1)",
        "ALTER TABLE t ADD COLUMN b INTEGER DEFAULT 2",
        "ALTER TABLE t RENAME COLUMN a TO c",
    ];
    let analyzed = bundle(&sql);
    let ProtocolStatement::Query(query) = &analyzed.inputs()[2].statements()[0] else {
        panic!("ALTER must retain canonical statement identity");
    };
    assert_eq!(query.write().unwrap().kind(), WriteKind::SchemaAlter);
    assert_eq!(query.write().unwrap().column_alter_steps().len(), 1);
    assert_eq!(analyzed.layers().len(), 4);

    let PhysicalWriteStatePlan::Feasible {
        transitions,
        final_states,
    } = physical_ordered_write_state_plan(&analyzed, &[])
    else {
        panic!("portable ordered ALTER must have a complete physical proof");
    };
    assert_eq!(transitions.len(), 4);
    assert_eq!(transitions[2].before().rows().len(), 3);
    assert_eq!(transitions[2].after().rows().len(), 3);
    assert_eq!(transitions[2].affected().inserted(), 0);
    let pre_schema = transitions[2].before_schema().unwrap();
    let add_schema = transitions[2].after_schema().unwrap();
    assert_eq!(pre_schema.columns()[0].name(), "a");
    assert_eq!(add_schema.columns()[1].name(), "b");
    assert_eq!(transitions[3].before_schema().unwrap(), add_schema);
    let final_schema = transitions[3].after_schema().unwrap();
    assert_eq!(final_schema.columns()[0].name(), "c");
    assert_eq!(final_schema.columns()[1].name(), "b");
    let final_state = &final_states[0];
    assert_eq!(
        expected_bag(final_state.rows()),
        expected_bag(&[
            row(Some(1), Some(2), ("c", "b")),
            row(None, Some(2), ("c", "b")),
            row(Some(1), Some(2), ("c", "b")),
        ])
    );
    let domains = final_state.exact_domains(final_schema).unwrap();
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
    for statement in &sql {
        db.execute_batch(statement).unwrap();
    }
    let mut native = db
        .prepare("SELECT c, b FROM t ORDER BY c NULLS LAST")
        .unwrap();
    let actual = native
        .query_map([], |r| {
            Ok((r.get::<_, Option<i64>>(0)?, r.get::<_, Option<i64>>(1)?))
        })
        .unwrap()
        .map(Result::unwrap)
        .collect::<Vec<_>>();
    assert_eq!(
        actual,
        vec![(Some(1), Some(2)), (Some(1), Some(2)), (None, Some(2))]
    );
    let native_columns = db
        .prepare("SELECT column_name FROM information_schema.columns WHERE table_name = 't' ORDER BY ordinal_position")
        .unwrap()
        .query_map([], |r| r.get::<_, String>(0))
        .unwrap()
        .map(Result::unwrap)
        .collect::<Vec<_>>();
    assert_eq!(native_columns, vec!["c", "b"]);
}

#[test]
fn drop_nullable_column_preserves_duplicate_row_occurrences() {
    let sql = [
        "CREATE TABLE t (a INTEGER, b INTEGER)",
        "INSERT INTO t (a, b) VALUES (1, NULL), (1, NULL), (NULL, 3)",
        "ALTER TABLE t DROP COLUMN b",
    ];
    let analyzed = bundle(&sql);
    let PhysicalWriteStatePlan::Feasible {
        transitions,
        final_states,
    } = physical_ordered_write_state_plan(&analyzed, &[])
    else {
        panic!("DROP must preserve full input bag");
    };
    assert_eq!(transitions.len(), 3);
    let schema = transitions[2].after_schema().unwrap();
    assert_eq!(schema.columns().len(), 1);
    let after = &final_states[0];
    assert_eq!(after.rows().len(), 3);
    assert_eq!(
        after.exact_domains(schema).unwrap()[0].values(),
        &[
            (LiteralValue::Number("1".into()), 2),
            (LiteralValue::Null, 1),
        ]
    );
    let db = Connection::open_in_memory().unwrap();
    for statement in sql {
        db.execute_batch(statement).unwrap();
    }
    let mut native = db.prepare("SELECT a FROM t ORDER BY a NULLS LAST").unwrap();
    let actual = native
        .query_map([], |r| r.get::<_, Option<i64>>(0))
        .unwrap()
        .map(Result::unwrap)
        .collect::<Vec<_>>();
    assert_eq!(actual, vec![Some(1), Some(1), None]);
}

#[test]
fn rejected_duplicate_is_impossible_and_cast_and_constraints_are_residual() {
    let duplicate = bundle(&[
        "CREATE TABLE t (a INTEGER)",
        "INSERT INTO t (a) VALUES (1)",
        "ALTER TABLE t ADD COLUMN a INTEGER",
    ]);
    assert!(matches!(
        physical_ordered_write_state_plan(&duplicate, &[]),
        PhysicalWriteStatePlan::Impossible { .. }
    ));
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE t(a INTEGER); INSERT INTO t VALUES (1)")
        .unwrap();
    assert!(db
        .execute_batch("ALTER TABLE t ADD COLUMN a INTEGER")
        .is_err());

    let typed = bundle(&[
        "CREATE TABLE t (a INTEGER)",
        "INSERT INTO t (a) VALUES (1)",
        "ALTER TABLE t ALTER COLUMN a SET DATA TYPE VARCHAR",
    ]);
    assert!(matches!(
        physical_ordered_write_state_plan(&typed, &[]),
        PhysicalWriteStatePlan::Residual { .. }
    ));

    let key = bundle(&[
        "CREATE TABLE t (a INTEGER PRIMARY KEY, b INTEGER)",
        "INSERT INTO t (a, b) VALUES (1, 2)",
        "ALTER TABLE t DROP COLUMN a",
    ]);
    assert!(matches!(
        physical_ordered_write_state_plan(&key, &[]),
        PhysicalWriteStatePlan::Residual { .. }
    ));
}

#[test]
fn unsupported_default_function_never_becomes_a_silent_null() {
    let analyzed = bundle(&[
        "CREATE TABLE t (a INTEGER)",
        "ALTER TABLE t ADD COLUMN b INTEGER DEFAULT (a + 1)",
    ]);
    assert!(matches!(
        analyzed.inputs()[1].statements()[0],
        ProtocolStatement::Unsupported(_)
    ));
    assert!(matches!(
        physical_ordered_write_state_plan(&analyzed, &[]),
        PhysicalWriteStatePlan::Residual { .. }
    ));
}
