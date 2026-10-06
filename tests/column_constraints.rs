use serde_json::{json, Value};
use sql_semantic_protocol::{
    analyze_dbt_manifest, parse_dbt_manifest, select_targets, to_bundle_json, ConstraintSourceKind,
    ConstraintValue, RelationConstraint,
};
use sqlparser::dialect::PostgreSqlDialect;

fn manifest_with_tests(tests: Vec<(&str, Value)>) -> String {
    let mut manifest: Value =
        serde_json::from_str(include_str!("fixtures/dbt/manifest-v12.json"))
            .expect("fixture manifest should parse");
    let nodes = manifest["nodes"]
        .as_object_mut()
        .expect("manifest nodes should be an object");
    for (id, value) in tests {
        nodes.insert(id.to_string(), value);
    }
    serde_json::to_string(&manifest).expect("manifest should serialize")
}

fn generic_test(
    id: &str,
    attached_node: &str,
    column_name: &str,
    name: &str,
    kwargs: Value,
) -> Value {
    json!({
        "unique_id": id,
        "resource_type": "test",
        "relation_name": null,
        "attached_node": attached_node,
        "column_name": column_name,
        "test_metadata": {
            "name": name,
            "kwargs": kwargs,
            "namespace": null
        },
        "depends_on": {"nodes": [attached_node]}
    })
}

#[test]
fn dbt_not_null_and_accepted_values_use_canonical_column_constraints() {
    let manifest = parse_dbt_manifest(&manifest_with_tests(vec![
        (
            "test.demo.not_null_stg_orders_id",
            generic_test(
                "test.demo.not_null_stg_orders_id",
                "model.demo.stg_orders",
                "id",
                "not_null",
                json!({"column_name": "id"}),
            ),
        ),
        (
            "test.demo.accepted_values_stg_orders_amount",
            generic_test(
                "test.demo.accepted_values_stg_orders_amount",
                "model.demo.stg_orders",
                "amount",
                "accepted_values",
                json!({
                    "column_name": "amount",
                    "values": [10, 20, true, null, "30", 3.5],
                    "quote": false
                }),
            ),
        ),
        (
            "test.demo.not_null_source_orders_id",
            generic_test(
                "test.demo.not_null_source_orders_id",
                "source.demo.orders",
                "id",
                "not_null",
                json!({"column_name": "id"}),
            ),
        ),
    ]))
    .expect("manifest should parse");

    let model = manifest
        .relation_constraints()
        .iter()
        .find(|set| set.relation() == "warehouse.analytics.stg_orders")
        .expect("model constraints");

    let not_null = model
        .constraints()
        .iter()
        .find_map(|constraint| match constraint {
            RelationConstraint::NotNull(constraint) if constraint.column() == "id" => {
                Some(constraint)
            }
            _ => None,
        })
        .expect("not-null constraint");
    assert_eq!(
        not_null.evidence()[0].provenance().source_kind(),
        ConstraintSourceKind::DbtTest
    );

    let accepted = model
        .constraints()
        .iter()
        .find_map(|constraint| match constraint {
            RelationConstraint::AcceptedValues(constraint) if constraint.column() == "amount" => {
                Some(constraint)
            }
            _ => None,
        })
        .expect("accepted-values constraint");
    assert!(!accepted.quote());
    assert!(accepted.values().contains(&ConstraintValue::Integer(10)));
    assert!(accepted.values().contains(&ConstraintValue::Integer(20)));
    assert!(accepted.values().contains(&ConstraintValue::Boolean(true)));
    assert!(accepted.values().contains(&ConstraintValue::Null));
    assert!(accepted
        .values()
        .contains(&ConstraintValue::String("30".to_string())));
    assert!(accepted
        .values()
        .contains(&ConstraintValue::Number("3.5".to_string())));

    let source = manifest
        .relation_constraints()
        .iter()
        .find(|set| set.relation() == "warehouse.raw.orders")
        .expect("source constraints");
    assert!(source.constraints().iter().any(|constraint| {
        matches!(
            constraint,
            RelationConstraint::NotNull(constraint) if constraint.column() == "id"
        )
    }));
}

#[test]
fn accepted_values_intersect_and_empty_intersection_is_explicit() {
    let manifest = parse_dbt_manifest(&manifest_with_tests(vec![
        (
            "test.demo.accepted_values_stg_orders_amount_one",
            generic_test(
                "test.demo.accepted_values_stg_orders_amount_one",
                "model.demo.stg_orders",
                "amount",
                "accepted_values",
                json!({"column_name": "amount", "values": [10, 20], "quote": false}),
            ),
        ),
        (
            "test.demo.accepted_values_stg_orders_amount_two",
            generic_test(
                "test.demo.accepted_values_stg_orders_amount_two",
                "model.demo.stg_orders",
                "amount",
                "accepted_values",
                json!({"column_name": "amount", "values": [30, 40], "quote": false}),
            ),
        ),
    ]))
    .expect("manifest should parse");

    let model = manifest
        .relation_constraints()
        .iter()
        .find(|set| set.relation() == "warehouse.analytics.stg_orders")
        .expect("model constraints");
    let accepted = model
        .constraints()
        .iter()
        .find_map(|constraint| match constraint {
            RelationConstraint::AcceptedValues(constraint) if constraint.column() == "amount" => {
                Some(constraint)
            }
            _ => None,
        })
        .expect("accepted-values constraint");

    assert!(accepted.values().is_empty());
    assert_eq!(accepted.evidence().len(), 2);
    assert!(model
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code() == "unsatisfiable_accepted_values"));
}

#[test]
fn unsupported_dbt_tests_are_reported_explicitly() {
    let manifest = parse_dbt_manifest(&manifest_with_tests(vec![(
        "test.demo.custom_stg_orders",
        json!({
            "unique_id": "test.demo.custom_stg_orders",
            "resource_type": "test",
            "relation_name": null,
            "attached_node": "model.demo.stg_orders",
            "column_name": "amount",
            "test_metadata": {
                "name": "expression_is_true",
                "kwargs": {"column_name": "amount"},
                "namespace": "dbt_utils"
            },
            "depends_on": {"nodes": ["model.demo.stg_orders"]}
        }),
    )]))
    .expect("manifest should parse");

    let model = manifest
        .relation_constraints()
        .iter()
        .find(|set| set.relation() == "warehouse.analytics.stg_orders")
        .expect("model constraints");
    assert!(model
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code() == "unsupported_dbt_test"));
}

#[test]
fn accepted_values_reject_non_scalar_arguments() {
    let error = parse_dbt_manifest(&manifest_with_tests(vec![(
        "test.demo.accepted_values_stg_orders_amount",
        generic_test(
            "test.demo.accepted_values_stg_orders_amount",
            "model.demo.stg_orders",
            "amount",
            "accepted_values",
            json!({"column_name": "amount", "values": [{"bad": "value"}]}),
        ),
    )]))
    .expect_err("object accepted value must fail");

    assert!(error.to_string().contains("scalar JSON values"));
}


#[test]
fn column_constraints_survive_target_selection_and_emission() {
    let manifest = parse_dbt_manifest(&manifest_with_tests(vec![
        (
            "test.demo.not_null_stg_orders_id",
            generic_test(
                "test.demo.not_null_stg_orders_id",
                "model.demo.stg_orders",
                "id",
                "not_null",
                json!({"column_name": "id"}),
            ),
        ),
        (
            "test.demo.accepted_values_stg_orders_amount",
            generic_test(
                "test.demo.accepted_values_stg_orders_amount",
                "model.demo.stg_orders",
                "amount",
                "accepted_values",
                json!({"column_name": "amount", "values": [20, 50], "quote": false}),
            ),
        ),
    ]))
    .expect("manifest should parse");
    let bundle = analyze_dbt_manifest(&manifest, "postgresql", &PostgreSqlDialect {})
        .expect("manifest should analyze");
    let selected = select_targets(
        &bundle,
        &["warehouse.analytics.final_orders".to_string()],
    )
    .expect("target should resolve");
    let json: Value =
        serde_json::from_str(&to_bundle_json(&selected)).expect("bundle JSON should parse");

    let stg = json["relation_constraints"]
        .as_array()
        .expect("relation constraints should be emitted")
        .iter()
        .find(|set| set["relation"] == "warehouse.analytics.stg_orders")
        .expect("stg_orders constraints should survive selection");
    assert!(stg["constraints"]
        .as_array()
        .expect("constraints should be an array")
        .iter()
        .any(|constraint| constraint["kind"] == "not_null"));
    assert!(stg["constraints"]
        .as_array()
        .expect("constraints should be an array")
        .iter()
        .any(|constraint| constraint["kind"] == "accepted_values"));
}
