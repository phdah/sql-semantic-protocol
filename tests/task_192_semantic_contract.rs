//! Temporary red-stage audit of public per-case exactness and domain shapes.
//! Replaced by a fixed independent expected semantic ledger after inspection.

use serde_json::Value;
use sql_semantic_protocol::{analyze_sql, dialect_from_name, to_json};

#[test]
fn current_task_192_public_exactness_must_be_compared_to_independent_contract() {
    let freeze: Value =
        serde_json::from_str(include_str!("../docs/task-192-acceptance-freeze.json"))
            .expect("committed case list");
    let parser = dialect_from_name("duckdb").expect("DuckDB parser");
    let mut observed = Vec::new();
    for case in freeze["cases"].as_array().expect("43 named variants") {
        let id = case["id"].as_str().expect("stable SQL case");
        let sql = case["sql"].as_str().expect("original SQL");
        let protocol = analyze_sql(sql, "duckdb", parser.as_ref())
            .expect("previously supported DuckDB parser");
        let json: Value = serde_json::from_str(&to_json(&protocol)).expect("public wire");
        let query = &json["inputs"][0]["statements"][0];
        observed.push(format!(
            "{id} domain={} exactness={} residuals={} required={}",
            query["output"]["columns"][0]["domain"]["kind"],
            query["condition_exactness"]["status"],
            query["condition_exactness"]["residual_conditions"],
            query["condition_exactness"]["required_assumptions"]
        ));
    }
    panic!("TASK-192 RED SEMANTIC AUDIT\\n{}", observed.join("\\n"));
}
