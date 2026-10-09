//! Deterministic DML state-conservation and canonical constraint evidence tests.

mod common;

use duckdb::Connection;
use sql_semantic_protocol::{
    analyze_inputs, dialect_from_name, to_bundle_json, RelationConstraint, SqlInput,
    WriteCardinalityRule, WriteCountError, WritePostState, WriteRowCounts,
};

fn bundle(sql: &str, dialect_name: &str) -> sql_semantic_protocol::AnalysisBundle {
    let dialect = dialect_from_name(dialect_name).expect("dialect");
    analyze_inputs(&[SqlInput::inline(sql)], dialect_name, dialect.as_ref())
        .expect("analyze DML")
}

fn count(db: &Connection, relation: &str) -> u64 {
    db.query_row(
        &format!("SELECT COUNT(*) FROM {relation}"),
        [],
        |row| row.get::<_, i64>(0),
    )
    .expect("count target rows") as u64
}

#[test]
fn row_conservation_matches_insert_update_delete_and_merge_in_duckdb() {
    struct Scenario {
        sql: &'static str,
        dialect: &'static str,
        setup: &'static str,
        inserted: u64,
        updated: u64,
        deleted: u64,
        rule: WriteCardinalityRule,
    }
    let cases = [
        Scenario {
            sql: "INSERT INTO target (id, score) SELECT id, score FROM source",
            dialect: "generic",
            setup: "CREATE TABLE target(id INTEGER PRIMARY KEY, score INTEGER);
                    CREATE TABLE source(id INTEGER, score INTEGER);
                    INSERT INTO target VALUES (1, 5);
                    INSERT INTO source VALUES (2, 8), (3, NULL);",
            inserted: 2, updated: 0, deleted: 0,
            rule: WriteCardinalityRule::Append,
        },
        Scenario {
            sql: "UPDATE target SET score = 7 WHERE id = 2",
            dialect: "generic",
            setup: "CREATE TABLE target(id INTEGER PRIMARY KEY, score INTEGER);
                    INSERT INTO target VALUES (1, 5), (2, NULL), (3, 8);",
            inserted: 0, updated: 1, deleted: 0,
            rule: WriteCardinalityRule::Preserve,
        },
        Scenario {
            sql: "DELETE FROM target WHERE score IS NULL",
            dialect: "generic",
            setup: "CREATE TABLE target(id INTEGER PRIMARY KEY, score INTEGER);
                    INSERT INTO target VALUES (1, 5), (2, NULL), (3, NULL);",
            inserted: 0, updated: 0, deleted: 2,
            rule: WriteCardinalityRule::SubtractDeletes,
        },
        Scenario {
            sql: "MERGE INTO target AS t USING source AS s ON t.id = s.id
                  WHEN MATCHED THEN UPDATE SET score = s.score
                  WHEN NOT MATCHED THEN INSERT (id, score) VALUES (s.id, s.score)",
            dialect: "snowflake",
            setup: "CREATE TABLE target(id INTEGER PRIMARY KEY, score INTEGER);
                    CREATE TABLE source(id INTEGER, score INTEGER);
                    INSERT INTO target VALUES (1, 5), (3, NULL);
                    INSERT INTO source VALUES (1, 7), (2, 9);",
            inserted: 1, updated: 1, deleted: 0,
            rule: WriteCardinalityRule::Merge,
        },
    ];
    for case in cases {
        let b = bundle(case.sql, case.dialect);
        let write = b.write_state_effects();
        assert_eq!(write.len(), 1, "{}", case.sql);
        let effect = write[0].effect();
        assert_eq!(effect.cardinality_rule(), case.rule, "{}", case.sql);
        assert_eq!(effect.post_state(), WritePostState::ApplyToInitial);
        assert_eq!(effect.affected_rows().minimum(), 0);
        assert_eq!(effect.affected_rows().maximum(), None);

        let db = Connection::open_in_memory().expect("DuckDB");
        db.execute_batch(case.setup).expect("populate initial target and source");
        let initial = count(&db, "target");
        db.execute_batch(case.sql).expect("execute DML successfully");
        let observed = count(&db, "target");
        let predicted = effect.resulting_rows(
            initial, WriteRowCounts::new(case.inserted, case.updated, case.deleted)
        ).expect("verified target row counts must conserve rows");
        assert_eq!(observed, predicted, "{}", case.sql);
    }
}

#[test]
fn count_proofs_reject_impossible_actions_oversubscribed_rows_and_overflow() {
    let insert = bundle("INSERT INTO target SELECT id FROM src", "generic");
    let effects = insert.write_state_effects();
    let effect = effects[0].effect();
    assert_eq!(effect.resulting_rows(2, WriteRowCounts::new(1, 1, 0)),
               Err(WriteCountError::InvalidActionCounts));
    assert_eq!(effect.resulting_rows(0, WriteRowCounts::new(1, 0, 1)),
               Err(WriteCountError::ExceedsInitialRows));
    assert_eq!(effect.resulting_rows(u64::MAX, WriteRowCounts::new(1, 0, 0)),
               Err(WriteCountError::Overflow));

    let update = bundle("UPDATE target SET id = 1", "generic");
    let effects = update.write_state_effects();
    assert_eq!(effects[0].effect().resulting_rows(2, WriteRowCounts::new(0, 3, 0)),
               Err(WriteCountError::ExceedsInitialRows));
    assert_eq!(effects[0].effect().resulting_rows(2, WriteRowCounts::new(0, 1, 0)),
               Ok(2));

    let delete = bundle("DELETE FROM target", "generic");
    let effects = delete.write_state_effects();
    assert_eq!(effects[0].effect().resulting_rows(3, WriteRowCounts::new(0, 0, 2)),
               Err(WriteCountError::UnconditionalDeleteMismatch));
    assert_eq!(effects[0].effect().resulting_rows(3, WriteRowCounts::new(0, 0, 3)),
               Ok(0));
}

#[test]
fn key_evidence_is_bound_to_correct_target_and_never_fabricated() {
    let b = bundle(
        "CREATE TABLE target(id INTEGER PRIMARY KEY, score INTEGER);
         INSERT INTO target (id, score) SELECT id, score FROM source;
         UPDATE other SET score = 3",
        "generic"
    );
    let effects = b.write_state_effects();
    assert_eq!(effects.len(), 2);
    assert_eq!(effects[0].target(), "target");
    assert_eq!(effects[0].sources(), &["source".to_string()]);
    let constraints = effects[0].target_constraints().expect("DDL key evidence");
    assert!(constraints.constraints().iter().any(|constraint|
        matches!(constraint, RelationConstraint::PrimaryKey(key) if key.columns() == ["id"])
    ));
    assert_eq!(effects[1].target(), "other");
    assert!(effects[1].target_constraints().is_none());

    let value: serde_json::Value = serde_json::from_str(&to_bundle_json(&b)).expect("valid JSON");
    assert_eq!(value["write_effects"][0]["target"], "target");
    assert_eq!(value["write_effects"][0]["sources"][0], "source");
    assert!(value["write_effects"][0]["target_constraints"]["constraints"].is_array());
    assert!(value["write_effects"][1]["target_constraints"].is_null());
    assert_eq!(
        value["write_effects"][0]["state_effect"]["cardinality_rule"],
        "initial_plus_inserted"
    );

    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE target(id INTEGER PRIMARY KEY, score INTEGER);
         CREATE TABLE source(id INTEGER, score INTEGER);
         INSERT INTO target VALUES (1, 2);
         INSERT INTO source VALUES (1, 3);"
    ).unwrap();
    let initial = count(&db, "target");
    assert!(db.execute_batch("INSERT INTO target SELECT * FROM source").is_err());
    assert_eq!(count(&db, "target"), initial, "key conflict aborts mutation");
}

#[test]
fn dml_predicate_domains_and_null_rejection_are_visible_in_state_branches() {
    let b = bundle("DELETE FROM target WHERE amount BETWEEN 2 AND 4", "generic");
    let effects = b.write_state_effects();
    let effect = effects[0].effect();
    assert_eq!(effect.branches().len(), 1);
    assert!(!effect.branches()[0].domains().is_empty());
    let value: serde_json::Value = serde_json::from_str(&to_bundle_json(&b)).unwrap();
    assert_eq!(value["write_effects"][0]["state_effect"]["branches"][0]["domains"][0]["column"]["name"], "amount");

    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE target(id INTEGER, amount INTEGER);
         INSERT INTO target VALUES (1, 1), (2, 2), (3, 4), (4, 5), (5, NULL);
         DELETE FROM target WHERE amount BETWEEN 2 AND 4;"
    ).unwrap();
    assert_eq!(count(&db, "target"), 3);
    let null_rows: i64 = db.query_row(
        "SELECT COUNT(*) FROM target WHERE amount IS NULL", [],
        |row| row.get(0),
    ).unwrap();
    assert_eq!(null_rows, 1, "NULL is UNKNOWN and does not match DELETE");
}

#[test]
fn shared_update_and_delete_syntax_runs_across_all_supported_parser_dialects() {
    for dialect_name in common::DIALECTS {
        let dialect = dialect_from_name(dialect_name).expect("named dialect");
        for sql in ["UPDATE t SET score = 9 WHERE id = 1", "DELETE FROM t WHERE id = 1"] {
            match analyze_inputs(&[SqlInput::inline(sql)], dialect_name, dialect.as_ref()) {
                Ok(b) => {
                    assert_eq!(b.write_state_effects().len(), 1, "{sql} {dialect_name}");
                }
                Err(error) if matches!(error.error(), sql_semantic_protocol::Error::Parse(_)) => {
                    // Parser-boundary rejection is not silently treated as supported SQL.
                }
                Err(other) => panic!("unexpected failure for {dialect_name}: {other}"),
            }
        }
    }
}
