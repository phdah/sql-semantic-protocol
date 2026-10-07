use serde_json::{json, Value};
use sql_semantic_protocol::{
    analyze_dbt_manifest, parse_dbt_manifest, to_bundle_json, ConstraintSourceKind,
    ConstraintValue, RelationConstraint,
};
use sqlparser::dialect::PostgreSqlDialect;

fn manifest_with_constraints() -> String {
    let mut manifest: Value = serde_json::from_str(include_str!("fixtures/dbt/manifest-v12.json"))
        .expect("fixture manifest should parse");

    let stg = manifest["nodes"]
        .get_mut("model.demo.stg_orders")
        .expect("stg_orders fixture");
    stg["constraints"] = json!([
        {"type": "primary_key", "columns": ["id"]},
        {"type": "unique", "columns": ["id", "amount"]},
        {
            "type": "foreign_key",
            "columns": ["id"],
            "to": "warehouse.raw.orders",
            "to_columns": ["id"]
        }
    ]);
    stg["columns"] = json!({
        "id": {
            "name": "id",
            "constraints": [{"type": "unique"}]
        },
        "amount": {"name": "amount"}
    });

    let nodes = manifest["nodes"]
        .as_object_mut()
        .expect("manifest nodes should be an object");
    nodes.insert(
        "test.demo.unique_stg_orders_id".to_string(),
        json!({
            "unique_id": "test.demo.unique_stg_orders_id",
            "resource_type": "test",
            "relation_name": null,
            "attached_node": "model.demo.stg_orders",
            "column_name": "id",
            "test_metadata": {
                "name": "unique",
                "kwargs": {"column_name": "id"},
                "namespace": null
            },
            "depends_on": {"nodes": ["model.demo.stg_orders"]}
        }),
    );
    nodes.insert(
        "test.demo.relationships_stg_orders_id".to_string(),
        json!({
            "unique_id": "test.demo.relationships_stg_orders_id",
            "resource_type": "test",
            "relation_name": null,
            "attached_node": "model.demo.stg_orders",
            "column_name": "id",
            "test_metadata": {
                "name": "relationships",
                "kwargs": {
                    "column_name": "id",
                    "field": "id",
                    "to": "warehouse.raw.orders"
                },
                "namespace": null
            },
            "depends_on": {
                "nodes": ["model.demo.stg_orders", "source.demo.orders"]
            }
        }),
    );

    serde_json::to_string(&manifest).expect("manifest should serialize")
}

fn manifest_with_source_accepted_values(
    column: &str,
    data_type: &str,
    values: Value,
    quote: bool,
) -> String {
    let mut manifest: Value = serde_json::from_str(include_str!("fixtures/dbt/manifest-v12.json"))
        .expect("fixture manifest should parse");

    manifest["sources"]["source.demo.orders"]["columns"][column] = json!({
        "name": column,
        "data_type": data_type
    });

    let test_id = format!("test.demo.accepted_values_{column}");
    manifest["nodes"]
        .as_object_mut()
        .expect("manifest nodes should be an object")
        .insert(
            test_id.clone(),
            json!({
                "unique_id": test_id,
                "resource_type": "test",
                "relation_name": null,
                "attached_node": "source.demo.orders",
                "column_name": column,
                "test_metadata": {
                    "name": "accepted_values",
                    "kwargs": {
                        "column_name": column,
                        "values": values,
                        "quote": quote
                    },
                    "namespace": null
                },
                "depends_on": {"nodes": ["source.demo.orders"]}
            }),
        );

    serde_json::to_string(&manifest).expect("manifest should serialize")
}

fn source_accepted_values(
    column: &str,
    data_type: &str,
    values: Value,
    quote: bool,
) -> (Vec<ConstraintValue>, Vec<String>) {
    let json = manifest_with_source_accepted_values(column, data_type, values, quote);
    let manifest = parse_dbt_manifest(&json).expect("manifest should parse");
    let metadata = manifest
        .relation_constraints()
        .iter()
        .find(|metadata| metadata.relation() == "warehouse.raw.orders")
        .expect("source constraint metadata");
    let values = metadata
        .constraints()
        .iter()
        .find_map(|constraint| match constraint {
            RelationConstraint::AcceptedValues(accepted)
                if accepted.column() == column && accepted.quote() == quote =>
            {
                Some(accepted.values().to_vec())
            }
            _ => None,
        })
        .unwrap_or_default();
    let diagnostics = metadata
        .diagnostics()
        .iter()
        .map(|diagnostic| diagnostic.code().to_string())
        .collect();

    (values, diagnostics)
}

#[test]
fn dbt_constraints_and_generic_tests_normalize_to_one_canonical_model() {
    let manifest = parse_dbt_manifest(&manifest_with_constraints()).expect("manifest should parse");
    let metadata = manifest
        .relation_constraints()
        .iter()
        .find(|metadata| metadata.relation() == "warehouse.analytics.stg_orders")
        .expect("stg_orders constraint metadata");

    assert!(metadata.constraints().iter().any(|constraint| {
        matches!(
            constraint,
            RelationConstraint::PrimaryKey(key) if key.columns() == ["id"]
        )
    }));
    assert!(metadata.constraints().iter().any(|constraint| {
        matches!(
            constraint,
            RelationConstraint::UniqueKey(key)
                if key.columns() == ["id", "amount"]
        )
    }));

    let single_unique = metadata
        .constraints()
        .iter()
        .find_map(|constraint| match constraint {
            RelationConstraint::UniqueKey(key) if key.columns() == ["id"] => Some(key),
            _ => None,
        })
        .expect("single-column unique metadata");
    assert_eq!(single_unique.evidence().len(), 2);
    assert!(
        single_unique
            .evidence()
            .iter()
            .any(|evidence| evidence.provenance().source_kind()
                == ConstraintSourceKind::DbtConstraint)
    );
    assert!(single_unique
        .evidence()
        .iter()
        .any(|evidence| evidence.provenance().source_kind() == ConstraintSourceKind::DbtTest));

    let foreign_key = metadata
        .constraints()
        .iter()
        .find_map(|constraint| match constraint {
            RelationConstraint::ForeignKey(key) => Some(key),
            _ => None,
        })
        .expect("foreign-key metadata");
    assert_eq!(foreign_key.columns(), ["id"]);
    assert_eq!(foreign_key.referenced_relation(), "warehouse.raw.orders");
    assert_eq!(foreign_key.referenced_columns(), ["id"]);
    assert_eq!(foreign_key.evidence().len(), 2);
}

#[test]
fn dbt_constraint_metadata_survives_bundle_emission() {
    let manifest = parse_dbt_manifest(&manifest_with_constraints()).expect("manifest should parse");
    let bundle = analyze_dbt_manifest(&manifest, "postgresql", &PostgreSqlDialect {})
        .expect("dbt manifest should analyze");
    let json: Value =
        serde_json::from_str(&to_bundle_json(&bundle)).expect("bundle JSON should parse");

    let metadata = json["relation_constraints"]
        .as_array()
        .expect("relation constraints should be emitted")
        .iter()
        .find(|metadata| metadata["relation"] == "warehouse.analytics.stg_orders")
        .expect("stg_orders constraints should be emitted");
    assert!(metadata["constraints"]
        .as_array()
        .expect("constraints should be an array")
        .iter()
        .any(|constraint| constraint["kind"] == "primary_key"));
    assert!(metadata.to_string().contains("dbt_constraint"));
    assert!(metadata.to_string().contains("dbt_test"));
    assert!(!metadata
        .to_string()
        .contains("\"enforcement\":\"enforced\""));
}

#[test]
fn dbt_relationships_self_reference_resolves_to_attached_relation() {
    let mut manifest: Value = serde_json::from_str(include_str!("fixtures/dbt/manifest-v12.json"))
        .expect("fixture manifest should parse");
    manifest["nodes"]
        .as_object_mut()
        .expect("manifest nodes should be an object")
        .insert(
            "test.demo.relationships_stg_orders_self".to_string(),
            json!({
                "unique_id": "test.demo.relationships_stg_orders_self",
                "resource_type": "test",
                "relation_name": null,
                "attached_node": "model.demo.stg_orders",
                "column_name": "id",
                "test_metadata": {
                    "name": "relationships",
                    "kwargs": {
                        "column_name": "id",
                        "field": "id",
                        "to": "ref('stg_orders')"
                    },
                    "namespace": null
                },
                "depends_on": {"nodes": ["model.demo.stg_orders"]}
            }),
        );

    let json = serde_json::to_string(&manifest).expect("manifest should serialize");
    let manifest = parse_dbt_manifest(&json).expect("manifest should parse");
    let bundle = analyze_dbt_manifest(&manifest, "postgresql", &PostgreSqlDialect {})
        .expect("dbt manifest should analyze");
    let metadata = bundle
        .relation_constraints()
        .iter()
        .find(|metadata| metadata.relation() == "warehouse.analytics.stg_orders")
        .expect("stg_orders constraint metadata");
    let foreign_key = metadata
        .constraints()
        .iter()
        .find_map(|constraint| match constraint {
            RelationConstraint::ForeignKey(key) => Some(key),
            _ => None,
        })
        .expect("self-referencing foreign key");

    assert_eq!(
        foreign_key.referenced_relation(),
        "warehouse.analytics.stg_orders"
    );
}

#[test]
fn dbt_foreign_key_ref_target_resolves_to_canonical_relation() {
    let mut manifest: Value = serde_json::from_str(include_str!("fixtures/dbt/manifest-v12.json"))
        .expect("fixture manifest should parse");
    manifest["nodes"]["model.demo.final_orders"]["constraints"] = json!([{
        "type": "foreign_key",
        "columns": ["id"],
        "to": "ref('stg_orders')",
        "to_columns": ["id"]
    }]);

    let json = serde_json::to_string(&manifest).expect("manifest should serialize");
    let manifest = parse_dbt_manifest(&json).expect("manifest should parse");
    let bundle = analyze_dbt_manifest(&manifest, "postgresql", &PostgreSqlDialect {})
        .expect("dbt manifest should analyze");
    let metadata = bundle
        .relation_constraints()
        .iter()
        .find(|metadata| metadata.relation() == "warehouse.analytics.final_orders")
        .expect("final_orders constraint metadata");
    let foreign_key = metadata
        .constraints()
        .iter()
        .find_map(|constraint| match constraint {
            RelationConstraint::ForeignKey(key) => Some(key),
            _ => None,
        })
        .expect("foreign key");

    assert_eq!(
        foreign_key.referenced_relation(),
        "warehouse.analytics.stg_orders"
    );
}

#[test]
fn dbt_foreign_key_source_target_resolves_to_canonical_relation() {
    let mut manifest: Value = serde_json::from_str(include_str!("fixtures/dbt/manifest-v12.json"))
        .expect("fixture manifest should parse");
    manifest["nodes"]["model.demo.stg_orders"]["constraints"] = json!([{
        "type": "foreign_key",
        "columns": ["id"],
        "to": "source('demo', 'orders')",
        "to_columns": ["id"]
    }]);

    let json = serde_json::to_string(&manifest).expect("manifest should serialize");
    let manifest = parse_dbt_manifest(&json).expect("manifest should parse");
    let bundle = analyze_dbt_manifest(&manifest, "postgresql", &PostgreSqlDialect {})
        .expect("dbt manifest should analyze");
    let metadata = bundle
        .relation_constraints()
        .iter()
        .find(|metadata| metadata.relation() == "warehouse.analytics.stg_orders")
        .expect("stg_orders constraint metadata");
    let foreign_key = metadata
        .constraints()
        .iter()
        .find_map(|constraint| match constraint {
            RelationConstraint::ForeignKey(key) => Some(key),
            _ => None,
        })
        .expect("foreign key");

    assert_eq!(foreign_key.referenced_relation(), "warehouse.raw.orders");
}

#[test]
fn dbt_foreign_key_with_unresolved_target_fails_explicitly() {
    let mut manifest: Value = serde_json::from_str(include_str!("fixtures/dbt/manifest-v12.json"))
        .expect("fixture manifest should parse");
    manifest["nodes"]["model.demo.stg_orders"]["constraints"] = json!([{
        "type": "foreign_key",
        "columns": ["id"],
        "to": "ref('missing_orders')",
        "to_columns": ["id"]
    }]);

    let json = serde_json::to_string(&manifest).expect("manifest should serialize");
    let error = parse_dbt_manifest(&json).expect_err("unresolved target should fail");

    assert!(error
        .to_string()
        .contains("does not resolve to a canonical dbt relation"));
    assert!(error.to_string().contains("ref('missing_orders')"));
}

#[test]
fn dbt_relationships_with_unresolved_target_fails_explicitly() {
    let mut manifest: Value = serde_json::from_str(include_str!("fixtures/dbt/manifest-v12.json"))
        .expect("fixture manifest should parse");
    manifest["nodes"]
        .as_object_mut()
        .expect("manifest nodes should be an object")
        .insert(
            "test.demo.relationships_stg_orders_missing".to_string(),
            json!({
                "unique_id": "test.demo.relationships_stg_orders_missing",
                "resource_type": "test",
                "relation_name": null,
                "attached_node": "model.demo.stg_orders",
                "column_name": "id",
                "test_metadata": {
                    "name": "relationships",
                    "kwargs": {
                        "column_name": "id",
                        "field": "id",
                        "to": "ref('missing_orders')"
                    },
                    "namespace": null
                },
                "depends_on": {"nodes": ["model.demo.stg_orders"]}
            }),
        );

    let json = serde_json::to_string(&manifest).expect("manifest should serialize");
    let error = parse_dbt_manifest(&json).expect_err("unresolved target should fail");

    assert!(error
        .to_string()
        .contains("does not resolve to a canonical dbt relation"));
    assert!(error.to_string().contains("ref('missing_orders')"));
}

#[test]
fn dbt_singular_test_attached_to_relation_is_reported() {
    let mut manifest: Value = serde_json::from_str(include_str!("fixtures/dbt/manifest-v12.json"))
        .expect("fixture manifest should parse");
    manifest["nodes"]
        .as_object_mut()
        .expect("manifest nodes should be an object")
        .insert(
            "test.demo.singular_stg_orders".to_string(),
            json!({
                "unique_id": "test.demo.singular_stg_orders",
                "resource_type": "test",
                "relation_name": null,
                "attached_node": "model.demo.stg_orders",
                "depends_on": {"nodes": ["model.demo.stg_orders"]}
            }),
        );

    let json = serde_json::to_string(&manifest).expect("manifest should serialize");
    let manifest = parse_dbt_manifest(&json).expect("manifest should parse");
    let metadata = manifest
        .relation_constraints()
        .iter()
        .find(|metadata| metadata.relation() == "warehouse.analytics.stg_orders")
        .expect("stg_orders diagnostic metadata");

    assert!(metadata
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code() == "unsupported_dbt_singular_test"));
}

#[test]
fn dbt_test_config_that_changes_semantics_is_reported_and_not_promoted() {
    let mut manifest: Value = serde_json::from_str(include_str!("fixtures/dbt/manifest-v12.json"))
        .expect("fixture manifest should parse");
    manifest["nodes"]
        .as_object_mut()
        .expect("manifest nodes should be an object")
        .insert(
            "test.demo.configured_unique_stg_orders_id".to_string(),
            json!({
                "unique_id": "test.demo.configured_unique_stg_orders_id",
                "resource_type": "test",
                "relation_name": null,
                "attached_node": "model.demo.stg_orders",
                "column_name": "id",
                "config": {
                    "where": "id > 5",
                    "severity": "warn",
                    "warn_if": "> 1",
                    "error_if": "> 2",
                    "limit": 10,
                    "fail_calc": "sum(failures)"
                },
                "test_metadata": {
                    "name": "unique",
                    "kwargs": {"column_name": "id"},
                    "namespace": null
                },
                "depends_on": {"nodes": ["model.demo.stg_orders"]}
            }),
        );

    let json = serde_json::to_string(&manifest).expect("manifest should serialize");
    let manifest = parse_dbt_manifest(&json).expect("manifest should parse");
    let metadata = manifest
        .relation_constraints()
        .iter()
        .find(|metadata| metadata.relation() == "warehouse.analytics.stg_orders")
        .expect("stg_orders diagnostic metadata");
    let diagnostic = metadata
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code() == "unsupported_dbt_test_config")
        .expect("configured test diagnostic");

    for key in [
        "where",
        "severity",
        "warn_if",
        "error_if",
        "limit",
        "fail_calc",
    ] {
        assert!(diagnostic.message().contains(key));
    }
    assert!(!metadata.constraints().iter().any(|constraint| {
        matches!(
            constraint,
            RelationConstraint::UniqueKey(key)
                if key.columns() == ["id"]
                    && key.evidence().iter().any(|evidence| {
                        evidence.provenance().source_kind() == ConstraintSourceKind::DbtTest
                            && evidence.provenance().source_id()
                                == "test.demo.configured_unique_stg_orders_id"
                    })
        )
    }));
}

#[test]
fn dbt_check_and_custom_constraints_are_reported() {
    let mut manifest: Value = serde_json::from_str(include_str!("fixtures/dbt/manifest-v12.json"))
        .expect("fixture manifest should parse");
    manifest["nodes"]["model.demo.stg_orders"]["constraints"] = json!([
        {"type": "check", "expression": "amount > 0"},
        {"type": "custom", "name": "warehouse_specific"}
    ]);

    let json = serde_json::to_string(&manifest).expect("manifest should serialize");
    let manifest = parse_dbt_manifest(&json).expect("manifest should parse");
    let metadata = manifest
        .relation_constraints()
        .iter()
        .find(|metadata| metadata.relation() == "warehouse.analytics.stg_orders")
        .expect("stg_orders diagnostic metadata");

    assert!(metadata
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code() == "unsupported_check_constraint"));
    assert!(metadata
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code() == "unsupported_dbt_constraint"));
}

#[test]
fn dbt_accepted_values_normalize_quoted_and_unquoted_literals() {
    let (quoted_strings, diagnostics) =
        source_accepted_values("status", "VARCHAR", json!(["new", "done"]), true);
    assert_eq!(
        quoted_strings,
        vec![
            ConstraintValue::String("done".to_string()),
            ConstraintValue::String("new".to_string()),
        ]
    );
    assert!(diagnostics.is_empty());

    let (unquoted_strings, diagnostics) =
        source_accepted_values("status", "VARCHAR", json!(["'new'", "'done'"]), false);
    assert_eq!(
        unquoted_strings,
        vec![
            ConstraintValue::String("done".to_string()),
            ConstraintValue::String("new".to_string()),
        ]
    );
    assert!(diagnostics.is_empty());

    let (quoted_numbers, diagnostics) =
        source_accepted_values("amount", "BIGINT", json!([1, 2]), true);
    assert_eq!(
        quoted_numbers,
        vec![ConstraintValue::Integer(1), ConstraintValue::Integer(2)]
    );
    assert!(diagnostics.is_empty());

    let (unquoted_numbers, diagnostics) =
        source_accepted_values("amount", "BIGINT", json!(["1", "2"]), false);
    assert_eq!(
        unquoted_numbers,
        vec![ConstraintValue::Integer(1), ConstraintValue::Integer(2)]
    );
    assert!(diagnostics.is_empty());
}

#[test]
fn dbt_unquoted_accepted_value_that_is_not_a_scalar_literal_is_reported() {
    let (values, diagnostics) =
        source_accepted_values("status", "VARCHAR", json!(["current_date"]), false);

    assert!(values.is_empty());
    assert_eq!(
        diagnostics,
        vec!["unsupported_dbt_accepted_value".to_string()]
    );
}

#[test]
fn dbt_unquoted_accepted_values_emit_normalized_scalar_types() {
    let json = manifest_with_source_accepted_values("amount", "BIGINT", json!(["1", "2"]), false);
    let manifest = parse_dbt_manifest(&json).expect("manifest should parse");
    let bundle = analyze_dbt_manifest(&manifest, "postgresql", &PostgreSqlDialect {})
        .expect("dbt manifest should analyze");
    let emitted: Value =
        serde_json::from_str(&to_bundle_json(&bundle)).expect("bundle JSON should parse");
    let constraint = emitted["relation_constraints"]
        .as_array()
        .expect("relation constraints")
        .iter()
        .find(|metadata| metadata["relation"] == "warehouse.raw.orders")
        .and_then(|metadata| metadata["constraints"].as_array())
        .and_then(|constraints| {
            constraints
                .iter()
                .find(|constraint| constraint["kind"] == "accepted_values")
        })
        .expect("accepted-values constraint");

    assert_eq!(
        constraint["values"],
        json!([
            {"type": "integer", "value": 1},
            {"type": "integer", "value": 2}
        ])
    );
    assert_eq!(constraint["quote"], false);
}

#[test]
fn relationless_unsupported_dbt_test_is_emitted_at_bundle_level() {
    let mut manifest: Value = serde_json::from_str(include_str!("fixtures/dbt/manifest-v12.json"))
        .expect("fixture manifest should parse");
    let nodes = manifest["nodes"]
        .as_object_mut()
        .expect("manifest nodes should be an object");
    nodes.insert(
        "seed.demo.relationless".to_string(),
        json!({
            "unique_id": "seed.demo.relationless",
            "resource_type": "seed",
            "relation_name": null,
            "columns": {}
        }),
    );
    nodes.insert(
        "test.demo.unscoped_custom".to_string(),
        json!({
            "unique_id": "test.demo.unscoped_custom",
            "resource_type": "test",
            "relation_name": null,
            "attached_node": "seed.demo.relationless",
            "test_metadata": {
                "name": "expression_is_true",
                "kwargs": {},
                "namespace": "dbt_utils"
            },
            "depends_on": {"nodes": ["seed.demo.relationless"]}
        }),
    );

    let json = serde_json::to_string(&manifest).expect("manifest should serialize");
    let manifest = parse_dbt_manifest(&json).expect("manifest should parse");
    assert!(manifest
        .constraint_diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code() == "unsupported_dbt_test"));

    let bundle = analyze_dbt_manifest(&manifest, "postgresql", &PostgreSqlDialect {})
        .expect("dbt manifest should analyze");
    assert!(bundle
        .constraint_diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code() == "unsupported_dbt_test"));

    let emitted: Value =
        serde_json::from_str(&to_bundle_json(&bundle)).expect("bundle JSON should parse");
    assert_eq!(
        emitted["constraint_diagnostics"][0]["code"],
        "unsupported_dbt_test"
    );
}
