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

fn assert_unknown_domain(sql: &str) {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(sql, "generic", &dialect).unwrap();
    assert!(
        matches!(
            first_query(&protocol).output().columns()[0].domain(),
            ValueDomain::Unknown(_)
        ),
        "{sql}: implicit arithmetic must not claim a portable exact value"
    );
}

#[test]
fn implicit_integer_division_is_not_portable_exact() {
    assert_unknown_domain("SELECT 3 / 2 AS value");
}

#[test]
fn zero_divisor_division_is_not_portable_exact() {
    assert_unknown_domain("SELECT 9 / 0 AS value");
}

#[test]
fn zero_divisor_remainder_is_not_portable_exact() {
    assert_unknown_domain("SELECT 0 % 0 AS value");
}

#[test]
fn implicit_integer_overflow_is_not_portable_exact() {
    assert_unknown_domain("SELECT 2147483647 + 1 AS value");
    assert_unknown_domain("SELECT 9223372036854775807 + 1 AS value");
}

#[test]
fn decimal_literal_sum_is_not_portable_exact() {
    assert_unknown_domain("SELECT 1.2 + 0.3 AS value");
}

#[test]
fn null_arithmetic_is_singleton_null() {
    let dialect = GenericDialect {};
    for sql in [
        "SELECT NULL + 1 AS value",
        "SELECT 1 - NULL AS value",
        "SELECT NULL * 3 AS value",
    ] {
        let protocol = analyze_sql(sql, "generic", &dialect).unwrap();
        let ValueDomain::Set(values) = first_query(&protocol).output().columns()[0].domain() else {
            panic!("{sql}: total NULL arithmetic must yield singleton SQL NULL");
        };
        assert_eq!(values.values()[0].value(), &LiteralValue::Null);
    }
}

#[test]
fn explicit_bigint_widening_carries_exact_result() {
    let dialect = GenericDialect {};
    let widened = analyze_sql(
        "SELECT CAST(2147483647 AS BIGINT) + 1 AS value",
        "generic",
        &dialect,
    )
    .unwrap();
    let ValueDomain::Set(values) = first_query(&widened).output().columns()[0].domain() else {
        panic!("explicit BIGINT arithmetic should carry an exact width");
    };
    assert_eq!(
        values.values()[0].value(),
        &LiteralValue::Number("2147483648".into())
    );
}

#[test]
fn portable_positive_remainder_is_exact() {
    let dialect = GenericDialect {};
    let positive = analyze_sql("SELECT 7 % 3 AS value", "generic", &dialect).unwrap();
    let ValueDomain::Set(positive_values) = first_query(&positive).output().columns()[0].domain()
    else {
        panic!("positive integer remainder is portable and exact");
    };
    assert_eq!(
        positive_values.values()[0].value(),
        &LiteralValue::Number("1".into())
    );
}

#[test]
fn modulo_one_is_exact_zero() {
    let dialect = GenericDialect {};
    let safe = analyze_sql("SELECT 7 % 1 AS value", "generic", &dialect).unwrap();
    let ValueDomain::Set(values) = first_query(&safe).output().columns()[0].domain() else {
        panic!("modulo-one has a safe integer value domain");
    };
    assert_eq!(
        values.values()[0].value(),
        &LiteralValue::Number("0".into())
    );
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
    assert_eq!(domain.values().len(), 1);
    assert!(domain
        .values()
        .iter()
        .any(|value| value.value() == &LiteralValue::Text("high".to_string())));
    assert!(!domain
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
    assert_eq!(domain.values().len(), 1);
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

#[test]
fn case_semantics_survive_plain_copy_through_ctes_and_derived_tables() {
    let dialect = GenericDialect {};

    for sql in [
        "WITH classified AS (
            SELECT CASE WHEN amount > 10 THEN 'high' ELSE 'low' END AS bucket
            FROM raw.orders
         )
         SELECT bucket FROM classified",
        "SELECT rank
         FROM (
            SELECT CASE status WHEN 'new' THEN 1 ELSE 0 END AS rank
            FROM raw.orders
         ) AS classified",
    ] {
        let protocol = analyze_sql(sql, "generic", &dialect)
            .unwrap_or_else(|error| panic!("local CASE copy should analyze: {error}"));
        let Expression::Case(case_expression) =
            first_query(&protocol).output().columns()[0].expression()
        else {
            panic!("plain local CASE copy should preserve CASE semantics: {sql}");
        };

        let alternatives =
            reachable_case_alternatives(case_expression.branches()[0].source_domains());
        assert!(!alternatives.is_empty(), "{sql}");
        assert!(
            alternatives
                .iter()
                .flat_map(|alternative| alternative.column_domains())
                .all(|domain| domain.column().relation() == Some("raw.orders")),
            "{sql}"
        );
        assert!(
            matches!(
                case_expression.else_source_domains(),
                CaseSourceDomains::Reachable { .. }
            ),
            "{sql}"
        );
    }
}

#[test]
fn case_over_aggregate_stays_unknown_through_cte_copy() {
    let dialect = GenericDialect {};
    let protocol = analyze_sql(
        "WITH classified AS (
            SELECT CASE WHEN SUM(amount) > 100 THEN 'high' ELSE 'low' END AS bucket
            FROM raw.orders
         )
         SELECT bucket FROM classified",
        "generic",
        &dialect,
    )
    .expect("aggregate CASE CTE should analyze");

    let Expression::Case(case_expression) =
        first_query(&protocol).output().columns()[0].expression()
    else {
        panic!("plain local CASE copy should preserve CASE semantics");
    };

    assert!(matches!(
        case_expression.branches()[0].source_domains(),
        CaseSourceDomains::Unknown(_)
    ));
    assert!(matches!(
        case_expression.else_source_domains(),
        CaseSourceDomains::Unknown(_)
    ));
}

#[test]
fn local_case_copy_is_consistent_across_exposed_dialects() {
    let sql = "WITH classified AS (
        SELECT CASE WHEN amount > 10 THEN 'high' ELSE 'low' END AS bucket
        FROM orders
    )
    SELECT bucket FROM classified";

    for dialect_name in DIALECTS {
        let dialect =
            dialect_from_str(dialect_name).expect("documented dialect should be recognized");
        let protocol = analyze_sql(sql, dialect_name, dialect.as_ref()).unwrap_or_else(|error| {
            panic!("dialect {dialect_name} failed local CASE copy: {error}")
        });
        assert!(
            matches!(
                first_query(&protocol).output().columns()[0].expression(),
                Expression::Case(_)
            ),
            "dialect {dialect_name}"
        );
    }
}

#[test]
fn portable_decimal_literal_arithmetic_has_exact_domains() {
    let dialect = GenericDialect {};
    for (sql, expected) in [
        ("SELECT 1.25 + 0.5 AS value", "1.75"),
        ("SELECT 1.25 - 0.5 AS value", "0.75"),
        ("SELECT 1.5 * 2.0 AS value", "3.0"),
        ("SELECT 1.5 / 0.5 AS value", "3.0"),
        ("SELECT 1.25 % 0.5 AS value", "0.25"),
        ("SELECT -1.25 + 0.5 AS value", "-0.75"),
    ] {
        let protocol = analyze_sql(sql, "generic", &dialect).unwrap();
        let ValueDomain::Set(values) = first_query(&protocol).output().columns()[0].domain() else {
            panic!("{sql}: expected exact finite decimal output");
        };
        assert_eq!(
            values.values()[0].value(),
            &LiteralValue::Number(expected.to_string()),
            "{sql}",
        );
    }

    for sql in [
        "SELECT 0.1 + 0.2 AS value",
        "SELECT 1.5 / 0.0 AS value",
        "SELECT 1.25 / 3.0 AS value",
        "SELECT -1.25 % 0.5 AS value",
    ] {
        let protocol = analyze_sql(sql, "generic", &dialect).unwrap();
        assert!(
            matches!(
                first_query(&protocol).output().columns()[0].domain(),
                ValueDomain::Unknown(_)
            ),
            "{sql}: nonportable decimal evaluation must stay unknown"
        );
    }
}

#[test]
fn finite_signed_casts_preserve_total_values_and_failure_modes() {
    let dialect = GenericDialect {};
    for (sql, expected) in [
        ("SELECT CAST(123 AS SMALLINT) AS value", "123"),
        ("SELECT CAST('42' AS INTEGER) AS value", "42"),
        ("SELECT CAST(25.0 AS BIGINT) AS value", "25"),
    ] {
        let protocol = analyze_sql(sql, "generic", &dialect).unwrap();
        let ValueDomain::Set(values) = first_query(&protocol).output().columns()[0].domain() else {
            panic!("{sql}: expected finite total cast");
        };
        assert_eq!(
            values.values()[0].value(),
            &LiteralValue::Number(expected.into())
        );
    }
    let protocol = analyze_sql(
        "SELECT TRY_CAST(40000 AS SMALLINT) AS value",
        "generic",
        &dialect,
    )
    .unwrap();
    let ValueDomain::Set(values) = first_query(&protocol).output().columns()[0].domain() else {
        panic!("TRY_CAST of an out-of-range exact integer should yield NULL");
    };
    assert_eq!(values.values()[0].value(), &LiteralValue::Null);
    for sql in [
        "SELECT CAST(40000 AS SMALLINT) AS value",
        "SELECT CAST('1.5' AS INTEGER) AS value",
    ] {
        let protocol = analyze_sql(sql, "generic", &dialect).unwrap();
        assert!(
            matches!(
                first_query(&protocol).output().columns()[0].domain(),
                ValueDomain::Unknown(_)
            ),
            "{sql}: unproved or exceptional cast must not be exact"
        );
    }
}
