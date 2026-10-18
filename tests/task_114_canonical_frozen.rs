//! TASK-114: exact canonical physical source assignments independently
//! reproduce the immutable positive/NOT TRUE row identity bags.
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_joint_source_plan,
    ConfiguredSqlInput, RelationCatalog, RelationSchema, SchemaColumn, SqlInput, WitnessDirection,
    WitnessObligation,
};

const MATRIX: &str = include_str!("../docs/task-114-acceptance-freeze.json");

fn pairs(rows: &Value, right_column: usize) -> Vec<String> {
    let mut out = rows
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            let cells = row.as_array().unwrap();
            Value::Array(vec![cells[0].clone(), cells[right_column].clone()]).to_string()
        })
        .collect::<Vec<_>>();
    out.sort();
    out
}

fn observed_values(values: &[Option<i64>]) -> Vec<Value> {
    values
        .iter()
        .map(|value| value.map_or(Value::Null, |n| Value::Number(n.into())))
        .collect()
}

#[test]
fn asof_and_correlated_frozen_bags_are_exactly_generated_by_canonical_laws() {
    let matrix: Value = serde_json::from_str(MATRIX).unwrap();
    let mut verified = 0;
    for case in matrix["cases"].as_array().unwrap() {
        let variant = case["variant"].as_str().unwrap();
        if variant == "time_inequality" {
            continue;
        }
        let dialect = match variant {
            "asof_nearest_prior" => "snowflake",
            "range_inequality" => "generic",
            "lateral_derived" => "generic",
            "cross_apply" | "outer_apply" => "mssql",
            _ => panic!("unregistered frozen variant {variant}"),
        };
        let schemas = ["l", "r"]
            .iter()
            .map(|name| {
                RelationSchema::new(
                    *name,
                    vec![
                        SchemaColumn::from_sql_type("id", "VARCHAR", dialect).unwrap(),
                        SchemaColumn::from_sql_type("t", "INTEGER", dialect).unwrap(),
                    ],
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        let catalog = RelationCatalog::from_schemas(&schemas).unwrap();
        let sql = SqlInput::inline(case["sql"].as_str().unwrap());
        let parser = dialect_from_name(dialect).unwrap();
        let input = ConfiguredSqlInput::new("frozen", &sql, dialect, parser.as_ref());
        let bundle = analyze_configured_inputs_with_catalog(&[input], &catalog).unwrap();
        let layer = bundle.layers().last().unwrap().id();
        let expected_rows = case["expected"].as_array().unwrap().len() as u64;
        let plan = physical_joint_source_plan(&bundle, &[(layer, expected_rows)]);
        let WitnessDirection::Feasible(cases) = plan.outcome() else {
            panic!("{variant}: no complete source proof {plan:?}");
        };
        let frozen_left = case["left"].as_array().unwrap();
        let frozen_right = case["right"].as_array().unwrap();
        let left_values = frozen_left
            .iter()
            .map(|r| r[1].as_i64().map(Some).unwrap_or(None))
            .collect::<Vec<_>>();
        let right_values = frozen_right
            .iter()
            .map(|r| r[1].as_i64().map(Some).unwrap_or(None))
            .collect::<Vec<_>>();
        let proved = cases
            .iter()
            .find_map(|case| {
                case.obligations().iter().find_map(|obligation| {
                    if let WitnessObligation::FiniteJoinPopulation {
                        law,
                        left_values: left,
                        right_values: right,
                        closed_world: true,
                        output_rows,
                        ..
                    } = obligation
                    {
                        (left == &left_values
                            && right == &right_values
                            && *output_rows == expected_rows)
                            .then_some(*law)
                    } else {
                        None
                    }
                })
            })
            .unwrap_or_else(|| panic!("{variant}: missing exact frozen source population"));
        let selected = proved.selected_rows(&left_values, &right_values).unwrap();
        let rejected = proved
            .unselected_pairs(&left_values, &right_values)
            .unwrap();
        let mut observed_selected = selected
            .iter()
            .map(|(l, r)| {
                Value::Array(vec![
                    frozen_left[*l][0].clone(),
                    r.map_or(Value::Null, |i| frozen_right[i][0].clone()),
                ])
                .to_string()
            })
            .collect::<Vec<_>>();
        observed_selected.sort();
        let right_column = if matches!(variant, "asof_nearest_prior" | "range_inequality") {
            2
        } else {
            1
        };
        assert_eq!(
            observed_selected,
            pairs(&case["expected"], right_column),
            "{variant}: exactly the full winning/NULL output bag"
        );
        let mut observed_rejected = rejected
            .iter()
            .map(|(l, r)| {
                Value::Array(vec![
                    frozen_left[*l][0].clone(),
                    frozen_right[*r][0].clone(),
                ])
                .to_string()
            })
            .collect::<Vec<_>>();
        observed_rejected.sort();
        assert_eq!(
            observed_rejected,
            pairs(&case["not_true"], 1),
            "{variant}: entire unselected Cartesian complement"
        );
        assert_eq!(
            observed_values(&left_values),
            frozen_left.iter().map(|r| r[1].clone()).collect::<Vec<_>>()
        );
        assert_eq!(
            observed_values(&right_values),
            frozen_right
                .iter()
                .map(|r| r[1].clone())
                .collect::<Vec<_>>()
        );
        verified += 1;
    }
    assert_eq!(
        verified, 10,
        "both P and N directions for five physical numeric laws"
    );
}
