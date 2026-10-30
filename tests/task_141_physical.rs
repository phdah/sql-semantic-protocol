//! Full TASK-141 physical proof, checked against independent frozen bags.
//! These assertions consume the public canonical API, not SQL AST or DuckDB rows.

use std::collections::BTreeMap;

mod common;
use common::DIALECTS;

use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_finite_cte_bag_plan,
    ConfiguredSqlInput, LiteralValue, PhysicalCteProof, PhysicalWriteRow, PhysicalWriteState,
    RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
};

fn literal(value: &Value) -> LiteralValue {
    match value.as_i64() {
        Some(n) => LiteralValue::Number(n.to_string()),
        None if value.is_null() => LiteralValue::Null,
        _ => panic!("unfrozen CTE scalar type: {value}"),
    }
}

fn expected_row(row: &Value, columns: &[String]) -> BTreeMap<String, LiteralValue> {
    columns
        .iter()
        .map(|name| (name.clone(), literal(&row[name])))
        .collect()
}

fn case_bundle(
    sql: &str,
    input: &Value,
) -> (
    sql_semantic_protocol::AnalysisBundle,
    Vec<PhysicalWriteState>,
) {
    case_bundle_with_dialect(sql, input, "duckdb")
}

fn case_bundle_with_dialect(
    sql: &str,
    input: &Value,
    dialect_name: &str,
) -> (
    sql_semantic_protocol::AnalysisBundle,
    Vec<PhysicalWriteState>,
) {
    let dialect = dialect_from_name(dialect_name).unwrap();
    let mut schemas = Vec::new();
    let mut snapshots = Vec::new();
    for (name, rows) in input.as_object().unwrap() {
        schemas.push(
            RelationSchema::new(
                name.clone(),
                vec![SchemaColumn::from_sql_type("k", "INTEGER", "duckdb").unwrap()],
            )
            .unwrap(),
        );
        let rows = rows
            .as_array()
            .unwrap()
            .iter()
            .map(|row| {
                PhysicalWriteRow::new(BTreeMap::from([("k".to_string(), literal(&row["k"]))]))
                    .unwrap()
            })
            .collect();
        snapshots.push(PhysicalWriteState::new(name.clone(), rows).unwrap());
    }
    let catalog = RelationCatalog::from_schemas(&schemas).unwrap();
    let input = SqlInput::inline(sql);
    let configured = [ConfiguredSqlInput::new(
        "cte",
        &input,
        dialect_name,
        dialect.as_ref(),
    )];
    let bundle = analyze_configured_inputs_with_catalog(&configured, &catalog).unwrap();
    (bundle, snapshots)
}

#[test]
fn all_frozen_cte_bags_are_constructed_from_one_complete_source_population() {
    let freeze: Value =
        serde_json::from_str(include_str!("../docs/task-141-acceptance-freeze.json")).unwrap();
    let mut proved = Vec::new();
    for case in freeze["cases"].as_array().unwrap() {
        if case["classification"] != "mandatory_exact" {
            continue;
        }
        let id = case["id"].as_str().unwrap();
        let (bundle, snapshots) = case_bundle(case["sql"].as_str().unwrap(), &case["initial_bags"]);
        let target = bundle.layers().last().unwrap();
        let PhysicalCteProof::Exact(proof) =
            physical_finite_cte_bag_plan(&bundle, target.id(), &snapshots, None)
        else {
            panic!("{id}: canonical CTE source-complete proof missing");
        };

        assert_eq!(
            proof.sources.len(),
            snapshots.len(),
            "{id}: one shared source graph"
        );
        for (name, rows) in case["initial_bags"].as_object().unwrap() {
            let observed = &proof.source_bags[name];
            assert_eq!(
                observed.len(),
                rows.as_array().unwrap().len(),
                "{id}: source bag"
            );
            for (index, row) in rows.as_array().unwrap().iter().enumerate() {
                assert_eq!(
                    observed[index].values["k"],
                    literal(&row["k"]),
                    "{id}: source value"
                );
                assert_eq!(
                    observed[index].source_row_ids,
                    vec![row["id"].as_str().unwrap()]
                );
            }
        }

        let names = case["terminal_domains"]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        let mut actual = proof
            .terminal
            .iter()
            .map(|row| (row.values.clone(), row.source_row_ids.clone()))
            .collect::<Vec<_>>();
        let mut expected = case["terminal_bag"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| {
                let ids = row["source_ids"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|id| id.as_str().unwrap().to_string())
                    .collect::<Vec<_>>();
                (expected_row(row, &names), ids)
            })
            .collect::<Vec<_>>();
        actual.sort_by(|left, right| left.1.cmp(&right.1));
        expected.sort_by(|left, right| left.1.cmp(&right.1));
        assert_eq!(
            actual, expected,
            "{id}: terminal exact values, IDs and multiplicity"
        );

        for (name, domain) in case["terminal_domains"].as_object().unwrap() {
            let observed = &proof.terminal_domains[name];
            assert_eq!(
                observed.null_allowed,
                domain["nullable"].as_bool().unwrap(),
                "{id}/{name}: NULL"
            );
            let bound = Some((
                domain["lower"]["value"].as_i64().unwrap() as i128,
                domain["upper"]["value"].as_i64().unwrap() as i128,
            ));
            assert_eq!(
                observed.integer_bounds, bound,
                "{id}/{name}: inclusive bounds"
            );
            assert_eq!(domain["lower"]["inclusive"], true);
            assert_eq!(domain["upper"]["inclusive"], true);
        }

        let stages = if id.ends_with("SHADOW") {
            vec![
                ("outer/shared", "terminal/cte:0:shared"),
                ("chosen/shared", "terminal/cte:1:chosen/cte:0:shared"),
                ("chosen", "terminal/cte:1:chosen"),
            ]
        } else if id.ends_with("CHAIN") {
            vec![
                ("first", "terminal/cte:0:first"),
                ("second", "terminal/cte:1:second"),
            ]
        } else {
            vec![("shared", "terminal/cte:0:shared")]
        };
        for (label, stage) in stages {
            let observed = &proof.stage_bags[stage];
            let rows = case["producer_bags"][label].as_array().unwrap();
            assert_eq!(observed.len(), rows.len(), "{id}/{label}: producer count");
            for (n, row) in rows.iter().enumerate() {
                assert_eq!(
                    observed[n].values["k"],
                    literal(&row["k"]),
                    "{id}/{label}: value"
                );
                assert_eq!(
                    observed[n].source_row_ids,
                    row["source_ids"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|id| id.as_str().unwrap().to_string())
                        .collect::<Vec<_>>(),
                    "{id}/{label}: physical provenance"
                );
            }
        }
        if id == "78-C141-N-NULL-DUP" {
            assert_eq!(proof.rejected["terminal"].len(), 11);
            assert_eq!(proof.false_pairs["terminal"].len(), 4);
            assert_eq!(proof.unknown_pairs["terminal"].len(), 7);
            assert_eq!(proof.terminal.len(), 5);
            for (a, b) in [
                (LiteralValue::Null, LiteralValue::Null),
                (
                    LiteralValue::Number("1".into()),
                    LiteralValue::Number("2".into()),
                ),
            ] {
                let wanted = BTreeMap::from([("ak".to_string(), a), ("bk".to_string(), b)]);
                assert!(
                    matches!(
                        physical_finite_cte_bag_plan(
                            &bundle,
                            target.id(),
                            &snapshots,
                            Some(&wanted)
                        ),
                        PhysicalCteProof::Impossible { .. }
                    ),
                    "{id}: terminal counterexample"
                );
            }
        }
        proved.push(id);
    }
    proved.sort();
    assert_eq!(
        proved,
        vec![
            "78-C141-N-NULL-DUP",
            "78-C141-P-CHAIN",
            "78-C141-P-REUSE",
            "78-C141-P-SHADOW"
        ]
    );
}

#[test]
fn unsupported_cte_projection_and_incomplete_source_evidence_are_residual() {
    let sql = "WITH shared AS (SELECT k + 1 AS k FROM s) SELECT k FROM shared";
    let input = serde_json::json!({"s":[{"id":"s#0","k":1}]});
    let (bundle, snapshots) = case_bundle(sql, &input);
    let layer = bundle.layers().last().unwrap();
    assert!(matches!(
        physical_finite_cte_bag_plan(&bundle, layer.id(), &snapshots, None),
        PhysicalCteProof::Residual { .. }
    ));
    let sql = "WITH shared AS (SELECT k FROM s) SELECT k FROM shared";
    let (bundle, _) = case_bundle(sql, &input);
    let layer = bundle.layers().last().unwrap();
    assert!(matches!(
        physical_finite_cte_bag_plan(&bundle, layer.id(), &[], None),
        PhysicalCteProof::Residual { .. }
    ));
}

#[test]
fn every_exposed_parser_dialect_builds_identical_frozen_physical_cte_bags() {
    let freeze: Value =
        serde_json::from_str(include_str!("../docs/task-141-acceptance-freeze.json")).unwrap();
    let mut cells = 0;
    for case in freeze["cases"].as_array().unwrap() {
        if case["classification"] != "mandatory_exact" {
            continue;
        }
        let sql = case["sql"].as_str().unwrap();
        let id = case["id"].as_str().unwrap();
        let (baseline, states) = case_bundle(sql, &case["initial_bags"]);
        let expected = match physical_finite_cte_bag_plan(
            &baseline,
            baseline.layers().last().unwrap().id(),
            &states,
            None,
        ) {
            PhysicalCteProof::Exact(result) => result,
            other => panic!("{id}: canonical baseline not Exact: {other:?}"),
        };
        for dialect in DIALECTS {
            let (bundle, states) = case_bundle_with_dialect(sql, &case["initial_bags"], dialect);
            let result = physical_finite_cte_bag_plan(
                &bundle,
                bundle.layers().last().unwrap().id(),
                &states,
                None,
            );
            let PhysicalCteProof::Exact(proved) = result else {
                panic!("{id}/{dialect}: no complete physical CTE proof: {result:?}");
            };
            assert_eq!(proved.source_bags, expected.source_bags, "{id}/{dialect}");
            assert_eq!(proved.stage_bags, expected.stage_bags, "{id}/{dialect}");
            assert_eq!(proved.rejected, expected.rejected, "{id}/{dialect}");
            assert_eq!(
                proved.terminal_domains, expected.terminal_domains,
                "{id}/{dialect}"
            );
            cells += 1;
        }
    }
    assert_eq!(cells, 4 * DIALECTS.len());
}

#[test]
fn conditional_cte_materialization_recursion_and_forward_binding_fail_closed() {
    use sql_semantic_protocol::{analyze_inputs, dialect_from_name};
    let dialect = dialect_from_name("duckdb").unwrap();
    let freeze: Value =
        serde_json::from_str(include_str!("../docs/task-141-acceptance-freeze.json")).unwrap();
    let mut classified = Vec::new();
    for case in freeze["cases"].as_array().unwrap() {
        if case["classification"] != "conditional" {
            continue;
        }
        let sql = case["sql"].as_str().unwrap();
        let bundle = analyze_inputs(&[SqlInput::inline(sql)], "duckdb", dialect.as_ref()).unwrap();
        let layer = bundle.layers().last().unwrap();
        let proof = physical_finite_cte_bag_plan(&bundle, layer.id(), &[], None);
        assert!(
            matches!(proof, PhysicalCteProof::Residual { .. }),
            "{}: conditional source law must never become Exact",
            case["id"]
        );
        classified.push(case["id"].as_str().unwrap());
    }
    classified.sort();
    assert_eq!(
        classified,
        [
            "78-C141-N-FORWARD",
            "78-C141-N-MATERIALIZATION",
            "78-C141-N-RECURSIVE-SHADOW",
        ]
    );
}

#[test]
fn every_conditional_cte_variant_has_explicit_parser_or_residual_disposition() {
    use sql_semantic_protocol::analyze_inputs;
    let freeze: Value =
        serde_json::from_str(include_str!("../docs/task-141-acceptance-freeze.json")).unwrap();
    let mut checked = 0;
    for case in freeze["cases"].as_array().unwrap() {
        if case["classification"] != "conditional" {
            continue;
        }
        let id = case["id"].as_str().unwrap();
        for dialect_name in DIALECTS {
            let dialect = dialect_from_name(dialect_name).unwrap();
            match analyze_inputs(
                &[SqlInput::inline(case["sql"].as_str().unwrap())],
                dialect_name,
                dialect.as_ref(),
            ) {
                Ok(bundle) => {
                    if let Some(layer) = bundle.layers().last() {
                        assert!(
                            matches!(
                                physical_finite_cte_bag_plan(&bundle, layer.id(), &[], None),
                                PhysicalCteProof::Residual { .. }
                            ),
                            "{id}/{dialect_name}: conditional CTE cannot be certified Exact"
                        );
                    }
                    // A parsed statement without an analyzable transformation
                    // layer is also explicitly unsupported, never Exact.
                }
                Err(error) => {
                    assert!(
                        !error.to_string().is_empty(),
                        "{id}/{dialect_name}: parser boundary"
                    );
                }
            }
            checked += 1;
        }
    }
    assert_eq!(checked, 3 * DIALECTS.len());
}
