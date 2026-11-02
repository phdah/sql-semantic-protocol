//! Sound lifecycle metadata: surviving defaults must follow table identity;
//! duplicate CREATE after DROP needs a fresh ordered declaration, never a stale schema.

use sql_semantic_protocol::{
    analyze_inputs, dialect_from_name, physical_ordered_write_state_plan,
    LiteralValue, PhysicalWriteStatePlan, SqlInput,
};

fn prove(sql: &[&str]) -> PhysicalWriteStatePlan {
    let dialect = dialect_from_name("duckdb").unwrap();
    let inputs = sql.iter().map(|sql| SqlInput::inline(*sql)).collect::<Vec<_>>();
    let bundle = analyze_inputs(&inputs, "duckdb", dialect.as_ref()).unwrap();
    physical_ordered_write_state_plan(&bundle, &[])
}

#[test]
fn exact_rename_preserves_an_attested_scalar_default_for_later_insert() {
    let statements = [
        "CREATE TABLE u (a INTEGER)",
        "DROP TABLE u",
        "CREATE TABLE t (a INTEGER)",
        "ALTER TABLE t ADD COLUMN b INTEGER DEFAULT 5",
        "ALTER TABLE t RENAME TO u",
        "INSERT INTO u (a) VALUES (1)",
    ];
    let PhysicalWriteStatePlan::Feasible { transitions, final_states } =
        prove(&statements)
    else {
        panic!("renaming constraint-free table must preserve scalar default evidence");
    };
    assert_eq!(transitions.len(), statements.len());
    let row = &final_states[0].rows()[0];
    assert_eq!(row.values().get("a"), Some(&LiteralValue::Number("1".into())));
    assert_eq!(row.values().get("b"), Some(&LiteralValue::Number("5".into())));
}

#[test]
fn dropping_then_recreating_a_table_cannot_reuse_stale_alter_schema() {
    let statements = [
        "CREATE TABLE t (a INTEGER)",
        "ALTER TABLE t ADD COLUMN b INTEGER DEFAULT 7",
        "DROP TABLE t",
        "CREATE TABLE t (a INTEGER)",
    ];
    assert!(matches!(
        prove(&statements),
        PhysicalWriteStatePlan::Residual { .. }
    ));
}
