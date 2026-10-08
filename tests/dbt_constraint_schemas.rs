use serde_json::{json, Value};
use sql_semantic_protocol::{
    analyze_dbt_artifacts, analyze_dbt_manifest_with_schemas, parse_dbt_catalog,
    parse_dbt_manifest, to_bundle_json, DbtArtifactsError,
};
use sqlparser::dialect::PostgreSqlDialect;

const CHILD: &str = "warehouse.archive.order_items";
const PARENT: &str = "warehouse.archive.orders";

fn constraint_only_manifest() -> Value {
    let mut manifest: Value = serde_json::from_str(include_str!("fixtures/dbt/manifest-v12.json"))
        .expect("manifest fixture should parse");
    let sources = manifest["sources"]
        .as_object_mut()
        .expect("sources should be an object");
    sources.insert(
        "source.demo.archive.orders".to_string(),
        json!({
            "unique_id": "source.demo.archive.orders",
            "resource_type": "source",
            "relation_name": PARENT,
            "columns": {
                "id": {"name": "id", "data_type": "BIGINT"},
                "description": {"name": "description", "data_type": "VARCHAR"}
            }
        }),
    );
    sources.insert(
        "source.demo.archive.order_items".to_string(),
        json!({
            "unique_id": "source.demo.archive.order_items",
            "resource_type": "source",
            "relation_name": CHILD,
            "columns": {
                "id": {"name": "id", "data_type": "INTEGER"},
                "order_id": {"name": "order_id", "data_type": "BIGINT"}
            }
        }),
    );
    manifest["nodes"]
        .as_object_mut()
        .expect("nodes should be an object")
        .insert(
            "test.demo.archive_relationships".to_string(),
            json!({
                "unique_id": "test.demo.archive_relationships",
                "resource_type": "test",
                "relation_name": null,
                "attached_node": null,
                "column_name": "order_id",
                "test_metadata": {
                    "name": "relationships",
                    "namespace": null,
                    "kwargs": {
                        "model": "{{ get_where_subquery(source('archive', 'order_items')) }}",
                        "column_name": "order_id",
                        "arguments": {
                            "to": "source('archive', 'orders')",
                            "field": "id"
                        }
                    }
                },
                "depends_on": {
                    "nodes": [
                        "source.demo.archive.order_items",
                        "source.demo.archive.orders"
                    ]
                }
            }),
        );
    manifest
}

fn analyze_manifest(value: &Value) -> Result<Value, DbtArtifactsError> {
    let manifest = parse_dbt_manifest(&value.to_string()).expect("manifest should parse");
    analyze_dbt_manifest_with_schemas(&manifest, "postgresql", &PostgreSqlDialect {})
        .map(|bundle| serde_json::from_str(&to_bundle_json(&bundle)).expect("protocol JSON"))
}

fn schema<'a>(protocol: &'a Value, relation: &str) -> &'a Value {
    protocol["source_schemas"]
        .as_array()
        .expect("source schemas should be emitted")
        .iter()
        .find(|schema| schema["relation"] == relation)
        .unwrap_or_else(|| panic!("missing typed schema for {relation}"))
}

fn assert_foreign_key(protocol: &Value, referenced_relation: &str) {
    let child_constraints = protocol["relation_constraints"]
        .as_array()
        .expect("relation constraints should be emitted")
        .iter()
        .find(|set| set["relation"] == CHILD)
        .expect("child constraint set");
    assert!(child_constraints["constraints"]
        .as_array()
        .expect("constraints should be an array")
        .iter()
        .any(|constraint| {
            constraint["kind"] == "foreign_key"
                && constraint["columns"] == json!(["order_id"])
                && constraint["referenced_relation"] == referenced_relation
                && constraint["referenced_columns"] == json!(["id"])
        }));
}

#[test]
fn constraint_only_source_relationships_emit_both_typed_physical_schemas() {
    let manifest = constraint_only_manifest();
    let protocol = analyze_manifest(&manifest).expect("both source schemas are declared");
    assert_foreign_key(&protocol, PARENT);
    for relation in [CHILD, PARENT] {
        assert_eq!(schema(&protocol, relation)["source_kind"], "dbt_manifest");
    }
    assert_eq!(
        schema(&protocol, PARENT)["columns"][0]["data_type"]["kind"],
        "string"
    );
    assert_eq!(
        schema(&protocol, PARENT)["columns"][1]["data_type"]["kind"],
        "signed_integer"
    );
    assert!(protocol["layers"]
        .as_array()
        .expect("layers should be emitted")
        .iter()
        .all(|layer| !layer.to_string().contains("warehouse.archive")));
}

#[test]
fn declared_foreign_keys_derive_schema_coverage_without_sql_dependencies() {
    let mut manifest = constraint_only_manifest();
    manifest["nodes"]
        .as_object_mut()
        .expect("nodes should be an object")
        .remove("test.demo.archive_relationships");
    manifest["sources"]["source.demo.archive.order_items"]["constraints"] = json!([{
        "type": "foreign_key",
        "columns": ["order_id"],
        "to": "source('archive', 'orders')",
        "to_columns": ["id"]
    }]);
    let protocol = analyze_manifest(&manifest).expect("declared foreign key should analyze");
    assert_foreign_key(&protocol, PARENT);
    assert_eq!(schema(&protocol, CHILD)["source_kind"], "dbt_manifest");
    assert_eq!(schema(&protocol, PARENT)["source_kind"], "dbt_manifest");
}

#[test]
fn catalog_schema_is_authoritative_for_constraint_only_sources() {
    let manifest_json = constraint_only_manifest();
    let manifest = parse_dbt_manifest(&manifest_json.to_string()).expect("manifest should parse");
    let mut catalog: Value = serde_json::from_str(include_str!("fixtures/dbt/catalog-v1.json"))
        .expect("catalog fixture should parse");
    catalog["sources"]["source.demo.archive.orders"] = json!({
        "unique_id": "source.demo.archive.orders",
        "columns": {
            "id": {"name": "id", "type": "INTEGER", "index": 1},
            "description": {"name": "description", "type": "TEXT", "index": 2}
        }
    });
    let catalog = parse_dbt_catalog(&catalog.to_string()).expect("catalog should parse");
    let bundle = analyze_dbt_artifacts(&manifest, &catalog, "postgresql", &PostgreSqlDialect {})
        .expect("catalog parent and manifest child should analyze");
    let protocol: Value = serde_json::from_str(&to_bundle_json(&bundle)).expect("protocol JSON");
    assert_foreign_key(&protocol, PARENT);
    assert_eq!(schema(&protocol, PARENT)["source_kind"], "dbt_catalog");
    assert_eq!(schema(&protocol, CHILD)["source_kind"], "dbt_manifest");
    assert_eq!(schema(&protocol, PARENT)["columns"][0]["name"], "id");
    assert_eq!(
        schema(&protocol, PARENT)["columns"][0]["data_type"]["kind"],
        "signed_integer"
    );
}

#[test]
fn missing_constraint_only_parent_datatype_is_reported() {
    let mut manifest = constraint_only_manifest();
    manifest["sources"]["source.demo.archive.orders"]["columns"]["id"]
        .as_object_mut()
        .expect("parent id should be an object")
        .remove("data_type");
    let error = analyze_manifest(&manifest).expect_err("incomplete declared schema must fail");
    assert!(matches!(
        error,
        DbtArtifactsError::MissingDeclaredColumnTypes { relation, columns }
            if relation == PARENT && columns == ["id"]
    ));
}

#[test]
fn missing_constraint_only_parent_schema_names_the_required_column() {
    let mut manifest = constraint_only_manifest();
    manifest["sources"]["source.demo.archive.orders"]["columns"] = json!({});
    let error = analyze_manifest(&manifest).expect_err("missing parent schema must fail");
    assert!(matches!(
        error,
        DbtArtifactsError::MissingConstraintSchema { relation, columns }
            if relation == PARENT && columns == ["id"]
    ));
}

#[test]
fn catalog_missing_referenced_column_fails_instead_of_dropping_foreign_key() {
    let manifest_json = constraint_only_manifest();
    let manifest = parse_dbt_manifest(&manifest_json.to_string()).expect("manifest should parse");
    let mut catalog: Value = serde_json::from_str(include_str!("fixtures/dbt/catalog-v1.json"))
        .expect("catalog fixture should parse");
    catalog["sources"]["source.demo.archive.orders"] = json!({
        "unique_id": "source.demo.archive.orders",
        "columns": {
            "description": {"name": "description", "type": "TEXT", "index": 1}
        }
    });
    let catalog = parse_dbt_catalog(&catalog.to_string()).expect("catalog should parse");
    let error = analyze_dbt_artifacts(&manifest, &catalog, "postgresql", &PostgreSqlDialect {})
        .expect_err("catalog schema missing referenced id must fail");
    assert!(matches!(
        error,
        DbtArtifactsError::MissingConstraintSchema { relation, columns }
            if relation == PARENT && columns == ["id"]
    ));
}

#[test]
fn contradictory_manifest_schemas_for_one_constraint_relation_fail() {
    let mut manifest = constraint_only_manifest();
    manifest["sources"]["source.demo.archive.orders_alias"] = json!({
        "unique_id": "source.demo.archive.orders_alias",
        "resource_type": "source",
        "relation_name": PARENT,
        "columns": {
            "id": {"name": "id", "data_type": "VARCHAR"},
            "description": {"name": "description", "data_type": "VARCHAR"}
        }
    });
    let error = analyze_manifest(&manifest).expect_err("contradictory schema must fail");
    assert!(matches!(
        error,
        DbtArtifactsError::ConflictingCatalogSchemas { relation, .. } if relation == PARENT
    ));
}

#[test]
fn produced_model_foreign_key_target_is_not_created_as_physical_source() {
    let mut manifest = constraint_only_manifest();
    manifest["nodes"]
        .as_object_mut()
        .expect("nodes should be an object")
        .remove("test.demo.archive_relationships");
    manifest["sources"]["source.demo.archive.order_items"]["constraints"] = json!([{
        "type": "foreign_key",
        "columns": ["order_id"],
        "to": "ref('stg_orders')",
        "to_columns": ["id"]
    }]);
    let protocol = analyze_manifest(&manifest).expect("produced target should analyze");
    assert_foreign_key(&protocol, "warehouse.analytics.stg_orders");
    assert_eq!(schema(&protocol, CHILD)["source_kind"], "dbt_manifest");
    assert!(protocol["source_schemas"]
        .as_array()
        .expect("source schemas")
        .iter()
        .all(|schema| schema["relation"] != "warehouse.analytics.stg_orders"));
}
