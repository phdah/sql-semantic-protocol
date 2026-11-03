//! TASK-166: one complete physical source shared by transparent and filtered terminals.
//! The SQL and DuckDB snapshots are independent of the proof implementation.

use duckdb::Connection;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_joint_distribution_plan,
    ConfiguredSqlInput, ConstraintValue, OutcomeGoal, OutcomeWitness, OutputDistribution,
    OutputValueCount, RelationCatalog, RelationSchema, SchemaColumn, SqlInput, WitnessDirection,
    WitnessObligation,
};

mod common;
use common::DIALECTS;

fn analyze(
    statements: &[&str],
    dialect: &str,
    typed: bool,
) -> sql_semantic_protocol::AnalysisBundle {
    let schemas = if typed {
        vec![RelationSchema::new(
            "t",
            vec![SchemaColumn::from_sql_type("k", "INTEGER", "postgresql").unwrap()],
        )
        .unwrap()]
    } else {
        Vec::new()
    };
    let catalog = RelationCatalog::from_schemas(&schemas).unwrap();
    let dialect_impl = dialect_from_name(dialect).unwrap();
    let sources = statements
        .iter()
        .map(|sql| SqlInput::inline(*sql))
        .collect::<Vec<_>>();
    let ids = (0..sources.len())
        .map(|n| format!("task-166-{n}"))
        .collect::<Vec<_>>();
    let configured = sources
        .iter()
        .zip(&ids)
        .map(|(source, id)| ConfiguredSqlInput::new(id, source, dialect, dialect_impl.as_ref()))
        .collect::<Vec<_>>();
    analyze_configured_inputs_with_catalog(&configured, &catalog).unwrap()
}

fn histogram(column: &str, items: &[(ConstraintValue, u64)]) -> OutputDistribution {
    OutputDistribution::new(
        column,
        items
            .iter()
            .map(|(value, count)| OutputValueCount::new(value.clone(), *count))
            .collect(),
    )
    .unwrap()
}

fn goals(
    bundle: &sql_semantic_protocol::AnalysisBundle,
    whole: &[(ConstraintValue, u64)],
    filtered_rows: u64,
    filtered: Option<&[(ConstraintValue, u64)]>,
) -> Vec<OutcomeGoal> {
    vec![
        OutcomeGoal::new(
            bundle.layers()[0].id(),
            Some(whole.iter().map(|(_, n)| *n).sum()),
            None,
            vec![histogram("whole", whole)],
        )
        .unwrap(),
        OutcomeGoal::new(
            bundle.layers()[1].id(),
            Some(filtered_rows),
            None,
            filtered
                .map(|values| vec![histogram("filtered", values)])
                .unwrap_or_default(),
        )
        .unwrap(),
    ]
}

fn expected_sql() -> [&'static str; 2] {
    [
        "SELECT k AS whole FROM t",
        "SELECT k AS filtered FROM t WHERE k = 1",
    ]
}

#[test]
fn complete_shared_bag_proves_two_terminals_and_is_order_independent_for_13_dialects() {
    for &dialect in DIALECTS {
        let bundle = analyze(&expected_sql(), dialect, true);
        let targets = goals(
            &bundle,
            &[
                (ConstraintValue::Integer(1), 2),
                (ConstraintValue::Integer(2), 1),
            ],
            2,
            Some(&[(ConstraintValue::Integer(1), 2)]),
        );
        let witness = physical_joint_distribution_plan(&bundle, &targets);
        let WitnessDirection::Feasible(cases) = &witness else {
            panic!("{dialect}: expected a joint complete-bag proof, got {witness:?}");
        };
        assert_eq!(cases.len(), 1);
        let obligations = cases[0].obligations();
        let source = obligations
            .iter()
            .filter_map(|obligation| {
                if let WitnessObligation::OperatorPopulation {
                    witness:
                        OutcomeWitness::SourceRows {
                            relation,
                            rows,
                            columns,
                        },
                } = obligation
                {
                    Some((relation, rows, columns))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(source.len(), 1, "{dialect}");
        assert_eq!((source[0].0.as_str(), *source[0].1), ("t", 3));
        assert_eq!(source[0].2[0].column(), "k");
        assert_eq!(
            source[0].2[0].values(),
            targets[0].distributions()[0].values()
        );
        assert_eq!(
            obligations
                .iter()
                .filter(|item| matches!(item, WitnessObligation::OutputRows { .. }))
                .count(),
            2
        );
        let reversed = targets.iter().rev().cloned().collect::<Vec<_>>();
        assert_eq!(
            physical_joint_distribution_plan(&bundle, &reversed),
            witness
        );
    }
}

#[test]
fn subset_cannot_exceed_the_exact_whole_source_or_contradict_filter_histogram() {
    for &dialect in DIALECTS {
        let bundle = analyze(&expected_sql(), dialect, true);
        let whole = [
            (ConstraintValue::Integer(1), 2),
            (ConstraintValue::Integer(2), 1),
        ];
        assert_eq!(
            physical_joint_distribution_plan(&bundle, &goals(&bundle, &whole, 4, None)),
            WitnessDirection::Impossible,
            "{dialect}: filtered count > source count"
        );
        assert_eq!(
            physical_joint_distribution_plan(
                &bundle,
                &goals(
                    &bundle,
                    &whole,
                    2,
                    Some(&[(ConstraintValue::Integer(2), 2)])
                )
            ),
            WitnessDirection::Impossible,
            "{dialect}: WHERE k=1 never emits k=2"
        );
    }
}

#[test]
fn nullable_source_requires_sql_true_and_not_true_membership() {
    for &dialect in DIALECTS {
        let b = analyze(
            &[
                "SELECT k AS whole FROM t",
                "SELECT k AS filtered FROM t WHERE k IS NULL",
            ],
            dialect,
            true,
        );
        let targets = goals(
            &b,
            &[(ConstraintValue::Integer(1), 2), (ConstraintValue::Null, 1)],
            1,
            Some(&[(ConstraintValue::Null, 1)]),
        );
        assert!(
            matches!(
                physical_joint_distribution_plan(&b, &targets),
                WitnessDirection::Feasible(_)
            ),
            "{dialect}: SQL NULL is selected only by IS NULL"
        );
    }
}

#[test]
fn opaque_filter_and_untyped_source_remain_residual() {
    let b = analyze(
        &[
            "SELECT k AS whole FROM t",
            "SELECT k AS filtered FROM t WHERE k = opaque(k)",
        ],
        "postgresql",
        true,
    );
    let targets = goals(
        &b,
        &[
            (ConstraintValue::Integer(1), 2),
            (ConstraintValue::Integer(2), 1),
        ],
        2,
        None,
    );
    assert!(matches!(
        physical_joint_distribution_plan(&b, &targets),
        WitnessDirection::Residual { .. }
    ));

    let untyped = analyze(&expected_sql(), "postgresql", false);
    let targets = goals(
        &untyped,
        &[
            (ConstraintValue::Integer(1), 2),
            (ConstraintValue::Integer(2), 1),
        ],
        2,
        None,
    );
    assert!(matches!(
        physical_joint_distribution_plan(&untyped, &targets),
        WitnessDirection::Residual { .. }
    ));
}

#[test]
fn computed_whole_projection_does_not_extend_the_frozen_shared_source_law() {
    let bundle = analyze(
        &[
            "SELECT k + 1 AS whole FROM t",
            "SELECT k AS filtered FROM t WHERE k = 1",
        ],
        "postgresql",
        true,
    );
    let targets = goals(
        &bundle,
        &[
            (ConstraintValue::Integer(2), 2),
            (ConstraintValue::Integer(3), 1),
        ],
        2,
        None,
    );
    assert!(
        matches!(
            physical_joint_distribution_plan(&bundle, &targets),
            WitnessDirection::Residual { .. }
        ),
        "a reversible scalar whole projection is not the frozen transparent source case"
    );
}

#[test]
fn duckdb_observes_exact_full_source_and_both_terminal_bags() {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE t(k INTEGER); INSERT INTO t VALUES (1), (1), (2);")
        .unwrap();
    for (sql, expected) in [
        (
            "SELECT k FROM t ORDER BY k",
            vec![Some(1_i64), Some(1), Some(2)],
        ),
        (
            "SELECT k FROM t WHERE k = 1 ORDER BY k",
            vec![Some(1_i64), Some(1)],
        ),
    ] {
        let mut stmt = db.prepare(sql).unwrap();
        let actual = stmt
            .query_map([], |row| row.get::<_, Option<i64>>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(actual, expected);
    }
    db.execute_batch("DELETE FROM t; INSERT INTO t VALUES (1), (1), (NULL);")
        .unwrap();
    let mut stmt = db.prepare("SELECT k FROM t WHERE k IS NULL").unwrap();
    let actual = stmt
        .query_map([], |row| row.get::<_, Option<i64>>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(actual, vec![None]);
}
