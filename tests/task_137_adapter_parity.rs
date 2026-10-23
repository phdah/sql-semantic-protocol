//! TASK-137 source-free finite values preserve canonical semantics across
//! direct SQL and compiled dbt manifests. ODCS owns physical relation schemas,
//! not source-free SQL VALUES and literal-array rows.

use serde_json::{json, Value};
use sql_semantic_protocol::{
    analyze_dbt_manifest_with_schemas, analyze_inputs, dialect_from_name,
    parse_dbt_manifest, physical_finite_table_plan, to_bundle_json,
    PhysicalFiniteTableProof, SqlInput,
};

fn all_cases() -> Value {
    serde_json::from_str(include_str!("../docs/task-137-acceptance-freeze.json")).unwrap()
}

fn dbt_bundle(sql: &str) -> sql_semantic_protocol::AnalysisBundle {
    let manifest = json!({
        "metadata": {
            "dbt_schema_version":"https://schemas.getdbt.com/dbt/manifest/v12.json",
            "dbt_version":"1.11.8","adapter_type":"duckdb"
        },
        "nodes": {
            "model.demo.task137": {
                "unique_id":"model.demo.task137",
                "resource_type":"model",
                "relation_name":"main.task137",
                "database":"main","schema":"main","name":"task137",
                "original_file_path":"models/task137.sql",
                "language":"sql","raw_code":"SELECT 0 WHERE FALSE",
                "compiled_code":sql,
                "depends_on":{"nodes":[]}
            }
        },
        "sources": {}
    });
    let manifest = parse_dbt_manifest(&manifest.to_string()).expect("valid dbt compiled manifest");
    let parser = dialect_from_name("duckdb").unwrap();
    analyze_dbt_manifest_with_schemas(&manifest, "duckdb", parser.as_ref())
        .expect("source-free compiled dbt model")
}

#[test]
fn frozen_source_free_tables_have_direct_and_compiled_dbt_parity() {
    let inventory = all_cases();
    let parser = dialect_from_name("duckdb").unwrap();
    for case in inventory["cases"].as_array().unwrap() {
        if case["classification"] != "mandatory" { continue; }
        let id = case["id"].as_str().unwrap();
        let sql = case["sql"].as_str().unwrap();
        let direct = analyze_inputs(&[SqlInput::inline(sql)], "duckdb", parser.as_ref())
            .unwrap_or_else(|e| panic!("{id}: {e:?}"));
        let dbt = dbt_bundle(sql);
        let proof = |bundle: &sql_semantic_protocol::AnalysisBundle| {
            let target = bundle.layers().last().expect("finite output layer");
            let exact = physical_finite_table_plan(bundle, target.id(), None);
            let PhysicalFiniteTableProof::Exact(exact) = exact else {
                panic!("{id}: missing source-free adapter proof: {exact:?}");
            };
            (
                exact.row_ids,
                exact.terminal,
                exact.terminal_domains,
            )
        };
        assert_eq!(proof(&direct), proof(&dbt), "{id}: canonical dbt output and domains");

        let direct_json: Value = serde_json::from_str(&to_bundle_json(&direct)).unwrap();
        let dbt_json: Value = serde_json::from_str(&to_bundle_json(&dbt)).unwrap();
        let direct_table = &direct_json["inputs"][0]["statements"][0]["finite_table"];
        let compiled_table = &dbt_json["inputs"][0]["statements"][0]["finite_table"];
        assert!(direct_table.is_object(), "{id}: JSON protocol must carry canonical table");
        assert_eq!(direct_table, compiled_table, "{id}: compiled SQL must share protocol facts");
        assert_eq!(direct_table["rows"].as_array().unwrap().len(), case["expected"].as_array().unwrap().len());
    }
}

#[test]
fn protocol_wire_rows_keep_typed_ordinality_null_and_duplicates() {
    let parser = dialect_from_name("duckdb").unwrap();
    for (sql, expected) in [
        (
            "VALUES (1), (1), (NULL)",
            json!([[{"kind":"literal","type":"integer","value":1}],
                   [{"kind":"literal","type":"integer","value":1}],
                   [{"kind":"literal","type":"null","value":null}]]),
        ),
        (
            "SELECT * FROM UNNEST([1, 2]) WITH ORDINALITY AS t(v, ord)",
            json!([[{"kind":"literal","type":"integer","value":1},
                    {"kind":"literal","type":"integer","value":1}],
                   [{"kind":"literal","type":"integer","value":2},
                    {"kind":"literal","type":"integer","value":2}]]),
        ),
    ] {
        let bundle = analyze_inputs(&[SqlInput::inline(sql)], "duckdb", parser.as_ref()).unwrap();
        let wire: Value = serde_json::from_str(&to_bundle_json(&bundle)).unwrap();
        assert_eq!(wire["inputs"][0]["statements"][0]["finite_table"]["rows"], expected);
        let repeat: Value = serde_json::from_str(&to_bundle_json(&bundle)).unwrap();
        assert_eq!(wire, repeat, "stable canonical emission");
    }
}
