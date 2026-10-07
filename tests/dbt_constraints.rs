use serde_json::{json, Value};
use sql_semantic_protocol::{
    analyze_dbt_manifest, parse_dbt_manifest, to_bundle_json, ConstraintSourceKind,
    RelationConstraint,
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
