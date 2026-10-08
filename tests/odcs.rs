#![cfg(feature = "odcs")]

use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_inputs, parse_odcs_documents, parse_odcs_yaml,
    select_targets, to_bundle_json, ConfiguredSqlInput, ConstraintEnforcement,
    ConstraintSourceKind, ConstraintValue, DataType, OdcsDocument, OdcsError, RelationCatalog,
    RelationConstraint, RelationSchema, SchemaColumn, SchemaSourceKind, SqlInput,
};
use sqlparser::dialect::PostgreSqlDialect;

const COMMERCE: &str = include_str!("fixtures/odcs/commerce.odcs.yaml");
const CUSTOMERS: &str = include_str!("fixtures/odcs/customer-contract.yaml");
const EXTERNAL_ORDERS: &str = include_str!("fixtures/odcs/orders-external.odcs.yaml");

#[test]
fn odcs_maps_typed_schemas_keys_required_enums_and_composite_relationships() {
    let catalog = RelationCatalog::new(&["analytics.customers", "analytics.orders"])
        .expect("catalog should be valid");
    let metadata =
        parse_odcs_yaml(COMMERCE, "postgresql", &catalog).expect("ODCS contract should parse");

    assert_eq!(metadata.source_schemas().len(), 2);
    let orders_schema = metadata
        .source_schemas()
        .iter()
        .find(|schema| schema.relation() == "analytics.orders")
        .expect("orders schema");
    assert_eq!(
        orders_schema.source_kind(),
        Some(SchemaSourceKind::ExternalMetadata)
    );
    assert_eq!(
        orders_schema
            .columns()
            .iter()
            .find(|column| column.name() == "id")
            .expect("id column")
            .data_type(),
        &DataType::SignedInteger { bits: Some(64) }
    );
    assert_eq!(
        orders_schema
            .columns()
            .iter()
            .find(|column| column.name() == "created_at")
            .expect("created_at column")
            .data_type(),
        &DataType::Timestamp { precision: None }
    );

    let customers = metadata
        .relation_constraints()
        .iter()
        .find(|set| set.relation() == "analytics.customers")
        .expect("customer constraints");
    assert!(customers.constraints().iter().any(|constraint| {
        matches!(
            constraint,
            RelationConstraint::PrimaryKey(key)
                if key.columns() == ["tenant_id", "customer_id"]
        )
    }));
    assert!(customers.constraints().iter().any(|constraint| {
        matches!(
            constraint,
            RelationConstraint::UniqueKey(key) if key.columns() == ["email"]
        )
    }));

    let orders = metadata
        .relation_constraints()
        .iter()
        .find(|set| set.relation() == "analytics.orders")
        .expect("order constraints");
    assert!(orders.constraints().iter().any(|constraint| {
        matches!(
            constraint,
            RelationConstraint::NotNull(not_null) if not_null.column() == "status"
        )
    }));

    let status = orders
        .constraints()
        .iter()
        .find_map(|constraint| match constraint {
            RelationConstraint::AcceptedValues(values) if values.column() == "status" => {
                Some(values)
            }
            _ => None,
        })
        .expect("status enum");
    assert_eq!(
        status.values(),
        [
            ConstraintValue::String("paid".to_string()),
            ConstraintValue::String("pending".to_string())
        ]
    );
    assert!(status.quote());

    let priority = orders
        .constraints()
        .iter()
        .find_map(|constraint| match constraint {
            RelationConstraint::AcceptedValues(values) if values.column() == "priority" => {
                Some(values)
            }
            _ => None,
        })
        .expect("priority enum");
    assert_eq!(
        priority.values(),
        [
            ConstraintValue::Integer(1),
            ConstraintValue::Integer(2),
            ConstraintValue::Integer(3)
        ]
    );
    assert!(!priority.quote());

    let foreign_key = orders
        .constraints()
        .iter()
        .find_map(|constraint| match constraint {
            RelationConstraint::ForeignKey(key) => Some(key),
            _ => None,
        })
        .expect("composite foreign key");
    assert_eq!(foreign_key.columns(), ["tenant_id", "customer_id"]);
    assert_eq!(foreign_key.referenced_relation(), "analytics.customers");
    assert_eq!(
        foreign_key.referenced_columns(),
        ["tenant_id", "customer_id"]
    );
    assert_eq!(
        foreign_key.evidence()[0].provenance().source_kind(),
        ConstraintSourceKind::ExternalMetadata
    );
    assert_eq!(
        foreign_key.evidence()[0].enforcement(),
        ConstraintEnforcement::Unknown
    );
}

#[test]
fn odcs_external_references_require_explicitly_supplied_contracts() {
    let catalog = RelationCatalog::new(&["analytics.customers", "analytics.orders"])
        .expect("catalog should be valid");
    let orders =
        OdcsDocument::new("orders-external.odcs.yaml", EXTERNAL_ORDERS).expect("orders document");

    let error = parse_odcs_documents(std::slice::from_ref(&orders), "postgresql", &catalog)
        .expect_err("external reference should require its document");
    assert!(matches!(
        error,
        OdcsError::MissingReferencedContract {
            referenced_source,
            ..
        } if referenced_source == "customer-contract.yaml"
    ));

    let customers =
        OdcsDocument::new("customer-contract.yaml", CUSTOMERS).expect("customer document");
    let metadata = parse_odcs_documents(&[orders, customers], "postgresql", &catalog)
        .expect("explicit external contract should resolve");
    let orders = metadata
        .relation_constraints()
        .iter()
        .find(|set| set.relation() == "analytics.orders")
        .expect("orders constraints");
    assert!(orders.constraints().iter().any(|constraint| {
        matches!(
            constraint,
            RelationConstraint::ForeignKey(key)
                if key.columns() == ["customer_id"]
                    && key.referenced_relation() == "analytics.customers"
                    && key.referenced_columns() == ["id"]
        )
    }));
}

#[test]
fn odcs_rejects_unsupported_versions() {
    let yaml = COMMERCE.replace("apiVersion: v3.2.0", "apiVersion: v3.1.0");
    let catalog = RelationCatalog::new(&["analytics.customers", "analytics.orders"])
        .expect("catalog should be valid");

    assert!(matches!(
        parse_odcs_yaml(&yaml, "postgresql", &catalog),
        Err(OdcsError::UnsupportedApiVersion { version, .. }) if version == "v3.1.0"
    ));
}

#[test]
fn odcs_ambiguous_relation_identity_is_an_error() {
    let yaml = r#"
apiVersion: v3.2.0
kind: DataContract
id: ambiguous-contract
name: Ambiguous
version: 1.0.0
status: active
schema:
  - id: orders_tbl
    name: orders
    properties:
      - id: order_id
        name: id
        logicalType: integer
"#;
    let catalog = RelationCatalog::new(&[
        "warehouse_a.analytics.orders",
        "warehouse_b.analytics.orders",
    ])
    .expect("catalog should be valid");

    assert!(matches!(
        parse_odcs_yaml(yaml, "postgresql", &catalog),
        Err(OdcsError::RelationResolution { .. })
    ));
}

#[test]
fn odcs_unmatched_relation_is_an_error_when_catalog_is_supplied() {
    let yaml = r#"
apiVersion: v3.2.0
kind: DataContract
id: missing-contract
name: Missing
version: 1.0.0
status: active
schema:
  - id: missing_tbl
    name: missing
    properties:
      - id: missing_id
        name: id
        logicalType: integer
"#;
    let catalog = RelationCatalog::new(&["analytics.orders"]).expect("catalog should be valid");

    assert!(matches!(
        parse_odcs_yaml(yaml, "postgresql", &catalog),
        Err(OdcsError::UnresolvedRelation { .. })
    ));
}

#[test]
fn odcs_physical_type_wins_over_logical_type_but_disagreement_is_explicit() {
    let yaml = r#"
apiVersion: v3.2.0
kind: DataContract
id: type-contract
name: Types
version: 1.0.0
status: active
schema:
  - id: orders_tbl
    name: orders
    physicalName: analytics.orders
    properties:
      - id: order_id
        name: id
        logicalType: string
        physicalType: BIGINT
"#;
    let catalog = RelationCatalog::new(&["analytics.orders"]).expect("catalog should be valid");
    let metadata =
        parse_odcs_yaml(yaml, "postgresql", &catalog).expect("ODCS contract should parse");

    assert_eq!(
        metadata.source_schemas()[0].columns()[0].data_type(),
        &DataType::SignedInteger { bits: Some(64) }
    );
    assert!(metadata
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code() == "odcs_data_type_disagreement"));
}

#[test]
fn odcs_enrichment_surfaces_conflicting_higher_authority_datatypes() {
    let existing = RelationSchema::new(
        "analytics.orders",
        vec![SchemaColumn::from_sql_type("id", "BIGINT", "postgresql")
            .expect("column should be valid")],
    )
    .expect("schema should be valid")
    .with_source_kind(SchemaSourceKind::DbtManifest);
    let catalog = RelationCatalog::from_schemas(std::slice::from_ref(&existing))
        .expect("catalog should build");
    let input = SqlInput::inline("SELECT id FROM analytics.orders");
    let configured = [ConfiguredSqlInput::new(
        "orders",
        &input,
        "postgresql",
        &PostgreSqlDialect {},
    )];
    let mut bundle = analyze_configured_inputs_with_catalog(&configured, &catalog)
        .expect("analysis should succeed");

    let yaml = r#"
apiVersion: v3.2.0
kind: DataContract
id: conflicting-contract
name: Conflicting
version: 1.0.0
status: active
schema:
  - id: orders_tbl
    name: orders
    physicalName: analytics.orders
    properties:
      - id: order_id
        name: id
        logicalType: string
        physicalType: TEXT
"#;
    let metadata = parse_odcs_yaml(yaml, "postgresql", &catalog).expect("contract should parse");

    assert!(matches!(
        metadata.enrich_bundle(&mut bundle),
        Err(OdcsError::DatatypeConflict { relation, column, .. })
            if relation == "analytics.orders" && column == "id"
    ));
    assert_eq!(
        bundle.source_schemas()[0].source_kind(),
        Some(SchemaSourceKind::DbtManifest)
    );
}

#[test]
fn odcs_constraint_conflicts_use_the_canonical_merge_policy() {
    let mut bundle = analyze_inputs(
        &[SqlInput::inline(
            "CREATE TABLE analytics.orders (id BIGINT PRIMARY KEY, alternate_id BIGINT)",
        )],
        "postgresql",
        &PostgreSqlDialect {},
    )
    .expect("SQL should analyze");
    let catalog = RelationCatalog::new(&["analytics.orders"]).expect("catalog should be valid");
    let yaml = r#"
apiVersion: v3.2.0
kind: DataContract
id: conflicting-key-contract
name: Conflicting key
version: 1.0.0
status: active
schema:
  - id: orders_tbl
    name: orders
    physicalName: analytics.orders
    properties:
      - id: order_id
        name: id
        logicalType: integer
      - id: alternate_id
        name: alternate_id
        logicalType: integer
        primaryKey: true
"#;
    let metadata = parse_odcs_yaml(yaml, "postgresql", &catalog).expect("contract should parse");
    metadata
        .enrich_bundle(&mut bundle)
        .expect("constraint evidence should merge");

    let constraints = bundle
        .relation_constraints()
        .iter()
        .find(|set| set.relation() == "analytics.orders")
        .expect("orders constraints");
    assert!(constraints
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code() == "conflicting_primary_key"));
}

#[test]
fn odcs_enrichment_survives_target_selection_and_emits_deterministically() {
    fn enriched() -> sql_semantic_protocol::AnalysisBundle {
        let mut bundle = analyze_inputs(
            &[
                SqlInput::inline(
                    "CREATE TABLE analytics.stage_orders AS SELECT id FROM analytics.orders",
                ),
                SqlInput::inline(
                    "CREATE TABLE analytics.final_orders AS SELECT id FROM analytics.stage_orders",
                ),
            ],
            "postgresql",
            &PostgreSqlDialect {},
        )
        .expect("analysis should succeed");
        let catalog = RelationCatalog::new(&["analytics.customers", "analytics.orders"])
            .expect("catalog should be valid");
        let metadata =
            parse_odcs_yaml(COMMERCE, "postgresql", &catalog).expect("ODCS should parse");
        metadata
            .enrich_bundle(&mut bundle)
            .expect("ODCS evidence should enrich");
        bundle
    }

    let first = enriched();
    let second = enriched();
    assert_eq!(to_bundle_json(&first), to_bundle_json(&second));

    let selected = select_targets(&first, &["analytics.final_orders".to_string()])
        .expect("target should resolve");
    assert!(selected
        .relation_constraints()
        .iter()
        .any(|set| set.relation() == "analytics.orders"));
    let emitted = to_bundle_json(&selected);
    assert!(emitted.contains(r#""source_kind":"external_metadata""#));
    assert!(emitted.contains(r#""source_kind":"external_metadata""#));
}
