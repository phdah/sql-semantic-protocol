use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, ComposedSemantics, ConfiguredSqlInput, LiteralType,
    RelationCatalog, RelationSchema, ResolvedComposedSemantics, SchemaColumn, SqlInput, ValueDomain,
};
use sqlparser::dialect::PostgreSqlDialect;

fn analyze(sql: &str, columns: &[(&str, &str)]) -> ResolvedComposedSemantics {
    let schema = RelationSchema::new(
        "t",
        columns
            .iter()
            .map(|(name, data_type)| {
                SchemaColumn::from_sql_type(*name, data_type, "postgresql")
                    .expect("schema datatype should parse")
            })
            .collect(),
    )
    .expect("schema should be valid");
    let catalog = RelationCatalog::from_schemas(&[schema]).expect("catalog should be valid");
    let input = SqlInput::inline(sql);
    let dialect = PostgreSqlDialect {};
    let configured = [ConfiguredSqlInput::new(
        "typed-domain",
        &input,
        "postgresql",
        &dialect,
    )];
    let bundle = analyze_configured_inputs_with_catalog(&configured, &catalog)
        .expect("analysis should succeed");
    match bundle.layers()[0].composed_semantics() {
        ComposedSemantics::Resolved(semantics) => semantics.clone(),
        other => panic!("expected resolved semantics, got {other:?}"),
    }
}

#[test]
fn integer_domain_keeps_exact_in_range_literal() {
    let semantics = analyze("SELECT a FROM t WHERE a >= 5", &[("a", "INTEGER")]);
    let domain = semantics.column_domains()[0].domain();
    let ValueDomain::Ranges(ranges) = domain else {
        panic!("expected integer range, got {domain:?}");
    };
    assert_eq!(
        ranges.ranges()[0]
            .lower()
            .expect("lower bound")
            .value()
            .literal_type(),
        LiteralType::Integer
    );
}

#[test]
fn decimal_domain_normalizes_integer_literal_to_decimal() {
    let semantics = analyze("SELECT a FROM t WHERE a = 5", &[("a", "DECIMAL(10,2)")]);
    let domain = semantics.column_domains()[0].domain();
    let ValueDomain::Set(set) = domain else {
        panic!("expected decimal set, got {domain:?}");
    };
    assert_eq!(set.values()[0].literal_type(), LiteralType::Decimal);
}

#[test]
fn decimal_literal_against_integer_is_unknown() {
    let semantics = analyze("SELECT a FROM t WHERE a = 1.5", &[("a", "INTEGER")]);
    let domain = semantics.column_domains()[0].domain();
    assert!(
        matches!(domain, ValueDomain::Unknown(_)),
        "lossy decimal-to-integer coercion must not be exact: {domain:?}"
    );
    assert!(!semantics.condition_exactness().residual_conditions().is_empty());
}

#[test]
fn string_comparison_is_unknown_without_collation_evidence() {
    let semantics = analyze("SELECT name FROM t WHERE name > 'b'", &[("name", "TEXT")]);
    let domain = semantics.column_domains()[0].domain();
    assert!(
        matches!(domain, ValueDomain::Unknown(_)),
        "string ordering must be residual without collation evidence: {domain:?}"
    );
}

#[test]
fn string_to_date_coercion_is_unknown() {
    let semantics = analyze("SELECT d FROM t WHERE d > '2024-01-01'", &[("d", "DATE")]);
    let domain = semantics.column_domains()[0].domain();
    assert!(
        matches!(domain, ValueDomain::Unknown(_)),
        "implicit string-to-date coercion must not be exact: {domain:?}"
    );
}

#[test]
fn typed_date_literal_remains_exact() {
    let semantics = analyze(
        "SELECT d FROM t WHERE d >= DATE '2024-01-01'",
        &[("d", "DATE")],
    );
    let domain = semantics.column_domains()[0].domain();
    let ValueDomain::Ranges(ranges) = domain else {
        panic!("expected date range, got {domain:?}");
    };
    assert_eq!(
        ranges.ranges()[0]
            .lower()
            .expect("lower bound")
            .value()
            .literal_type(),
        LiteralType::Date
    );
}

#[test]
fn timestamp_domain_is_residual_until_timezone_semantics_are_represented() {
    let semantics = analyze(
        "SELECT ts FROM t WHERE ts >= TIMESTAMP '2024-01-01 00:00:00'",
        &[("ts", "TIMESTAMPTZ")],
    );
    assert!(matches!(
        semantics.column_domains()[0].domain(),
        ValueDomain::Unknown(_)
    ));
}
