//! TASK-189: real dbt Core execution, warehouse catalog and complete bag oracle.
//! Run with `make dbt-e2e` after dbt seeds, run, and docs generate.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_dbt_artifacts, parse_dbt_catalog, parse_dbt_manifest, physical_finite_window_bag_plan,
    LiteralValue, PhysicalFiniteWindowProof, PhysicalWriteRow, PhysicalWriteState,
};
use sqlparser::dialect::dialect_from_str;

fn canonical(value: &str) -> String {
    match value.split_once('.') {
        Some((whole, fraction)) => {
            let fraction = fraction.trim_end_matches('0');
            if fraction.is_empty() {
                whole.to_string()
            } else {
                format!("{whole}.{fraction}")
            }
        }
        None => value.to_string(),
    }
}

fn rows_as_strings(rows: &[BTreeMap<String, LiteralValue>]) -> Vec<String> {
    let mut values = rows
        .iter()
        .map(|row| {
            row.iter()
                .map(|(name, value)| {
                    let scalar = match value {
                        LiteralValue::Null => "NULL".into(),
                        LiteralValue::Number(number) => canonical(number),
                        _ => panic!("unexpected text-valued decimal oracle output"),
                    };
                    format!("{name}={scalar}")
                })
                .collect::<Vec<_>>()
                .join("|")
        })
        .collect::<Vec<_>>();
    values.sort();
    values
}

fn from_json(case: &Value) -> Vec<BTreeMap<String, LiteralValue>> {
    case["full_terminal_bag"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            row.as_object()
                .unwrap()
                .iter()
                .map(|(name, value)| {
                    let scalar = if value.is_null() {
                        LiteralValue::Null
                    } else {
                        LiteralValue::Number(value.to_string())
                    };
                    (name.to_string(), scalar)
                })
                .collect()
        })
        .collect()
}

#[test]
#[ignore = "requires an executed dbt Core fixture; run make dbt-e2e"]
fn real_compiled_dbt_decimal_windows_have_exact_physical_and_warehouse_bags() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let project = root.join("tests/fixtures/dbt_core_project/target");
    let manifest_text = fs::read_to_string(project.join("manifest.json")).unwrap();
    let catalog_text = fs::read_to_string(project.join("catalog.json")).unwrap();
    let manifest_json: Value = serde_json::from_str(&manifest_text).unwrap();
    let model_id = "model.sql_semantic_protocol_e2e.task189_decimal_windows";
    let model = &manifest_json["nodes"][model_id];
    assert_eq!(model["resource_type"], "model");
    let compiled = model["compiled_code"].as_str().unwrap();
    assert!(!compiled.contains("{{"));
    assert!(compiled.to_ascii_lowercase().contains("partition by k"));
    let output_relation = model["relation_name"].as_str().unwrap();
    let source = manifest_json["sources"]
        .as_object()
        .unwrap()
        .values()
        .find(|value| value["name"] == "task189_decimal_source")
        .expect("real dbt source metadata must include the DECIMAL seed");
    let source_relation = source["relation_name"].as_str().unwrap();
    assert_eq!(source["columns"]["x"]["data_type"], "DECIMAL(10,2)");

    let manifest = parse_dbt_manifest(&manifest_text).unwrap();
    let catalog = parse_dbt_catalog(&catalog_text).unwrap();
    let dialect = dialect_from_str(manifest.adapter_type()).unwrap();
    let bundle = analyze_dbt_artifacts(
        &manifest,
        &catalog,
        manifest.adapter_type(),
        dialect.as_ref(),
    )
    .unwrap();
    let source_name = bundle
        .source_schemas()
        .iter()
        .find(|schema| schema.relation().contains("task189_decimal_source"))
        .expect("attested dbt catalog source relation")
        .relation();
    let layer = bundle
        .layers()
        .iter()
        .find(|layer| {
            layer
                .consumes()
                .iter()
                .any(|relation| relation == source_name)
        })
        .expect("compiled decimal aggregate window model consumes catalog source");

    let path = std::env::var("DBT_E2E_DATABASE").expect("make dbt-e2e sets the real dbt database");
    let db = Connection::open(path).unwrap();
    let mut stmt = db
        .prepare(&format!(
            "SELECT id, k, CAST(x AS VARCHAR) FROM {source_relation} ORDER BY id"
        ))
        .unwrap();
    let records = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        })
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    let freeze: Value =
        serde_json::from_str(include_str!("../docs/task-189-acceptance-freeze.json")).unwrap();
    let case = freeze["numeric_oracle_cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["id"] == "74-C189-P6")
        .unwrap();
    let expected_source = case["source_bag"].as_array().unwrap();
    assert_eq!(records.len(), expected_source.len());
    let mut rows = Vec::new();
    for (actual, expected) in records.iter().zip(expected_source) {
        assert_eq!(actual.0, expected["id"].as_i64().unwrap());
        assert_eq!(actual.1, expected["k"].as_i64().unwrap());
        assert_eq!(
            actual.2.as_deref().map(canonical),
            expected["x"]
                .as_f64()
                .map(|number| canonical(&number.to_string()))
        );
        rows.push(
            PhysicalWriteRow::new(BTreeMap::from([
                ("id".into(), LiteralValue::Number(actual.0.to_string())),
                ("k".into(), LiteralValue::Number(actual.1.to_string())),
                (
                    "x".into(),
                    actual.2.as_ref().map_or(LiteralValue::Null, |number| {
                        LiteralValue::Number(number.clone())
                    }),
                ),
            ]))
            .unwrap(),
        );
    }
    let source_bag = PhysicalWriteState::new(source_name, rows).unwrap();
    let proof = physical_finite_window_bag_plan(&bundle, layer.id(), &[source_bag]);
    let PhysicalFiniteWindowProof::Exact(exact) = proof else {
        panic!("compiled real dbt manifest/catalog lost numeric exactness: {proof:?}");
    };
    let wanted = rows_as_strings(&from_json(case));
    assert_eq!(rows_as_strings(&exact.terminal), wanted);
    assert_eq!(exact.terminal_row_ids.len(), records.len());
    assert!(exact.rejected_row_ids.is_empty());

    let keys = case["full_terminal_bag"][0]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let sql = format!(
        "SELECT {} FROM {output_relation} ORDER BY id",
        keys.iter()
            .map(|key| format!("CAST({key} AS VARCHAR) AS {key}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let mut output = db.prepare(&sql).unwrap();
    let actual = output
        .query_map([], |row| {
            keys.iter()
                .enumerate()
                .map(|(i, key)| {
                    let number: Option<String> = row.get(i)?;
                    Ok((
                        (*key).to_string(),
                        number.map_or(LiteralValue::Null, LiteralValue::Number),
                    ))
                })
                .collect::<Result<BTreeMap<_, _>, duckdb::Error>>()
        })
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(rows_as_strings(&actual), wanted);
}
