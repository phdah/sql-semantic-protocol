//! Public API acceptance seam for bounded TASK-158 physical column transitions.
//! DuckDB independently checks portable row and schema oracles.

use std::collections::BTreeMap;

use duckdb::Connection;
use sql_semantic_protocol::{
    physical_alter_column_snapshot, LiteralValue, PhysicalColumnAlterProof,
    PhysicalColumnAlterStep, PhysicalWriteRow, PhysicalWriteState, RelationSchema, SchemaColumn,
    SchemaSourceKind,
};

fn column(name: &str, data_type: &str) -> SchemaColumn {
    SchemaColumn::from_sql_type(name, data_type, "generic").unwrap()
}

fn schema(names: &[&str]) -> RelationSchema {
    RelationSchema::new(
        "t",
        names.iter().map(|name| column(name, "INTEGER")).collect(),
    )
    .unwrap()
    .with_source_kind(SchemaSourceKind::SqlDdl)
}

fn row(entries: &[(&str, Option<i64>)]) -> PhysicalWriteRow {
    PhysicalWriteRow::new(
        entries
            .iter()
            .map(|(name, value)| {
                (
                    (*name).to_string(),
                    value.map_or(LiteralValue::Null, |number| {
                        LiteralValue::Number(number.to_string())
                    }),
                )
            })
            .collect::<BTreeMap<_, _>>(),
    )
    .unwrap()
}

fn state(rows: Vec<PhysicalWriteRow>) -> PhysicalWriteState {
    PhysicalWriteState::new("t", rows).unwrap()
}

fn exact(
    source: &RelationSchema,
    rows: &PhysicalWriteState,
    operations: &[PhysicalColumnAlterStep],
) -> (RelationSchema, PhysicalWriteState) {
    match physical_alter_column_snapshot(source, rows, operations, Some(&[])) {
        PhysicalColumnAlterProof::Exact { schema, state } => (schema, state),
        other => panic!("expected an exact complete-bag transition: {other:?}"),
    }
}

#[test]
fn add_default_populates_every_occurrence_without_discarding_nulls() {
    let original = state(vec![row(&[("a", Some(1))]), row(&[("a", None)])]);
    let (changed_schema, changed) = exact(
        &schema(&["a"]),
        &original,
        &[PhysicalColumnAlterStep::Add {
            column: column("b", "INTEGER"),
            default: Some(LiteralValue::Number("2".to_string())),
        }],
    );
    assert_eq!(
        changed_schema
            .columns()
            .iter()
            .map(SchemaColumn::name)
            .collect::<Vec<_>>(),
        vec!["a", "b"]
    );
    assert_eq!(changed_schema.source_kind(), Some(SchemaSourceKind::SqlDdl));
    assert_eq!(
        changed.rows(),
        &[
            row(&[("a", Some(1)), ("b", Some(2))]),
            row(&[("a", None), ("b", Some(2))])
        ]
    );
    let domains = changed.exact_domains(&changed_schema).unwrap();
    assert_eq!(
        domains[1].values(),
        &[(LiteralValue::Number("2".to_string()), 2)]
    );

    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE t (a INTEGER); INSERT INTO t VALUES (1), (NULL); ALTER TABLE t ADD COLUMN b INTEGER DEFAULT 2;").unwrap();
    let mut query = db
        .prepare("SELECT a, b FROM t ORDER BY a NULLS LAST")
        .unwrap();
    let actual: Vec<(Option<i32>, i32)> = query
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(actual, vec![(Some(1), 2), (None, 2)]);
}

#[test]
fn missing_default_backfills_sql_null() {
    let original = state(vec![row(&[("a", Some(1))])]);
    let (changed_schema, changed) = exact(
        &schema(&["a"]),
        &original,
        &[PhysicalColumnAlterStep::Add {
            column: column("b", "INTEGER"),
            default: None,
        }],
    );
    assert_eq!(changed.rows(), &[row(&[("a", Some(1)), ("b", None)])]);
    assert_eq!(
        changed.exact_domains(&changed_schema).unwrap()[1].values(),
        &[(LiteralValue::Null, 1)]
    );

    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE t (a INTEGER); INSERT INTO t VALUES (1); ALTER TABLE t ADD COLUMN b INTEGER;",
    )
    .unwrap();
    let b: Option<i32> = db
        .query_row("SELECT b FROM t", [], |row| row.get(0))
        .unwrap();
    assert_eq!(b, None);
}

#[test]
fn rename_then_drop_preserves_duplicate_row_occurrences() {
    let original = state(vec![
        row(&[("a", Some(1)), ("b", Some(2))]),
        row(&[("a", Some(1)), ("b", Some(2))]),
        row(&[("a", None), ("b", Some(3))]),
    ]);
    let (changed_schema, changed) = exact(
        &schema(&["a", "b"]),
        &original,
        &[
            PhysicalColumnAlterStep::Rename {
                old: "a".into(),
                new: "c".into(),
            },
            PhysicalColumnAlterStep::Drop { column: "b".into() },
        ],
    );
    assert_eq!(
        changed_schema
            .columns()
            .iter()
            .map(SchemaColumn::name)
            .collect::<Vec<_>>(),
        vec!["c"]
    );
    assert_eq!(
        changed.rows(),
        &[
            row(&[("c", Some(1))]),
            row(&[("c", Some(1))]),
            row(&[("c", None)]),
        ]
    );
    assert_eq!(
        changed.exact_domains(&changed_schema).unwrap()[0].values(),
        &[
            (LiteralValue::Number("1".to_string()), 2),
            (LiteralValue::Null, 1),
        ]
    );

    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE t (a INTEGER, b INTEGER); INSERT INTO t VALUES (1,2),(1,2),(NULL,3); ALTER TABLE t RENAME COLUMN a TO c; ALTER TABLE t DROP COLUMN b;").unwrap();
    let mut stmt = db.prepare("SELECT c FROM t ORDER BY c NULLS LAST").unwrap();
    let values: Vec<Option<i32>> = stmt
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(values, vec![Some(1), Some(1), None]);
}

#[test]
fn duplicate_and_missing_columns_are_proven_impossible() {
    let old_schema = schema(&["a"]);
    let initial = state(vec![row(&[("a", Some(1))])]);
    for change in [
        PhysicalColumnAlterStep::Add {
            column: column("a", "INTEGER"),
            default: None,
        },
        PhysicalColumnAlterStep::Drop {
            column: "absent".into(),
        },
        PhysicalColumnAlterStep::Rename {
            old: "a".into(),
            new: "".into(),
        },
    ] {
        let outcome = physical_alter_column_snapshot(&old_schema, &initial, &[change], Some(&[]));
        assert!(
            matches!(outcome, PhysicalColumnAlterProof::Impossible { .. }),
            "{outcome:?}"
        );
    }
}

#[test]
fn data_type_rewrite_and_unattested_constraints_fail_closed() {
    let old_schema = schema(&["a"]);
    let initial = state(vec![row(&[("a", Some(1))])]);
    let operation = PhysicalColumnAlterStep::SetType {
        column: column("a", "VARCHAR"),
    };
    let result = physical_alter_column_snapshot(&old_schema, &initial, &[operation], Some(&[]));
    match result {
        PhysicalColumnAlterProof::Residual {
            before_schema,
            attempted_schema: Some(attempted_schema),
            ..
        } => {
            assert_eq!(
                before_schema.columns()[0].data_type(),
                column("a", "INTEGER").data_type()
            );
            assert_eq!(
                attempted_schema.columns()[0].data_type(),
                column("a", "VARCHAR").data_type()
            );
            assert_eq!(
                attempted_schema.source_kind(),
                Some(SchemaSourceKind::SqlDdl)
            );
        }
        other => panic!("unproved SQL cast must preserve typed schemas: {other:?}"),
    }

    let add = PhysicalColumnAlterStep::Add {
        column: column("b", "INTEGER"),
        default: None,
    };
    assert!(matches!(
        physical_alter_column_snapshot(&old_schema, &initial, std::slice::from_ref(&add), None),
        PhysicalColumnAlterProof::Residual { .. }
    ));
    let invalid_post = PhysicalColumnAlterStep::Add {
        column: column("b", "INTEGER"),
        default: Some(LiteralValue::Number("9999999999999".to_string())),
    };
    assert!(matches!(
        physical_alter_column_snapshot(&old_schema, &initial, &[invalid_post], Some(&[])),
        PhysicalColumnAlterProof::Impossible { .. }
    ));
}
