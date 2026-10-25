//! TASK-140 independent compiled dbt Core and DuckDB full-bag oracles.
//! The test runs only after make dbt-e2e generates real manifest/catalog files.

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_dbt_artifacts, analyze_inputs, dialect_from_name, parse_dbt_catalog,
    parse_dbt_manifest, physical_finite_table_plan, LiteralValue, PhysicalFiniteTableProof,
    SqlInput,
};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

fn sorted(rows: &[BTreeMap<String, LiteralValue>]) -> Vec<String> {
    let mut entries = rows
        .iter()
        .map(|row| format!("{row:?}"))
        .collect::<Vec<_>>();
    entries.sort();
    entries
}

#[test]
#[ignore = "requires real dbt run/docs generate, executed by make dbt-e2e"]
fn compiled_dbt_fanout_and_join_preserve_full_physical_and_terminal_bags() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let target = root.join("tests/fixtures/dbt_core_project/target");
    let manifest: Value = serde_json::from_str(
        &fs::read_to_string(target.join("manifest.json")).expect("dbt manifest"),
    )
    .expect("valid manifest");
    let catalog: Value = serde_json::from_str(
        &fs::read_to_string(target.join("catalog.json")).expect("dbt catalog"),
    )
    .expect("valid catalog");
    let db = Connection::open(std::env::var("DBT_E2E_DATABASE").expect("warehouse path"))
        .expect("actual DuckDB dbt warehouse");
    for (model, rows, joined) in [
        ("task140_grouped_fanout", 2, false),
        ("task140_joined_fanout", 2, true),
    ] {
        let model_id = format!("model.sql_semantic_protocol_e2e.{model}");
        let node = &manifest["nodes"][&model_id];
        assert_eq!(node["resource_type"], "model");
        let sql = node["compiled_code"]
            .as_str()
            .expect("actual compiled dbt SQL");
        let relation = node["relation_name"]
            .as_str()
            .expect("actual warehouse relation");
        assert!(!sql.contains("{{"));

        let mut selected_manifest = manifest.clone();
        selected_manifest["nodes"]
            .as_object_mut()
            .unwrap()
            .retain(|id, _| id == &model_id);
        selected_manifest["sources"]
            .as_object_mut()
            .unwrap()
            .clear();
        let mut selected_catalog = catalog.clone();
        selected_catalog["nodes"]
            .as_object_mut()
            .unwrap()
            .retain(|id, _| id == &model_id);
        selected_catalog["sources"].as_object_mut().unwrap().clear();
        let manifest = parse_dbt_manifest(&selected_manifest.to_string()).expect("real dbt");
        let catalog = parse_dbt_catalog(&selected_catalog.to_string()).expect("real catalog");
        let parser = dialect_from_name(manifest.adapter_type()).expect("dbt dialect");
        let bundle = analyze_dbt_artifacts(
            &manifest,
            &catalog,
            manifest.adapter_type(),
            parser.as_ref(),
        )
        .expect("canonical dbt adapter");
        let target = bundle.layers().last().expect("dbt terminal");
        let PhysicalFiniteTableProof::Exact(proof) =
            physical_finite_table_plan(&bundle, target.id(), None)
        else {
            panic!("{model}: canonical dbt must prove finite complete bag");
        };
        assert_eq!(proof.terminal.len(), rows);
        if joined {
            assert_eq!(proof.joined_selected.len(), 5);
            assert_eq!(proof.joined_not_true.len(), 4);
            assert_eq!(proof.physical_inputs["left"].len(), 3);
            assert_eq!(proof.physical_inputs["right"].len(), 3);
        } else {
            assert!(proof.joined_selected.is_empty());
            assert_eq!(proof.source_rows.len(), 3);
        }

        let mut stmt = db
            .prepare(&format!("SELECT v,n FROM {relation}"))
            .expect("native query");
        let warehouse = stmt
            .query_map([], |row| {
                let v = row.get::<_, Option<i64>>(0)?;
                let n = row.get::<_, i64>(1)?;
                Ok(BTreeMap::from([
                    (
                        "v".to_string(),
                        v.map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string())),
                    ),
                    ("n".to_string(), LiteralValue::Number(n.to_string())),
                ]))
            })
            .expect("native model rows")
            .collect::<Result<Vec<_>, _>>()
            .expect("entire warehouse bag");
        assert_eq!(
            sorted(&proof.terminal),
            sorted(&warehouse),
            "{model}: actual SQL"
        );

        let direct_parser = dialect_from_name("duckdb").expect("parser");
        let direct = analyze_inputs(&[SqlInput::inline(sql)], "duckdb", direct_parser.as_ref())
            .expect("same compiled SQL via public direct entry");
        let target = direct.layers().last().expect("direct layer");
        let PhysicalFiniteTableProof::Exact(other) =
            physical_finite_table_plan(&direct, target.id(), None)
        else {
            panic!("{model}: direct SQL must certify same complete bag");
        };
        assert_eq!(proof.terminal, other.terminal);
        assert_eq!(proof.source_rows, other.source_rows);
        assert_eq!(proof.contributors, other.contributors);
        assert_eq!(proof.joined_selected, other.joined_selected);
        assert_eq!(proof.joined_not_true, other.joined_not_true);
        assert_eq!(proof.terminal_domains, other.terminal_domains);
    }
}
