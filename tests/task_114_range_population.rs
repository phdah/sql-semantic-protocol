//! TASK-114: typed complete integer interval comparisons at the physical boundary.
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_joint_source_plan,
    ConfiguredSqlInput, RelationCatalog, RelationSchema, SchemaColumn, SqlInput, WitnessDirection,
    WitnessObligation,
};

#[test]
fn non_strict_and_reversed_range_joins_have_constructive_integer_bags() {
    let schemas = ["l", "r"]
        .iter()
        .map(|relation| {
            RelationSchema::new(
                *relation,
                vec![SchemaColumn::from_sql_type("k", "INTEGER", "postgresql").unwrap()],
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let catalog = RelationCatalog::from_schemas(&schemas).unwrap();
    let dialect = dialect_from_name("generic").unwrap();

    for (predicate, expected_law) in [
        ("l.k < r.k", "less_than"),
        ("l.k > r.k", "greater_than"),
        ("l.k <= r.k", "less_than_or_equal"),
        ("l.k >= r.k", "greater_than_or_equal"),
        ("r.k < l.k", "greater_than"),
        ("r.k >= l.k", "less_than_or_equal"),
    ] {
        let sql = SqlInput::inline(format!(
            "SELECT l.k FROM l JOIN r ON {predicate}"
        ));
        let input = ConfiguredSqlInput::new(
            "task-114-range", &sql, "generic", dialect.as_ref()
        );
        let bundle = analyze_configured_inputs_with_catalog(&[input], &catalog).unwrap();
        let layer = bundle.layers().last().unwrap().id();
        for wanted in [0, 2] {
            let plan = physical_joint_source_plan(&bundle, &[(layer, wanted)]);
            let WitnessDirection::Feasible(candidates) = plan.outcome() else {
                panic!("{predicate} with count {wanted} must be exact: {plan:?}");
            };
            assert!(
                candidates.iter().any(|candidate| {
                    let obligations = candidate.obligations();
                    let closed = obligations.iter().filter(|obligation| {
                        matches!(obligation, WitnessObligation::ClosedWorld { .. })
                    }).count() == 2;
                    closed && obligations.iter().any(|obligation| {
                        matches!(obligation, WitnessObligation::UniformJoinPopulation {
                            law, output_rows, closed_world: true, ..
                        } if law.as_str() == expected_law && *output_rows == wanted)
                    })
                }),
                "{predicate}: complete physical source bag for output count {wanted}"
            );
        }
    }
}
