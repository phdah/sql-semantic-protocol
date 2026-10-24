//! TASK-138 real dbt Core and DuckDB catalog/run oracle.
use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_dbt_artifacts, parse_dbt_catalog, parse_dbt_manifest, physical_finite_table_plan,
    LiteralValue, PhysicalFiniteTableProof,
};
use sqlparser::dialect::dialect_from_str;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

fn sorted(rows: &[BTreeMap<String, LiteralValue>]) -> Vec<String> {
    let mut rows = rows
        .iter()
        .map(|row| format!("{row:?}"))
        .collect::<Vec<_>>();
    rows.sort();
    rows
}

#[test]
#[ignore = "requires dbt run/docs generate; executed by make dbt-e2e"]
fn real_dbt_models_preserve_complete_source_terminal_bags_and_native_outcomes() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let target = root.join("tests/fixtures/dbt_core_project/target");
    let manifest: Value = serde_json::from_str(
        &fs::read_to_string(target.join("manifest.json")).expect("generated manifest"),
    )
    .expect("manifest JSON");
    let catalog: Value = serde_json::from_str(
        &fs::read_to_string(target.join("catalog.json")).expect("generated catalog"),
    )
    .expect("catalog JSON");
    let conn = Connection::open(std::env::var("DBT_E2E_DATABASE").expect("warehouse path"))
        .expect("DuckDB warehouse");

    for (model, columns) in [
        ("task138_finite_star", vec!["id", "a"]),
        ("task138_fixed_pivot", vec!["id", "one", "two"]),
        ("task138_named_unpivot", vec!["id", "v", "n"]),
    ] {
        let model_id = format!("model.sql_semantic_protocol_e2e.{model}");
        let node = &manifest["nodes"][&model_id];
        assert_eq!(node["resource_type"], "model");
        let compiled = node["compiled_code"].as_str().expect("compiled SQL");
        assert!(!compiled.contains("{{"), "dbt Core did not compile SQL");
        let relation = node["relation_name"].as_str().expect("warehouse relation");

        let mut one_manifest = manifest.clone();
        one_manifest["nodes"]
            .as_object_mut()
            .unwrap()
            .retain(|id, _| id == &model_id);
        one_manifest["sources"].as_object_mut().unwrap().clear();
        let mut one_catalog = catalog.clone();
        one_catalog["nodes"]
            .as_object_mut()
            .unwrap()
            .retain(|id, _| id == &model_id);
        one_catalog["sources"].as_object_mut().unwrap().clear();

        let parsed = parse_dbt_manifest(&one_manifest.to_string()).expect("dbt model");
        let parsed_catalog =
            parse_dbt_catalog(&one_catalog.to_string()).expect("dbt warehouse catalog");
        let dialect = dialect_from_str(parsed.adapter_type()).expect("DuckDB dialect");
        let bundle = analyze_dbt_artifacts(
            &parsed,
            &parsed_catalog,
            parsed.adapter_type(),
            dialect.as_ref(),
        )
        .expect("adapter normalized");
        let layer = bundle.layers().last().expect("model target");
        let PhysicalFiniteTableProof::Exact(exact) =
            physical_finite_table_plan(&bundle, layer.id(), None)
        else {
            panic!("{model}: dbt compiled model lost exact finite transformation");
        };

        let mut statement = conn
            .prepare(&format!("SELECT {} FROM {relation}", columns.join(",")))
            .expect("native SQL");
        let native = statement
            .query_map([], |row| {
                let mut values = BTreeMap::new();
                for (i, name) in columns.iter().enumerate() {
                    let value = if *name == "n" {
                        row.get::<_, Option<String>>(i)?
                            .map_or(LiteralValue::Null, LiteralValue::Text)
                    } else {
                        row.get::<_, Option<i64>>(i)?
                            .map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string()))
                    };
                    values.insert((*name).to_string(), value);
                }
                Ok(values)
            })
            .expect("execute native")
            .collect::<Result<Vec<_>, _>>()
            .expect("native rows");
        assert_eq!(
            sorted(&exact.terminal),
            sorted(&native),
            "{model}: warehouse whole terminal bag"
        );
        assert_eq!(exact.source_rows.len(), 3, "{model}: complete source");
        assert_eq!(
            exact.source_row_ids,
            vec!["values:0", "values:1", "values:2"]
        );
        assert_eq!(
            exact.contributors.len(),
            exact.terminal.len(),
            "{model}: source witnesses"
        );
        assert_eq!(
            exact.terminal_domains.len(),
            columns.len(),
            "{model}: typed domains"
        );
    }
}
