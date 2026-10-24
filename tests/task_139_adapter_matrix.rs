//! TASK-139 direct SQL, compiled dbt and ODCS schema-evidence matrix.
use serde_json::{json, Value};
use sql_semantic_protocol::{
    analyze_dbt_manifest_with_schemas, analyze_inputs, dialect_from_name, parse_dbt_manifest,
    physical_finite_table_plan, PhysicalFiniteTableProof, SqlInput,
};

fn compiled_dbt(sql: &str, dialect: &str) -> Result<sql_semantic_protocol::AnalysisBundle, String> {
    let manifest = json!({
        "metadata": {
            "dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v12.json",
            "dbt_version": "1.11.8",
            "adapter_type": dialect
        },
        "nodes": {"model.demo.task139": {
            "unique_id": "model.demo.task139",
            "resource_type": "model",
            "relation_name": "main.task139",
            "database": "main",
            "schema": "main",
            "name": "task139",
            "original_file_path": "models/task139.sql",
            "language": "sql",
            "raw_code": "select 0 where false",
            "compiled_code": sql,
            "depends_on": {"nodes": []}
        }},
        "sources": {}
    });
    let parsed = parse_dbt_manifest(&manifest.to_string()).map_err(|e| e.to_string())?;
    let parser = dialect_from_name(dialect).ok_or("missing parser")?;
    analyze_dbt_manifest_with_schemas(&parsed, dialect, parser.as_ref())
        .map_err(|e| e.to_string())
}

fn evidence(bundle: &sql_semantic_protocol::AnalysisBundle) -> PhysicalFiniteTableProof {
    let layer = bundle.layers().last().expect("query layer");
    physical_finite_table_plan(bundle, layer.id(), None)
}

#[test]
fn task_139_all_21_adapter_dispositions_are_reproducible() {
    let m: Value = serde_json::from_str(include_str!("../docs/task-139-acceptance-freeze.json"))
        .expect("frozen matrix");
    let mut checked = 0;
    for case in m["cases"].as_array().unwrap() {
        let sql = case["sql"].as_str().expect("SQL");
        let dialect = case["canonical_dialect"].as_str().expect("chosen dialect");
        let parser = dialect_from_name(dialect).expect("supported dialect");
        let direct = analyze_inputs(&[SqlInput::inline(sql)], dialect, parser.as_ref());
        let direct_status = match &direct {
            Err(_) => "parser_boundary",
            Ok(bundle) => match bundle.layers().last() {
                None => "residual",
                Some(layer) => match physical_finite_table_plan(bundle, layer.id(), None) {
                    PhysicalFiniteTableProof::Exact(_) => "exact",
                    PhysicalFiniteTableProof::Residual { .. } | PhysicalFiniteTableProof::Impossible { .. } => "residual",
                },
            },
        };
        let dbt = compiled_dbt(sql, dialect);
        let dbt_status = match &dbt {
            Err(_) => "parser_boundary",
            Ok(bundle) => match bundle.layers().last() {
                None => "residual",
                Some(layer) => match physical_finite_table_plan(bundle, layer.id(), None) {
                    PhysicalFiniteTableProof::Exact(_) => "exact",
                    PhysicalFiniteTableProof::Residual { .. } | PhysicalFiniteTableProof::Impossible { .. } => "residual",
                },
            },
        };

        for cell in case["adapter_matrix"].as_array().unwrap() {
            assert_eq!(cell["certified"], true, "{sql}: adapter cell not verified");
            match cell["adapter"].as_str().unwrap() {
                "direct_sql" => assert_eq!(cell["classification"], direct_status, "{sql}"),
                "dbt_compiled_sql_catalog" => {
                    assert_eq!(cell["classification"], dbt_status, "{sql}");
                    if let (Ok(direct), Ok(dbt)) = (&direct, &dbt) {
                        match (evidence(direct), evidence(dbt)) {
                            (PhysicalFiniteTableProof::Exact(a), PhysicalFiniteTableProof::Exact(b)) => {
                                assert_eq!(a.terminal, b.terminal, "{sql}: exact bag");
                                assert_eq!(a.terminal_domains, b.terminal_domains, "{sql}: domains");
                                assert_eq!(a.row_ids, b.row_ids, "{sql}: stable identities");
                                assert_eq!(a.source_rows, b.source_rows, "{sql}: physical source");
                                assert_eq!(a.contributors, b.contributors, "{sql}: source witnesses");
                            }
                            (PhysicalFiniteTableProof::Residual { .. }, PhysicalFiniteTableProof::Residual { .. }) => {}
                            (a, b) => panic!("{sql}: SQL/dbt disagree {a:?} / {b:?}"),
                        }
                    }
                }
                "odcs_equivalent_schema" => {
                    assert_eq!(cell["classification"], "inapplicable_without_literal_rows");
                    assert!(
                        case["classification"] == "conditional"
                            || case["source_rows"].as_array().is_some_and(Vec::is_empty),
                        "{sql}: source-free array facts are not ODCS relation schema facts"
                    );
                }
                other => panic!("unreviewed adapter {other}"),
            }
            checked += 1;
        }
    }
    assert_eq!(checked, 21);
}

#[cfg(feature = "odcs")]
#[test]
fn real_odcs_schema_evidence_does_not_provide_a_generator_row_population() {
    use sql_semantic_protocol::{parse_odcs_yaml, RelationCatalog};
    let contract = include_str!("fixtures/odcs/commerce.odcs.yaml");
    let catalog = RelationCatalog::new(&["analytics.customers", "analytics.orders"])
        .expect("ODCS relation names");
    let metadata = parse_odcs_yaml(contract, "postgresql", &catalog)
        .expect("real ODCS schema contract");
    assert!(!metadata.source_schemas().is_empty());
    assert!(metadata.source_schemas().iter().all(|schema| schema.columns().len() > 0));

    let parser = dialect_from_name("duckdb").unwrap();
    for sql in [
        "SELECT * FROM EXPLODE(orders.array_column) AS t(v)",
        "SELECT * FROM custom_table_generator(orders.array_column) AS t(v)",
    ] {
        if let Ok(bundle) = analyze_inputs(&[SqlInput::inline(sql)], "duckdb", parser.as_ref()) {
            if let Some(layer) = bundle.layers().last() {
                assert!(matches!(
                    physical_finite_table_plan(&bundle, layer.id(), None),
                    PhysicalFiniteTableProof::Residual { .. }
                ));
            }
        }
    }
}
