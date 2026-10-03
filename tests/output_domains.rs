mod common;

use common::DIALECTS;
use sql_semantic_protocol::{
    analyze_inputs, analyze_sql, Expression, LiteralType, LiteralValue, Protocol,
    ProtocolStatement, QueryStatement, SetMode, SqlInput, ValueDomain,
};
use sqlparser::dialect::{dialect_from_str, GenericDialect, SnowflakeDialect};

fn first_query(protocol: &Protocol) -> &QueryStatement {
    match protocol.statements().first() {
        Some(ProtocolStatement::Query(query)) => query,
        other => panic!("expected query statement, got {other:?}"),
    }
}

type IntegerBound = Option<(String, bool)>;

fn integer_bounds(domain: &ValueDomain) -> (IntegerBound, IntegerBound) {
    let ValueDomain::Ranges(ranges) = domain else {
        panic!("expected ranges domain, got {domain:?}");
    };
    let [range] = ranges.ranges() else {
        panic!("expected one range");
    };
    let bound = |bound: Option<&sql_semantic_protocol::Bound>| {
        bound.map(|bound| match bound.value().value() {
            LiteralValue::Number(value) => (value.clone(), bound.inclusive()),
            other => panic!("expected numeric bound, got {other:?}"),
        })
    };
    (bound(range.lower()), bound(range.upper()))
}

#[test]
fn filtered_projected_column_exposes_its_outcome_interval() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "SELECT amount FROM orders WHERE amount > 10 AND amount <= 100",
        "generic",
        &dialect,
    )
    .expect("filtered projection should analyze");

    assert_eq!(
        integer_bounds(first_query(&protocol).output().columns()[0].domain()),
        (
            Some(("10".to_string(), false)),
            Some(("100".to_string(), true))
        )
    );
}

#[test]
fn case_boolean_output_has_explicit_expression_domain_and_lineage() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "SELECT CASE WHEN amount > 10 THEN TRUE ELSE FALSE END AS expensive FROM orders",
        "generic",
        &dialect,
    )
    .expect("CASE should analyze");

    let column = &first_query(&protocol).output().columns()[0];
    assert!(matches!(column.expression(), Expression::Case(_)));
    assert_eq!(column.lineage().len(), 1);
    assert_eq!(column.lineage()[0].relation(), "orders");
    assert_eq!(column.lineage()[0].column(), "amount");

    let ValueDomain::Set(domain) = column.domain() else {
        panic!("expected finite boolean domain");
    };
    assert_eq!(domain.mode(), SetMode::Include);
    assert_eq!(domain.values().len(), 2);
    assert!(domain
        .values()
        .iter()
        .all(|literal| literal.literal_type() == LiteralType::Boolean));
}

#[test]
fn boolean_derived_expression_has_boolean_domain() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "SELECT amount > 10 AS expensive FROM orders",
        "generic",
        &dialect,
    )
    .expect("boolean expression should analyze");

    let column = &first_query(&protocol).output().columns()[0];
    assert!(matches!(
        column.expression(),
        Expression::BooleanPredicate(_)
    ));
    let ValueDomain::Set(domain) = column.domain() else {
        panic!("expected boolean set domain");
    };
    assert_eq!(domain.values().len(), 2);
}

#[test]
fn row_number_qualify_refines_intrinsic_domain_without_source_domain_claim() {
    let dialect = SnowflakeDialect {};
    let protocol = analyze_sql(
        "SELECT ROW_NUMBER() OVER (PARTITION BY account_id ORDER BY created_at) AS rn FROM events QUALIFY rn <= 10",
        "snowflake",
        &dialect,
    )
    .expect("QUALIFY should analyze");

    let query = first_query(&protocol);
    assert!(query.column_domains().is_empty());
    assert_eq!(
        integer_bounds(query.output().columns()[0].domain()),
        (
            Some(("1".to_string(), true)),
            Some(("10".to_string(), true))
        )
    );
}

#[test]
fn count_and_constant_arithmetic_have_safe_intrinsic_domains() {
    let dialect = GenericDialect {};
    let count = analyze_sql("SELECT COUNT(*) AS n FROM events", "generic", &dialect)
        .expect("COUNT should analyze");
    assert_eq!(
        integer_bounds(first_query(&count).output().columns()[0].domain()),
        (Some(("0".to_string(), true)), None)
    );

    let arithmetic = analyze_sql("SELECT 40 + 2 AS answer", "generic", &dialect)
        .expect("constant arithmetic should analyze");
    let ValueDomain::Set(domain) = first_query(&arithmetic).output().columns()[0].domain() else {
        panic!("expected singleton arithmetic result");
    };
    let [value] = domain.values() else {
        panic!("expected one arithmetic value");
    };
    assert_eq!(value.value(), &LiteralValue::Number("42".to_string()));
}

#[test]
fn derived_output_domain_survives_multi_layer_composition() {
    let dialect = GenericDialect {};
    let bundle = analyze_inputs(
        &[
            SqlInput::inline(
                "CREATE TABLE stage.orders AS SELECT CASE WHEN amount > 10 THEN TRUE ELSE FALSE END AS expensive FROM raw.orders",
            ),
            SqlInput::inline(
                "CREATE TABLE mart.orders AS SELECT expensive FROM stage.orders",
            ),
        ],
        "generic",
        &dialect,
    )
    .expect("derived chain should analyze");

    let mart = bundle
        .layers()
        .iter()
        .find(|layer| {
            layer
                .produces()
                .iter()
                .any(|dataset| dataset.relation_name() == Some("mart.orders"))
        })
        .expect("mart layer");

    let sql_semantic_protocol::ComposedSemantics::Resolved(semantics) = mart.composed_semantics()
    else {
        panic!("mart composition should resolve");
    };
    assert!(matches!(
        semantics.output().columns()[0].domain(),
        ValueDomain::Set(_)
    ));
}

#[test]
fn shared_case_semantics_are_consistent_across_exposed_dialects() {
    let sql = "SELECT CASE WHEN amount > 10 THEN TRUE ELSE FALSE END AS expensive FROM orders";

    for dialect_name in DIALECTS {
        let dialect =
            dialect_from_str(dialect_name).expect("documented dialect should be recognized");
        let protocol = analyze_sql(sql, dialect_name, dialect.as_ref())
            .unwrap_or_else(|error| panic!("dialect {dialect_name} failed CASE syntax: {error}"));
        assert!(matches!(
            first_query(&protocol).output().columns()[0].expression(),
            Expression::Case(_)
        ));
    }
}
