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
        "SELECT TRY_CAST(a AS SMALLINT) AS v FROM t WHERE a = 40000",
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
