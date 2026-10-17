//! Temporal range comparisons: complete closed physical wall-clock bags.
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_joint_source_plan,
    ConfiguredSqlInput, RelationCatalog, RelationSchema, SchemaColumn, SqlInput, TemporalJoinLaw,
    TimestampZone, WitnessDirection, WitnessObligation,
};

fn physical_plan(left_type: &str, right_type: &str, wanted: u64) -> WitnessDirection {
    let parser = dialect_from_name("generic").unwrap();
    let schemas = [("l", left_type), ("r", right_type)]
        .iter()
        .map(|(name, sql_type)| {
            RelationSchema::new(
                *name,
                vec![
                    SchemaColumn::from_sql_type("t", sql_type, "postgresql").unwrap(),
                ],
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let catalog = RelationCatalog::from_schemas(&schemas).unwrap();
    let sql = SqlInput::inline("SELECT l.t, r.t FROM l JOIN r ON l.t >= r.t");
    let configured = [ConfiguredSqlInput::new("time", &sql, "generic", parser.as_ref())];
    let bundle = analyze_configured_inputs_with_catalog(&configured, &catalog).unwrap();
    let layer = bundle.layers().last().unwrap().id();
    physical_joint_source_plan(&bundle, &[(layer, wanted)]).outcome().clone()
}

#[test]
fn frozen_timestamp_bags_are_source_backed_and_rejected_pairs_are_exact() {
    let WitnessDirection::Feasible(cases) = physical_plan(
        "TIMESTAMP WITHOUT TIME ZONE",
        "TIMESTAMP WITHOUT TIME ZONE",
        3,
    ) else {
        panic!("attested wall-clock comparison must admit complete physical bag");
    };
    let frozen_left = vec![
        Some("2026-10-17 00:05:00".to_string()),
        Some("2026-10-17 00:10:00".to_string()),
    ];
    let frozen_right = vec![
        Some("2026-10-17 00:04:00".to_string()),
        Some("2026-10-17 00:07:00".to_string()),
    ];
    assert!(cases.iter().any(|case| {
        let closed = case.obligations().iter().filter(|ob| matches!(ob, WitnessObligation::ClosedWorld { .. })).count();
        closed == 2 && case.obligations().iter().any(|ob| matches!(ob, WitnessObligation::TemporalJoinPopulation {
            law: TemporalJoinLaw::GreaterThanOrEqual,
            left_values,
            right_values,
            output_rows: 3,
            closed_world: true,
            ..
        } if left_values == &frozen_left
            && right_values == &frozen_right
            && TemporalJoinLaw::GreaterThanOrEqual.selected_rows(left_values,right_values)
                == Some(vec![(0,0),(1,0),(1,1)])
            && TemporalJoinLaw::GreaterThanOrEqual.unselected_pairs(left_values,right_values)
                == Some(vec![(0,1)])))
    }));
}

#[test]
fn absent_session_timezone_evidence_refuses_precise_temporal_population() {
    for (a,b) in [
        ("TIMESTAMP", "TIMESTAMP"),
        ("TIMESTAMP WITH TIME ZONE", "TIMESTAMP WITH TIME ZONE"),
        ("TIMESTAMP WITHOUT TIME ZONE", "TIMESTAMP"),
    ] {
        let direction = physical_plan(a,b,3);
        assert!(!matches!(direction, WitnessDirection::Feasible(ref cases)
            if cases.iter().any(|case| case.obligations().iter().any(|ob| matches!(ob, WitnessObligation::TemporalJoinPopulation{..})))));
    }
    assert_eq!(TemporalJoinLaw::GreaterThanOrEqual.selected_rows(
        &[Some("2026-02-30 00:00:00".to_string())],
        &[Some("2026-02-01 00:00:00".to_string())]
    ), None);
}

#[test]
fn timestamp_source_type_attestation_is_explicit() {
    let column = SchemaColumn::from_sql_type("t","TIMESTAMP WITHOUT TIME ZONE","postgresql").unwrap();
    assert_eq!(column.timestamp_zone(),Some(TimestampZone::WithoutTimeZone));
}
