//! TASK-178: isolate remaining canonical document drift across parser ASTs.
//! Historical TASK-177 drift registry is immutable and remains an independent
//! observation. This test fails on unreviewed differences rather than
//! silently normalizing away semantic diagnostics or source correlation.

use serde_json::Value;
use sql_semantic_protocol::{analyze_sql, dialect_from_name, to_json};

fn wire(sql: &str, name: &str) -> Value {
    let dialect = dialect_from_name(name).expect("exposed public dialect");
    let protocol =
        analyze_sql(sql, name, dialect.as_ref()).expect("SQL must parse with both public dialects");
    let mut value: Value = serde_json::from_str(&to_json(&protocol)).expect("public JSON");
    for input in value["inputs"].as_array_mut().expect("input") {
        input["dialect"] = Value::Null;
    }
    value
}

#[test]
fn mssql_on_true_is_an_explicit_identifier_truth_boundary_not_a_portable_lateral_equivalence() {
    let sql = "SELECT o.id FROM orders o JOIN LATERAL (SELECT o.id) l ON TRUE";
    let ansi = wire(sql, "ansi");
    let mssql = wire(sql, "mssql");
    assert_ne!(
        ansi, mssql,
        "MSSQL interprets unquoted TRUE as a source identifier; rewriting \
         its equality predicate into an ANSI boolean literal would be unsound"
    );
    let lhs = &ansi["inputs"][0]["statements"][0]["joins"][0]["condition"];
    let rhs = &mssql["inputs"][0]["statements"][0]["joins"][0]["condition"];
    assert_eq!(lhs["kind"], "boolean_expression");
    assert_eq!(rhs["kind"], "comparison");
    assert_eq!(rhs["left"]["name"], "TRUE");
    assert_eq!(rhs["left"]["relation"], "orders");
    for (name, value) in [("ansi", &ansi), ("mssql", &mssql)] {
        assert_eq!(
            value["graph"]["physical_source_plans"][0]["qualifying"]["status"], "residual",
            "{name}: no invented cross-row physical witness"
        );
        assert_eq!(
            value["graph"]["physical_source_plans"][0]["rejected"]["status"], "residual",
            "{name}: no invented NOT TRUE/absence witness"
        );
        assert_eq!(
            value["inputs"][0]["statements"][0]["condition_exactness"]["status"], "residual",
            "{name}: correlated LATERAL has no proved engine-independent scope"
        );
    }
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
