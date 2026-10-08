use serde_json::{json, Value};
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_artifacts, parse_dbt_catalog,
    parse_dbt_manifest, ComposedSemantics, ConfiguredSqlInput, RelationCatalog, RelationSchema,
    SchemaColumn, SchemaSourceKind, SqlInput,
};
use sqlparser::dialect::PostgreSqlDialect;

fn analyzed(sql: &str, schemas: &[RelationSchema]) -> sql_semantic_protocol::AnalysisBundle {
    let catalog = RelationCatalog::from_schemas(schemas).expect("schema catalog");
    let input = SqlInput::inline(sql);
    let dialect = PostgreSqlDialect {};
    analyze_configured_inputs_with_catalog(
        &[ConfiguredSqlInput::new(
            "schema-check",
            &input,
            "postgresql",
            &dialect,
        )],
        &catalog,
    )
    .expect("analysis")
}

fn schema(relation: &str, columns: &[(&str, &str)]) -> RelationSchema {
    RelationSchema::new(
        relation,
        columns
            .iter()
            .map(|(name, ty)| SchemaColumn::from_sql_type(*name, ty, "postgresql").expect("type"))
            .collect(),
    )
    .expect("schema")
}

#[test]
fn missing_select_and_predicate_columns_block_composed_exactness_for_each_schema_source() {
    let schema = schema("t", &[("id", "INTEGER")]);
    let cases = [
        schema.clone(),
        schema
            .clone()
            .with_source_kind(SchemaSourceKind::DbtCatalog),
        schema
            .clone()
            .with_source_kind(SchemaSourceKind::DbtManifest),
        schema.with_source_kind(SchemaSourceKind::ExternalMetadata),
    ];
    for schema in cases {
        for sql in ["SELECT ghost FROM t", "SELECT id FROM t WHERE ghost = 1"] {
            let bundle = analyzed(sql, &[schema.clone()]);
            let query = match &bundle.inputs()[0].statements()[0] {
                sql_semantic_protocol::ProtocolStatement::Query(query) => query,
                other => panic!("expected query: {other:?}"),
            };
            assert!(
                query
                    .diagnostics()
                    .iter()
                    .any(|item| item.code() == "unknown_schema_column"),
                "{sql} schema kind {:?}",
                schema.source_kind()
            );
            assert!(!query.condition_exactness().is_exact());
            match bundle.layers()[0].composed_semantics() {
                ComposedSemantics::Resolved(composed) => {
                    assert!(!composed.condition_exactness().is_exact());
                }
                ComposedSemantics::Unresolved(_) => {}
                other => panic!("unexpected composed status: {other:?}"),
            }
        }
    }
}

#[test]
fn existing_columns_do_not_create_reference_diagnostics() {
    let bundle = analyzed(
        "SELECT id FROM t WHERE id > 1",
        &[schema("t", &[("id", "INTEGER")])],
    );
    let query = match &bundle.inputs()[0].statements()[0] {
        sql_semantic_protocol::ProtocolStatement::Query(query) => query,
        other => panic!("expected query: {other:?}"),
    };
    assert!(!query
        .diagnostics()
        .iter()
        .any(|item| item.code() == "unknown_schema_column"));
}

#[test]
fn nonexistent_key_column_and_incompatible_accepted_values_are_rejected() {
    let bundle = analyzed(
        "CREATE TABLE t (id INTEGER, UNIQUE (ghost), CONSTRAINT allowed CHECK (id IN ('bad')))",
        &[schema("t", &[("id", "INTEGER")])],
    );
    let constraints = bundle
        .relation_constraints()
        .first()
        .expect("constraint set");
    assert!(constraints.constraints().is_empty());
    assert!(constraints
        .diagnostics()
        .iter()
        .any(|d| d.code() == "invalid_constraint_column"));
    assert!(constraints
        .diagnostics()
        .iter()
        .any(|d| d.code() == "incompatible_accepted_value"));
}

#[test]
fn foreign_key_target_column_is_checked_when_target_schema_exists() {
    let bundle = analyzed(
        "CREATE TABLE child (id INTEGER, pid INTEGER, FOREIGN KEY (pid) REFERENCES parent (ghost))",
        &[
            schema("child", &[("id", "INTEGER"), ("pid", "INTEGER")]),
            schema("parent", &[("id", "INTEGER")]),
        ],
    );
    let constraints = bundle
        .relation_constraints()
        .iter()
        .find(|set| set.relation() == "child")
        .expect("child constraints");
    assert!(constraints.constraints().is_empty());
    assert!(constraints
        .diagnostics()
        .iter()
        .any(|d| d.code() == "invalid_constraint_column"));
}

fn manifest_with_test(test: Value) -> sql_semantic_protocol::DbtManifest {
    let mut manifest: Value = serde_json::from_str(include_str!("fixtures/dbt/manifest-v12.json"))
        .expect("manifest fixture");
    manifest["nodes"]
        .as_object_mut()
        .expect("nodes")
        .insert("test.demo.reference".to_owned(), test);
    parse_dbt_manifest(&serde_json::to_string(&manifest).expect("json")).expect("manifest parse")
}

#[test]
fn dbt_relationship_disagreement_does_not_emit_a_foreign_key() {
    let manifest = manifest_with_test(json!({
        "unique_id": "test.demo.reference",
        "resource_type": "test",
        "relation_name": null,
        "attached_node": "model.demo.stg_orders",
        "column_name": "id",
        "test_metadata": {
            "name": "relationships",
            "kwargs": {"column_name": "id", "field": "id", "to": "warehouse.analytics.final_orders"}
        },
        "depends_on": {"nodes": ["model.demo.stg_orders", "source.demo.orders"]}
    }));
    let set = manifest
        .relation_constraints()
        .iter()
        .find(|set| set.relation() == "warehouse.analytics.stg_orders")
        .expect("relation");
    assert!(set.constraints().is_empty());
    assert!(set
        .diagnostics()
        .iter()
        .any(|d| d.code() == "inconsistent_relationship_target"));
}

#[test]
fn dbt_accepted_values_are_validated_against_catalog_datatypes() {
    let manifest = manifest_with_test(json!({
        "unique_id": "test.demo.reference",
        "resource_type": "test",
        "relation_name": null,
        "attached_node": "source.demo.orders",
        "column_name": "id",
        "test_metadata": {
            "name": "accepted_values",
            "kwargs": {"column_name": "id", "values": ["invalid"], "quote": true}
        },
        "depends_on": {"nodes": ["source.demo.orders"]}
    }));
    let catalog = parse_dbt_catalog(include_str!("fixtures/dbt/catalog-v1.json")).expect("catalog");
    let dialect = PostgreSqlDialect {};
    let bundle =
        analyze_dbt_artifacts(&manifest, &catalog, "postgresql", &dialect).expect("dbt analysis");
    let set = bundle
        .relation_constraints()
        .iter()
        .find(|set| set.relation() == "warehouse.raw.orders")
        .expect("source metadata");
    assert!(set.constraints().is_empty());
    assert!(set
        .diagnostics()
        .iter()
        .any(|d| d.code() == "incompatible_accepted_value"));
}
