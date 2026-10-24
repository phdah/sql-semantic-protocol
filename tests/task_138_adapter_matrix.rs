//! TASK-138 exhaustive frozen 30-cell source/adapter disposition matrix.
use serde_json::{json, Value};
use sql_semantic_protocol::{
    analyze_dbt_manifest_with_schemas, analyze_inputs, dialect_from_name, parse_dbt_manifest,
    physical_finite_table_plan, PhysicalFiniteTableProof, SqlInput,
};

fn dbt(sql: &str, dialect: &str) -> Result<sql_semantic_protocol::AnalysisBundle, String> {
    let manifest = json!({
        "metadata":{
            "dbt_schema_version":"https://schemas.getdbt.com/dbt/manifest/v12.json",
            "dbt_version":"1.11.8","adapter_type":dialect
        },
        "nodes":{"model.demo.task138":{
            "unique_id":"model.demo.task138","resource_type":"model",
            "relation_name":"main.task138","database":"main","schema":"main",
            "name":"task138","original_file_path":"models/task138.sql",
            "language":"sql","raw_code":"select 0 where false",
            "compiled_code":sql,"depends_on":{"nodes":[]}
        }},"sources":{}
    });
    let parsed = parse_dbt_manifest(&manifest.to_string()).map_err(|e| e.to_string())?;
    let parser = dialect_from_name(dialect).ok_or("missing parser")?;
    analyze_dbt_manifest_with_schemas(&parsed, dialect, parser.as_ref()).map_err(|e| e.to_string())
}

fn expected_dialect(variant: &str) -> &str {
    match variant {
        "star_except" | "star_except_missing" => "bigquery",
        "star_exclude"
        | "pivot_static"
        | "unpivot_exclude_null"
        | "unpivot_include_null"
        | "pivot_opaque" => "duckdb",
        "star_rename" => "snowflake",
        "star_replace" => "bigquery",
        "pivot_dynamic" => "snowflake",
        other => panic!("unregistered frozen variant: {other}"),
    }
}
#[test]
fn all_frozen_adapter_cells_have_executable_or_declared_source_boundary_evidence() {
    let m: Value = serde_json::from_str(include_str!("../docs/task-138-acceptance-freeze.json"))
        .expect("freeze");
    assert_eq!(m["version"], 4);
    let cases = m["cases"].as_array().expect("cases");
    assert_eq!(cases.len(), 10);
    let mut count = 0;
    for case in cases {
        let id = case["id"].as_str().unwrap();
        let sql = case["sql"].as_str().unwrap();
        let dialect = expected_dialect(case["variant"].as_str().unwrap());
        let parser = dialect_from_name(dialect).expect("known dialect");
        let direct = analyze_inputs(&[SqlInput::inline(sql)], dialect, parser.as_ref())
            .unwrap_or_else(|e| panic!("{id}: {e:?}"));
        let layer = direct.layers().last().expect("direct layer");
        let direct_proof = physical_finite_table_plan(&direct, layer.id(), None);
        let cells = case["adapter_matrix"].as_array().expect("cells");
        assert_eq!(cells.len(), 3);
        for cell in cells {
            count += 1;
            match (
                cell["adapter"].as_str().unwrap(),
                cell["classification"].as_str().unwrap(),
            ) {
                ("direct_sql", "exact") => assert!(
                    matches!(&direct_proof, PhysicalFiniteTableProof::Exact(_)),
                    "{id}: direct exact"
                ),
                ("direct_sql", "typed_residual") => assert!(
                    matches!(&direct_proof, PhysicalFiniteTableProof::Residual { .. }),
                    "{id}: direct residual"
                ),
                ("dbt_compiled_sql_catalog", "exact") => {
                    let bundle = dbt(sql, dialect).unwrap_or_else(|e| panic!("{id}: {e}"));
                    let layer = bundle.layers().last().expect("dbt layer");
                    let parsed = physical_finite_table_plan(&bundle, layer.id(), None);
                    let (PhysicalFiniteTableProof::Exact(d), PhysicalFiniteTableProof::Exact(a)) =
                        (&direct_proof, parsed)
                    else {
                        panic!("{id}: dbt parity missing exact");
                    };
                    assert_eq!(d.terminal, a.terminal, "{id}: exact SQL/dbt bag");
                    assert_eq!(
                        d.terminal_domains, a.terminal_domains,
                        "{id}: exact SQL/dbt domains"
                    );
                    assert_eq!(
                        d.source_rows, a.source_rows,
                        "{id}: source complete identity"
                    );
                    assert_eq!(
                        d.contributors, a.contributors,
                        "{id}: source contributor identity"
                    );
                }
                ("dbt_compiled_sql_catalog", "typed_residual") => {
                    // A source that dbt has not materialized is not finite
                    // just because the AST includes an accepted vendor construct.
                    if let Ok(bundle) = dbt(sql, dialect) {
                        let layer = bundle.layers().last().expect("layer");
                        assert!(matches!(
                            physical_finite_table_plan(&bundle, layer.id(), None),
                            PhysicalFiniteTableProof::Residual { .. }
                        ));
                    }
                }
                ("odcs_equivalent_schema", "inapplicable_without_literal_rows") => {
                    assert!(
                        case["source_rows"].is_array(),
                        "{id}: only a declared SQL source-bag boundary is supported"
                    );
                    // Even a complete ODCS schema alone is not a source row snapshot.
                }
                other => panic!("{id}: unfrozen adapter proof cell {other:?}"),
            }
        }
    }
    assert_eq!(count, 30);
}
