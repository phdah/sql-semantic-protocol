//! TASK-101: catalog-certified integer arithmetic and SQL 3VL oracles.
mod common;

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_artifacts, analyze_sql, dialect_from_name,
    parse_dbt_catalog, parse_dbt_manifest, to_json, BooleanRowConstraint, BooleanTruthCase,
    BooleanWitness, BooleanWitnessDirection, ComposedSemantics, ConfiguredSqlInput, LiteralValue,
    ProtocolStatement, RelationCatalog, RelationSchema, SchemaColumn, SetMode, SqlInput,
    ValueDomain,
};
use sqlparser::dialect::PostgreSqlDialect;

fn witness(predicate: &str) -> BooleanWitness {
    let schema = RelationSchema::new(
        "t",
        vec![
            SchemaColumn::from_sql_type("a", "INTEGER", "postgresql").unwrap(),
            SchemaColumn::from_sql_type("b", "BIGINT", "postgresql").unwrap(),
        ],
    )
    .unwrap();
    let catalog = RelationCatalog::from_schemas(&[schema]).unwrap();
    let sql = SqlInput::inline(format!("SELECT a FROM t WHERE {predicate}"));
    let dialect = PostgreSqlDialect {};
    let bundle = analyze_configured_inputs_with_catalog(
        &[ConfiguredSqlInput::new(
            "arithmetic",
            &sql,
            "postgresql",
            &dialect,
        )],
        &catalog,
    )
    .unwrap();
    let ComposedSemantics::Resolved(semantics) = bundle.layers()[0].composed_semantics() else {
        panic!("resolved arithmetic source required");
    };
    semantics.boolean_witnesses()[0].witness().clone()
}

fn exact(predicate: &str) -> BooleanWitness {
    let proof = witness(predicate);
    assert!(
        matches!(
            proof.qualifying(),
            BooleanWitnessDirection::Exact(BooleanTruthCase::True)
        ),
        "{predicate}: {:?} with condition {:?}",
        proof.qualifying(),
        proof.condition()
    );
    assert!(
        matches!(
            proof.rejected(),
            BooleanWitnessDirection::Exact(BooleanTruthCase::NotTrue)
        ),
        "{predicate}: {:?}",
        proof.rejected()
    );
    proof
}

#[test]
fn signed_multiplication_inverts_rounded_thresholds() {
    for (sql, expected_operator, expected_literal) in [
        ("CAST(a AS BIGINT) * 3 >= 11", "gte", 4),
        ("CAST(a AS BIGINT) * 3 < 11", "lt", 4),
        ("CAST(a AS BIGINT) * 3 > -11", "gt", -4),
        ("CAST(a AS BIGINT) * 3 <= -11", "lte", -4),
        ("CAST(a AS BIGINT) * -3 <= 11", "gte", -3),
        ("7 - CAST(a AS BIGINT) * 2 > 1", "lt", 3),
        ("CAST(a AS BIGINT) * 3 + 2 = 14", "eq", 4),
    ] {
        let proof = exact(sql);
        let BooleanRowConstraint::IntegerComparison {
            operator, literal, ..
        } = proof.condition()
        else {
            panic!(
                "{sql}: expected inverted integer comparison, got {:?}",
                proof.condition()
            );
        };
        assert_eq!(
            format!("{operator:?}").to_lowercase(),
            expected_operator,
            "{sql}"
        );
        assert_eq!(*literal, expected_literal, "{sql}");
    }
}

#[test]
fn nondivisible_equality_is_impossible_without_losing_null_semantics() {
    let impossible = witness("CAST(a AS BIGINT) * 3 = 7");
    assert!(matches!(
        impossible.qualifying(),
        BooleanWitnessDirection::Impossible
    ));
    assert!(matches!(
        impossible.rejected(),
        BooleanWitnessDirection::Exact(BooleanTruthCase::NotTrue)
    ));
    exact("CAST(a AS BIGINT) * 3 <> 7");
    exact("CAST(a AS BIGINT) * 0 = 0");
    exact("CAST(a AS BIGINT) % 1 = 0");
    let always = witness("CAST(a AS BIGINT) * 3 IS DISTINCT FROM 7");
    assert!(matches!(
        always.rejected(),
        BooleanWitnessDirection::Impossible
    ));
    assert!(matches!(
        always.qualifying(),
        BooleanWitnessDirection::Exact(BooleanTruthCase::True)
    ));
}

#[test]
fn zero_arithmetic_preserves_sql_nullsafe_truth() {
    let not_distinct = witness("CAST(a AS BIGINT) * 0 IS NOT DISTINCT FROM 0");
    assert!(matches!(
        not_distinct.condition(),
        BooleanRowConstraint::NullTest { negated: true, .. }
    ));
    let distinct = witness("CAST(a AS BIGINT) % 1 IS DISTINCT FROM 0");
    assert!(matches!(
        distinct.condition(),
        BooleanRowConstraint::NullTest { negated: false, .. }
    ));
    let tautology = witness("CAST(a AS BIGINT) % 1 = 0 OR a IS NULL");
    assert!(matches!(
        tautology.qualifying(),
        BooleanWitnessDirection::Exact(BooleanTruthCase::True)
    ));
    assert!(matches!(
        tautology.rejected(),
        BooleanWitnessDirection::Impossible
    ));
}

#[test]
fn width_guards_and_unsupported_operators_remain_residual() {
    for sql in [
        "a * 3 > 3",
        "b * 2 > 10",
        "CAST(a AS SMALLINT) * 3 > 0",
        "CAST(a AS BIGINT) / 2 > 0",
        "CAST(a AS BIGINT) / 0 > 0",
        "CAST(a AS BIGINT) % 2 = 0",
        "CAST(a AS DOUBLE) * 2 > 1",
    ] {
        let proof = witness(sql);
        assert!(
            matches!(proof.qualifying(), BooleanWitnessDirection::Residual { .. }),
            "{sql}: unexpected exact proof {:?}",
            proof.qualifying()
        );
        if sql.contains(" / ") || sql.contains(" % ") {
            let BooleanRowConstraint::Residual { reason } = proof.condition() else {
                panic!("{sql}: expected canonical operator-specific residual");
            };
            assert!(
                reason.contains("divide") || reason.contains("modulo"),
                "{sql}: residual must identify unproven numeric operator: {reason}"
            );
        }
    }
}

#[test]
fn total_arithmetic_null_tests_and_signed_negation_preserve_3vl() {
    for sql in [
        "CAST(a AS BIGINT) * -1 IS NULL",
        "CAST(a AS BIGINT) * 3 IS NOT NULL",
        "CAST(a AS BIGINT) * 3 + 7 >= 13 OR a IS NULL",
        "NOT (CAST(a AS BIGINT) * 3 = 9 AND a IS NOT NULL)",
        "-CAST(a AS BIGINT) < -1",
    ] {
        exact(sql);
    }
    let proof = exact("CAST(a AS BIGINT) * 3 + 2 >= 11");
    assert!(proof.condition().is_exact());
}

#[test]
fn sql_three_valued_duckdb_arithmetic_oracle() {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE t(a INTEGER);
         INSERT INTO t VALUES (NULL), (-2147483648), (-4), (0), (3), (3), (2147483647);",
    )
    .unwrap();
    for (predicate, expected) in [
        ("CAST(a AS BIGINT) * 3 >= 11", 1_i64),
        ("CAST(a AS BIGINT) * 3 < 11", 5),
        ("CAST(a AS BIGINT) * 3 = 7", 0),
        ("CAST(a AS BIGINT) * 3 <> 7", 6),
        ("CAST(a AS BIGINT) * 3 IS DISTINCT FROM 7", 7),
        ("-CAST(a AS BIGINT) > 0", 2),
        ("CAST(a AS BIGINT) * 3 IS NULL", 1),
        ("CAST(a AS BIGINT) * 3 + 2 >= 11", 3),
    ] {
        let count: i64 = db
            .query_row(
                &format!("SELECT COUNT(*) FROM t WHERE {predicate}"),
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, expected, "{predicate}");
    }
}

#[test]
fn parser_capable_dialects_preserve_the_canonical_arithmetic_boundary() {
    let sql = "SELECT a FROM t WHERE CAST(a AS BIGINT) * 3 >= 11";
    for name in common::DIALECTS {
        let dialect = dialect_from_name(name).expect("registered dialect");
        match analyze_sql(sql, name, dialect.as_ref()) {
            Ok(protocol) => {
                let Some(ProtocolStatement::Query(query)) = protocol.statements().first() else {
                    panic!("{name}: expected query");
                };
                let proof = query.boolean_witness().expect("computed witness");
                assert!(
                    matches!(proof.qualifying(), BooleanWitnessDirection::Residual { .. }),
                    "{name}: no catalog type must remain residual"
                );
                let catalog = RelationCatalog::from_schemas(&[RelationSchema::new(
                    "t",
                    vec![SchemaColumn::from_sql_type("a", "INTEGER", "postgresql").unwrap()],
                )
                .unwrap()])
                .unwrap();
                let input = SqlInput::inline(sql);
                let configured = [ConfiguredSqlInput::new(
                    "typed",
                    &input,
                    name,
                    dialect.as_ref(),
                )];
                let bundle = analyze_configured_inputs_with_catalog(&configured, &catalog)
                    .unwrap_or_else(|error| panic!("{name}: {error}"));
                let ComposedSemantics::Resolved(composed) = bundle.layers()[0].composed_semantics()
                else {
                    panic!("{name}: expected resolved typed source");
                };
                let typed = composed.boolean_witnesses()[0].witness();
                assert!(
                    matches!(
                        typed.qualifying(),
                        BooleanWitnessDirection::Exact(BooleanTruthCase::True)
                    ),
                    "{name}: {:?}",
                    typed.qualifying()
                );
                assert!(
                    matches!(
                        typed.condition(),
                        BooleanRowConstraint::IntegerComparison { literal: 4, .. }
                    ),
                    "{name}: {:?}",
                    typed.condition()
                );
            }
            Err(_) => {
                // Parser rejection is a documented boundary, not a semantic proof.
            }
        }
    }
}

#[test]
fn direct_sql_and_dbt_catalog_produce_the_same_arithmetic_witness() {
    const SQL: &str =
        "SELECT id, amount FROM warehouse.raw.orders WHERE CAST(amount AS BIGINT) * 3 > 9";
    let mut manifest: Value =
        serde_json::from_str(include_str!("fixtures/dbt/manifest-v12.json")).unwrap();
    manifest["nodes"]["model.demo.stg_orders"]["compiled_code"] = SQL.into();
    let manifest = parse_dbt_manifest(&manifest.to_string()).unwrap();
    let dbt_catalog = parse_dbt_catalog(include_str!("fixtures/dbt/catalog-v1.json")).unwrap();
    let dialect = PostgreSqlDialect {};
    let from_dbt = analyze_dbt_artifacts(&manifest, &dbt_catalog, "postgresql", &dialect).unwrap();

    let physical = RelationSchema::new(
        "warehouse.raw.orders",
        vec![
            SchemaColumn::from_sql_type("id", "BIGINT", "postgresql").unwrap(),
            SchemaColumn::from_sql_type("amount", "INTEGER", "postgresql").unwrap(),
        ],
    )
    .unwrap();
    let direct_catalog = RelationCatalog::from_schemas(&[physical]).unwrap();
    let source = SqlInput::inline(SQL);
    let from_sql = analyze_configured_inputs_with_catalog(
        &[ConfiguredSqlInput::new(
            "direct",
            &source,
            "postgresql",
            &dialect,
        )],
        &direct_catalog,
    )
    .unwrap();
    let condition = |bundle: &sql_semantic_protocol::AnalysisBundle| {
        let layer = bundle
            .layers()
            .iter()
            .find(|layer| layer.consumes().iter().any(|s| s == "warehouse.raw.orders"))
            .expect("source-consuming layer");
        let ComposedSemantics::Resolved(semantics) = layer.composed_semantics() else {
            panic!("resolved layer expected");
        };
        semantics.boolean_witnesses()[0].witness().clone()
    };
    let dbt = condition(&from_dbt);
    let direct = condition(&from_sql);
    assert_eq!(dbt.condition(), direct.condition());
    assert_eq!(dbt.qualifying(), direct.qualifying());
    assert_eq!(dbt.rejected(), direct.rejected());
    assert!(matches!(
        dbt.qualifying(),
        BooleanWitnessDirection::Exact(BooleanTruthCase::True)
    ));
}

fn projected_domain(sql: &str) -> ValueDomain {
    let schema = RelationSchema::new(
        "t",
        vec![SchemaColumn::from_sql_type("a", "INTEGER", "postgresql").unwrap()],
    )
    .unwrap();
    let catalog = RelationCatalog::from_schemas(&[schema]).unwrap();
    let dialect = PostgreSqlDialect {};
    let input = SqlInput::inline(sql);
    let bundle = analyze_configured_inputs_with_catalog(
        &[ConfiguredSqlInput::new(
            "projection",
            &input,
            "postgresql",
            &dialect,
        )],
        &catalog,
    )
    .unwrap();
    let ComposedSemantics::Resolved(semantics) = bundle.layers()[0].composed_semantics() else {
        panic!("expected composed output");
    };
    semantics.output().columns()[0].domain().clone()
}

#[test]
fn catalog_integer_projection_preserves_exact_finite_sets() {
    let domain = projected_domain("SELECT a + 2 AS v FROM t WHERE a IN (1, 3)");
    let ValueDomain::Set(values) = domain else {
        panic!("a bounded integer addition must retain finite domain: {domain:?}");
    };
    assert_eq!(values.mode(), SetMode::Include);
    let numbers = values
        .values()
        .iter()
        .map(|value| value.value().clone())
        .collect::<Vec<_>>();
    assert!(numbers.contains(&LiteralValue::Number("3".into())));
    assert!(numbers.contains(&LiteralValue::Number("5".into())));
    assert_eq!(numbers.len(), 2);
}

#[test]
fn catalog_integer_projection_retains_exact_signed_finite_values() {
    let domain =
        projected_domain("SELECT CAST(a AS SMALLINT) * -2 AS v FROM t WHERE a BETWEEN 0 AND 10");
    let ValueDomain::Set(values) = domain else {
        panic!("expected exhaustive finite arithmetic domain, got {domain:?}");
    };
    assert_eq!(values.mode(), SetMode::Include);
    assert_eq!(values.values().len(), 11);
    for source in 0..=10 {
        assert!(values
            .values()
            .iter()
            .any(|value| { value.value() == &LiteralValue::Number((-2 * source).to_string()) }));
    }
}

#[test]
fn catalog_cast_and_arithmetic_widths_fail_closed() {
    let exact = projected_domain("SELECT CAST(a AS BIGINT) + 1 AS v FROM t WHERE a = 2147483647");
    let ValueDomain::Set(values) = exact else {
        panic!("widened addition should be exact: {exact:?}");
    };
    assert_eq!(
        values.values()[0].value(),
        &LiteralValue::Number("2147483648".into())
    );

    for sql in [
        "SELECT a + 1 AS v FROM t WHERE a = 2147483647",
        "SELECT CAST(a AS SMALLINT) AS v FROM t WHERE a = 40000",
    ] {
        let domain = projected_domain(sql);
        assert!(
            matches!(domain, ValueDomain::Unknown(_)),
            "{sql}: unchecked overflow or cast failure must not be exact: {domain:?}"
        );
    }
}

#[test]
fn distinct_integer_cast_failure_contract_is_serialized() {
    let dialect = PostgreSqlDialect {};
    for (name, sql, expected) in [
        (
            "cast",
            "SELECT a FROM t WHERE CAST(a AS SMALLINT) > 0",
            "exception",
        ),
        (
            "try_cast",
            "SELECT a FROM t WHERE TRY_CAST(a AS SMALLINT) > 0",
            "null",
        ),
    ] {
        let protocol = analyze_sql(sql, "postgresql", &dialect).unwrap();
        let wire = to_json(&protocol);
        assert!(
            wire.contains(&format!(r#""cast_kind":"{name}""#)),
            "{sql}: {wire}"
        );
        assert!(
            wire.contains(&format!(r#""on_failure":"{expected}""#)),
            "{sql}: {wire}"
        );
    }
}

#[test]
fn unit_division_and_nonzero_zero_dividends_have_exhaustive_witnesses() {
    // INTEGER source evidence fits binary64 exactly even if the engine
    // promotes integer division to floating arithmetic.
    for predicate in ["CAST(a AS BIGINT) / 1 > 10", "CAST(a AS BIGINT) / -1 < -10"] {
        exact(predicate);
    }
    for predicate in [
        "0 / (CAST(a AS BIGINT) + 2147483648) = 0",
        "0 % (CAST(a AS BIGINT) + 2147483648) = 0",
    ] {
        // The divisor might equal zero for a=-2147483648, so no proof.
        assert!(
            matches!(
                witness(predicate).qualifying(),
                BooleanWitnessDirection::Residual { .. }
            ),
            "{predicate}"
        );
    }
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE t(a INTEGER); INSERT INTO t VALUES (NULL), (-3), (0), (2), (2147483647);",
    )
    .unwrap();
    for predicate in ["CAST(a AS BIGINT) / 1 > 10", "CAST(a AS BIGINT) / -1 < -10"] {
        let count: i64 = db
            .query_row(
                &format!("SELECT COUNT(*) FROM t WHERE {predicate}"),
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "{predicate}");
    }
}

#[test]
fn total_injective_projection_preserves_excluded_values() {
    let domain = projected_domain("SELECT CAST(a AS BIGINT) * 3 + 2 AS v FROM t WHERE a <> 0");
    let ValueDomain::Set(set) = domain else {
        panic!("expected excluded transformed value: {domain:?}");
    };
    assert_eq!(set.mode(), SetMode::Exclude);
    assert!(set
        .values()
        .iter()
        .any(|v| v.value() == &LiteralValue::Number("2".into())));

    for sql in [
        "SELECT a + 1 AS v FROM t WHERE a <> 0",
        "SELECT CAST(a AS BIGINT) * 0 AS v FROM t WHERE a <> 0",
    ] {
        assert!(
            matches!(projected_domain(sql), ValueDomain::Unknown(_)),
            "{sql}: overflow or non-injectivity must not exclude a value"
        );
    }
}

#[test]
fn portable_casts_agree_across_all_parser_capable_dialects() {
    for name in common::DIALECTS {
        let dialect = dialect_from_name(name).unwrap();
        let Ok(protocol) = analyze_sql(
            "SELECT CAST('42' AS BIGINT) AS converted",
            name,
            dialect.as_ref(),
        ) else {
            continue;
        };
        let Some(ProtocolStatement::Query(query)) = protocol.statements().first() else {
            panic!("{name}: expected SELECT");
        };
        let ValueDomain::Set(values) = query.output().columns()[0].domain() else {
            panic!("{name}: canonical literal cast must be exact");
        };
        assert_eq!(values.mode(), SetMode::Include, "{name}");
        assert_eq!(
            values.values()[0].value(),
            &LiteralValue::Number("42".into()),
            "{name}",
        );
    }
}

#[test]
fn duckdb_cast_arithmetic_and_exception_oracles() {
    let db = Connection::open_in_memory().unwrap();
    for (expression, expected) in [
        ("CAST('42' AS BIGINT)", "42"),
        ("1.25 + 0.5", "1.75"),
        ("1.25 - 0.5", "0.75"),
        ("1.5 * 2.0", "3.00"),
        ("1.5 / 0.5", "3.0"),
        ("1.25 % 0.5", "0.25"),
    ] {
        let actual: String = db
            .query_row(
                &format!("SELECT CAST({expression} AS VARCHAR)"),
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(
            actual.parse::<f64>().unwrap() == expected.parse::<f64>().unwrap(),
            "{expression}: expected {expected}, got {actual}",
        );
    }
    let cast_would_throw = db.execute_batch("SELECT CAST(40000 AS SMALLINT)");
    assert!(
        cast_would_throw.is_err(),
        "unsafe ordinary cast must not be called total"
    );
    let safe_failure: bool = db
        .query_row("SELECT TRY_CAST(40000 AS SMALLINT) IS NULL", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert!(safe_failure);
    let exact_negative: i64 = db
        .query_row("SELECT CAST('-32768' AS SMALLINT)", [], |row| row.get(0))
        .unwrap();
    assert_eq!(exact_negative, -32768);
}

#[test]
fn exact_integer_division_and_nonnegative_modulo_have_typed_domains() {
    let division = projected_domain("SELECT CAST(a AS BIGINT) / 3 AS v FROM t WHERE a = 9");
    let ValueDomain::Set(quotient) = division else {
        panic!("catalog-typed integral division must be exact: {division:?}");
    };
    assert_eq!(
        quotient.values()[0].value(),
        &LiteralValue::Number("3".into())
    );

    let modulo = projected_domain("SELECT a % 3 AS v FROM t WHERE a BETWEEN 0 AND 100");
    let ValueDomain::Ranges(bounds) = modulo else {
        panic!("nonnegative modulo has bounded output: {modulo:?}");
    };
    let [range] = bounds.ranges() else {
        panic!("expected one bounded remainder range");
    };
    assert_eq!(
        range.lower().unwrap().value().value(),
        &LiteralValue::Number("0".into())
    );
    assert_eq!(
        range.upper().unwrap().value().value(),
        &LiteralValue::Number("2".into())
    );
    assert!(range.lower().unwrap().inclusive());
    assert!(range.upper().unwrap().inclusive());

    let finite = projected_domain("SELECT a % 3 AS v FROM t WHERE a IN (7, 8)");
    let ValueDomain::Set(values) = finite else {
        panic!("finite remainder must preserve values: {finite:?}");
    };
    assert_eq!(values.values().len(), 2);
    assert!(values
        .values()
        .iter()
        .any(|v| v.value() == &LiteralValue::Number("1".into())));
    assert!(values
        .values()
        .iter()
        .any(|v| v.value() == &LiteralValue::Number("2".into())));

    for sql in [
        "SELECT a / 3 AS v FROM t WHERE a = 10",
        "SELECT a / 0 AS v FROM t WHERE a = 9",
        "SELECT a % 3 AS v FROM t WHERE a = -8",
    ] {
        assert!(
            matches!(projected_domain(sql), ValueDomain::Unknown(_)),
            "{sql}: fractional division, zero divisor and negative remainder laws are residual"
        );
    }
}

#[test]
fn try_cast_narrowing_preserves_finite_null_on_failure_evidence() {
    let failed = projected_domain("SELECT TRY_CAST(a AS SMALLINT) AS v FROM t WHERE a = 40000");
    let ValueDomain::Set(values) = failed else {
        panic!("certified integer TRY_CAST failure must be SQL NULL: {failed:?}");
    };
    assert_eq!(values.values().len(), 1);
    assert_eq!(values.values()[0].value(), &LiteralValue::Null);

    let mixed =
        projected_domain("SELECT TRY_CAST(a AS SMALLINT) AS v FROM t WHERE a IN (1, 40000)");
    let ValueDomain::Set(values) = mixed else {
        panic!("finite TRY_CAST must retain successful and failing values: {mixed:?}");
    };
    assert_eq!(values.values().len(), 2);
    assert!(values
        .values()
        .iter()
        .any(|v| v.value() == &LiteralValue::Null));
    assert!(values
        .values()
        .iter()
        .any(|v| v.value() == &LiteralValue::Number("1".into())));

    let ordinary = projected_domain("SELECT CAST(a AS SMALLINT) AS v FROM t WHERE a = 40000");
    assert!(matches!(ordinary, ValueDomain::Unknown(_)));
}

#[test]
fn duckdb_positive_remainder_and_integral_division_oracles() {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE t(a INTEGER);
         INSERT INTO t VALUES (NULL), (7), (8), (9), (99), (100);",
    )
    .unwrap();
    for (predicate, expected) in [
        ("a >= 0 AND a % 3 = 0", 2_i64),
        ("a >= 0 AND a % 3 = 1", 2),
        ("a >= 0 AND a % 3 = 2", 1),
        ("a = 9 AND CAST(a AS BIGINT) / 3 = 3", 1),
        ("TRY_CAST(a AS SMALLINT) IS NULL", 1),
    ] {
        let count: i64 = db
            .query_row(
                &format!("SELECT COUNT(*) FROM t WHERE {predicate}"),
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, expected, "{predicate}");
    }
}

#[test]
fn correlated_integer_projection_does_not_cartesian_expand_one_source() {
    let difference = projected_domain("SELECT a - a AS v FROM t WHERE a BETWEEN -100 AND 100");
    let ValueDomain::Ranges(zero) = difference else {
        panic!("same-source cancellation must be bounded: {difference:?}");
    };
    let [range] = zero.ranges() else {
        panic!("expected one zero range")
    };
    assert_eq!(
        range.lower().unwrap().value().value(),
        &LiteralValue::Number("0".into())
    );
    assert_eq!(
        range.upper().unwrap().value().value(),
        &LiteralValue::Number("0".into())
    );

    let square = projected_domain("SELECT a * a AS v FROM t WHERE a BETWEEN -10 AND 20");
    let ValueDomain::Ranges(bounds) = square else {
        panic!("same-source square must not have a negative lower bound: {square:?}");
    };
    let [range] = bounds.ranges() else {
        panic!("expected one square range")
    };
    assert_eq!(
        range.lower().unwrap().value().value(),
        &LiteralValue::Number("0".into())
    );
    assert_eq!(
        range.upper().unwrap().value().value(),
        &LiteralValue::Number("400".into())
    );

    let quotient = projected_domain("SELECT a / a AS v FROM t WHERE a BETWEEN 1 AND 100");
    let ValueDomain::Ranges(one) = quotient else {
        panic!("nonzero self-division must be total: {quotient:?}");
    };
    let [range] = one.ranges() else {
        panic!("expected one unit range")
    };
    assert_eq!(
        range.lower().unwrap().value().value(),
        &LiteralValue::Number("1".into())
    );
    assert_eq!(
        range.upper().unwrap().value().value(),
        &LiteralValue::Number("1".into())
    );

    for sql in [
        "SELECT a / a AS v FROM t WHERE a BETWEEN -1 AND 1",
        "SELECT a % a AS v FROM t WHERE a BETWEEN 0 AND 3",
        "SELECT a * a AS v FROM t WHERE a BETWEEN -100000 AND 100000",
    ] {
        assert!(
            matches!(projected_domain(sql), ValueDomain::Unknown(_)),
            "{sql}: zero divisor and signed overflow must remain residual"
        );
    }
}

#[test]
fn unit_division_retains_certified_excluded_source_values() {
    let domain = projected_domain("SELECT CAST(a AS BIGINT) / -1 AS v FROM t WHERE a <> 0");
    let ValueDomain::Set(exclusions) = domain else {
        panic!("total, injective unit division must preserve exclusions: {domain:?}");
    };
    assert_eq!(exclusions.mode(), SetMode::Exclude);
    assert!(exclusions
        .values()
        .iter()
        .any(|value| { value.value() == &LiteralValue::Number("0".into()) }));
}

fn projected_decimal_domain(sql: &str) -> ValueDomain {
    let schema = RelationSchema::new(
        "t",
        vec![SchemaColumn::from_sql_type("d", "DECIMAL(8, 2)", "postgresql").unwrap()],
    )
    .unwrap();
    let catalog = RelationCatalog::from_schemas(&[schema]).unwrap();
    let dialect = PostgreSqlDialect {};
    let input = SqlInput::inline(sql);
    let bundle = analyze_configured_inputs_with_catalog(
        &[ConfiguredSqlInput::new(
            "decimal",
            &input,
            "postgresql",
            &dialect,
        )],
        &catalog,
    )
    .unwrap();
    let ComposedSemantics::Resolved(semantics) = bundle.layers()[0].composed_semantics() else {
        panic!("expected composed decimal output");
    };
    semantics.output().columns()[0].domain().clone()
}

#[test]
fn catalog_decimal_finite_operations_have_precision_scale_evidence() {
    let domain = projected_decimal_domain("SELECT d + 0.5 AS v FROM t WHERE d IN (1.25, 2.25)");
    let ValueDomain::Set(values) = domain else {
        panic!("exact fixed decimal additions should be finite: {domain:?}");
    };
    assert_eq!(values.values().len(), 2);
    assert!(values
        .values()
        .iter()
        .any(|value| { value.value() == &LiteralValue::Number("1.75".into()) }));
    assert!(values
        .values()
        .iter()
        .any(|value| { value.value() == &LiteralValue::Number("2.75".into()) }));

    for (sql, expected) in [
        ("SELECT d * 2 AS v FROM t WHERE d = 1.25", "2.5"),
        ("SELECT d / 2.0 AS v FROM t WHERE d = 1.5", "0.75"),
        ("SELECT d % 0.5 AS v FROM t WHERE d = 1.25", "0.25"),
        ("SELECT -d AS v FROM t WHERE d = 1.25", "-1.25"),
    ] {
        let domain = projected_decimal_domain(sql);
        let ValueDomain::Set(values) = domain else {
            panic!("{sql}: expected certified decimal singleton: {domain:?}");
        };
        assert_eq!(
            values.values()[0].value(),
            &LiteralValue::Number(expected.into()),
            "{sql}"
        );
    }

    for sql in [
        "SELECT d + 0.1 AS v FROM t WHERE d = 1.25",
        "SELECT d / 0.0 AS v FROM t WHERE d = 1.25",
        "SELECT d * 10000000 AS v FROM t WHERE d = 5.0",
    ] {
        assert!(
            matches!(projected_decimal_domain(sql), ValueDomain::Unknown(_)),
            "{sql}: fractional rounding, division errors and precision overflow stay residual"
        );
    }
}

#[test]
fn exact_integral_catalog_decimal_casts_preserve_failure_modes() {
    let integral =
        projected_decimal_domain("SELECT CAST(d AS SMALLINT) AS v FROM t WHERE d = 2.00");
    let ValueDomain::Set(values) = integral else {
        panic!("integral DECIMAL to SMALLINT should be exact: {integral:?}");
    };
    assert_eq!(
        values.values()[0].value(),
        &LiteralValue::Number("2".into())
    );

    let narrowing =
        projected_decimal_domain("SELECT TRY_CAST(d AS SMALLINT) AS v FROM t WHERE d = 40000.00");
    let ValueDomain::Set(values) = narrowing else {
        panic!("failed total TRY_CAST should be NULL: {narrowing:?}");
    };
    assert_eq!(values.values()[0].value(), &LiteralValue::Null);

    for sql in [
        "SELECT CAST(d AS SMALLINT) AS v FROM t WHERE d = 40000.00",
        "SELECT TRY_CAST(d AS SMALLINT) AS v FROM t WHERE d = 1.25",
    ] {
        assert!(
            matches!(projected_decimal_domain(sql), ValueDomain::Unknown(_)),
            "{sql}: throwing or rounded conversion must not be claimed exact"
        );
    }
}

#[test]
fn correlated_subtraction_proves_sql_true_and_not_true() {
    let always = witness("CAST(a AS BIGINT) - CAST(a AS BIGINT) = 0");
    assert!(matches!(
        always.qualifying(),
        BooleanWitnessDirection::Exact(BooleanTruthCase::True)
    ));
    assert!(matches!(
        always.rejected(),
        BooleanWitnessDirection::Exact(BooleanTruthCase::NotTrue)
    ));
    let excluded = witness("CAST(a AS BIGINT) - CAST(a AS BIGINT) <> 0");
    assert!(matches!(
        excluded.qualifying(),
        BooleanWitnessDirection::Impossible
    ));
    assert!(matches!(
        excluded.rejected(),
        BooleanWitnessDirection::Exact(BooleanTruthCase::NotTrue)
    ));
    let nullsafe = witness("CAST(a AS BIGINT) - CAST(a AS BIGINT) IS DISTINCT FROM 0");
    assert!(matches!(
        nullsafe.qualifying(),
        BooleanWitnessDirection::Exact(BooleanTruthCase::True)
    ));

    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE t(a INTEGER);
         INSERT INTO t VALUES (NULL), (-2147483648), (0), (2147483647);",
    )
    .unwrap();
    for (predicate, expected) in [
        ("CAST(a AS BIGINT) - CAST(a AS BIGINT) = 0", 3_i64),
        ("CAST(a AS BIGINT) - CAST(a AS BIGINT) <> 0", 0),
        (
            "CAST(a AS BIGINT) - CAST(a AS BIGINT) IS DISTINCT FROM 0",
            1,
        ),
    ] {
        let count: i64 = db
            .query_row(
                &format!("SELECT COUNT(*) FROM t WHERE {predicate}"),
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, expected, "{predicate}");
    }
}

#[test]
fn finite_decimal_catalog_has_direct_sql_and_dbt_adapter_parity() {
    const SQL: &str =
        "SELECT amount + 0.5 AS calculated FROM warehouse.raw.orders WHERE amount IN (1.25, 2.25)";
    let mut manifest: Value =
        serde_json::from_str(include_str!("fixtures/dbt/manifest-v12.json")).unwrap();
    manifest["nodes"]["model.demo.stg_orders"]["compiled_code"] = SQL.into();
    let manifest = parse_dbt_manifest(&manifest.to_string()).unwrap();

    let mut catalog_json: Value =
        serde_json::from_str(include_str!("fixtures/dbt/catalog-v1.json")).unwrap();
    catalog_json["sources"]["source.demo.orders"]["columns"]["amount"]["type"] =
        "DECIMAL(8, 2)".into();
    let catalog = parse_dbt_catalog(&catalog_json.to_string()).unwrap();
    let dialect = PostgreSqlDialect {};
    let dbt_bundle = analyze_dbt_artifacts(&manifest, &catalog, "postgresql", &dialect).unwrap();

    let physical = RelationSchema::new(
        "warehouse.raw.orders",
        vec![
            SchemaColumn::from_sql_type("id", "BIGINT", "postgresql").unwrap(),
            SchemaColumn::from_sql_type("amount", "DECIMAL(8, 2)", "postgresql").unwrap(),
        ],
    )
    .unwrap();
    let sql_catalog = RelationCatalog::from_schemas(&[physical]).unwrap();
    let input = SqlInput::inline(SQL);
    let sql_bundle = analyze_configured_inputs_with_catalog(
        &[ConfiguredSqlInput::new(
            "decimal_direct",
            &input,
            "postgresql",
            &dialect,
        )],
        &sql_catalog,
    )
    .unwrap();
    let output = |bundle: &sql_semantic_protocol::AnalysisBundle| {
        let layer = bundle
            .layers()
            .iter()
            .find(|layer| {
                layer
                    .consumes()
                    .iter()
                    .any(|source| source == "warehouse.raw.orders")
            })
            .expect("source-consuming arithmetic layer");
        let ComposedSemantics::Resolved(semantics) = layer.composed_semantics() else {
            panic!("decimal source should have composed semantics");
        };
        semantics.output().columns()[0].domain().clone()
    };
    let dbt_domain = output(&dbt_bundle);
    let sql_domain = output(&sql_bundle);
    assert_eq!(dbt_domain, sql_domain);
    let ValueDomain::Set(set) = dbt_domain else {
        panic!("source-attested decimal result must be finite: {dbt_domain:?}");
    };
    assert_eq!(set.values().len(), 2);
}

#[test]
fn portable_decimal_literal_outcomes_match_parser_capable_dialects() {
    for name in common::DIALECTS {
        let dialect = dialect_from_name(name).unwrap();
        for (sql, expected) in [
            ("SELECT 1.25 + 0.5 AS result", "1.75"),
            ("SELECT 1.5 / 0.5 AS result", "3.0"),
        ] {
            let Ok(protocol) = analyze_sql(sql, name, dialect.as_ref()) else {
                continue;
            };
            let Some(ProtocolStatement::Query(query)) = protocol.statements().first() else {
                panic!("{name}: expected SELECT");
            };
            let ValueDomain::Set(values) = query.output().columns()[0].domain() else {
                panic!("{name}: finite portable decimal literal must be exact");
            };
            assert_eq!(
                values.values()[0].value(),
                &LiteralValue::Number(expected.into()),
                "{name}: {sql}"
            );
        }
    }
}

#[test]
fn duckdb_fixed_decimal_catalog_oracles_cover_nulls_and_conversion_failures() {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE t(d DECIMAL(8, 2));
         INSERT INTO t VALUES (NULL), (1.25), (2.25), (40000.00);",
    )
    .unwrap();
    for (predicate, expected) in [
        ("d IN (1.25, 2.25) AND d + 0.5 IN (1.75, 2.75)", 2_i64),
        ("d = 1.25 AND d * 2 = 2.5", 1),
        ("d = 1.25 AND d % 0.5 = 0.25", 1),
        ("d = 40000.00 AND TRY_CAST(d AS SMALLINT) IS NULL", 1),
        ("d IS NULL AND TRY_CAST(d AS SMALLINT) IS NULL", 1),
    ] {
        let count: i64 = db
            .query_row(
                &format!("SELECT COUNT(*) FROM t WHERE {predicate}"),
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, expected, "{predicate}");
    }
}

fn projected_binary_text_cast(sql: &str, binary_attested: bool) -> ValueDomain {
    use sql_semantic_protocol::{
        ConstraintEnforcement, ConstraintEvidence, ConstraintProvenance, ConstraintSourceKind,
        TextComparisonLaw,
    };

    let mut column = SchemaColumn::from_sql_type("s", "VARCHAR(20)", "postgresql").unwrap();
    if binary_attested {
        column = column
            .with_text_comparison_law(
                TextComparisonLaw::Binary,
                vec![ConstraintEvidence::new(
                    ConstraintProvenance::new(
                        ConstraintSourceKind::ExternalMetadata,
                        "warehouse:binary",
                    )
                    .unwrap(),
                    ConstraintEnforcement::Enforced,
                )],
            )
            .unwrap();
    }
    let catalog =
        RelationCatalog::from_schemas(&[RelationSchema::new("t", vec![column]).unwrap()]).unwrap();
    let dialect = PostgreSqlDialect {};
    let input = SqlInput::inline(sql);
    let bundle = analyze_configured_inputs_with_catalog(
        &[ConfiguredSqlInput::new(
            "text",
            &input,
            "postgresql",
            &dialect,
        )],
        &catalog,
    )
    .unwrap();
    let ComposedSemantics::Resolved(semantics) = bundle.layers()[0].composed_semantics() else {
        panic!("text source should have composed semantics");
    };
    semantics.output().columns()[0].domain().clone()
}

#[test]
fn attested_binary_text_casts_preserve_finite_ascii_and_failure_semantics() {
    let sql = "SELECT CAST(s AS INTEGER) AS v FROM t WHERE s IN ('42', '-7')";
    let domain = projected_binary_text_cast(sql, true);
    let ValueDomain::Set(set) = domain else {
        panic!("attested binary source and ASCII text must be exact: {domain:?}");
    };
    assert_eq!(set.values().len(), 2);
    assert!(set
        .values()
        .iter()
        .any(|value| value.value() == &LiteralValue::Number("42".into())));
    assert!(set
        .values()
        .iter()
        .any(|value| value.value() == &LiteralValue::Number("-7".into())));

    let narrowing = projected_binary_text_cast(
        "SELECT TRY_CAST(s AS SMALLINT) AS v FROM t WHERE s IN ('32767', '32768')",
        true,
    );
    let ValueDomain::Set(values) = narrowing else {
        panic!("finite TRY_CAST must distinguish successful and NULL results: {narrowing:?}");
    };
    assert!(values
        .values()
        .iter()
        .any(|value| value.value() == &LiteralValue::Null));
    assert!(values
        .values()
        .iter()
        .any(|value| { value.value() == &LiteralValue::Number("32767".into()) }));

    for (sql, attested) in [
        (
            "SELECT CAST(s AS INTEGER) AS v FROM t WHERE s = '42'",
            false,
        ),
        (
            "SELECT CAST(s AS INTEGER) AS v FROM t WHERE s = '0042'",
            true,
        ),
        (
            "SELECT TRY_CAST(s AS INTEGER) AS v FROM t WHERE s = '1.5'",
            true,
        ),
    ] {
        assert!(
            matches!(
                projected_binary_text_cast(sql, attested),
                ValueDomain::Unknown(_)
            ),
            "{sql}: nonbinary equality or noncanonical conversion must stay residual"
        );
    }
}

#[test]
fn narrowing_try_cast_null_tests_have_both_exact_truth_directions() {
    for predicate in [
        "TRY_CAST(a AS SMALLINT) IS NULL",
        "TRY_CAST(a AS SMALLINT) IS NOT NULL",
        "TRY_CAST(a AS SMALLINT) IS NULL OR a = 0",
    ] {
        exact(predicate);
    }
    let ordinary = witness("CAST(a AS SMALLINT) IS NULL");
    assert!(
        matches!(
            ordinary.qualifying(),
            BooleanWitnessDirection::Residual { .. }
        ),
        "throwing CAST with out-of-range sources must not claim a total witness"
    );

    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE t(a INTEGER);
         INSERT INTO t VALUES (NULL), (-2147483648), (-32768), (0), (32767), (2147483647);",
    )
    .unwrap();
    for (predicate, expected) in [
        ("TRY_CAST(a AS SMALLINT) IS NULL", 3_i64),
        ("TRY_CAST(a AS SMALLINT) IS NOT NULL", 3),
        ("TRY_CAST(a AS SMALLINT) IS NULL OR a = 0", 4),
    ] {
        let actual: i64 = db
            .query_row(
                &format!("SELECT COUNT(*) FROM t WHERE {predicate}"),
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(actual, expected, "{predicate}");
    }
}

#[test]
fn modulo_bounds_prove_impossibility_without_assuming_negative_sign_laws() {
    for predicate in ["a % 3 = 4", "a % 3 > 2", "a % 3 < -2", "4 = a % 3"] {
        let proof = witness(predicate);
        assert!(
            matches!(proof.qualifying(), BooleanWitnessDirection::Impossible),
            "{predicate}: modulus bounds exclude every qualifying source row"
        );
        assert!(
            matches!(
                proof.rejected(),
                BooleanWitnessDirection::Exact(BooleanTruthCase::NotTrue)
            ),
            "{predicate}: NOT TRUE must include SQL NULL"
        );
    }
    for predicate in ["a % 3 < 3", "a % 3 >= -2"] {
        exact(predicate);
    }
    let all = witness("a % 3 IS DISTINCT FROM 4");
    assert!(matches!(
        all.qualifying(),
        BooleanWitnessDirection::Exact(BooleanTruthCase::True)
    ));
    assert!(matches!(
        all.rejected(),
        BooleanWitnessDirection::Impossible
    ));

    for predicate in ["a % 3 = 1", "a % 0 = 1", "a % -3 = 4"] {
        assert!(
            matches!(
                witness(predicate).qualifying(),
                BooleanWitnessDirection::Residual { .. }
            ),
            "{predicate}: periodic and nonportable modulo must remain residual"
        );
    }

    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE t(a INTEGER);
         INSERT INTO t VALUES (NULL), (-2147483648), (-7), (0), (7), (2147483647);",
    )
    .unwrap();
    for (predicate, expected) in [
        ("a % 3 = 4", 0_i64),
        ("a % 3 < 3", 5),
        ("a % 3 IS DISTINCT FROM 4", 6),
    ] {
        let actual: i64 = db
            .query_row(
                &format!("SELECT COUNT(*) FROM t WHERE {predicate}"),
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(actual, expected, "{predicate}");
    }
}

#[test]
fn finite_union_of_bounded_ranges_retains_exact_arithmetic_values() {
    let domain =
        projected_domain("SELECT a * 2 AS v FROM t WHERE a BETWEEN 1 AND 3 OR a BETWEEN 7 AND 9");
    let ValueDomain::Set(values) = domain else {
        panic!("two small disjoint source intervals should be exhaustive: {domain:?}");
    };
    assert_eq!(values.mode(), SetMode::Include);
    let actual = values
        .values()
        .iter()
        .map(|value| value.value().clone())
        .collect::<Vec<_>>();
    for number in [2, 4, 6, 14, 16, 18] {
        assert!(
            actual.contains(&LiteralValue::Number(number.to_string())),
            "missing {number} in {actual:?}"
        );
    }
    assert_eq!(actual.len(), 6);
}

#[test]
fn catalog_integer_arithmetic_proofs_agree_across_parser_capable_dialects() {
    let catalog = RelationCatalog::from_schemas(&[RelationSchema::new(
        "t",
        vec![SchemaColumn::from_sql_type("a", "INTEGER", "postgresql").unwrap()],
    )
    .unwrap()])
    .unwrap();
    for name in common::DIALECTS {
        let dialect = dialect_from_name(name).unwrap();
        for (predicate, should_qualify) in [
            ("CAST(a AS BIGINT) * 3 >= 11", true),
            ("CAST(a AS BIGINT) / 1 >= 11", true),
            ("CAST(a AS BIGINT) - CAST(a AS BIGINT) = 0", true),
            ("a % 3 = 4", false),
        ] {
            let sql = SqlInput::inline(format!("SELECT a FROM t WHERE {predicate}"));
            let configured = [ConfiguredSqlInput::new(
                "dialect_arithmetic",
                &sql,
                name,
                dialect.as_ref(),
            )];
            let Ok(bundle) = analyze_configured_inputs_with_catalog(&configured, &catalog) else {
                continue;
            };
            let ComposedSemantics::Resolved(semantics) = bundle.layers()[0].composed_semantics()
            else {
                panic!("{name}: expected source semantics");
            };
            let proof = semantics.boolean_witnesses()[0].witness();
            if should_qualify {
                assert!(
                    matches!(
                        proof.qualifying(),
                        BooleanWitnessDirection::Exact(BooleanTruthCase::True)
                    ),
                    "{name}: {predicate}: {:?}",
                    proof.qualifying()
                );
            } else {
                assert!(
                    matches!(proof.qualifying(), BooleanWitnessDirection::Impossible),
                    "{name}: {predicate}: {:?}",
                    proof.qualifying()
                );
            }
            assert!(
                matches!(
                    proof.rejected(),
                    BooleanWitnessDirection::Exact(BooleanTruthCase::NotTrue)
                ) || !should_qualify,
                "{name}: {predicate}: {:?}",
                proof.rejected()
            );
        }
    }
}

#[test]
fn parser_supported_safe_cast_preserves_null_on_failure() {
    let mut supported = false;
    for name in common::DIALECTS {
        let dialect = dialect_from_name(name).unwrap();
        let Ok(protocol) = analyze_sql(
            "SELECT SAFE_CAST(40000 AS SMALLINT) AS v",
            name,
            dialect.as_ref(),
        ) else {
            continue;
        };
        let wire = to_json(&protocol);
        if !wire.contains(r#""cast_kind":"safe_cast""#) {
            // Parsing SAFE_CAST as an opaque function is not certified.
            continue;
        }
        supported = true;
        assert!(wire.contains(r#""on_failure":"null""#), "{name}");
        let Some(ProtocolStatement::Query(query)) = protocol.statements().first() else {
            panic!("{name}: expected a SELECT");
        };
        let ValueDomain::Set(values) = query.output().columns()[0].domain() else {
            panic!("{name}: explicit SAFE_CAST failure must be NULL");
        };
        assert_eq!(values.values()[0].value(), &LiteralValue::Null);
    }
    assert!(
        supported,
        "at least one parser dialect must recognize SAFE_CAST"
    );
}

#[test]
fn unsupported_operator_type_pairs_emit_specific_residual_reasons() {
    for (predicate, operator, left, right) in [
        ("a + 1 > 0", "add", "source-column", "integer-literal"),
        ("a - 1 > 0", "subtract", "source-column", "integer-literal"),
        ("a * 3 > 0", "multiply", "source-column", "integer-literal"),
        ("a / 2 > 0", "divide", "source-column", "integer-literal"),
        ("a % 2 = 1", "modulo", "source-column", "integer-literal"),
        ("a + b > 0", "add", "source-column", "source-column"),
        ("a + 0.25 > 1", "add", "source-column", "decimal-literal"),
    ] {
        let proof = witness(predicate);
        let BooleanRowConstraint::Residual { reason } = proof.condition() else {
            panic!("{predicate}: unsupported/unbounded type pair needs typed residual: {:?}", proof.condition());
        };
        assert!(
            reason.contains(operator) && reason.contains(left) && reason.contains(right),
            "{predicate}: expected {operator}({left},{right}) residual, got {reason}"
        );
        assert!(
            matches!(proof.qualifying(), BooleanWitnessDirection::Residual { .. }),
            "{predicate}: unchecked arithmetic must not claim universal exactness"
        );
    }
}
