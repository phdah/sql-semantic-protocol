mod common;

use common::DIALECTS;
use duckdb::Connection;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_inputs, analyze_sql, to_json,
    BooleanRowConstraint, BooleanTruthCase, BooleanWitnessDirection, ComposedSemantics,
    ConfiguredSqlInput, ProtocolStatement, RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
};
use sqlparser::dialect::{dialect_from_str, GenericDialect, PostgreSqlDialect};

fn query(sql: &str) -> sql_semantic_protocol::QueryStatement {
    let protocol = analyze_sql(sql, "generic", &GenericDialect {}).unwrap();
    let Some(ProtocolStatement::Query(query)) = protocol.statements().first() else {
        panic!("expected query statement");
    };
    query.clone()
}

fn typed_bundle(sql: &str) -> sql_semantic_protocol::AnalysisBundle {
    let schema = RelationSchema::new(
        "t",
        vec![
            SchemaColumn::from_sql_type("a", "INTEGER", "postgresql").unwrap(),
            SchemaColumn::from_sql_type("b", "INTEGER", "postgresql").unwrap(),
        ],
    )
    .unwrap();
    let catalog = RelationCatalog::from_schemas(&[schema]).unwrap();
    let input = SqlInput::inline(sql);
    let dialect = PostgreSqlDialect {};
    let configured = [ConfiguredSqlInput::new(
        "typed",
        &input,
        "postgresql",
        &dialect,
    )];
    analyze_configured_inputs_with_catalog(&configured, &catalog).unwrap()
}

#[test]
fn null_disjunction_has_jointly_evaluated_exact_truth_directions() {
    let query = query("SELECT a FROM t WHERE a IS NULL OR b IS NULL");
    let witness = query.boolean_witness().expect("coupled witness");
    assert_eq!(witness.source_relation(), "t");
    assert!(matches!(
        witness.qualifying(),
        BooleanWitnessDirection::Exact(BooleanTruthCase::True)
    ));
    assert!(matches!(
        witness.rejected(),
        BooleanWitnessDirection::Exact(BooleanTruthCase::NotTrue)
    ));
    let BooleanRowConstraint::Any(branches) = witness.condition() else {
        panic!("source predicate must preserve OR");
    };
    assert_eq!(branches.len(), 2);
    assert!(branches.iter().all(BooleanRowConstraint::is_exact));
    // Independent scalar domains do not establish an exact OR across two columns.
    assert!(!query.condition_exactness().is_exact());
}

#[test]
fn untyped_integer_conditions_and_computed_branches_default_to_residual() {
    for sql in [
        "SELECT a FROM t WHERE a > 2 OR b < 0",
        "SELECT a FROM t WHERE a IS NULL OR b IS NULL OR CAST(c AS INT) > 0",
        "SELECT a FROM t WHERE a IS NULL OR b IS NULL OR c LIKE 'x%'",
        "SELECT a FROM t WHERE a IS NULL OR b IS NULL OR abs(c) = 3",
    ] {
        let query = query(sql);
        let witness = query.boolean_witness().expect("correlation evidence");
        assert!(
            matches!(
                witness.qualifying(),
                BooleanWitnessDirection::Residual { .. }
            ),
            "{sql}"
        );
        assert!(
            matches!(witness.rejected(), BooleanWitnessDirection::Residual { .. }),
            "{sql}"
        );
    }
}

#[test]
fn typed_integer_disjunction_retains_comparison_operators_without_cross_product() {
    let bundle = typed_bundle("SELECT a FROM t WHERE a > 2 OR b < 0");
    let ComposedSemantics::Resolved(semantics) = bundle.layers()[0].composed_semantics() else {
        panic!("expected composition");
    };
    let item = &semantics.boolean_witnesses()[0];
    let witness = item.witness();
    assert!(matches!(
        witness.qualifying(),
        BooleanWitnessDirection::Exact(BooleanTruthCase::True)
    ));
    assert!(matches!(
        witness.rejected(),
        BooleanWitnessDirection::Exact(BooleanTruthCase::NotTrue)
    ));
    let BooleanRowConstraint::Any(operands) = witness.condition() else {
        panic!("expected correlated disjunction");
    };
    assert!(matches!(
        operands[0],
        BooleanRowConstraint::IntegerComparison { literal: 2, .. }
    ));
    assert!(matches!(
        operands[1],
        BooleanRowConstraint::IntegerComparison { literal: 0, .. }
    ));
    assert_ne!(operands[0], operands[1]);
}

#[test]
fn source_witness_is_emitted_locally_and_retains_origin_through_composition() {
    let sql = "CREATE TABLE selected AS SELECT a FROM t WHERE a IS NULL OR b IS NULL";
    let bundle = analyze_inputs(
        &[
            SqlInput::inline(sql),
            SqlInput::inline("CREATE TABLE downstream AS SELECT a FROM selected"),
        ],
        "generic",
        &GenericDialect {},
    )
    .unwrap();
    let downstream = bundle
        .layers()
        .iter()
        .find(|layer| {
            layer
                .produces()
                .iter()
                .any(|output| output.relation_name() == Some("downstream"))
        })
        .unwrap();
    let ComposedSemantics::Resolved(composed) = downstream.composed_semantics() else {
        panic!("downstream composition should resolve");
    };
    assert_eq!(composed.boolean_witnesses().len(), 1);
    assert_eq!(
        composed.boolean_witnesses()[0].witness().source_relation(),
        "t"
    );
    assert_ne!(
        composed.boolean_witnesses()[0].origin_layer_id(),
        downstream.id()
    );

    let protocol = analyze_sql(sql, "generic", &GenericDialect {}).unwrap();
    let value: serde_json::Value = serde_json::from_str(&to_json(&protocol)).unwrap();
    let witness = &value["inputs"][0]["statements"][0]["boolean_witness"];
    assert_eq!(witness["condition"]["kind"], "any");
    assert_eq!(witness["qualifying"]["truth"], "true");
    assert_eq!(witness["rejected"]["truth"], "not_true");
    assert_eq!(
        witness["condition"]["operands"][0]["column"]["relation"],
        "t"
    );
}

#[test]
fn dialects_preserve_the_same_null_sensitive_source_tree() {
    let sql = "SELECT a FROM t WHERE a IS NULL OR b IS NOT NULL";
    for name in DIALECTS {
        let dialect = dialect_from_str(name).expect("dialect");
        let protocol = analyze_sql(sql, name, dialect.as_ref()).expect("shared syntax");
        let Some(ProtocolStatement::Query(query)) = protocol.statements().first() else {
            panic!("expected query in {name}");
        };
        let witness = query.boolean_witness().expect("witness for every dialect");
        assert!(
            matches!(witness.qualifying(), BooleanWitnessDirection::Exact(_)),
            "{name}"
        );
        assert!(
            matches!(witness.rejected(), BooleanWitnessDirection::Exact(_)),
            "{name}"
        );
    }
}

#[test]
fn duckdb_confirms_both_three_valued_directions_of_coupled_conditions() {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE t(a INTEGER, b INTEGER);
         INSERT INTO t VALUES (NULL,1), (1,NULL), (NULL,NULL), (1,1);
         INSERT INTO t VALUES (3,NULL), (NULL,-1);",
    )
    .unwrap();
    let cases = [
        ("a IS NULL OR b IS NULL", 5_i64, 1_i64),
        ("a > 2 OR b < 0", 2_i64, 4_i64),
    ];
    for (predicate, qualifying, rejected) in cases {
        let selected: i64 = db
            .query_row(
                &format!("SELECT COUNT(*) FROM t WHERE {predicate}"),
                [],
                |row| row.get(0),
            )
            .unwrap();
        let excluded: i64 = db
            .query_row(
                &format!("SELECT COUNT(*) FROM t WHERE ({predicate}) IS NOT TRUE"),
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(selected, qualifying, "{predicate}");
        assert_eq!(excluded, rejected, "{predicate}");
    }
}
