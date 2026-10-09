mod common;

use common::DIALECTS;
use duckdb::Connection;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_inputs, analyze_sql, to_json,
    BooleanRowConstraint, BooleanTruthCase, BooleanWitnessDirection, ComparisonOperator,
    ComposedSemantics, ConfiguredSqlInput, ProtocolStatement, RelationCatalog, RelationSchema,
    SchemaColumn, SqlInput,
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
    // Either disjunct can select a row with any value in the other column.
    // The independent output domain must not be narrowed to NULL.
    assert!(matches!(
        query.output().columns()[0].domain(),
        sql_semantic_protocol::ValueDomain::Unbounded
            | sql_semantic_protocol::ValueDomain::Unknown(_)
    ));
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
fn impossible_positive_direction_is_residual_even_with_known_integer_types() {
    let bundle = typed_bundle("SELECT a FROM t WHERE a > 2147483647 OR b > 2147483647");
    let ComposedSemantics::Resolved(semantics) = bundle.layers()[0].composed_semantics() else {
        panic!("expected composition");
    };
    let witness = semantics.boolean_witnesses()[0].witness();
    assert!(matches!(
        witness.qualifying(),
        BooleanWitnessDirection::Residual { .. }
    ));
    assert!(matches!(
        witness.rejected(),
        BooleanWitnessDirection::Exact(BooleanTruthCase::NotTrue)
    ));
}

#[test]
fn signed_integer_literals_are_proven_without_string_based_reparsing() {
    let bundle = typed_bundle("SELECT a FROM t WHERE a > -2 OR b < +3");
    let ComposedSemantics::Resolved(semantics) = bundle.layers()[0].composed_semantics() else {
        panic!("expected composition");
    };
    let witness = semantics.boolean_witnesses()[0].witness();
    assert!(matches!(
        witness.qualifying(),
        BooleanWitnessDirection::Exact(BooleanTruthCase::True)
    ));
    let BooleanRowConstraint::Any(operands) = witness.condition() else {
        panic!("expected disjunction");
    };
    assert!(matches!(
        operands[0],
        BooleanRowConstraint::IntegerComparison { literal: -2, .. }
    ));
    assert!(matches!(
        operands[1],
        BooleanRowConstraint::IntegerComparison { literal: 3, .. }
    ));
}

fn sql_and(left: Option<bool>, right: Option<bool>) -> Option<bool> {
    match (left, right) {
        (Some(false), _) | (_, Some(false)) => Some(false),
        (Some(true), Some(true)) => Some(true),
        _ => None,
    }
}

fn sql_or(left: Option<bool>, right: Option<bool>) -> Option<bool> {
    match (left, right) {
        (Some(true), _) | (_, Some(true)) => Some(true),
        (Some(false), Some(false)) => Some(false),
        _ => None,
    }
}

fn witness_truth(
    constraint: &BooleanRowConstraint,
    a: Option<i32>,
    b: Option<i32>,
) -> Option<bool> {
    match constraint {
        BooleanRowConstraint::All(operands) => {
            operands.iter().fold(Some(true), |previous, item| {
                sql_and(previous, witness_truth(item, a, b))
            })
        }
        BooleanRowConstraint::Any(operands) => {
            operands.iter().fold(Some(false), |previous, item| {
                sql_or(previous, witness_truth(item, a, b))
            })
        }
        BooleanRowConstraint::NullTest { column, negated } => {
            let value = match column.name() {
                "a" => a,
                "b" => b,
                other => panic!("unexpected source column {other}"),
            };
            Some(value.is_none() != *negated)
        }
        BooleanRowConstraint::IntegerComparison {
            column,
            operator,
            literal,
        } => {
            let value = match column.name() {
                "a" => a,
                "b" => b,
                other => panic!("unexpected source column {other}"),
            };
            value.map(|v| {
                let v = i64::from(v);
                match operator {
                    ComparisonOperator::Eq => v == *literal,
                    ComparisonOperator::Neq => v != *literal,
                    ComparisonOperator::Lt => v < *literal,
                    ComparisonOperator::Lte => v <= *literal,
                    ComparisonOperator::Gt => v > *literal,
                    ComparisonOperator::Gte => v >= *literal,
                    ComparisonOperator::IsDistinctFrom | ComparisonOperator::IsNotDistinctFrom => {
                        panic!("null-safe comparisons must remain residual")
                    }
                }
            })
        }
        BooleanRowConstraint::Residual { reason } => {
            panic!("differential fixture cannot evaluate residual: {reason}")
        }
    }
}

#[test]
fn duckdb_differential_matches_generated_witness_for_every_source_row() {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE t(a INTEGER, b INTEGER);
         INSERT INTO t VALUES (NULL,1), (1,NULL), (NULL,NULL), (1,1);
         INSERT INTO t VALUES (3,NULL), (NULL,-1), (2147483647,-2147483648);",
    )
    .unwrap();
    for predicate in [
        "a IS NULL OR b IS NULL",
        "a > 2 OR b < 0",
        "a > -2 OR b <= 1",
    ] {
        let bundle = typed_bundle(&format!("SELECT a FROM t WHERE {predicate}"));
        let ComposedSemantics::Resolved(semantics) = bundle.layers()[0].composed_semantics() else {
            panic!("expected resolved composed semantics");
        };
        let witness = semantics.boolean_witnesses()[0].witness();
        assert!(matches!(
            witness.qualifying(),
            BooleanWitnessDirection::Exact(_)
        ));

        let mut statement = db
            .prepare(&format!("SELECT a, b, ({predicate}) FROM t"))
            .unwrap();
        let actual = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, Option<i32>>(0)?,
                    row.get::<_, Option<i32>>(1)?,
                    row.get::<_, Option<bool>>(2)?,
                ))
            })
            .unwrap();
        for row in actual {
            let (a, b, sql_result) = row.unwrap();
            let computed = witness_truth(witness.condition(), a, b);
            assert_eq!(
                computed, sql_result,
                "witness differs from DuckDB for {predicate} at ({a:?}, {b:?})"
            );
            assert_eq!(computed == Some(true), sql_result == Some(true));
            assert_eq!(computed != Some(true), sql_result != Some(true));
        }
    }
}
