//! TASK-103: correlated cross-column and mixed Boolean joint constraints.
mod common;

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_artifacts, dialect_from_name,
    parse_dbt_catalog, parse_dbt_manifest, BooleanRowConstraint, BooleanTruthCase, BooleanWitness,
    BooleanWitnessDirection, ComposedSemantics, ConfiguredSqlInput, RelationCatalog,
    RelationSchema, SchemaColumn, SqlInput, ValueDomain,
};
use sqlparser::dialect::PostgreSqlDialect;

fn typed_witness_abc(predicate: &str) -> BooleanWitness {
    let schema = RelationSchema::new(
        "t",
        vec![
            SchemaColumn::from_sql_type("a", "INTEGER", "postgresql").unwrap(),
            SchemaColumn::from_sql_type("b", "INTEGER", "postgresql").unwrap(),
            SchemaColumn::from_sql_type("c", "INTEGER", "postgresql").unwrap(),
        ],
    )
    .unwrap();
    let catalog = RelationCatalog::from_schemas(&[schema]).unwrap();
    let input = SqlInput::inline(format!("SELECT a FROM t WHERE {predicate}"));
    let dialect = PostgreSqlDialect {};
    let configured = [ConfiguredSqlInput::new(
        "task103",
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

fn assert_impossible(predicate: &str) -> BooleanWitness {
    let witness = typed_witness_abc(predicate);
    assert!(
        matches!(witness.qualifying(), BooleanWitnessDirection::Impossible),
        "{predicate}: expected impossible TRUE, got {:?}",
        witness.qualifying()
    );
    assert!(
        matches!(
            witness.rejected(),
            BooleanWitnessDirection::Exact(BooleanTruthCase::NotTrue)
        ),
        "{predicate}: expected exact NOT TRUE, got {:?}",
        witness.rejected()
    );
    witness
}

fn assert_both_exact(predicate: &str) -> BooleanWitness {
    let witness = typed_witness_abc(predicate);
    assert!(
        matches!(
            witness.qualifying(),
            BooleanWitnessDirection::Exact(BooleanTruthCase::True)
        ),
        "{predicate}: expected exact TRUE, got {:?}",
        witness.qualifying()
    );
    assert!(
        matches!(
            witness.rejected(),
            BooleanWitnessDirection::Exact(BooleanTruthCase::NotTrue)
        ),
        "{predicate}: expected exact NOT TRUE, got {:?}",
        witness.rejected()
    );
    witness
}

#[test]
fn strict_and_nonstrict_order_cycles_are_impossible() {
    for predicate in [
        "a > b AND b > a",
        "a >= b AND b > a",
        "a > b AND b >= a",
        "a < b AND b < c AND c < a",
        "a <= b AND b <= c AND c < a",
        "NOT (a <= b OR b <= a)",
    ] {
        assert_impossible(predicate);
    }
}

#[test]
fn equality_and_order_conflicts_are_impossible() {
    for predicate in [
        "a = b AND a <> b",
        "a = b AND b = c AND a <> c",
        "a = b AND a > b",
        "a = b AND a < b",
        "(a IS NOT DISTINCT FROM b) AND (a IS DISTINCT FROM b)",
        "a > 5 AND b < 3 AND a <= b",
        "a BETWEEN 1 AND 3 AND b BETWEEN 5 AND 7 AND a >= b",
        "a = b AND a IS NULL",
        "a > b AND a IS NULL",
        "a = b AND a IS NULL AND b IS NOT NULL",
    ] {
        assert_impossible(predicate);
    }
}

#[test]
fn mixed_disjunctions_share_one_conjunctive_assignment() {
    for predicate in [
        "a < b AND (a > b OR a = b)",
        "a = b AND (a > b OR a < b)",
        "(a < b OR a = b) AND (b < a OR a = b) AND a <> b",
        "(a > b OR a = b) AND a < b AND c IS NOT NULL",
    ] {
        assert_impossible(predicate);
    }
    assert_both_exact("(a > b OR a = b) AND a >= b");
    assert_both_exact("a < b AND (a > b OR a < b)");
}

#[test]
fn mixed_disjunction_budget_exhaustion_is_residual() {
    // Seven independent binary alternatives require 128 cases, exceeding
    // the deterministic 64-case cap. Sampling cannot justify impossibility.
    let predicate = format!("a < b AND {}", vec!["(a > b OR a = b)"; 7].join(" AND "));
    let witness = typed_witness_abc(&predicate);
    assert!(matches!(
        witness.qualifying(),
        BooleanWitnessDirection::Residual { .. }
    ));
}

#[test]
fn jointly_satisfiable_multi_column_predicates_stay_exact() {
    for predicate in [
        "a > b AND b > 0 AND a < 10",
        "a = b AND a > 2 AND b < 5",
        "a < b AND b < c",
        "a >= b OR b >= a",
        "a BETWEEN 1 AND 5 AND b BETWEEN 1 AND 5 AND a <> b",
        "(a IS DISTINCT FROM b) AND (a IS NOT NULL)",
    ] {
        assert_both_exact(predicate);
    }
}

#[test]
fn null_safe_complementary_disjunction_is_tautological() {
    let witness = typed_witness_abc("(a IS NOT DISTINCT FROM b) OR (a IS DISTINCT FROM b)");
    assert!(
        matches!(
            witness.qualifying(),
            BooleanWitnessDirection::Exact(BooleanTruthCase::True)
        ),
        "tautology must have exact TRUE"
    );
    assert!(
        matches!(witness.rejected(), BooleanWitnessDirection::Impossible),
        "tautology must have impossible NOT TRUE, got {:?}",
        witness.rejected()
    );
    // Ordinary total-order coverage still admits UNKNOWN on NULL, so it is
    // satisfiable in both directions rather than tautological.
    assert_both_exact("a > b OR b >= a");
}

#[test]
fn unparenthesized_null_safe_conjunction_is_a_parser_boundary_residual() {
    // sqlparser binds `IS [NOT] DISTINCT FROM b AND ...` as
    // `a IS [NOT] DISTINCT FROM (b AND ...)`, so the unparenthesized form
    // cannot reach the coupled integer witness. It must fail closed.
    for predicate in [
        "a IS DISTINCT FROM b AND a IS NOT NULL",
        "a IS NOT DISTINCT FROM b AND a IS DISTINCT FROM b",
    ] {
        let witness = typed_witness_abc(predicate);
        assert!(
            matches!(
                witness.qualifying(),
                BooleanWitnessDirection::Residual { .. }
            ),
            "{predicate}: parser-boundary form must stay residual"
        );
    }
}

#[test]
fn mixed_boolean_normalization_preserves_three_valued_joint_shape() {
    let witness = assert_both_exact("NOT (a > b AND b IS NULL) OR c BETWEEN 1 AND 3");
    assert!(witness.condition().is_exact());
    let witness = assert_both_exact("a IN (1, 2) AND b NOT IN (1, 2)");
    assert!(matches!(witness.condition(), BooleanRowConstraint::All(_)));
    // A NULL-bearing NOT IN can never be TRUE, so the conjunction with a
    // satisfiable IN is impossible while NOT TRUE remains exact via UNKNOWN.
    assert_impossible("a IN (1, 2) AND b NOT IN (1, NULL) AND a = b AND b = 99");
    let witness = typed_witness_abc("a IN (1, NULL)");
    assert!(
        matches!(
            witness.qualifying(),
            BooleanWitnessDirection::Exact(BooleanTruthCase::True)
        ),
        "nullable IN keeps an exact TRUE branch"
    );
}

#[test]
fn opaque_and_unbounded_cross_column_cases_stay_residual() {
    for predicate in ["ABS(a) > b", "a + b > c"] {
        let witness = typed_witness_abc(predicate);
        assert!(
            matches!(
                witness.qualifying(),
                BooleanWitnessDirection::Residual { .. }
            ),
            "{predicate}: unproven computed cross-column must stay residual"
        );
    }
}

#[test]
fn impossible_filter_produces_empty_output_domain() {
    let schema = RelationSchema::new(
        "t",
        vec![
            SchemaColumn::from_sql_type("a", "INTEGER", "postgresql").unwrap(),
            SchemaColumn::from_sql_type("b", "INTEGER", "postgresql").unwrap(),
        ],
    )
    .unwrap();
    let catalog = RelationCatalog::from_schemas(&[schema]).unwrap();
    let dialect = PostgreSqlDialect {};
    let input = SqlInput::inline("SELECT a FROM t WHERE a > b AND b > a");
    let bundle = analyze_configured_inputs_with_catalog(
        &[ConfiguredSqlInput::new(
            "typed",
            &input,
            "postgresql",
            &dialect,
        )],
        &catalog,
    )
    .unwrap();
    let ComposedSemantics::Resolved(composed) = bundle.layers()[0].composed_semantics() else {
        panic!("resolved semantics required");
    };
    let witness = composed.boolean_witnesses()[0].witness();
    assert!(matches!(
        witness.qualifying(),
        BooleanWitnessDirection::Impossible
    ));
    let output = composed.output();
    assert!(
        matches!(output.columns()[0].domain(), ValueDomain::Empty),
        "impossible joint filter must yield an empty output domain"
    );
}

#[test]
fn duckdb_exhaustive_small_domain_matches_claimed_directions() {
    let connection = Connection::open_in_memory().unwrap();
    connection
        .execute_batch(
            "CREATE TABLE t(a INTEGER, b INTEGER, c INTEGER);
             INSERT INTO t
             SELECT a, b, c FROM
               (VALUES (NULL), (-1), (2), (9)) AS va(a),
               (VALUES (NULL), (-1), (2), (9)) AS vb(b),
               (VALUES (NULL), (-1), (2), (9)) AS vc(c);",
        )
        .unwrap();
    let total: i64 = connection
        .query_row("SELECT COUNT(*) FROM t", [], |row| row.get(0))
        .unwrap();
    assert_eq!(total, 64);
    for (predicate, expect_true, expect_not_true) in [
        ("a > b AND b > a", false, true),
        ("a < b AND b < c AND c < a", false, true),
        ("a = b AND a <> b", false, true),
        ("a = b AND b = c AND a <> c", false, true),
        ("a > 5 AND b < 3 AND a <= b", false, true),
        (
            "(a IS NOT DISTINCT FROM b) OR (a IS DISTINCT FROM b)",
            true,
            false,
        ),
        ("a > b AND b > 0 AND a < 10", true, true),
        ("a < b AND b < c", true, true),
        ("a > b OR b >= a", true, true),
        ("a = b AND a IS NULL", false, true),
    ] {
        let true_count: i64 = connection
            .query_row(
                &format!("SELECT COUNT(*) FROM t WHERE {predicate}"),
                [],
                |row| row.get(0),
            )
            .unwrap();
        let has_true = true_count > 0;
        let has_not_true = true_count < total;
        assert_eq!(has_true, expect_true, "{predicate}: DuckDB TRUE existence");
        assert_eq!(
            has_not_true, expect_not_true,
            "{predicate}: DuckDB NOT TRUE existence"
        );
        let witness = typed_witness_abc(predicate);
        match (expect_true, expect_not_true) {
            (false, true) => {
                assert!(
                    matches!(witness.qualifying(), BooleanWitnessDirection::Impossible),
                    "{predicate}: protocol must prove impossibility"
                );
                assert!(
                    matches!(
                        witness.rejected(),
                        BooleanWitnessDirection::Exact(BooleanTruthCase::NotTrue)
                    ),
                    "{predicate}"
                );
            }
            (true, false) => {
                assert!(
                    matches!(
                        witness.qualifying(),
                        BooleanWitnessDirection::Exact(BooleanTruthCase::True)
                    ),
                    "{predicate}"
                );
                assert!(
                    matches!(witness.rejected(), BooleanWitnessDirection::Impossible),
                    "{predicate}: tautology"
                );
            }
            _ => {
                assert!(
                    matches!(
                        witness.qualifying(),
                        BooleanWitnessDirection::Exact(BooleanTruthCase::True)
                    ),
                    "{predicate}: satisfiable"
                );
                assert!(
                    matches!(
                        witness.rejected(),
                        BooleanWitnessDirection::Exact(BooleanTruthCase::NotTrue)
                    ),
                    "{predicate}"
                );
            }
        }
    }
}

#[test]
fn typed_dialects_share_canonical_cross_column_impossibility() {
    for name in common::DIALECTS {
        let dialect = dialect_from_name(name).expect("registered dialect");
        let schema = RelationSchema::new(
            "t",
            vec![
                SchemaColumn::from_sql_type("a", "INTEGER", "postgresql").unwrap(),
                SchemaColumn::from_sql_type("b", "INTEGER", "postgresql").unwrap(),
            ],
        )
        .unwrap();
        let catalog = RelationCatalog::from_schemas(&[schema]).unwrap();
        let sql = SqlInput::inline("SELECT a FROM t WHERE a > b AND b > a");
        let bundle = analyze_configured_inputs_with_catalog(
            &[ConfiguredSqlInput::new(
                "typed",
                &sql,
                name,
                dialect.as_ref(),
            )],
            &catalog,
        )
        .unwrap_or_else(|error| panic!("{name} typed analysis: {error}"));
        let ComposedSemantics::Resolved(composed) = bundle.layers()[0].composed_semantics() else {
            panic!("{name}: expected resolved semantics");
        };
        let witness = composed.boolean_witnesses()[0].witness();
        let BooleanRowConstraint::All(children) = witness.condition() else {
            panic!("{name}: conjunction must stay conjunctive");
        };
        assert_eq!(children.len(), 2, "{name}");
        assert!(
            children.iter().all(BooleanRowConstraint::is_exact),
            "{name}"
        );
        assert!(
            matches!(witness.qualifying(), BooleanWitnessDirection::Impossible),
            "{name}: strict cycle must be impossible"
        );
    }
}

#[test]
fn dbt_compiled_cross_column_sql_matches_direct_analysis() {
    const SQL: &str =
        "SELECT id, amount FROM warehouse.raw.orders WHERE id > amount AND amount > id";
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
    let direct = analyze_configured_inputs_with_catalog(
        &[ConfiguredSqlInput::new(
            "direct",
            &SqlInput::inline(SQL),
            "postgresql",
            &dialect,
        )],
        &catalog,
    )
    .unwrap();
    let witness = |bundle: &sql_semantic_protocol::AnalysisBundle| {
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
    let from_dbt = witness(&dbt);
    let from_sql = witness(&direct);
    assert_eq!(from_dbt.condition(), from_sql.condition());
    assert_eq!(from_dbt.qualifying(), from_sql.qualifying());
    assert_eq!(from_dbt.rejected(), from_sql.rejected());
    assert!(matches!(
        from_dbt.qualifying(),
        BooleanWitnessDirection::Impossible
    ));
}
