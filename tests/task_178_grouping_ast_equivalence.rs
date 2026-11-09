//! Red/green TASK-178 grouping AST drift: GROUP BY ROLLUP and CUBE
//! must not vary the physical empty-input proof solely with parser dialect.
//! TASK-177 historical observed drift remains separately frozen.

use serde_json::Value;
use sql_semantic_protocol::{analyze_sql, dialect_from_name, to_json};

fn canonical(sql: &str, name: &str) -> Value {
    let dialect = dialect_from_name(name).expect("named dialect");
    let analyzed = analyze_sql(sql, name, dialect.as_ref()).expect("SQL parse");
    let mut json: Value = serde_json::from_str(&to_json(&analyzed)).unwrap();
    for input in json["inputs"].as_array_mut().expect("inputs") {
        input["dialect"] = Value::Null;
    }
    json
}

#[test]
fn rollup_cube_and_grouping_have_one_portable_empty_source_proof() {
    for sql in [
        "SELECT id, COUNT(*) FROM orders GROUP BY ROLLUP(id)",
        "SELECT id, COUNT(*) FROM orders GROUP BY CUBE(id)",
        "SELECT GROUPING(id), COUNT(*) FROM orders GROUP BY ROLLUP(id)",
    ] {
        let ansi = canonical(sql, "ansi");
        let duckdb = canonical(sql, "duckdb");
        assert_eq!(
            ansi, duckdb,
            "TASK-178: same GROUP BY must have the same full public document across equivalent dialect ASTs; \
             ansi.group_by={}, duckdb.group_by={}, ansi.zero={}, duckdb.zero={}",
            ansi["inputs"][0]["statements"][0]["aggregation"]["group_by"],
            duckdb["inputs"][0]["statements"][0]["aggregation"]["group_by"],
            ansi["graph"]["physical_source_plans"][0]["zero_output"],
            duckdb["graph"]["physical_source_plans"][0]["zero_output"]
        );
    }
}
