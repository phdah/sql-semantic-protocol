//! TASK-178: isolate remaining canonical document drift across parser ASTs.
//! Historical TASK-177 drift registry is immutable and remains an independent
//! observation. This test fails on unreviewed differences rather than
//! silently normalizing away semantic diagnostics or source correlation.

use serde_json::Value;
use sql_semantic_protocol::{analyze_sql, dialect_from_name, to_json};

fn wire(sql: &str, name: &str) -> Value {
    let dialect = dialect_from_name(name).expect("exposed public dialect");
    let protocol = analyze_sql(sql, name, dialect.as_ref())
        .expect("SQL must parse with both public dialects");
    let mut value: Value = serde_json::from_str(&to_json(&protocol)).expect("public JSON");
    for input in value["inputs"].as_array_mut().expect("input") {
        input["dialect"] = Value::Null;
    }
    value
}

#[test]
fn lateral_source_correlation_retains_same_full_physical_plan_across_ansi_and_mssql() {
    let sql = "SELECT o.id FROM orders o JOIN LATERAL (SELECT o.id) l ON TRUE";
    let ansi = wire(sql, "ansi");
    let mssql = wire(sql, "mssql");
    assert_eq!(
        ansi, mssql,
        "TASK-178 correlated LATERAL drift: ansi.witness={}; mssql.witness={}; ansi.join={}; mssql.join={}",
        ansi["graph"]["physical_nodes"][1]["operator_witnesses"],
        mssql["graph"]["physical_nodes"][1]["operator_witnesses"],
        ansi["inputs"][0]["statements"][0]["joins"],
        mssql["inputs"][0]["statements"][0]["joins"],
    );
}

#[test]
fn nested_struct_cast_does_not_emit_dialect_dependent_error_detail() {
    let sql = "SELECT CAST(ROW(1, 'a') AS STRUCT(a INT, b VARCHAR)) AS v";
    let ansi = wire(sql, "ansi");
    let duckdb = wire(sql, "duckdb");
    assert_eq!(
        ansi, duckdb,
        "TASK-178 STRUCT cast drift: ansi diagnostics={}; duckdb diagnostics={}; ansi.output={}; duckdb.output={}",
        ansi["inputs"][0]["statements"][0]["diagnostics"],
        duckdb["inputs"][0]["statements"][0]["diagnostics"],
        ansi["inputs"][0]["statements"][0]["output"],
        duckdb["inputs"][0]["statements"][0]["output"]
    );
}
