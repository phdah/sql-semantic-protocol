//! Closed-world temporal and correlated join proofs through the public API.
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_joint_source_plan,
    ConfiguredSqlInput, FiniteJoinLaw, JoinExecution, ProtocolStatement, RelationCatalog,
    RelationSchema, SchemaColumn, SqlInput, WitnessDirection, WitnessObligation,
};

fn bundle_for(sql: &str, dialect: &str) -> sql_semantic_protocol::AnalysisBundle {
    let schemas = ["l", "r"]
        .iter()
        .map(|name| {
            RelationSchema::new(
                *name,
                vec![
                    SchemaColumn::from_sql_type("id", "VARCHAR", "postgresql").unwrap(),
                    SchemaColumn::from_sql_type("t", "INTEGER", "postgresql").unwrap(),
                ],
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let catalog = RelationCatalog::from_schemas(&schemas).unwrap();
    let source = SqlInput::inline(sql);
    let parser = dialect_from_name(dialect).unwrap();
    let input = ConfiguredSqlInput::new("task114", &source, dialect, parser.as_ref());
    analyze_configured_inputs_with_catalog(&[input], &catalog).unwrap()
}

#[test]
fn source_backed_asof_and_correlated_operators_have_exact_finite_bags() {
    let cases = [
        (
            "asof",
            "SELECT l.id, l.t, r.id, r.t FROM l ASOF JOIN r MATCH_CONDITION(l.t >= r.t)",
            "snowflake",
            JoinExecution::AsOfNearest,
            "asof_nearest_prior_inclusive",
        ),
        (
            "lateral",
            "SELECT l.id, q.id FROM l CROSS JOIN LATERAL (SELECT r.id FROM r WHERE r.t = l.t) AS q",
            "generic",
            JoinExecution::LateralInner,
            "correlated_inner_equality",
        ),
        (
            "cross_apply",
            "SELECT l.id, q.id FROM l CROSS APPLY (SELECT r.id FROM r WHERE r.t = l.t) AS q",
            "mssql",
            JoinExecution::CrossApply,
            "correlated_inner_equality",
        ),
        (
            "outer_apply",
            "SELECT l.id, q.id FROM l OUTER APPLY (SELECT r.id FROM r WHERE r.t = l.t) AS q",
            "mssql",
            JoinExecution::OuterApply,
            "correlated_outer_equality",
        ),
    ];
    for (variant, sql, dialect, execution, expected_law) in cases {
        let bundle = bundle_for(sql, dialect);
        let query = bundle.inputs()[0]
            .statements()
            .iter()
            .find_map(|statement| match statement {
                ProtocolStatement::Query(query) => Some(query),
                _ => None,
            })
            .unwrap();
        assert_eq!(query.joins().len(), 1, "{variant}");
        assert_eq!(query.joins()[0].execution(), Some(execution), "{variant}");
        let layer = bundle.layers().last().unwrap().id();
        for wanted in [0, 2] {
            let plan = physical_joint_source_plan(&bundle, &[(layer, wanted)]);
            let WitnessDirection::Feasible(cases) = plan.outcome() else {
                panic!("{variant}/{wanted}: no canonical exact plan: {plan:?}");
            };
            assert!(
                cases.iter().any(|case| {
                    let obligations = case.obligations();
                    let count = obligations
                        .iter()
                        .filter(|o| matches!(o, WitnessObligation::ClosedWorld { .. }))
                        .count();
                    count == 2
                        && obligations.iter().any(|obligation| {
                            matches!(obligation, WitnessObligation::FiniteJoinPopulation {
                                law, left_values, right_values, output_rows, closed_world: true, ..
                            } if law.as_str() == expected_law
                                && *output_rows == wanted
                                && law.selected_rows(left_values, right_values)
                                    .is_some_and(|selected| selected.len() as u64 == wanted)
                            )
                        })
                }),
                "{variant}/{wanted}: complete exact source-level witness"
            );
        }
    }
}

#[test]
fn witness_laws_reject_ambiguous_asof_ties_and_distinguish_unselected_true_pairs() {
    let asof = FiniteJoinLaw::AsOfNearestPriorInclusive;
    let left = [Some(5), Some(10)];
    let right = [Some(4), Some(7)];
    assert_eq!(
        asof.selected_rows(&left, &right),
        Some(vec![(0, Some(0)), (1, Some(1))])
    );
    assert_eq!(
        asof.unselected_pairs(&left, &right),
        Some(vec![(0, 1), (1, 0)])
    );
    assert_eq!(asof.selected_rows(&[Some(10)], &[Some(7), Some(7)]), None);
    assert_eq!(
        asof.selected_rows(&[None], &[Some(7)]),
        Some(vec![(0, None)])
    );
    let inner = FiniteJoinLaw::CorrelatedInnerEquality;
    assert_eq!(
        inner.selected_rows(&[Some(1)], &[Some(1), Some(1)]),
        Some(vec![(0, Some(0)), (0, Some(1))])
    );
    let outer = FiniteJoinLaw::CorrelatedOuterEquality;
    assert_eq!(
        outer.selected_rows(&[Some(1), Some(2)], &[Some(1)]),
        Some(vec![(0, Some(0)), (1, None)])
    );
}
