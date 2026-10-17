//! TASK-111 frozen, independent SQL execution and canonical source-side checks.

mod common;

use common::DIALECTS;
use duckdb::{params, Connection};
use serde_json::{json, Value};
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_joint_source_plan,
    to_bundle_json, ConfiguredSqlInput, JoinKind, JoinPopulationPattern, RelationCatalog,
    RelationSchema, SchemaColumn, SqlInput, WitnessDirection, WitnessObligation,
};

const MATRIX: &str = include_str!("../docs/task-111-acceptance-freeze.json");

fn bundle(sql: &str, dialect: &str) -> sql_semantic_protocol::AnalysisBundle {
    let schemas = ["l", "r"]
        .into_iter()
        .map(|relation| {
            RelationSchema::new(
                relation,
                vec![
                    SchemaColumn::from_sql_type("id", "VARCHAR", "postgresql")
                        .expect("typed row identifier"),
                    SchemaColumn::from_sql_type("k", "INTEGER", "postgresql")
                        .expect("typed signed join key"),
                ],
            )
            .expect("source schema")
        })
        .collect::<Vec<_>>();
    let catalog = RelationCatalog::from_schemas(&schemas).expect("catalog");
    let input = SqlInput::inline(sql);
    let parser = dialect_from_name(dialect).expect("known dialect");
    let configured = ConfiguredSqlInput::new("task-111", &input, dialect, parser.as_ref());
    analyze_configured_inputs_with_catalog(&[configured], &catalog).expect("SQL analysis")
}

#[test]
fn frozen_complete_duckdb_source_and_terminal_bags_agree() {
    let matrix: Value = serde_json::from_str(MATRIX).expect("frozen acceptance matrix");
    assert_eq!(matrix["version"], 2, "AST and physical source proof freeze");
    assert_eq!(matrix["cases"].as_array().expect("cases").len(), 8);
    for case in matrix["cases"].as_array().expect("cases") {
        let id = case["id"].as_str().expect("fixture id");
        let db = Connection::open_in_memory().expect("DuckDB");
        db.execute_batch(
            "CREATE TABLE l(id VARCHAR, k INTEGER); CREATE TABLE r(id VARCHAR, k INTEGER);",
        )
        .expect("closed physical tables");
        for relation in ["l", "r"] {
            let sql = format!("INSERT INTO {relation} VALUES (?, ?)");
            for source_row in case[if relation == "l" { "left" } else { "right" }]
                .as_array()
                .expect("complete source bag")
            {
                let name = source_row[0].as_str().expect("source row ID");
                let key = source_row[1].as_i64();
                db.execute(&sql, params![name, key])
                    .expect("insert physical source row");
            }
        }
        let query = case["sql"].as_str().expect("fixture SQL");
        let mut statement = db.prepare(query).expect("DuckDB SQL");
        let actual_rows = statement
            .query_map([], |row| {
                let left_id: Option<String> = row.get(0)?;
                let left_key: Option<i64> = row.get(1)?;
                let right_id: Option<String> = row.get(2)?;
                let right_key: Option<i64> = row.get(3)?;
                Ok(json!([left_id, left_key, right_id, right_key]))
            })
            .expect("terminal query")
            .map(|r| r.expect("row").to_string())
            .collect::<Vec<_>>();
        let mut actual = actual_rows;
        actual.sort();
        let mut expected = case["expected"]
            .as_array()
            .expect("frozen row identities")
            .iter()
            .map(Value::to_string)
            .collect::<Vec<_>>();
        expected.sort();
        assert_eq!(
            actual, expected,
            "{id}: full bag and physical/synthetic NULL identities"
        );
        assert_eq!(
            actual.len() as u64,
            case["expected_output_count"].as_u64().expect("row count"),
            "{id}"
        );
        for (index, domain) in [(1usize, &case["domains"][0]), (3usize, &case["domains"][1])] {
            let values = case["expected"].as_array().expect("expected bag");
            let numeric = values
                .iter()
                .filter_map(|row| row[index].as_i64())
                .collect::<Vec<_>>();
            let minimum = numeric.iter().min().copied();
            let maximum = numeric.iter().max().copied();
            assert_eq!(
                minimum,
                domain["minimum"].as_i64(),
                "{id}: lower inclusive bound"
            );
            assert_eq!(
                maximum,
                domain["maximum"].as_i64(),
                "{id}: upper inclusive bound"
            );
            assert_eq!(
                values.iter().any(|row| row[index].is_null()),
                domain["contains_null"].as_bool().expect("nullable"),
                "{id}: NULL domain"
            );
        }
        // Independently classify every physical ON pair as TRUE, FALSE or
        // UNKNOWN, including NULL comparisons. A NULL key can never be
        // mistaken for a synthetic output-only NULL extension.
        let mut pair_query = db
            .prepare(
                "SELECT l.id, r.id, CASE WHEN l.k = r.k THEN 'TRUE' 
                 WHEN NOT (l.k = r.k) THEN 'FALSE' ELSE 'UNKNOWN' END 
                 FROM l CROSS JOIN r",
            )
            .expect("physical pair oracle");
        let mut actual_pairs = pair_query
            .query_map([], |row| {
                let left: String = row.get(0)?;
                let right: String = row.get(1)?;
                let truth: String = row.get(2)?;
                Ok(json!([left, right, truth]).to_string())
            })
            .expect("classify physical ON pairs")
            .map(|row| row.expect("physical pair"))
            .collect::<Vec<_>>();
        actual_pairs.sort();
        let mut expected_pairs = Vec::new();
        for left in case["left"].as_array().expect("left physical source") {
            for right in case["right"].as_array().expect("right physical source") {
                let truth = match (left[1].as_i64(), right[1].as_i64()) {
                    (Some(a), Some(b)) if a == b => "TRUE",
                    (Some(_), Some(_)) => "FALSE",
                    _ => "UNKNOWN",
                };
                expected_pairs.push(json!([left[0], right[0], truth]).to_string());
            }
        }
        expected_pairs.sort();
        assert_eq!(
            actual_pairs, expected_pairs,
            "{id}: complete TRUE/FALSE/UNKNOWN pairs"
        );
    }
}

#[test]
fn reversed_on_never_reverses_preserved_outer_source_instance() {
    for &dialect in DIALECTS {
        for (kind, sql, pattern, left_count, right_count) in [
            (
                JoinKind::Left,
                "SELECT l.id FROM l LEFT JOIN r ON r.k = l.k",
                JoinPopulationPattern::EmptyRight,
                3,
                0,
            ),
            (
                JoinKind::Right,
                "SELECT r.id FROM l RIGHT JOIN r ON r.k = l.k",
                JoinPopulationPattern::EmptyLeft,
                0,
                3,
            ),
            (
                JoinKind::Full,
                "SELECT l.id FROM l FULL OUTER JOIN r ON r.k = l.k",
                JoinPopulationPattern::EmptyRight,
                3,
                0,
            ),
        ] {
            let b = bundle(sql, dialect);
            let plan = physical_joint_source_plan(&b, &[(b.layers()[0].id(), 3)]);
            let WitnessDirection::Feasible(cases) = plan.outcome() else {
                panic!("{dialect}: {sql}: expected feasible complete physical proof: {plan:?}");
            };
            assert!(
                cases.iter().any(|case| {
                    let complete = case
                        .obligations()
                        .iter()
                        .filter(|o| matches!(o, WitnessObligation::ClosedWorld { .. }))
                        .count()
                        == 2;
                    let oriented = case.obligations().iter().any(|o| {
                        matches!(
                            o,
                            WitnessObligation::JoinPopulation {
                                left_row,
                                right_row,
                                left_key,
                                right_key,
                                join_kind,
                                pattern: actual_pattern,
                                left_rows,
                                right_rows,
                                output_rows: 3,
                                closed_world: true,
                                ..
                            } if *join_kind == kind
                                && *actual_pattern == pattern
                                && *left_rows == left_count
                                && *right_rows == right_count
                                && left_row.relation() == "l"
                                && right_row.relation() == "r"
                                && left_key.relation() == Some("l")
                                && right_key.relation() == Some("r")
                        )
                    });
                    complete && oriented
                }),
                "{dialect}: SQL side identity and absence law: {plan:?}"
            );
        }
    }
}

#[test]
fn source_null_is_not_an_invented_matching_join_pair() {
    for &dialect in DIALECTS {
        let b = bundle("SELECT l.id, r.id FROM l FULL JOIN r ON l.k = r.k", dialect);
        let plan = physical_joint_source_plan(&b, &[(b.layers()[0].id(), 3)]);
        let WitnessDirection::Feasible(cases) = plan.outcome() else {
            panic!("{dialect}: closed empty partner must have an exact FULL witness");
        };
        assert!(
            cases
                .iter()
                .any(|case| case.obligations().iter().any(|o| matches!(
                    o,
                    WitnessObligation::NoMatchingPartner {
                        null_extended: Some(sql_semantic_protocol::JoinSide::Right),
                        closed_world: true,
                        ..
                    }
                ))),
            "{dialect}: no physical partner means synthetic NULL, not an inserted row"
        );
    }
}

#[test]
fn disjoint_nonempty_outer_inputs_are_closed_world_without_physical_null_insertion() {
    for &dialect in DIALECTS {
        for (kind, sql, requested, expected_left, expected_right) in [
            (
                JoinKind::Left,
                "SELECT l.id FROM l LEFT JOIN r ON l.k = r.k",
                3,
                3,
                1,
            ),
            (
                JoinKind::Right,
                "SELECT r.id FROM l RIGHT JOIN r ON l.k = r.k",
                3,
                1,
                3,
            ),
            (
                JoinKind::Full,
                "SELECT l.id FROM l FULL OUTER JOIN r ON r.k = l.k",
                3,
                2,
                1,
            ),
        ] {
            let b = bundle(sql, dialect);
            let plan = physical_joint_source_plan(&b, &[(b.layers()[0].id(), requested)]);
            let WitnessDirection::Feasible(cases) = plan.outcome() else {
                panic!("{dialect}: no-match complete nonempty proof: {plan:?}");
            };
            assert!(
                cases.iter().any(|case| {
                    let disjoint = case.obligations().iter().any(|o| {
                        matches!(
                            o,
                            WitnessObligation::JoinPopulation {
                                left_row,
                                right_row,
                                join_kind,
                                pattern: JoinPopulationPattern::DisjointKeys,
                                left_rows,
                                right_rows,
                                output_rows: 3,
                                closed_world: true,
                                ..
                            } if *join_kind == kind
                                && *left_rows == expected_left
                                && *right_rows == expected_right
                                && left_row.relation() == "l"
                                && right_row.relation() == "r"
                        )
                    });
                    let absences = case
                        .obligations()
                        .iter()
                        .filter(|o| {
                            matches!(
                                o,
                                WitnessObligation::NoMatchingPartner {
                                    closed_world: true,
                                    ..
                                }
                            )
                        })
                        .count();
                    let whole_bags = case
                        .obligations()
                        .iter()
                        .filter(|o| matches!(o, WitnessObligation::ClosedWorld { .. }))
                        .count();
                    disjoint && absences == 2 && whole_bags == 2
                }),
                "{dialect}: complete physical sources and two nonmatching sides: {plan:?}"
            );
        }
    }
}

#[test]
fn matched_and_unmatched_outer_populations_retain_both_truth_directions() {
    for &dialect in DIALECTS {
        for (kind, sql, output_rows, left_count, right_count) in [
            (
                JoinKind::Left,
                "SELECT l.id FROM l LEFT JOIN r ON r.k=l.k",
                3,
                3,
                2,
            ),
            (
                JoinKind::Right,
                "SELECT r.id FROM l RIGHT JOIN r ON r.k=l.k",
                3,
                2,
                3,
            ),
            (
                JoinKind::Full,
                "SELECT l.id FROM l FULL JOIN r ON r.k=l.k",
                3,
                2,
                2,
            ),
        ] {
            let b = bundle(sql, dialect);
            let plan = physical_joint_source_plan(&b, &[(b.layers()[0].id(), output_rows)]);
            let WitnessDirection::Feasible(cases) = plan.outcome() else {
                panic!("{dialect}: must construct matched and unmatched outer bag: {plan:?}");
            };
            assert!(
                cases.iter().any(|case| {
                    let matching = case.obligations().iter().any(|o| {
                        matches!(
                            o,
                            WitnessObligation::JoinPopulation {
                                join_kind,
                                pattern: JoinPopulationPattern::MixedMatchedUnmatched,
                                left_rows,
                                right_rows,
                                closed_world: true,
                                ..
                            } if *join_kind == kind
                                && *left_rows == left_count
                                && *right_rows == right_count
                        )
                    });
                    let positives = case
                        .obligations()
                        .iter()
                        .filter(|o| matches!(o, WitnessObligation::JoinPair { .. }))
                        .count();
                    let negative_rows = case
                        .obligations()
                        .iter()
                        .filter(|o| {
                            matches!(
                                o,
                                WitnessObligation::NoMatchingPartner {
                                    closed_world: true,
                                    ..
                                }
                            )
                        })
                        .count();
                    let closed_sources = case
                        .obligations()
                        .iter()
                        .filter(|o| matches!(o, WitnessObligation::ClosedWorld { .. }))
                        .count();
                    matching && positives == 1 && negative_rows == 2 && closed_sources == 2
                }),
                "{dialect}: matched+unmatched row IDs and complete source bags: {plan:?}"
            );
        }
    }
}

#[test]
fn frozen_sql_cases_are_canonical_equivalent_in_every_exposed_parser_dialect() {
    let matrix: Value = serde_json::from_str(MATRIX).expect("frozen matrix");
    for case in matrix["cases"].as_array().expect("frozen cases") {
        let sql = case["sql"].as_str().expect("SQL");
        let id = case["id"].as_str().expect("stable fixture ID");
        let kind = match case["kind"].as_str().expect("JOIN kind") {
            "LEFT" => JoinKind::Left,
            "RIGHT" => JoinKind::Right,
            "FULL" => JoinKind::Full,
            other => panic!("unrecognized frozen join kind {other}"),
        };
        for &dialect in DIALECTS {
            let canonical = if dialect == "postgres" {
                "postgresql"
            } else {
                dialect
            };
            assert_eq!(
                case["dialects"][canonical]["parser"], "parser_equivalent",
                "{id}/{dialect}: matrix must record executable parser status"
            );
            assert_eq!(
                case["dialects"][canonical]["native_execution"],
                if dialect == "duckdb" {
                    "duckdb_oracle"
                } else {
                    "not_claimed"
                },
                "{id}/{dialect}: parser does not certify vendor execution"
            );
            let analyzed = bundle(sql, dialect);
            let wire: Value =
                serde_json::from_str(&to_bundle_json(&analyzed)).expect("canonical protocol JSON");
            let statement = &wire["inputs"][0]["statements"][0];
            let expected_ast = &case["canonical_ast_expectation"];
            assert_eq!(
                statement["kind"], expected_ast["statement_kind"],
                "{id}/{dialect}"
            );
            let joins = statement["joins"].as_array().expect("canonical joins");
            assert_eq!(
                joins.len(),
                expected_ast["joins"].as_u64().expect("join count") as usize,
                "{id}/{dialect}"
            );
            assert_eq!(
                joins[0]["kind"], expected_ast["join_kind"],
                "{id}/{dialect}"
            );
            assert_eq!(
                joins[0]["condition"]["kind"], expected_ast["on"]["kind"],
                "{id}/{dialect}"
            );
            assert_eq!(
                joins[0]["condition"]["operator"], expected_ast["on"]["operator"],
                "{id}/{dialect}"
            );
            assert!(
                statement["predicates"]["where"].is_null(),
                "{id}/{dialect}: no WHERE restriction"
            );
            let layer = analyzed.layers().first().expect("joined terminal");
            let plan = physical_joint_source_plan(
                &analyzed,
                &[(
                    layer.id(),
                    case["expected_output_count"].as_u64().expect("count"),
                )],
            );
            assert!(
                matches!(plan.outcome(), WitnessDirection::Feasible(_)),
                "{id}/{dialect}: exact typed closed source population: {plan:?}"
            );
            let WitnessDirection::Feasible(cases) = plan.outcome() else {
                unreachable!("guarded above");
            };
            let frozen = &case["constructive_oracle"];
            let left_count = frozen["left_physical_count"].as_u64().expect("left bag");
            let right_count = frozen["right_physical_count"].as_u64().expect("right bag");
            let output_count = frozen["output_count"].as_u64().expect("output bag");
            let expected_pattern = frozen["expected_pattern"].as_str().expect("pattern");
            assert!(
                cases.iter().any(|witness| {
                    let same_population = witness.obligations().iter().any(|obligation| {
                        matches!(obligation,
                            WitnessObligation::JoinPopulation {
                                join_kind,
                                pattern,
                                left_rows,
                                right_rows,
                                output_rows,
                                closed_world: true,
                                ..
                            } if *join_kind == kind
                                && pattern.as_str() == expected_pattern
                                && *left_rows == left_count
                                && *right_rows == right_count
                                && *output_rows == output_count
                        )
                    });
                    let full_bags = witness.obligations().iter().filter(|obligation| {
                        matches!(obligation, WitnessObligation::ClosedWorld { .. })
                    }).count() == 2;
                    let absence = !frozen["requires_absence_proof"].as_bool().expect("absence law")
                        || witness.obligations().iter().any(|obligation| {
                            matches!(obligation,
                                WitnessObligation::NoMatchingPartner {
                                    closed_world: true,
                                    ..
                                }
                            )
                        });
                    same_population && full_bags && absence
                }),
                "{id}/{dialect}: same complete physical source bag sizes and match multiplicities as frozen independent oracle: {plan:?}"
            );
        }
    }
}

#[test]
fn mixed_population_semi_anti_counts_do_not_inherit_full_outer_cardinality() {
    assert_eq!(
        JoinPopulationPattern::MixedMatchedUnmatched.output_rows(JoinKind::Left, 3, 2),
        Some(3)
    );
    assert_eq!(
        JoinPopulationPattern::MixedMatchedUnmatched.output_rows(JoinKind::Right, 3, 2),
        Some(2)
    );
    assert_eq!(
        JoinPopulationPattern::MixedMatchedUnmatched.output_rows(JoinKind::Full, 3, 2),
        Some(4)
    );
    assert_eq!(
        JoinPopulationPattern::MixedMatchedUnmatched.output_rows(JoinKind::Inner, 3, 2),
        Some(1)
    );
    for kind in [JoinKind::LeftSemi, JoinKind::RightSemi] {
        assert_eq!(
            JoinPopulationPattern::MixedMatchedUnmatched.output_rows(kind, 3, 2),
            Some(1),
            "{kind:?}: semi output contains one matched physical source row"
        );
    }
    assert_eq!(
        JoinPopulationPattern::MixedMatchedUnmatched.output_rows(JoinKind::LeftAnti, 3, 2),
        Some(2)
    );
    assert_eq!(
        JoinPopulationPattern::MixedMatchedUnmatched.output_rows(JoinKind::RightAnti, 3, 2),
        Some(1)
    );
    for (kind, expected) in [
        (JoinKind::Inner, 2),
        (JoinKind::Left, 4),
        (JoinKind::Right, 4),
        (JoinKind::Full, 6),
        (JoinKind::LeftSemi, 1),
        (JoinKind::RightSemi, 2),
        (JoinKind::LeftAnti, 2),
        (JoinKind::RightAnti, 2),
    ] {
        assert_eq!(
            JoinPopulationPattern::MixedDuplicateRight.output_rows(kind, 3, 4),
            Some(expected),
            "{kind:?}: 3x4 bags have exactly two matched pairs"
        );
    }
}

#[test]
fn shared_physical_source_cannot_receive_two_disjoint_histograms() {
    let analyzed = bundle(
        "SELECT a.id FROM l AS a FULL OUTER JOIN l AS b ON a.k = b.k",
        "postgresql",
    );
    let plan = physical_joint_source_plan(&analyzed, &[(analyzed.layers()[0].id(), 3)]);
    assert!(
        !matches!(
            plan.outcome(),
            WitnessDirection::Feasible(cases) if cases.iter().any(|case| {
                case.obligations().iter().any(|obligation| {
                    matches!(
                        obligation,
                        WitnessObligation::JoinPopulation {
                            pattern: JoinPopulationPattern::DisjointKeys
                                | JoinPopulationPattern::MixedMatchedUnmatched,
                            ..
                        }
                    )
                })
            })
        ),
        "one physical table used by two aliases cannot carry incompatible complete key histograms: {plan:?}"
    );
}

#[test]
fn nonportable_null_safe_outer_condition_remains_a_typed_residual() {
    let analyzed = bundle(
        "SELECT l.id FROM l LEFT JOIN r ON l.k IS NOT DISTINCT FROM r.k",
        "postgresql",
    );
    let plan = physical_joint_source_plan(&analyzed, &[(analyzed.layers()[0].id(), 3)]);
    assert!(
        matches!(plan.outcome(), WitnessDirection::Residual { .. }),
        "without certified null-safe equality execution evidence no complete physical source assignment may be promoted to Exact: {plan:?}"
    );
}
