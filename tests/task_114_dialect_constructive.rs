//! TASK-114: every parser-recognized frozen cell is linked to a source-backed
//! canonical finite population, never merely to successful AST classification.
//! These assertions do not certify native vendor runtime equivalence.

use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_joint_source_plan,
    ConfiguredSqlInput, RelationCatalog, RelationSchema, SchemaColumn, SqlInput, WitnessDirection,
    WitnessObligation,
};

const MATRIX: &str = include_str!("../docs/task-114-acceptance-freeze.json");

#[test]
fn every_parsed_frozen_dialect_cell_has_the_expected_canonical_source_bag() {
    let matrix: Value = serde_json::from_str(MATRIX).unwrap();
    let mut proven = 0;
    let mut boundaries = 0;
    for case in matrix["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        let variant = case["variant"].as_str().unwrap();
        let is_temporal = variant == "time_inequality";
        let left = case["left"].as_array().unwrap();
        let right = case["right"].as_array().unwrap();
        let expected_rows = case["expected"].as_array().unwrap().len() as u64;

        for dialect in matrix["dialects"].as_array().unwrap() {
            let dialect = dialect.as_str().unwrap();
            if case["dialects"][dialect]["parser"] == "parser_boundary" {
                boundaries += 1;
                continue;
            }
            let schemas = ["l", "r"]
                .iter()
                .map(|name| {
                    RelationSchema::new(
                        *name,
                        vec![
                            SchemaColumn::from_sql_type("id", "VARCHAR", "postgresql").unwrap(),
                            SchemaColumn::from_sql_type(
                                "t",
                                if is_temporal {
                                    "TIMESTAMP WITHOUT TIME ZONE"
                                } else {
                                    "INTEGER"
                                },
                                "postgresql",
                            )
                            .unwrap(),
                        ],
                    )
                    .unwrap()
                })
                .collect::<Vec<_>>();
            let catalog = RelationCatalog::from_schemas(&schemas).unwrap();
            let sql = SqlInput::inline(case["sql"].as_str().unwrap());
            let parser = dialect_from_name(dialect).unwrap();
            let input = ConfiguredSqlInput::new(id, &sql, dialect, parser.as_ref());
            let bundle = analyze_configured_inputs_with_catalog(&[input], &catalog).unwrap();
            let layer = bundle.layers().last().unwrap().id();
            let plan = physical_joint_source_plan(&bundle, &[(layer, expected_rows)]);
            let WitnessDirection::Feasible(candidates) = plan.outcome() else {
                panic!("{id}/{dialect}: parsed SQL lacks complete canonical proof: {plan:?}");
            };
            let numeric_left = left.iter().map(|row| row[1].as_i64()).collect::<Vec<_>>();
            let numeric_right = right.iter().map(|row| row[1].as_i64()).collect::<Vec<_>>();
            let temporal_left = left
                .iter()
                .map(|row| row[1].as_str().map(ToString::to_string))
                .collect::<Vec<_>>();
            let temporal_right = right
                .iter()
                .map(|row| row[1].as_str().map(ToString::to_string))
                .collect::<Vec<_>>();
            let found = candidates.iter().any(|candidate| {
                let has_two_closed_sources = candidate
                    .obligations()
                    .iter()
                    .filter(|obligation| {
                        matches!(obligation, WitnessObligation::ClosedWorld { .. })
                    })
                    .count()
                    == 2;
                has_two_closed_sources
                    && candidate
                        .obligations()
                        .iter()
                        .any(|obligation| match obligation {
                            WitnessObligation::FiniteJoinPopulation {
                                law,
                                left_values,
                                right_values,
                                output_rows,
                                closed_world: true,
                                ..
                            } if !is_temporal => {
                                left_values == &numeric_left
                                    && right_values == &numeric_right
                                    && *output_rows == expected_rows
                                    && law
                                        .selected_rows(left_values, right_values)
                                        .is_some_and(|rows| rows.len() as u64 == expected_rows)
                                    && law.unselected_pairs(left_values, right_values).is_some_and(
                                        |pairs| {
                                            pairs.len()
                                                == case["not_true"].as_array().unwrap().len()
                                        },
                                    )
                            }
                            WitnessObligation::TemporalJoinPopulation {
                                law,
                                left_values,
                                right_values,
                                output_rows,
                                closed_world: true,
                                ..
                            } if is_temporal => {
                                left_values == &temporal_left
                                    && right_values == &temporal_right
                                    && *output_rows == expected_rows
                                    && law
                                        .selected_rows(left_values, right_values)
                                        .is_some_and(|rows| rows.len() as u64 == expected_rows)
                                    && law.unselected_pairs(left_values, right_values).is_some_and(
                                        |pairs| {
                                            pairs.len()
                                                == case["not_true"].as_array().unwrap().len()
                                        },
                                    )
                            }
                            _ => false,
                        })
            });
            assert!(
                found,
                "{id}/{dialect}: exact frozen source bag not in canonical proof"
            );
            proven += 1;
        }
    }
    assert_eq!(proven, 154);
    assert_eq!(boundaries, 2);
}

#[test]
fn reviewed_acceptance_metadata_has_no_unverified_mandatory_cells() {
    let matrix: Value = serde_json::from_str(MATRIX).unwrap();
    let manifest: Value =
        serde_json::from_str(include_str!("../docs/coverage-manifest.json")).unwrap();
    let closure = &matrix["closure"];
    let registered = &manifest["task_114_join_acceptance"];
    assert_eq!(matrix["version"], 7);
    assert_eq!(closure["verified_cases"], 12);
    assert_eq!(closure["verified_dialect_cells"], 156);
    assert_eq!(closure["verified_constructive_dialect_cells"], 154);
    assert_eq!(closure["reviewed_parser_boundary_cells"], 2);
    assert_eq!(registered["verified_canonical_cells"], 12);
    assert_eq!(registered["verified_constructive_dialect_cells"], 154);
    assert_eq!(registered["reviewed_parser_boundaries"], 2);
    assert_eq!(registered["release_approved"], false);

    let mut constructive = 0;
    let mut parser_boundaries = 0;
    for case in matrix["cases"].as_array().unwrap() {
        assert_eq!(
            case["verification"],
            "canonical_source_bag_and_duckdb_oracle_verified"
        );
        assert_eq!(
            case["canonical_proof_verification"],
            "exact_selected_and_not_selected_physical_pairs_verified"
        );
        for adapter in [
            "direct_sql",
            "dbt",
            "odcs_equivalent_metadata",
        ] {
            assert_eq!(
                case["adapters"][adapter],
                "verified_source_backed_portable_law"
            );
        }
        for dialect in matrix["dialects"].as_array().unwrap() {
            let dialect = dialect.as_str().unwrap();
            let evidence = &case["dialects"][dialect];
            if evidence["parser"] == "parser_boundary" {
                assert_eq!(
                    evidence["canonical"],
                    "explicit_parser_boundary_no_canonical_claim"
                );
                parser_boundaries += 1;
            } else {
                assert_eq!(
                    evidence["canonical"],
                    "portable_canonical_physical_population_verified"
                );
                constructive += 1;
            }
        }
    }
    assert_eq!(constructive, 154);
    assert_eq!(parser_boundaries, 2);
    for boundary in matrix["conditional_boundaries"].as_array().unwrap() {
        assert_eq!(boundary["owner"], "TASK-114");
        assert_eq!(boundary["disposition"], "pending_release_review");
        assert_eq!(boundary["native_engine_certified"], false);
        assert!(boundary["missing_evidence"]
            .as_str()
            .is_some_and(|evidence| !evidence.is_empty()));
    }
}
