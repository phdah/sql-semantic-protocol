use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, ComposedSemantics, ConfiguredSqlInput, LiteralType,
    RelationCatalog, RelationSchema, ResolvedComposedSemantics, SchemaColumn, SqlInput,
    ValueDomain,
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
    assert!(!semantics
        .condition_exactness()
        .residual_conditions()
        .is_empty());
}

#[test]
fn string_comparison_retains_range_but_needs_collation_evidence() {
    let semantics = analyze("SELECT name FROM t WHERE name > 'b'", &[("name", "TEXT")]);
    let domain = semantics.column_domains()[0].domain();
    assert!(
        matches!(domain, ValueDomain::Ranges(_)),
        "domain was discarded: {domain:?}"
    );
    assert_eq!(
        semantics.condition_exactness().status(),
        sql_semantic_protocol::ConditionExactnessStatus::Conditional
    );
    assert!(semantics
        .condition_exactness()
        .required_assumptions()
        .iter()
        .any(|requirement| requirement.assumption()
            == sql_semantic_protocol::ComparisonAssumption::BinaryCollation));
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
fn timestamp_domain_remains_conditional_on_timezone() {
    let semantics = analyze(
        "SELECT ts FROM t WHERE ts >= TIMESTAMP '2024-01-01 00:00:00'",
        &[("ts", "TIMESTAMPTZ")],
    );
    assert!(matches!(
        semantics.column_domains()[0].domain(),
        ValueDomain::Ranges(_)
    ));
    assert_eq!(
        semantics.condition_exactness().status(),
        sql_semantic_protocol::ConditionExactnessStatus::Conditional
    );
}

#[test]
fn float_domain_remains_conditional_on_nan_semantics() {
    let semantics = analyze("SELECT f FROM t WHERE f > 1.5", &[("f", "DOUBLE")]);
    assert!(matches!(
        semantics.column_domains()[0].domain(),
        ValueDomain::Ranges(_)
    ));
    assert!(semantics
        .condition_exactness()
        .required_assumptions()
        .iter()
        .any(|item| item.assumption() == sql_semantic_protocol::ComparisonAssumption::NoNan));
}

#[test]
fn declared_comparison_assumptions_make_string_predicate_exact() {
    let schema = RelationSchema::new(
        "t",
        vec![SchemaColumn::from_sql_type("name", "VARCHAR", "postgresql").unwrap()],
    )
    .unwrap();
    let catalog = RelationCatalog::from_schemas(&[schema]).unwrap();
    let input = SqlInput::inline("SELECT name FROM t WHERE name = 'x'");
    let dialect = PostgreSqlDialect {};
    let configured = [ConfiguredSqlInput::new(
        "sample",
        &input,
        "postgresql",
        &dialect,
    )];
    let mut bundle = analyze_configured_inputs_with_catalog(&configured, &catalog).unwrap();
    bundle.declare_comparison_assumptions(&[
        sql_semantic_protocol::ComparisonAssumption::BinaryCollation,
    ]);
    match bundle.layers()[0].composed_semantics() {
        ComposedSemantics::Resolved(semantics) => {
            assert!(semantics.condition_exactness().is_exact())
        }
        _ => panic!("unresolved query"),
    }
}

#[test]
fn timestamp_without_zone_and_offset_free_literal_is_exact() {
    let semantics = analyze(
        "SELECT ts FROM t WHERE ts >= TIMESTAMP '2024-01-01 00:00:00'",
        &[("ts", "TIMESTAMP WITHOUT TIME ZONE")],
    );
    assert!(semantics.condition_exactness().is_exact());
    assert!(matches!(
        semantics.column_domains()[0].domain(),
        ValueDomain::Ranges(_)
    ));
}

#[test]
fn timestamp_with_zone_requires_session_setting_if_literal_has_no_offset() {
    let semantics = analyze(
        "SELECT ts FROM t WHERE ts >= TIMESTAMP '2024-01-01 00:00:00'",
        &[("ts", "TIMESTAMP WITH TIME ZONE")],
    );
    assert_eq!(
        semantics.condition_exactness().status(),
        sql_semantic_protocol::ConditionExactnessStatus::Conditional
    );
}

fn timestamp_lower_bound(semantics: &ResolvedComposedSemantics) -> String {
    use sql_semantic_protocol::LiteralValue;
    let ValueDomain::Ranges(ranges) = semantics.column_domains()[0].domain() else {
        panic!("expected timestamp range");
    };
    let LiteralValue::Text(value) = ranges.ranges()[0].lower().unwrap().value().value() else {
        panic!("expected timestamp text");
    };
    value.clone()
}

#[test]
fn offset_bearing_literals_never_constrain_timezone_free_timestamp_columns() {
    use sql_semantic_protocol::{ConditionExactnessStatus, ResidualConditionReason};
    for offset in ["Z", "+02", "+0230", "+02:30", "-07:45"] {
        let sql = format!("SELECT ts FROM t WHERE ts >= TIMESTAMP '2024-01-01 12:34:56{offset}'");
        let semantics = analyze(&sql, &[("ts", "TIMESTAMP WITHOUT TIME ZONE")]);
        assert!(
            matches!(
                semantics.column_domains()[0].domain(),
                ValueDomain::Unknown(_)
            ),
            "{sql}"
        );
        assert_eq!(
            semantics.condition_exactness().status(),
            ConditionExactnessStatus::Residual,
            "{sql}"
        );
        assert!(
            semantics
                .condition_exactness()
                .residual_conditions()
                .iter()
                .any(|residual| residual.reason() == ResidualConditionReason::LiteralTypeMismatch),
            "{sql}"
        );
    }
}

#[test]
fn timestamp_domain_bounds_are_canonical_for_every_known_zone_kind() {
    use sql_semantic_protocol::ConditionExactnessStatus;
    for (schema_type, suffix, bound, status) in [
        (
            "TIMESTAMP WITHOUT TIME ZONE",
            "",
            "2024-01-01 12:34:56.12",
            ConditionExactnessStatus::Exact,
        ),
        (
            "TIMESTAMP WITH TIME ZONE",
            "+02",
            "2024-01-01 12:34:56.12+02:00",
            ConditionExactnessStatus::Exact,
        ),
        (
            "TIMESTAMP WITH TIME ZONE",
            "",
            "2024-01-01 12:34:56.12",
            ConditionExactnessStatus::Conditional,
        ),
        (
            "TIMESTAMP",
            "+02:30",
            "2024-01-01 12:34:56.12+02:30",
            ConditionExactnessStatus::Conditional,
        ),
        (
            "TIMESTAMP",
            "",
            "2024-01-01 12:34:56.12",
            ConditionExactnessStatus::Conditional,
        ),
    ] {
        let sql =
            format!("SELECT ts FROM t WHERE ts >= TIMESTAMP '2024-01-01T12:34:56.1200{suffix}'");
        let semantics = analyze(&sql, &[("ts", schema_type)]);
        assert_eq!(
            semantics.condition_exactness().status(),
            status,
            "{sql} with {schema_type}"
        );
        assert_eq!(
            timestamp_lower_bound(&semantics),
            bound,
            "{sql} with {schema_type}"
        );
    }
}

#[test]
fn timestamp_offset_normalization_is_independent_of_parser_dialect() {
    // The shared TIMESTAMP typed-string AST should have identical semantics across dialects.
    for dialect_name in ["postgresql", "duckdb"] {
        let schema = RelationSchema::new(
            "t",
            vec![
                SchemaColumn::from_sql_type("ts", "TIMESTAMP WITH TIME ZONE", dialect_name)
                    .unwrap(),
            ],
        )
        .unwrap();
        let catalog = RelationCatalog::from_schemas(&[schema]).unwrap();
        let input =
            SqlInput::inline("SELECT ts FROM t WHERE ts >= TIMESTAMP '2024-01-01 00:00:00Z'");
        let dialect = sql_semantic_protocol::dialect_from_name(dialect_name).unwrap();
        let configured = [ConfiguredSqlInput::new(
            "tz-test",
            &input,
            dialect_name,
            dialect.as_ref(),
        )];
        let bundle = analyze_configured_inputs_with_catalog(&configured, &catalog).unwrap();
        let ComposedSemantics::Resolved(semantics) = bundle.layers()[0].composed_semantics() else {
            panic!("unresolved: {dialect_name}");
        };
        assert!(semantics.condition_exactness().is_exact(), "{dialect_name}");
        assert_eq!(
            timestamp_lower_bound(semantics),
            "2024-01-01 00:00:00+00:00",
            "{dialect_name}"
        );
    }
}

#[test]
fn unsupported_timestamp_forms_do_not_claim_exactness() {
    use sql_semantic_protocol::ConditionExactnessStatus;
    let semantics = analyze(
        "SELECT ts FROM t WHERE ts >= TIMESTAMP '2024-02-30 12:00:00'",
        &[("ts", "TIMESTAMP WITHOUT TIME ZONE")],
    );
    assert!(matches!(
        semantics.column_domains()[0].domain(),
        ValueDomain::Unknown(_)
    ));
    assert_eq!(
        semantics.condition_exactness().status(),
        ConditionExactnessStatus::Residual
    );
}

#[test]
fn varchar_set_preserves_membership_after_typing() {
    let semantics = analyze(
        "SELECT name FROM t WHERE name IN ('x', 'y')",
        &[("name", "VARCHAR")],
    );
    assert!(matches!(
        semantics.column_domains()[0].domain(),
        ValueDomain::Set(_)
    ));
    assert_eq!(
        semantics.condition_exactness().status(),
        sql_semantic_protocol::ConditionExactnessStatus::Conditional
    );
}

#[test]
fn timestamp_metadata_preserves_explicit_timezone_and_rejects_invalid_type() {
    use sql_semantic_protocol::{DataType, TimestampZone};
    let tz = SchemaColumn::from_sql_type("ts", "TIMESTAMP WITH TIME ZONE", "postgresql").unwrap();
    assert_eq!(tz.timestamp_zone(), Some(TimestampZone::WithTimeZone));
    let ntz =
        SchemaColumn::from_sql_type("ts", "TIMESTAMP WITHOUT TIME ZONE", "postgresql").unwrap();
    assert_eq!(ntz.timestamp_zone(), Some(TimestampZone::WithoutTimeZone));
    let unknown = SchemaColumn::new("ts", DataType::Timestamp { precision: None }).unwrap();
    assert_eq!(unknown.timestamp_zone(), None);
    assert!(SchemaColumn::new(
        "name",
        DataType::String {
            length: None,
            fixed: false
        }
    )
    .unwrap()
    .with_timestamp_zone(TimestampZone::WithTimeZone)
    .is_err());
}

#[test]
fn single_input_library_supports_comparison_declarations() {
    use sql_semantic_protocol::{
        analyze_sql, to_json, ComparisonAssumption, ConditionExactnessStatus, ProtocolStatement,
    };
    let dialect = PostgreSqlDialect {};
    let mut protocol = analyze_sql(
        "SELECT name FROM t WHERE name = 'x'",
        "postgresql",
        &dialect,
    )
    .unwrap();
    let ProtocolStatement::Query(query) = &protocol.statements()[0] else {
        panic!("not a query")
    };
    assert_eq!(
        query.condition_exactness().status(),
        ConditionExactnessStatus::Conditional
    );
    protocol.declare_comparison_assumptions(&[ComparisonAssumption::BinaryCollation]);
    let ProtocolStatement::Query(query) = &protocol.statements()[0] else {
        panic!("not a query")
    };
    assert_eq!(
        query.condition_exactness().status(),
        ConditionExactnessStatus::Exact
    );
    let json: serde_json::Value = serde_json::from_str(&to_json(&protocol)).unwrap();
    assert_eq!(
        json["declared_comparison_assumptions"],
        serde_json::json!(["binary_collation"])
    );
}
