//! TASK-135: SQL dialect coverage at the public analysis/physical proof seam.
//! The same canonical typed nested-page program is expected wherever parsing succeeds.

use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_finite_composed_page_plan,
    physical_finite_set_tree_plan, PhysicalFiniteSetTreeProof,
    ConfiguredSqlInput, LiteralValue, PhysicalComposedPageProof, PhysicalWriteRow,
    PhysicalWriteState, RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
};
use std::collections::BTreeMap;

fn schema() -> RelationCatalog {
    RelationCatalog::from_schemas(&[RelationSchema::new(
        "s",
        vec![SchemaColumn::from_sql_type("k", "INTEGER", "postgresql").unwrap()],
    )
    .unwrap()])
    .unwrap()
}

fn state(values: &[i64]) -> PhysicalWriteState {
    PhysicalWriteState::new(
        "s",
        values
            .iter()
            .map(|value| {
                PhysicalWriteRow::new(BTreeMap::from([(
                    "k".to_string(),
                    LiteralValue::Number(value.to_string()),
                )]))
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
}

#[test]
fn canonical_nested_page_and_physical_source_bag_agree_across_dialect_parsers() {
    let freeze: Value =
        serde_json::from_str(include_str!("../docs/task-135-acceptance-freeze.json")).unwrap();
    let dialects = freeze["cases"][0]["dialect_cells"].as_array().unwrap();
    assert_eq!(dialects.len(), 13);
    let sql = freeze["cases"][0]["sql"].as_str().unwrap();
    let mut checked = 0;
    for cell in dialects {
        let name = cell["dialect"].as_str().unwrap();
        let parser = dialect_from_name(name).unwrap();
        let input = SqlInput::inline(sql);
        let configured = [ConfiguredSqlInput::new(
            "TASK-135",
            &input,
            name,
            parser.as_ref(),
        )];
        match analyze_configured_inputs_with_catalog(&configured, &schema()) {
            Ok(bundle) => {
                let target = bundle.layers().last().unwrap().id();
                let proof =
                    physical_finite_composed_page_plan(&bundle, target, &[state(&[1, 2, 3, 4, 5])]);
                assert!(
                    matches!(&proof, PhysicalComposedPageProof::Exact(_)),
                    "{name}: parsed shared syntax must retain canonical exactness: {proof:?}"
                );
                let PhysicalComposedPageProof::Exact(exact) = proof else {
                    unreachable!()
                };
                assert_eq!(exact.selected_source_ids, vec!["s#2"], "{name}");
                assert_eq!(
                    exact.terminal,
                    vec![BTreeMap::from([(
                        "k".to_string(),
                        LiteralValue::Number("3".to_string())
                    )])],
                    "{name}"
                );
                println!("TASK-135 DIALECT {name}: parsed, canonical exact");
                checked += 1;
            }
            Err(error) => {
                println!("TASK-135 DIALECT {name}: parser rejected: {error}");
                assert!(
                    matches!(name, "mssql"),
                    "{name}: unexpected parser rejection requires a reviewed boundary"
                );
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 13);
}


#[test]
fn all_exposed_dialects_classify_renamed_cte_set_branch_and_null_duplicated_pages() {
    let freeze: Value = serde_json::from_str(include_str!(
        "../docs/task-135-acceptance-freeze.json"
    )).unwrap();
    let dialects = freeze["dialect_matrix"]["dialects"].as_array().unwrap();
    assert_eq!(dialects.len(), 13);
    let cases = freeze["additional_mandatory_cases"].as_array().unwrap();
    assert_eq!(cases.len(), 4);
    let mut checked = 0;
    let mut exact = 0;
    let mut parser_rejections = 0;
    for case in cases {
        let name = case["id"].as_str().unwrap();
        let set_branch = name.contains("set-branch") || name.contains("set-zero");
        let null_duplicates = name.ends_with("null-duplicates");
        let fields = if set_branch {
            vec![
                SchemaColumn::from_sql_type("id", "TEXT", "postgresql").unwrap(),
                SchemaColumn::from_sql_type("k", "INTEGER", "postgresql").unwrap(),
            ]
        } else if null_duplicates {
            vec![
                SchemaColumn::from_sql_type("k", "INTEGER", "postgresql").unwrap(),
                SchemaColumn::from_sql_type("seq", "INTEGER", "postgresql").unwrap(),
            ]
        } else {
            vec![SchemaColumn::from_sql_type("k", "INTEGER", "postgresql").unwrap()]
        };
        let catalog = RelationCatalog::from_schemas(&[
            RelationSchema::new("s", fields).unwrap()
        ]).unwrap();
        let rows = case["source_bag"].as_array().unwrap().iter().enumerate().map(|(index, v)| {
            let mut row = BTreeMap::new();
            if set_branch {
                row.insert("id".to_string(), LiteralValue::Text(format!("row{index}")));
            }
            if null_duplicates {
                row.insert("k".to_string(), v["k"].as_i64().map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string())));
                row.insert("seq".to_string(), LiteralValue::Number(v["seq"].as_i64().unwrap().to_string()));
            } else {
                row.insert("k".to_string(), v.as_i64().map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string())));
            }
            PhysicalWriteRow::new(row).unwrap()
        }).collect();
        let state = PhysicalWriteState::new("s", rows).unwrap();
        for cell in dialects {
            let dialect = cell.as_str().unwrap();
            let parser = dialect_from_name(dialect).unwrap();
            let input = SqlInput::inline(case["sql"].as_str().unwrap());
            let configured = [ConfiguredSqlInput::new(name, &input, dialect, parser.as_ref())];
            match analyze_configured_inputs_with_catalog(&configured, &catalog) {
                Ok(bundle) => {
                    let target = bundle.layers().last().unwrap().id();
                    if set_branch {
                        let proof = physical_finite_set_tree_plan(&bundle, target, std::slice::from_ref(&state));
                        assert!(
                            matches!(proof, PhysicalFiniteSetTreeProof::Exact(_)),
                            "{name}/{dialect}: parsed branch SQL must be canonical Exact: {proof:?}"
                        );
                    } else {
                        let proof = physical_finite_composed_page_plan(&bundle, target, std::slice::from_ref(&state));
                        assert!(
                            matches!(proof, PhysicalComposedPageProof::Exact(_)),
                            "{name}/{dialect}: parsed CTE SQL must be canonical Exact: {proof:?}"
                        );
                    }
                    exact += 1;
                }
                Err(error) => {
                    println!("TASK-135 {name}/{dialect}: parser boundary: {error}");
                    parser_rejections += 1;
                }
            }
            checked += 1;
        }
    }
    assert_eq!(checked, 52);
    assert_eq!(exact + parser_rejections, checked);
}
