mod common;

use common::DIALECTS;
use sql_semantic_protocol::{
    analyze_inputs, analyze_sql, CaseSourceDomainAlternative, CaseSourceDomains, Expression,
    LiteralType, LiteralValue, Protocol, ProtocolStatement, QueryStatement, SetMode, SqlInput,
    ValueDomain,
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

fn reachable_case_alternatives(domains: &CaseSourceDomains) -> &[CaseSourceDomainAlternative] {
    match domains {
        CaseSourceDomains::Reachable { alternatives } => alternatives,
        other => panic!("expected reachable CASE source domains, got {other:?}"),
    }
}

#[test]
fn searched_case_branch_domains_account_for_prior_matches_and_else_nulls() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "SELECT CASE
            WHEN amount > 100 THEN 'high'
            WHEN amount > 50 THEN 'medium'
            ELSE 'low'
         END AS bucket
         FROM orders",
        "generic",
        &dialect,
    )
    .expect("searched CASE should analyze");

    let Expression::Case(case_expression) =
        first_query(&protocol).output().columns()[0].expression()
    else {
        panic!("expected CASE expression");
    };

    let first = reachable_case_alternatives(case_expression.branches()[0].source_domains());
    assert_eq!(first.len(), 1);
    let [first_domain] = first[0].column_domains() else {
        panic!("expected one source domain for first branch");
    };
    assert_eq!(first_domain.column().relation(), Some("orders"));
    assert_eq!(first_domain.column().name(), "amount");
    assert_eq!(
        integer_bounds(first_domain.domain()),
        (Some(("100".to_string(), false)), None)
    );

    let second = reachable_case_alternatives(case_expression.branches()[1].source_domains());
    assert_eq!(second.len(), 1);
    let [second_domain] = second[0].column_domains() else {
        panic!("expected one source domain for second branch");
    };
    assert_eq!(
        integer_bounds(second_domain.domain()),
        (
            Some(("50".to_string(), false)),
            Some(("100".to_string(), true))
        )
    );

    let else_alternatives = reachable_case_alternatives(case_expression.else_source_domains());
    assert_eq!(else_alternatives.len(), 2);
    assert!(else_alternatives.iter().any(|alternative| {
        let [domain] = alternative.column_domains() else {
            return false;
        };
        let ValueDomain::Ranges(_) = domain.domain() else {
            return false;
        };
        matches!(
            integer_bounds(domain.domain()),
            (None, Some((ref value, true))) if value == "50"
        )
    }));
    assert!(else_alternatives.iter().any(|alternative| {
        let [domain] = alternative.column_domains() else {
            return false;
        };
        matches!(
            domain.domain(),
            ValueDomain::Set(set)
                if set.mode() == SetMode::Include
                    && matches!(
                        set.values(),
                        [literal] if literal.value() == &LiteralValue::Null
                    )
        )
    }));
}

#[test]
fn simple_case_marks_overlapping_branch_unreachable() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "SELECT CASE status
            WHEN 'new' THEN 1
            WHEN 'new' THEN 2
            ELSE 3
         END AS rank
         FROM orders",
        "generic",
        &dialect,
    )
    .expect("simple CASE should analyze");

    let Expression::Case(case_expression) =
        first_query(&protocol).output().columns()[0].expression()
    else {
        panic!("expected CASE expression");
    };

    assert!(matches!(
        case_expression.branches()[1].source_domains(),
        CaseSourceDomains::Unreachable
    ));

    let alternatives = reachable_case_alternatives(case_expression.else_source_domains());
    assert_eq!(alternatives.len(), 1);
    let [domain] = alternatives[0].column_domains() else {
        panic!("expected one ELSE source domain");
    };
    let ValueDomain::Set(set) = domain.domain() else {
        panic!("expected exclusion set for ELSE");
    };
    assert_eq!(set.mode(), SetMode::Exclude);
    assert!(matches!(
        set.values(),
        [literal] if literal.value() == &LiteralValue::Text("new".to_string())
    ));
}

#[test]
fn case_branch_domains_preserve_unknown_reason_for_non_derivable_condition() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "SELECT CASE WHEN ABS(amount) > 10 THEN 'large' ELSE 'small' END AS bucket FROM orders",
        "generic",
        &dialect,
    )
    .expect("CASE with function condition should analyze");

    let Expression::Case(case_expression) =
        first_query(&protocol).output().columns()[0].expression()
    else {
        panic!("expected CASE expression");
    };

    let CaseSourceDomains::Unknown(reason) = case_expression.branches()[0].source_domains() else {
        panic!("expected unknown branch source domains");
    };
    assert!(reason.reason().contains("one source column"));
    assert!(matches!(
        case_expression.else_source_domains(),
        CaseSourceDomains::Unknown(_)
    ));
}

#[test]
fn case_branch_domains_resolve_through_derived_table_lineage() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "SELECT CASE WHEN d.value > 10 THEN 'high' ELSE 'low' END AS bucket
         FROM (
            SELECT amount AS value
            FROM raw.orders
         ) AS d",
        "generic",
        &dialect,
    )
    .expect("derived-table CASE should analyze");

    let Expression::Case(case_expression) =
        first_query(&protocol).output().columns()[0].expression()
    else {
        panic!("expected CASE expression");
    };
    let alternatives = reachable_case_alternatives(case_expression.branches()[0].source_domains());
    let [domain] = alternatives[0].column_domains() else {
        panic!("expected one physical source domain");
    };
    assert_eq!(domain.column().relation(), Some("raw.orders"));
    assert_eq!(domain.column().name(), "amount");
}

#[test]
fn case_branch_domains_resolve_through_cte_lineage() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "WITH base AS (
            SELECT amount AS value
            FROM raw.orders
         )
         SELECT CASE WHEN value > 10 THEN 'high' ELSE 'low' END AS bucket
         FROM base",
        "generic",
        &dialect,
    )
    .expect("CTE CASE should analyze");

    let Expression::Case(case_expression) =
        first_query(&protocol).output().columns()[0].expression()
    else {
        panic!("expected CASE expression");
    };
    let alternatives = reachable_case_alternatives(case_expression.branches()[0].source_domains());
    let [domain] = alternatives[0].column_domains() else {
        panic!("expected one physical source domain");
    };
    assert_eq!(domain.column().relation(), Some("raw.orders"));
    assert_eq!(domain.column().name(), "amount");
}

#[test]
fn computed_outputs_do_not_inherit_source_predicate_domains() {
    let dialect = GenericDialect {};

    let arithmetic = analyze_sql(
        "SELECT amount + 1 AS adjusted FROM orders WHERE amount = 100",
        "generic",
        &dialect,
    )
    .expect("arithmetic output should analyze");
    assert!(matches!(
        first_query(&arithmetic).output().columns()[0].domain(),
        ValueDomain::Unknown(_)
    ));

    let function = analyze_sql(
        "SELECT UPPER(name) AS normalized FROM users WHERE name = 'x'",
        "generic",
        &dialect,
    )
    .expect("function output should analyze");
    assert!(matches!(
        first_query(&function).output().columns()[0].domain(),
        ValueDomain::Unknown(_)
    ));

    let aggregate = analyze_sql(
        "SELECT SUM(amount) AS total FROM orders WHERE amount > 100",
        "generic",
        &dialect,
    )
    .expect("aggregate output should analyze");
    assert!(matches!(
        first_query(&aggregate).output().columns()[0].domain(),
        ValueDomain::Unknown(_)
    ));

    let window = analyze_sql(
        "SELECT ROW_NUMBER() OVER (ORDER BY amount) AS rn FROM orders WHERE amount > 10",
        "generic",
        &dialect,
    )
    .expect("window output should analyze");
    assert_eq!(
        integer_bounds(first_query(&window).output().columns()[0].domain()),
        (Some(("1".to_string(), true)), None)
    );
}

#[test]
fn filtered_case_output_keeps_its_expression_domain() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "SELECT CASE WHEN amount >= 100 THEN 'high' ELSE 'standard' END AS bucket
         FROM orders
         WHERE amount = 100",
        "generic",
        &dialect,
    )
    .expect("filtered CASE should analyze");

    let ValueDomain::Set(domain) = first_query(&protocol).output().columns()[0].domain() else {
        panic!("expected CASE output set");
    };
    assert_eq!(domain.mode(), SetMode::Include);
    assert_eq!(domain.values().len(), 2);
    assert!(domain
        .values()
        .iter()
        .any(|value| value.value() == &LiteralValue::Text("high".to_string())));
    assert!(domain
        .values()
        .iter()
        .any(|value| value.value() == &LiteralValue::Text("standard".to_string())));
}

#[test]
fn filtered_case_domain_survives_composition_without_becoming_empty() {
    let dialect = GenericDialect {};
    let bundle = analyze_inputs(
        &[
            SqlInput::inline(
                "CREATE TABLE stage.orders AS
                 SELECT CASE WHEN amount >= 100 THEN 'high' ELSE 'standard' END AS bucket
                 FROM raw.orders
                 WHERE amount = 100",
            ),
            SqlInput::inline("CREATE TABLE mart.orders AS SELECT bucket FROM stage.orders"),
        ],
        "generic",
        &dialect,
    )
    .expect("filtered CASE chain should analyze");

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

    let ValueDomain::Set(domain) = semantics.output().columns()[0].domain() else {
        panic!("expected composed CASE output set");
    };
    assert_eq!(domain.values().len(), 2);
}

#[test]
fn computed_domain_guard_is_consistent_across_exposed_dialects() {
    let sql = "SELECT amount + 1 AS adjusted FROM orders WHERE amount = 100";

    for dialect_name in DIALECTS {
        let dialect =
            dialect_from_str(dialect_name).expect("documented dialect should be recognized");
        let protocol = analyze_sql(sql, dialect_name, dialect.as_ref()).unwrap_or_else(|error| {
            panic!("dialect {dialect_name} failed arithmetic syntax: {error}")
        });
        assert!(
            matches!(
                first_query(&protocol).output().columns()[0].domain(),
                ValueDomain::Unknown(_)
            ),
            "dialect {dialect_name}"
        );
    }
}
