//! TASK-101: catalog-certified integer arithmetic and SQL 3VL oracles.
mod common;

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_artifacts, analyze_sql, dialect_from_name,
    parse_dbt_catalog, parse_dbt_manifest, BooleanRowConstraint, BooleanTruthCase, BooleanWitness,
    BooleanWitnessDirection, ComposedSemantics, ConfiguredSqlInput, ProtocolStatement,
    RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
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
    exact("CAST(a AS BIGINT) % 1 = 0 OR a IS NULL");
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
