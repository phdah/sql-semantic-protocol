//! TASK-70 bounded Boolean witness laws and SQL three-valued execution oracles.
mod common;

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_artifacts, analyze_sql, dialect_from_name,
    parse_dbt_catalog, parse_dbt_manifest, BooleanRowConstraint,
    BooleanTruthCase, BooleanWitness, BooleanWitnessDirection, ComposedSemantics,
    ConfiguredSqlInput, ProtocolStatement, RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
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
fn unsupported_null_membership_and_cross_column_comparisons_remain_residual() {
    for predicate in [
        "a NOT IN (1, NULL)",
        "a IN (1, NULL)",
        "NOT (a = b)",
        "a BETWEEN b AND 5",
    ] {
        let witness = typed_witness(predicate);
        assert!(
            matches!(
                witness.qualifying(),
                BooleanWitnessDirection::Residual { .. }
            ),
            "{predicate}: unproved semantics cannot be exact"
        );
        assert!(
            matches!(witness.rejected(), BooleanWitnessDirection::Residual { .. }),
            "{predicate}: unproved negative semantics cannot be exact"
        );
    }
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
    let catalog = parse_dbt_catalog(include_str!("fixtures/dbt/catalog-v1.json"))
        .expect("dbt catalog");
    let dialect = PostgreSqlDialect {};
    let dbt = analyze_dbt_artifacts(&manifest, &catalog, "postgresql", &dialect)
        .expect("dbt analysis");

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
        &[ConfiguredSqlInput::new("direct", &input, "postgresql", &dialect)],
        &catalog,
    )
    .expect("direct analysis");

    let witness = |bundle: &sql_semantic_protocol::AnalysisBundle| {
        let layer = bundle
            .layers()
            .iter()
            .find(|layer| layer.consumes().iter().any(|source| source == "warehouse.raw.orders"))
            .expect("source-consuming layer");
        let ComposedSemantics::Resolved(ref semantics) = layer.composed_semantics() else {
            panic!("expected composed semantics");
        };
        semantics.boolean_witnesses().first().expect("boolean witness").witness().clone()
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
