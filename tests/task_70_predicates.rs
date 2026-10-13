//! TASK-70 bounded Boolean witness laws and SQL three-valued execution oracles.
mod common;

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_artifacts, analyze_sql, dialect_from_name,
    parse_dbt_catalog, parse_dbt_manifest, to_json, BooleanRowConstraint, BooleanTruthCase,
    BooleanWitness, BooleanWitnessDirection, ComposedSemantics, ConfiguredSqlInput,
    ProtocolStatement, RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
};
use sqlparser::dialect::PostgreSqlDialect;

fn typed_witness(predicate: &str) -> BooleanWitness {
    let schema = RelationSchema::new(
        "t",
        vec![
            SchemaColumn::from_sql_type("a", "INTEGER", "postgresql").unwrap(),
            SchemaColumn::from_sql_type("b", "INTEGER", "postgresql").unwrap(),
        ],
    )
    .unwrap();
    let catalog = RelationCatalog::from_schemas(&[schema]).unwrap();
    let input = SqlInput::inline(format!("SELECT a FROM t WHERE {predicate}"));
    let dialect = PostgreSqlDialect {};
    let configured = [ConfiguredSqlInput::new(
        "task70",
        &input,
        "postgresql",
        &dialect,
    )];
    let bundle = analyze_configured_inputs_with_catalog(&configured, &catalog).unwrap();
    let ComposedSemantics::Resolved(composed) = bundle.layers()[0].composed_semantics() else {
        panic!("typed query must resolve");
    };
    composed.boolean_witnesses()[0].witness().clone()
}

fn assert_both_directions_exact(predicate: &str) -> BooleanWitness {
    let witness = typed_witness(predicate);
    assert!(
        matches!(
            witness.qualifying(),
            BooleanWitnessDirection::Exact(BooleanTruthCase::True)
        ),
        "{predicate}: expected exact SQL TRUE direction, got {:?}",
        witness.qualifying()
    );
    assert!(
        matches!(
            witness.rejected(),
            BooleanWitnessDirection::Exact(BooleanTruthCase::NotTrue)
        ),
        "{predicate}: expected exact SQL NOT TRUE direction, got {:?}",
        witness.rejected()
    );
    witness
}

#[test]
fn finite_integer_membership_and_ranges_preserve_boolean_structure() {
    for (predicate, all) in [
        ("a BETWEEN 2 AND 5", true),
        ("a NOT BETWEEN 2 AND 5", false),
        ("a IN (1, 3)", false),
        ("a NOT IN (1, 3)", true),
        ("NOT (a > 2 OR b IS NULL)", true),
        ("NOT (a > 2 AND b IS NULL)", false),
    ] {
        let witness = assert_both_directions_exact(predicate);
        assert!(
            match witness.condition() {
                BooleanRowConstraint::All(children) => all && children.len() == 2,
                BooleanRowConstraint::Any(children) => !all && children.len() == 2,
                _ => false,
            },
            "{predicate}: wrong De Morgan or IN/BETWEEN direction"
        );
    }
}

#[test]
fn null_safe_integer_comparison_is_two_valued() {
    for predicate in [
        "a IS DISTINCT FROM 2",
        "a IS NOT DISTINCT FROM 2",
        "NOT (a IS DISTINCT FROM 2)",
    ] {
        let witness = assert_both_directions_exact(predicate);
        assert!(matches!(
            witness.condition(),
            BooleanRowConstraint::IntegerComparison { .. }
        ));
    }
    for predicate in [
        "a IS DISTINCT FROM NULL",
        "a IS NOT DISTINCT FROM NULL",
        "NOT (a IS DISTINCT FROM NULL)",
    ] {
        let witness = assert_both_directions_exact(predicate);
        assert!(matches!(
            witness.condition(),
            BooleanRowConstraint::NullTest { .. }
        ));
    }
}

#[test]
fn unsupported_computed_membership_and_ranges_remain_residual() {
    for predicate in ["a IN (a * 2, 1)", "a BETWEEN a + b AND 5"] {
        let witness = typed_witness(predicate);
        assert!(
            matches!(
                witness.qualifying(),
                BooleanWitnessDirection::Residual { .. }
            ),
            "{predicate}"
        );
        assert!(
            matches!(witness.rejected(), BooleanWitnessDirection::Residual { .. }),
            "{predicate}"
        );
    }
}

#[test]
fn null_membership_obligations_preserve_unknown_in_both_directions() {
    for predicate in ["a IN (1, NULL)", "NOT (a NOT IN (1, NULL))"] {
        assert_both_directions_exact(predicate);
    }
    for predicate in [
        "a NOT IN (1, NULL)",
        "a IN (NULL)",
        "a NOT IN (NULL)",
        "a = NULL",
        "NOT (a <> NULL)",
        "a BETWEEN NULL AND 3",
    ] {
        let witness = typed_witness(predicate);
        assert!(
            matches!(witness.qualifying(), BooleanWitnessDirection::Impossible),
            "{predicate}: UNKNOWN cannot be SQL TRUE"
        );
        assert!(
            matches!(
                witness.rejected(),
                BooleanWitnessDirection::Exact(BooleanTruthCase::NotTrue)
            ),
            "{predicate}"
        );
    }
    let witness = typed_witness("a IN (1, NULL)");
    assert!(matches!(witness.condition(), BooleanRowConstraint::Any(_)));
}

#[test]
fn certified_computed_null_tests_have_exact_two_valued_outcomes() {
    for predicate in [
        "COALESCE(a, b) IS NULL",
        "COALESCE(a, b) IS NOT NULL",
        "COALESCE(a, NULL, b) IS NOT NULL",
        "NULLIF(a, b) IS NULL",
        "NULLIF(a, b) IS NOT NULL",
        "CAST(a AS BIGINT) IS NULL",
        "(CAST(a AS BIGINT) + 1) IS NOT NULL",
        "CASE WHEN a IS NULL THEN b ELSE a END IS NULL",
        "CASE WHEN a IS NOT NULL THEN b ELSE a END IS NOT NULL",
        "CASE WHEN a IS NULL THEN b WHEN b IS NULL THEN a ELSE b END IS NULL",
    ] {
        assert_both_directions_exact(predicate);
    }
    let impossible = typed_witness("a IS NOT NULL AND COALESCE(a, b) IS NULL");
    // If b is NULL this can still be false, but never SQL TRUE:
    assert!(matches!(
        impossible.qualifying(),
        BooleanWitnessDirection::Impossible
    ));
}

#[test]
fn opaque_or_mixed_type_computed_null_tests_fail_closed() {
    for predicate in [
        "COALESCE(a, ABS(b)) IS NULL",
        "COALESCE(a, 'text') IS NOT NULL",
        "NULLIF(a, a + 1) IS NULL",
        "CASE WHEN a > 1 THEN b ELSE a END IS NULL",
        "CASE WHEN a IS NULL THEN b END IS NULL",
    ] {
        let witness = typed_witness(predicate);
        assert!(
            matches!(
                witness.qualifying(),
                BooleanWitnessDirection::Residual { .. }
            ),
            "{predicate}"
        );
    }
}

#[test]
fn duckdb_oracle_certifies_coalesce_and_nullif_null_tests() {
    let connection = Connection::open_in_memory().unwrap();
    connection
        .execute_batch(
            "CREATE TABLE t(a INTEGER, b INTEGER);
         INSERT INTO t VALUES (NULL, NULL), (NULL, 1), (1, NULL), (2, 2), (3, 4), (4, 3);",
        )
        .unwrap();
    for (predicate, expected) in [
        ("COALESCE(a, b) IS NULL", 1_i64),
        ("COALESCE(a, b) IS NOT NULL", 5),
        ("NULLIF(a, b) IS NULL", 3),
        ("NULLIF(a, b) IS NOT NULL", 3),
        ("CAST(a AS BIGINT) IS NULL", 2),
        ("CASE WHEN a IS NULL THEN b ELSE a END IS NULL", 1),
        ("CASE WHEN a IS NOT NULL THEN b ELSE a END IS NOT NULL", 3),
    ] {
        let actual: i64 = connection
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
fn emitted_null_comparison_is_parser_independent_and_two_valued_at_filter_boundary() {
    let protocol = analyze_sql(
        "SELECT a FROM t WHERE a = NULL",
        "postgresql",
        &PostgreSqlDialect {},
    )
    .unwrap();
    let document: Value = serde_json::from_str(&to_json(&protocol)).unwrap();
    let witness = &document["inputs"][0]["statements"][0]["boolean_witness"];
    assert_eq!(witness["condition"]["kind"], "unknown_null_comparison");
    assert_eq!(witness["condition"]["column"]["name"], "a");
    assert_eq!(witness["qualifying"]["status"], "impossible");
    assert_eq!(witness["rejected"]["truth"], "not_true");
}

#[test]
fn all_registered_dialects_retain_computed_null_test_structure() {
    for name in common::DIALECTS {
        let dialect = dialect_from_name(name).expect("registered dialect");
        let sql = "SELECT a FROM t WHERE COALESCE(a, b) IS NULL";
        let protocol = analyze_sql(sql, name, dialect.as_ref())
            .unwrap_or_else(|error| panic!("{name} parser: {error}"));
        let Some(ProtocolStatement::Query(query)) = protocol.statements().first() else {
            panic!("{name}: expected query");
        };
        let witness = query.boolean_witness().expect("computed null witness");
        assert!(
            matches!(
                witness.qualifying(),
                BooleanWitnessDirection::Residual { .. }
            ),
            "{name}: untyped integer COALESCE must not become exact"
        );
    }
}

#[test]
fn dbt_compiled_nullable_expression_and_direct_sql_share_witnesses() {
    const SQL: &str =
        "SELECT id, amount FROM warehouse.raw.orders WHERE NULLIF(id, amount) IS NULL OR amount IN (3, NULL)";
    let mut manifest: Value =
        serde_json::from_str(include_str!("fixtures/dbt/manifest-v12.json")).unwrap();
    manifest["nodes"]["model.demo.stg_orders"]["compiled_code"] = SQL.into();
    let manifest = parse_dbt_manifest(&manifest.to_string()).unwrap();
    let catalog = parse_dbt_catalog(include_str!("fixtures/dbt/catalog-v1.json")).unwrap();
    let dialect = PostgreSqlDialect {};
    let dbt = analyze_dbt_artifacts(&manifest, &catalog, "postgresql", &dialect).unwrap();
    let schema = RelationSchema::new(
        "warehouse.raw.orders",
        vec![
            SchemaColumn::from_sql_type("id", "BIGINT", "postgresql").unwrap(),
            SchemaColumn::from_sql_type("amount", "INTEGER", "postgresql").unwrap(),
        ],
    )
    .unwrap();
    let catalog = RelationCatalog::from_schemas(&[schema]).unwrap();
    let sql_input = SqlInput::inline(SQL);
    let direct = analyze_configured_inputs_with_catalog(
        &[ConfiguredSqlInput::new(
            "direct",
            &sql_input,
            "postgresql",
            &dialect,
        )],
        &catalog,
    )
    .unwrap();
    let find_witness = |bundle: &sql_semantic_protocol::AnalysisBundle| {
        let layer = bundle
            .layers()
            .iter()
            .find(|layer| {
                layer
                    .consumes()
                    .iter()
                    .any(|relation| relation == "warehouse.raw.orders")
            })
            .expect("source-consuming layer");
        let ComposedSemantics::Resolved(ref semantics) = layer.composed_semantics() else {
            panic!("resolved semantics required");
        };
        semantics
            .boolean_witnesses()
            .first()
            .expect("witness")
            .witness()
            .clone()
    };
    let a = find_witness(&dbt);
    let b = find_witness(&direct);
    assert_eq!(a.condition(), b.condition());
    assert_eq!(a.qualifying(), b.qualifying());
    assert_eq!(a.rejected(), b.rejected());
}

#[test]
fn dialect_shared_three_valued_negation_has_identical_protocol_shape() {
    let sql = "SELECT a FROM t WHERE NOT (a IS NULL OR b IS NOT NULL)";
    for name in common::DIALECTS {
        let dialect = dialect_from_name(name).expect("registered parser dialect");
        let protocol = analyze_sql(sql, name, dialect.as_ref())
            .unwrap_or_else(|err| panic!("{name}: shared predicate must parse: {err}"));
        let Some(ProtocolStatement::Query(query)) = protocol.statements().first() else {
            panic!("{name}: expected query");
        };
        let witness = query.boolean_witness().expect("logical witness");
        let BooleanRowConstraint::All(children) = witness.condition() else {
            panic!("{name}: NOT OR must normalize to AND");
        };
        assert_eq!(children.len(), 2, "{name}");
        assert!(
            children.iter().all(BooleanRowConstraint::is_exact),
            "{name}"
        );
    }
}

#[test]
fn duckdb_oracle_distinguishes_false_from_unknown_and_null_safe_equality() {
    let connection = Connection::open_in_memory().unwrap();
    connection
        .execute_batch(
            "CREATE TABLE t(a INTEGER, b INTEGER);
         INSERT INTO t VALUES (NULL, 1), (2, NULL), (3, 3), (4, 1);",
        )
        .unwrap();
    for (predicate, expected) in [
        ("a BETWEEN 2 AND 3", 2_i64),
        ("a NOT BETWEEN 2 AND 3", 1),
        ("a IN (2, 3)", 2),
        ("a NOT IN (2, 3)", 1),
        ("a NOT IN (2, NULL)", 0),
        ("a IN (NULL)", 0),
        ("a = NULL", 0),
        ("a BETWEEN NULL AND 3", 0),
        ("NOT (a > 2 OR b IS NULL)", 0),
        ("a IS DISTINCT FROM 2", 3),
        ("a IS NOT DISTINCT FROM 2", 1),
        ("NOT (a IS DISTINCT FROM 2)", 1),
        ("a IS DISTINCT FROM NULL", 3),
        ("a IS NOT DISTINCT FROM NULL", 1),
    ] {
        let count: i64 = connection
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
fn dbt_compiled_sql_and_direct_sql_share_canonical_truth_obligations() {
    const SQL: &str =
        "SELECT id, amount FROM warehouse.raw.orders WHERE NOT (amount BETWEEN 2 AND 10 OR id IS NOT DISTINCT FROM NULL)";
    let mut manifest: Value = serde_json::from_str(include_str!("fixtures/dbt/manifest-v12.json"))
        .expect("dbt fixture manifest");
    manifest["nodes"]["model.demo.stg_orders"]["compiled_code"] = SQL.into();
    let manifest = parse_dbt_manifest(&manifest.to_string()).expect("dbt manifest");
    let catalog =
        parse_dbt_catalog(include_str!("fixtures/dbt/catalog-v1.json")).expect("dbt catalog");
    let dialect = PostgreSqlDialect {};
    let dbt =
        analyze_dbt_artifacts(&manifest, &catalog, "postgresql", &dialect).expect("dbt analysis");

    let schema = RelationSchema::new(
        "warehouse.raw.orders",
        vec![
            SchemaColumn::from_sql_type("id", "BIGINT", "postgresql").unwrap(),
            SchemaColumn::from_sql_type("amount", "INTEGER", "postgresql").unwrap(),
        ],
    )
    .unwrap();
    let catalog = RelationCatalog::from_schemas(&[schema]).unwrap();
    let input = SqlInput::inline(SQL);
    let direct = analyze_configured_inputs_with_catalog(
        &[ConfiguredSqlInput::new(
            "direct",
            &input,
            "postgresql",
            &dialect,
        )],
        &catalog,
    )
    .expect("direct analysis");

    let witness = |bundle: &sql_semantic_protocol::AnalysisBundle| {
        let layer = bundle
            .layers()
            .iter()
            .find(|layer| {
                layer
                    .consumes()
                    .iter()
                    .any(|source| source == "warehouse.raw.orders")
            })
            .expect("source-consuming layer");
        let ComposedSemantics::Resolved(ref semantics) = layer.composed_semantics() else {
            panic!("expected composed semantics");
        };
        semantics
            .boolean_witnesses()
            .first()
            .expect("boolean witness")
            .witness()
            .clone()
    };
    let from_dbt = witness(&dbt);
    let from_sql = witness(&direct);
    assert_eq!(from_dbt.condition(), from_sql.condition());
    assert_eq!(from_dbt.qualifying(), from_sql.qualifying());
    assert_eq!(from_dbt.rejected(), from_sql.rejected());
    assert!(matches!(
        from_dbt.qualifying(),
        BooleanWitnessDirection::Exact(BooleanTruthCase::True)
    ));
    assert!(matches!(
        from_dbt.rejected(),
        BooleanWitnessDirection::Exact(BooleanTruthCase::NotTrue)
    ));
}

#[test]
fn typed_cross_column_comparisons_prove_existence_without_independent_domains() {
    for predicate in [
        "a = b",
        "a <> b",
        "a < b",
        "a >= b",
        "a IS DISTINCT FROM b",
        "a IS NOT DISTINCT FROM b",
        "NOT (a = b)",
        "a = b AND a > 2 AND b < 5",
    ] {
        let witness = assert_both_directions_exact(predicate);
        assert!(
            witness.condition().is_exact(),
            "{predicate}: typed coupled condition must remain exact"
        );
    }
    let witness = assert_both_directions_exact("a = b");
    assert!(matches!(
        witness.condition(),
        BooleanRowConstraint::IntegerColumnComparison { .. }
    ));
}

#[test]
fn identical_source_column_comparison_regions_are_exhaustive() {
    for predicate in [
        "a <> a",
        "a < a",
        "a > a",
        "a IS DISTINCT FROM a",
    ] {
        let witness = typed_witness(predicate);
        assert!(matches!(witness.qualifying(), BooleanWitnessDirection::Impossible), "{predicate}");
        assert!(matches!(
            witness.rejected(),
            BooleanWitnessDirection::Exact(BooleanTruthCase::NotTrue)
        ), "{predicate}");
    }
    let same = assert_both_directions_exact("a = a");
    assert!(matches!(
        same.condition(),
        BooleanRowConstraint::IntegerColumnComparison { .. }
    ));
    let tautology = typed_witness("a IS NOT DISTINCT FROM a");
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
fn mutually_exclusive_correlated_comparisons_never_claim_impossibility_from_samples() {
    let witness = typed_witness("a > b AND b > a");
    assert!(
        matches!(
            witness.qualifying(),
            BooleanWitnessDirection::Residual { .. }
        ),
        "bounded representatives must not assert universal infeasibility"
    );
    assert!(matches!(
        witness.rejected(),
        BooleanWitnessDirection::Exact(BooleanTruthCase::NotTrue)
    ));
}

#[test]
fn duckdb_oracle_confirms_nullable_correlated_integer_truths() {
    let connection = Connection::open_in_memory().unwrap();
    connection
        .execute_batch(
            "CREATE TABLE t(a INTEGER, b INTEGER);
             INSERT INTO t VALUES (NULL, NULL), (NULL, 1), (1, NULL), (2, 2), (3, 4), (4, 3);",
        )
        .unwrap();
    for (predicate, expected) in [
        ("a = b", 1_i64),
        ("a <> b", 2),
        ("a < b", 1),
        ("a >= b", 2),
        ("a IS DISTINCT FROM b", 4),
        ("a IS NOT DISTINCT FROM b", 2),
        ("NOT (a = b)", 2),
    ] {
        let count: i64 = connection
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
fn exhaustive_scalar_regions_distinguish_impossible_from_unproved() {
    for predicate in [
        "a > 3 AND a <= 3",
        "a IN (1, 2) AND a NOT IN (1, 2)",
        "a IS NULL AND a IS NOT NULL",
    ] {
        let witness = typed_witness(predicate);
        assert!(
            matches!(witness.qualifying(), BooleanWitnessDirection::Impossible),
            "{predicate}: contradiction must be provably impossible"
        );
        assert!(matches!(
            witness.rejected(),
            BooleanWitnessDirection::Exact(BooleanTruthCase::NotTrue)
        ));
    }
    let tautology = typed_witness("a IS NULL OR a IS NOT NULL");
    assert!(matches!(
        tautology.qualifying(),
        BooleanWitnessDirection::Exact(BooleanTruthCase::True)
    ));
    assert!(matches!(
        tautology.rejected(),
        BooleanWitnessDirection::Impossible
    ));
}
