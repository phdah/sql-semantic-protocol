//! TASK-112: independently executed complete SQL bags and canonical semi/anti proofs.

use duckdb::{params, Connection};
use serde_json::{json, Value};
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_joint_source_plan,
    ConfiguredSqlInput, JoinKind, JoinPopulationPattern, RelationCatalog, RelationSchema,
    SchemaColumn, SqlInput, WitnessDirection, WitnessObligation,
};

const MATRIX: &str = include_str!("../docs/task-112-acceptance-freeze.json");

fn analyzed(sql: &str) -> sql_semantic_protocol::AnalysisBundle {
    let schemas = ["l", "r"]
        .into_iter()
        .map(|relation| {
            RelationSchema::new(
                relation,
                vec![
                    SchemaColumn::from_sql_type("id", "VARCHAR", "postgresql").unwrap(),
                    SchemaColumn::from_sql_type("k", "INTEGER", "postgresql").unwrap(),
                ],
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let catalog = RelationCatalog::from_schemas(&schemas).unwrap();
    let source = SqlInput::inline(sql);
    let parser = dialect_from_name("generic").unwrap();
    let configured = ConfiguredSqlInput::new("task-112", &source, "generic", parser.as_ref());
    analyze_configured_inputs_with_catalog(&[configured], &catalog).unwrap()
}

#[test]
fn duckdb_full_bags_prove_semi_anti_null_duplicate_and_absence_laws() {
    let matrix: Value = serde_json::from_str(MATRIX).unwrap();
    assert_eq!(matrix["cases"].as_array().unwrap().len(), 8);
    for case in matrix["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(
            "CREATE TABLE l(id VARCHAR, k INTEGER); CREATE TABLE r(id VARCHAR, k INTEGER);",
        )
        .unwrap();
        for relation in ["l", "r"] {
            let sql = format!("INSERT INTO {relation} VALUES (?, ?)");
            for physical in case[if relation == "l" { "left" } else { "right" }]
                .as_array()
                .unwrap()
            {
                db.execute(
                    &sql,
                    params![physical[0].as_str().unwrap(), physical[1].as_i64()],
                )
                .unwrap();
            }
        }

        let mut query = db.prepare(case["duckdb_sql"].as_str().unwrap()).unwrap();
        let mut actual = query
            .query_map([], |row| {
                let identity: String = row.get(0)?;
                let key: Option<i64> = row.get(1)?;
                Ok(json!([identity, key]).to_string())
            })
            .unwrap()
            .map(|row| row.unwrap())
            .collect::<Vec<_>>();
        let mut expected = case["expected"]
            .as_array()
            .unwrap()
            .iter()
            .map(Value::to_string)
            .collect::<Vec<_>>();
        actual.sort();
        expected.sort();
        assert_eq!(
            actual, expected,
            "{id}: every physical output row and multiplicity"
        );

        // SQL UNKNOWN is NOT TRUE. An anti survivor must have no TRUE
        // equality against any right/left partner, not just a sampled pair.
        let left = case["left"].as_array().unwrap();
        let right = case["right"].as_array().unwrap();
        let kind = case["kind"].as_str().unwrap();
        let preserved = if kind.starts_with("left") {
            left
        } else {
            right
        };
        let partners = if kind.starts_with("left") {
            right
        } else {
            left
        };
        let survivor_ids = case["expected"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row[0].as_str().unwrap())
            .collect::<Vec<_>>();
        for row in preserved {
            let key = row[1].as_i64();
            let has_true_partner = partners
                .iter()
                .any(|partner| key.is_some() && key == partner[1].as_i64());
            let selected = if kind.ends_with("semi") {
                has_true_partner
            } else {
                !has_true_partner
            };
            assert_eq!(
                survivor_ids.contains(&row[0].as_str().unwrap()),
                selected,
                "{id}: complete closed-world TRUE/NOT TRUE partner classification"
            );
        }
        let values = case["expected"].as_array().unwrap();
        let numeric = values
            .iter()
            .filter_map(|row| row[1].as_i64())
            .collect::<Vec<_>>();
        let domain = &case["output_domain"];
        assert_eq!(
            numeric.iter().min().copied(),
            domain["minimum"].as_i64(),
            "{id}"
        );
        assert_eq!(
            numeric.iter().max().copied(),
            domain["maximum"].as_i64(),
            "{id}"
        );
        assert_eq!(
            values.iter().any(|row| row[1].is_null()),
            domain["contains_null"].as_bool().unwrap(),
            "{id}"
        );
        assert_eq!(domain["lower_inclusive"], true);
        assert_eq!(domain["upper_inclusive"], true);
        assert_eq!(
            values.is_empty(),
            domain["empty"].as_bool().unwrap(),
            "{id}"
        );
    }
}

#[test]
fn canonical_physical_witness_matches_frozen_complete_nullable_bags() {
    let matrix: Value = serde_json::from_str(MATRIX).unwrap();
    for case in matrix["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        let bundle = analyzed(case["sql"].as_str().unwrap());
        let layer = bundle.layers().last().unwrap().id();
        let wanted = case["expected_output_count"].as_u64().unwrap();
        let plan = physical_joint_source_plan(&bundle, &[(layer, wanted)]);
        let WitnessDirection::Feasible(cases) = plan.outcome() else {
            panic!("{id}: expected complete physical proof: {plan:?}");
        };
        let expected_kind = match case["kind"].as_str().unwrap() {
            "left_semi" => JoinKind::LeftSemi,
            "left_anti" => JoinKind::LeftAnti,
            "right_semi" => JoinKind::RightSemi,
            "right_anti" => JoinKind::RightAnti,
            _ => unreachable!(),
        };
        let expected_pattern = match case["witness_pattern"].as_str().unwrap() {
            "mixed_duplicate_left" => JoinPopulationPattern::MixedDuplicateLeft,
            "disjoint_four_two" => JoinPopulationPattern::DisjointFourTwo,
            _ => unreachable!(),
        };
        assert!(
            cases.iter().any(|c| {
                let obligations = c.obligations();
                obligations
                    .iter()
                    .filter(|o| matches!(o, WitnessObligation::ClosedWorld { .. }))
                    .count()
                    == 2
                    && obligations.iter().any(|o| {
                        matches!(
                            o,
                            WitnessObligation::NoMatchingPartner {
                                closed_world: true,
                                null_extended: None,
                                ..
                            }
                        )
                    })
                    && obligations.iter().any(|o| {
                        matches!(
                            o,
                            WitnessObligation::JoinPopulation {
                                join_kind, pattern,
                                left_rows: 4, right_rows: 2,
                                output_rows, closed_world: true, ..
                            } if *join_kind == expected_kind
                                && *pattern == expected_pattern
                                && *output_rows == wanted
                        )
                    })
            }),
            "{id}: typed nullable duplicate source assignment and both closed-world boundaries"
        );
    }
}

#[test]
fn complete_nonnull_constructive_populations_preserve_sql_survivor_identities() {
    let matrix: Value = serde_json::from_str(MATRIX).unwrap();
    for case in matrix["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(
            "CREATE TABLE l(id VARCHAR, k INTEGER NOT NULL); CREATE TABLE r(id VARCHAR, k INTEGER NOT NULL);",
        )
        .unwrap();
        for (relation, rows, assignment) in [
            (
                "l",
                &case["left"],
                &case["constructive_physical_assignment"]["left_keys"],
            ),
            (
                "r",
                &case["right"],
                &case["constructive_physical_assignment"]["right_keys"],
            ),
        ] {
            let keys = assignment.as_array().unwrap();
            let source = rows.as_array().unwrap();
            assert_eq!(source.len(), keys.len(), "{id}: entire source bag");
            for (row, key) in source.iter().zip(keys) {
                db.execute(
                    &format!("INSERT INTO {relation}(id, k) VALUES (?, ?)"),
                    params![row[0].as_str().unwrap(), key.as_i64().unwrap()],
                )
                .unwrap();
            }
        }
        let mut query = db.prepare(case["duckdb_sql"].as_str().unwrap()).unwrap();
        let mut selected = query
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .map(|row| row.unwrap())
            .collect::<Vec<_>>();
        let mut expected = case["expected"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row[0].as_str().unwrap().to_string())
            .collect::<Vec<_>>();
        selected.sort();
        expected.sort();
        assert_eq!(
            selected, expected,
            "{id}: closed non-NULL population survivor identities"
        );
    }
}
