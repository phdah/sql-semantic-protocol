//! TASK-138 direct SQL and compiled dbt adapter parity on complete finite facts.
use serde_json::{json, Value};
use sql_semantic_protocol::{
    analyze_dbt_manifest_with_schemas, analyze_inputs, dialect_from_name, parse_dbt_manifest,
    physical_finite_table_plan, to_bundle_json, PhysicalFiniteTableProof, SqlInput,
};
fn dbt(sql: &str) -> sql_semantic_protocol::AnalysisBundle {
    let manifest = json!({
        "metadata": {
            "dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v12.json",
            "dbt_version": "1.11.8", "adapter_type": "duckdb"
        },
        "nodes": { "model.demo.task138": {
            "unique_id": "model.demo.task138", "resource_type": "model",
            "relation_name": "main.task138", "database": "main", "schema": "main",
            "name": "task138", "original_file_path": "models/task138.sql",
            "language": "sql", "raw_code": "SELECT 0 WHERE FALSE",
            "compiled_code": sql, "depends_on": {"nodes": []}
        }},
        "sources": {}
    });
    let parsed = parse_dbt_manifest(&manifest.to_string()).expect("compiled model");
    let dialect = dialect_from_name("duckdb").expect("dialect");
    analyze_dbt_manifest_with_schemas(&parsed, "duckdb", dialect.as_ref()).expect("compiled dbt")
}
#[derive(Debug, PartialEq)]
struct FiniteSnapshot {
    row_ids: Vec<String>,
    terminal: Vec<std::collections::BTreeMap<String, sql_semantic_protocol::LiteralValue>>,
    terminal_domains: std::collections::BTreeMap<String, sql_semantic_protocol::ValueDomain>,
}

fn result(bundle: &sql_semantic_protocol::AnalysisBundle) -> FiniteSnapshot {
    let layer = bundle.layers().last().expect("layer");
    let PhysicalFiniteTableProof::Exact(proof) =
        physical_finite_table_plan(bundle, layer.id(), None)
    else {
        panic!("source-free SQL must retain exact canonical proof");
    };
    FiniteSnapshot {
        row_ids: proof.row_ids,
        terminal: proof.terminal,
        terminal_domains: proof.terminal_domains,
    }
}

#[test]
fn each_bounded_finite_relational_form_is_adapter_independent() {
    let cases = [
        "SELECT * EXCLUDE b FROM (VALUES (1, 10, 20), (1, NULL, 30)) AS t(id, a, b)",
        "SELECT * REPLACE (9 AS b) FROM (VALUES (1, 10, 20)) AS t(id, a, b)",
        "SELECT * FROM (VALUES (1, 1, 10), (1, 2, 20)) AS t(id, k, v) PIVOT (SUM(v) FOR k IN (1 AS one, 2 AS two))",
        "SELECT * FROM (VALUES (1, 10, 20), (1, NULL, 30)) AS t(id, a, b) UNPIVOT INCLUDE NULLS (v FOR n IN (a, b))"
    ];
    let parser = dialect_from_name("duckdb").expect("dialect");
    for sql in cases {
        let direct =
            analyze_inputs(&[SqlInput::inline(sql)], "duckdb", parser.as_ref()).expect("direct");
        let compiled = dbt(sql);
        assert_eq!(
            result(&direct),
            result(&compiled),
            "SQL/dbt canonical parity: {sql}"
        );
        let raw: Value = serde_json::from_str(&to_bundle_json(&direct)).expect("direct JSON");
        let modeled: Value = serde_json::from_str(&to_bundle_json(&compiled)).expect("dbt JSON");
        assert_eq!(
            raw["inputs"][0]["statements"][0]["finite_table"],
            modeled["inputs"][0]["statements"][0]["finite_table"],
            "canonical wire parity for model: {sql}"
        );
    }
}
#[test]
fn no_odcs_schema_can_authorize_literal_value_rows() {
    let parser = dialect_from_name("duckdb").expect("dialect");
    let bundle = analyze_inputs(
        &[SqlInput::inline("SELECT * EXCLUDE b FROM events")],
        "duckdb",
        parser.as_ref(),
    )
    .expect("parsed");
    let target = bundle.layers().last().expect("layer");
    assert!(
        matches!(
            physical_finite_table_plan(&bundle, target.id(), None),
            PhysicalFiniteTableProof::Residual { .. }
        ),
        "schema-only ODCS must not be mistaken for a complete literal physical bag"
    );
}
