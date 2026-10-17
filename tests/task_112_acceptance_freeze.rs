//! Immutable TASK-112 variants, independently expected SQL outputs and schema.

use std::collections::BTreeSet;

use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, to_bundle_json, ConfiguredSqlInput,
    RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
};

const MATRIX: &str = include_str!("../docs/task-112-acceptance-freeze.json");
const MANIFEST: &str = include_str!("../docs/coverage-manifest.json");
const SCHEMA: &str = include_str!("../schema/protocol.schema.json");

#[test]
fn task112_has_eight_bounded_complete_source_and_terminal_bag_cases() {
    let freeze: Value = serde_json::from_str(MATRIX).unwrap();
    let cases = freeze["cases"].as_array().unwrap();
    assert_eq!(freeze["version"], 1);
    assert_eq!(
        freeze["fixture_ids"],
        serde_json::json!(["71-C112-P", "71-C112-N"])
    );
    assert_eq!(cases.len(), 8);
    assert_eq!(freeze["dialects"].as_array().unwrap().len(), 13);
    let mut ids = BTreeSet::new();
    let mut families = BTreeSet::new();
    for case in cases {
        assert!(ids.insert(case["id"].as_str().unwrap()));
        families.insert(case["kind"].as_str().unwrap());
        assert_eq!(case["classification"], "exact_required");
        assert_eq!(case["left"].as_array().unwrap().len(), 4);
        assert_eq!(case["right"].as_array().unwrap().len(), 2);
        assert_eq!(
            case["expected"].as_array().unwrap().len(),
            case["expected_output_count"].as_u64().unwrap() as usize
        );
        assert_eq!(case["source_instances"]["l"], "l");
        assert_eq!(case["source_instances"]["r"], "r");
        assert_eq!(case["source_column_types"]["k"], "INTEGER");
        assert_eq!(case["canonical_ast_expectation"]["comparison"], "eq");
        assert_eq!(case["output_domain"]["type"], "signed_integer");
        assert_eq!(case["output_domain"]["lower_inclusive"], true);
        assert_eq!(case["output_domain"]["upper_inclusive"], true);
        assert!(case["output_domain"]["contains_null"].is_boolean());
        assert_eq!(case["adapter_facts"].as_array().unwrap().len(), 3);
        for dialect in freeze["dialects"].as_array().unwrap() {
            assert!(case["dialects"][dialect.as_str().unwrap()].is_object());
        }
    }
    assert_eq!(
        families,
        BTreeSet::from(["left_semi", "left_anti", "right_semi", "right_anti",])
    );
    let manifest: Value = serde_json::from_str(MANIFEST).unwrap();
    assert_eq!(
        manifest["task_112_join_acceptance"]["acceptance_freeze"],
        "docs/task-112-acceptance-freeze.json"
    );
    let schema: Value = serde_json::from_str(SCHEMA).unwrap();
    let types = schema["$defs"]["constructiveObligation"]["oneOf"]
        .as_array()
        .unwrap();
    let join = types
        .iter()
        .find(|v| v["properties"]["kind"]["const"] == "join_population")
        .unwrap();
    let patterns = join["properties"]["pattern"]["enum"].as_array().unwrap();
    for p in ["nullable_duplicate_mixed", "nullable_duplicate_disjoint"] {
        assert!(patterns.contains(&Value::String(p.to_string())));
    }
}

#[test]
fn every_parsable_dialect_preserves_the_frozen_join_kind_and_equality() {
    let freeze: Value = serde_json::from_str(MATRIX).unwrap();
    let schemas = ["l", "r"]
        .into_iter()
        .map(|relation| {
            RelationSchema::new(
                relation,
                vec![
                    SchemaColumn::from_sql_type("id", "VARCHAR", "postgresql").unwrap(),
                    SchemaColumn::from_sql_type("k", "INTEGER", "postgresql").unwrap(),
                ],
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let catalog = RelationCatalog::from_schemas(&schemas).unwrap();
    let mut classification_cells = 0;
    let mut unclassified = Vec::new();
    for case in freeze["cases"].as_array().unwrap() {
        let input = SqlInput::inline(case["sql"].as_str().unwrap());
        for dialect in freeze["dialects"].as_array().unwrap() {
            let dialect = dialect.as_str().unwrap();
            let parser = dialect_from_name(dialect).unwrap();
            let configured = ConfiguredSqlInput::new("task112", &input, dialect, parser.as_ref());
            let analyzed = analyze_configured_inputs_with_catalog(&[configured], &catalog);
            let parser_class = if analyzed.is_ok() {
                "parser_equivalent"
            } else {
                "parser_boundary"
            };
            if case["dialects"][dialect]["parser"] != parser_class {
                unclassified.push(format!(
                    "{}/{dialect}={parser_class}",
                    case["id"].as_str().unwrap()
                ));
            }
            if let Ok(bundle) = analyzed {
                let wire: Value = serde_json::from_str(&to_bundle_json(&bundle)).unwrap();
                let joins = wire["inputs"][0]["statements"][0]["joins"]
                    .as_array()
                    .unwrap();
                assert_eq!(joins.len(), 1, "{} / {dialect}", case["id"]);
                assert_eq!(joins[0]["kind"], case["kind"], "{} / {dialect}", case["id"]);
                assert_eq!(
                    joins[0]["condition"]["operator"], "eq",
                    "{} / {dialect}",
                    case["id"]
                );
            }
            // A parser boundary is not a claim of native vendor execution.
            classification_cells += 1;
        }
    }
    assert_eq!(classification_cells, 104);
    assert!(
        unclassified.is_empty(),
        "freeze every dialect cell rather than silently skipping parser boundaries: {}",
        unclassified.join(", ")
    );
}
