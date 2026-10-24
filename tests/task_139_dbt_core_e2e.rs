//! TASK-139 real dbt Core/DuckDB execution oracle for generator-equivalent arrays.
//! DuckDB UNNEST is a reference array-bag law, not native EXPLODE/FLATTEN support.
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

fn bag(rows: &[BTreeMap<String, LiteralValue>]) -> Vec<String> {
    let mut sorted = rows.iter().map(|r| format!("{r:?}")).collect::<Vec<_>>();
    sorted.sort();
    sorted
}

#[test]
#[ignore = "requires dbt run/docs generate; executed by make dbt-e2e"]
fn compiled_models_match_native_duckdb_and_canonical_generator_value_domains() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let target = root.join("tests/fixtures/dbt_core_project/target");
    let manifest: Value = serde_json::from_str(
        &fs::read_to_string(target.join("manifest.json")).expect("generated manifest"),
    )
    .expect("manifest");
    let catalog: Value = serde_json::from_str(
        &fs::read_to_string(target.join("catalog.json")).expect("generated catalog"),
    )
    .expect("catalog");
    let conn = Connection::open(std::env::var("DBT_E2E_DATABASE").expect("warehouse"))
        .expect("DuckDB database");

    for (model, generator, expected_count) in [
        ("task139_literal_elements", "SELECT EXPLODE([5,6]) AS v", 2),
        (
            "task139_nullable_duplicate_elements",
            "SELECT EXPLODE([5,NULL,5]) AS v",
            3,
        ),
    ] {
        let model_id = format!("model.sql_semantic_protocol_e2e.{model}");
        let node = &manifest["nodes"][&model_id];
        assert_eq!(node["resource_type"], "model", "{model}: real dbt model");
        let sql = node["compiled_code"].as_str().expect("compiled SQL");
        let relation = node["relation_name"]
            .as_str()
            .expect("materialized relation");
        assert!(!sql.contains("{{"));

        let mut selected_manifest = manifest.clone();
        selected_manifest["nodes"]
            .as_object_mut()
            .unwrap()
            .retain(|name, _| name == &model_id);
        selected_manifest["sources"]
            .as_object_mut()
            .unwrap()
            .clear();
        let mut selected_catalog = catalog.clone();
        selected_catalog["nodes"]
            .as_object_mut()
            .unwrap()
            .retain(|name, _| name == &model_id);
        selected_catalog["sources"].as_object_mut().unwrap().clear();
        let manifest = parse_dbt_manifest(&selected_manifest.to_string()).expect("dbt model");
        let catalog = parse_dbt_catalog(&selected_catalog.to_string()).expect("dbt catalog");
        let dialect = dialect_from_name(manifest.adapter_type()).expect("adapter dialect");
        let bundle = analyze_dbt_artifacts(
            &manifest,
            &catalog,
            manifest.adapter_type(),
            dialect.as_ref(),
        )
        .expect("dbt canonical adapter");
        let target = bundle.layers().last().expect("dbt layer");
        let PhysicalFiniteTableProof::Exact(dbt) =
            physical_finite_table_plan(&bundle, target.id(), None)
        else {
            panic!("{model}: compiled DuckDB UNNEST must have a complete bag");
        };
        assert_eq!(dbt.terminal.len(), expected_count);
        assert!(dbt.source_rows.is_empty(), "source-free constructor");
        assert!(dbt.source_row_ids.is_empty(), "no invented physical rows");

        let mut stmt = conn
            .prepare(&format!("SELECT v FROM {relation}"))
            .expect("native model");
        let warehouse = stmt
            .query_map([], |r| {
                let n = r.get::<_, Option<i64>>(0)?;
                Ok(BTreeMap::from([(
                    "v".into(),
                    n.map_or(LiteralValue::Null, |v| LiteralValue::Number(v.to_string())),
                )]))
            })
            .expect("native terminal rows")
            .collect::<Result<Vec<_>, _>>()
            .expect("native full output bag");
        assert_eq!(
            bag(&dbt.terminal),
            bag(&warehouse),
            "{model}: whole warehouse bag"
        );

        let generic = dialect_from_name("generic").unwrap();
        let original = analyze_inputs(&[SqlInput::inline(generator)], "generic", generic.as_ref())
            .expect("parser-independent EXPLODE normalization");
        let target = original.layers().last().expect("generator");
        let PhysicalFiniteTableProof::Exact(explode) =
            physical_finite_table_plan(&original, target.id(), None)
        else {
            panic!("{model}: exact source-free generator input");
        };
        assert_eq!(bag(&explode.terminal), bag(&warehouse));
        assert_eq!(explode.terminal_domains, dbt.terminal_domains);
        assert_eq!(explode.terminal_column_types, dbt.terminal_column_types);
        // Operator identities remain different even when the complete
        // type/NULL/value-array laws coincide.
        assert_ne!(explode.row_ids, dbt.row_ids);
    }
}
