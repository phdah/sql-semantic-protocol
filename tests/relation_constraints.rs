use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_inputs, analyze_sql, select_targets,
    to_bundle_json, ConfiguredSqlInput, ConstraintEnforcement, ConstraintSourceKind,
    RelationCatalog, RelationConstraint, SqlInput,
};
use sqlparser::dialect::PostgreSqlDialect;

#[test]
fn sql_ddl_emits_single_and_composite_key_constraints() {
    let sql = r#"
        CREATE TABLE parent (
            tenant_id BIGINT,
            id BIGINT,
            external_id BIGINT UNIQUE,
            PRIMARY KEY (tenant_id, id)
        );
        CREATE TABLE child (
            tenant_id BIGINT,
            parent_id BIGINT,
            local_id BIGINT PRIMARY KEY,
            CONSTRAINT child_parent_fk
                FOREIGN KEY (tenant_id, parent_id)
                REFERENCES parent (tenant_id, id),
            UNIQUE (tenant_id, local_id)
        );
    "#;

    let protocol =
        analyze_sql(sql, "postgresql", &PostgreSqlDialect {}).expect("DDL should analyze");
    assert_eq!(protocol.relation_constraints().len(), 2);

    let parent = protocol
        .relation_constraints()
        .iter()
        .find(|constraints| constraints.relation() == "parent")
        .expect("parent constraints");
    assert!(parent.constraints().iter().any(|constraint| {
        matches!(
            constraint,
            RelationConstraint::PrimaryKey(key)
                if key.columns() == ["tenant_id", "id"]
        )
    }));
    assert!(parent.constraints().iter().any(|constraint| {
        matches!(
            constraint,
            RelationConstraint::UniqueKey(key)
                if key.columns() == ["external_id"]
        )
    }));

    let child = protocol
        .relation_constraints()
        .iter()
        .find(|constraints| constraints.relation() == "child")
        .expect("child constraints");
    let foreign_key = child
        .constraints()
        .iter()
        .find_map(|constraint| match constraint {
            RelationConstraint::ForeignKey(key) => Some(key),
            _ => None,
        })
        .expect("child foreign key");
    assert_eq!(foreign_key.columns(), ["tenant_id", "parent_id"]);
    assert_eq!(foreign_key.referenced_relation(), "parent");
    assert_eq!(foreign_key.referenced_columns(), ["tenant_id", "id"]);
    assert_eq!(
        foreign_key.evidence()[0].provenance().source_kind(),
        ConstraintSourceKind::SqlDdl
    );
    assert_eq!(
        foreign_key.evidence()[0].enforcement(),
        ConstraintEnforcement::Unknown
    );
}

#[test]
fn conflicting_primary_keys_are_explicit_diagnostics() {
    let sql = r#"
        CREATE TABLE conflicted (
            id BIGINT PRIMARY KEY,
            other_id BIGINT,
            PRIMARY KEY (other_id)
        )
    "#;
    let protocol =
        analyze_sql(sql, "postgresql", &PostgreSqlDialect {}).expect("DDL should analyze");
    let constraints = &protocol.relation_constraints()[0];

    assert_eq!(
        constraints
            .constraints()
            .iter()
            .filter(|constraint| matches!(constraint, RelationConstraint::PrimaryKey(_)))
            .count(),
        2
    );
    assert!(constraints
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code() == "conflicting_primary_key"));
}

#[test]
fn queryless_ddl_constraints_are_emitted_without_inventing_a_layer() {
    let bundle = analyze_inputs(
        &[SqlInput::inline(
            "CREATE TABLE orders (id BIGINT PRIMARY KEY, code TEXT UNIQUE)",
        )],
        "postgresql",
        &PostgreSqlDialect {},
    )
    .expect("DDL should analyze");

    assert!(bundle.layers().is_empty());
    assert_eq!(bundle.relation_constraints().len(), 1);

    let json: serde_json::Value =
        serde_json::from_str(&to_bundle_json(&bundle)).expect("protocol JSON should parse");
    assert_eq!(json["relation_constraints"][0]["relation"], "orders");
    assert_eq!(
        json["relation_constraints"][0]["constraints"][0]["evidence"][0]["source_kind"],
        "sql_ddl"
    );
}

#[test]
fn target_selection_preserves_relation_constraint_metadata() {
    let inputs = [
        SqlInput::inline("CREATE TABLE stage AS SELECT id FROM raw_orders WHERE id > 0"),
        SqlInput::inline("CREATE TABLE final (id BIGINT PRIMARY KEY) AS SELECT id FROM stage"),
    ];
    let bundle = analyze_inputs(&inputs, "postgresql", &PostgreSqlDialect {})
        .expect("bundle should analyze");
    let selected = select_targets(&bundle, &["final".to_string()]).expect("target should resolve");

    assert_eq!(
        selected.relation_constraints(),
        bundle.relation_constraints()
    );
}

#[test]
fn constraint_emission_is_byte_deterministic() {
    let input = SqlInput::inline(
        "CREATE TABLE child (id BIGINT PRIMARY KEY, parent_id BIGINT, UNIQUE (parent_id), FOREIGN KEY (parent_id) REFERENCES parent(id))",
    );
    let first = analyze_inputs(
        std::slice::from_ref(&input),
        "postgresql",
        &PostgreSqlDialect {},
    )
    .expect("first analysis");
    let second =
        analyze_inputs(&[input], "postgresql", &PostgreSqlDialect {}).expect("second analysis");

    assert_eq!(to_bundle_json(&first), to_bundle_json(&second));
}

#[test]
fn unique_and_not_null_do_not_imply_primary_key() {
    let protocol = analyze_sql(
        "CREATE TABLE candidate (id BIGINT NOT NULL UNIQUE)",
        "postgresql",
        &PostgreSqlDialect {},
    )
    .expect("DDL should analyze");
    let metadata = &protocol.relation_constraints()[0];

    assert!(metadata
        .constraints()
        .iter()
        .any(|constraint| matches!(constraint, RelationConstraint::UniqueKey(_))));
    assert!(!metadata
        .constraints()
        .iter()
        .any(|constraint| matches!(constraint, RelationConstraint::PrimaryKey(_))));
}

#[test]
fn self_referential_foreign_key_is_preserved() {
    let protocol = analyze_sql(
        "CREATE TABLE node (id BIGINT PRIMARY KEY, parent_id BIGINT, FOREIGN KEY (parent_id) REFERENCES node(id))",
        "postgresql",
        &PostgreSqlDialect {},
    )
    .expect("DDL should analyze");
    let metadata = &protocol.relation_constraints()[0];
    let foreign_key = metadata
        .constraints()
        .iter()
        .find_map(|constraint| match constraint {
            RelationConstraint::ForeignKey(key) => Some(key),
            _ => None,
        })
        .expect("self-reference should be preserved");

    assert_eq!(foreign_key.columns(), ["parent_id"]);
    assert_eq!(foreign_key.referenced_relation(), "node");
    assert_eq!(foreign_key.referenced_columns(), ["id"]);
}

#[test]
fn ambiguous_foreign_key_reference_fails_instead_of_guessing() {
    let input = SqlInput::inline(
        "CREATE TABLE child (parent_id BIGINT, FOREIGN KEY (parent_id) REFERENCES parent(id))",
    );
    let configured = [ConfiguredSqlInput::new(
        "child-ddl",
        &input,
        "postgresql",
        &PostgreSqlDialect {},
    )];
    let catalog =
        RelationCatalog::new(&["warehouse_a.public.parent", "warehouse_b.public.parent"])
            .expect("catalog should be valid");

    let error = analyze_configured_inputs_with_catalog(&configured, &catalog)
        .expect_err("ambiguous foreign-key target must fail");
    assert!(error.to_string().contains("parent"));
    assert!(error.to_string().contains("ambiguous"));
}

#[test]
fn source_keys_are_not_invented_on_derived_relations() {
    let bundle = analyze_inputs(
        &[
            SqlInput::inline("CREATE TABLE source_keys (id BIGINT PRIMARY KEY)"),
            SqlInput::inline("CREATE TABLE copied AS SELECT id FROM source_keys"),
        ],
        "postgresql",
        &PostgreSqlDialect {},
    )
    .expect("bundle should analyze");

    assert!(bundle
        .relation_constraints()
        .iter()
        .any(|metadata| metadata.relation() == "source_keys"));
    assert!(!bundle
        .relation_constraints()
        .iter()
        .any(|metadata| metadata.relation() == "copied"));
}
