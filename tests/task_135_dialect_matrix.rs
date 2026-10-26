//! TASK-135: SQL dialect coverage at the public analysis/physical proof seam.
//! The same canonical typed nested-page program is expected wherever parsing succeeds.

use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name,
    physical_finite_composed_page_plan, ConfiguredSqlInput, LiteralValue,
    PhysicalComposedPageProof, PhysicalWriteRow, PhysicalWriteState,
    RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
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
        values.iter().map(|value| {
            PhysicalWriteRow::new(BTreeMap::from([(
                "k".to_string(), LiteralValue::Number(value.to_string())
            )])).unwrap()
        }).collect(),
    ).unwrap()
}

#[test]
fn canonical_nested_page_and_physical_source_bag_agree_across_dialect_parsers() {
    let freeze: Value = serde_json::from_str(include_str!(
        "../docs/task-135-acceptance-freeze.json"
    )).unwrap();
    let dialects = freeze["cases"][0]["dialect_cells"].as_array().unwrap();
    assert_eq!(dialects.len(), 13);
    let sql = freeze["cases"][0]["sql"].as_str().unwrap();
    let mut checked = 0;
    for cell in dialects {
        let name = cell["dialect"].as_str().unwrap();
        let parser = dialect_from_name(name).unwrap();
        let input = SqlInput::inline(sql);
        let configured = [ConfiguredSqlInput::new(
            "TASK-135", &input, name, parser.as_ref()
        )];
        match analyze_configured_inputs_with_catalog(&configured, &schema()) {
            Ok(bundle) => {
                let target = bundle.layers().last().unwrap().id();
                let proof = physical_finite_composed_page_plan(
                    &bundle, target, &[state(&[1, 2, 3, 4, 5])]
                );
                assert!(
                    matches!(&proof, PhysicalComposedPageProof::Exact(_)),
                    "{name}: parsed shared syntax must retain canonical exactness: {proof:?}"
                );
                let PhysicalComposedPageProof::Exact(exact) = proof else { unreachable!() };
                assert_eq!(exact.selected_source_ids, vec!["s#2"], "{name}");
                assert_eq!(
                    exact.terminal,
                    vec![BTreeMap::from([(
                        "k".to_string(), LiteralValue::Number("3".to_string())
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
