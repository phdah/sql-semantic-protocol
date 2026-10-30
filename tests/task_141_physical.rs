//! Full TASK-141 physical proof, checked against independent frozen bags.
//! These assertions consume the public canonical API, not SQL AST or DuckDB rows.

use std::collections::{BTreeMap, BTreeSet};

mod common;
use common::DIALECTS;

use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_finite_cte_bag_plan,
    ConfiguredSqlInput, LiteralValue, PhysicalCteProof, PhysicalCteRef, PhysicalWriteRow,
    PhysicalWriteState, RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
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
        let mut declared = BTreeSet::new();
        assert_eq!(
            proof.nodes.len(),
            proof.sources.len() + proof.stage_bags.len(),
            "{id}: every physical and lexical producer appears exactly once"
        );
        for node in &proof.nodes {
            assert!(
                declared.insert(node.id.clone()),
                "{id}: duplicate canonical producer"
            );
            for input in &node.inputs {
                assert!(
                    declared.contains(&input.reference),
                    "{id}: producer DAG must be ordered"
                );
            }
        }
        if id.ends_with("REUSE") || id.ends_with("NULL-DUP") {
            let terminal = proof.nodes.last().unwrap();
            assert_eq!(
                terminal
                    .inputs
                    .iter()
                    .map(|input| input.alias.as_str())
                    .collect::<Vec<_>>(),
                ["a", "b"],
                "{id}: distinct SQL source instances"
            );
            assert!(
                terminal.inputs.iter().all(|input| input.reference
                    == PhysicalCteRef::Producer("terminal/cte:0:shared".to_string())),
                "{id}: both aliases reuse the same producer rather than inserting independent CTEs"
            );
        }
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
        // Every mandatory fixture must prove one positive membership and
        // disprove its own independently frozen negative counterexample.
        let first_row = &case["terminal_bag"][0];
        let selected = expected_row(first_row, &names);
        assert!(
            matches!(
                physical_finite_cte_bag_plan(&bundle, target.id(), &snapshots, Some(&selected)),
                PhysicalCteProof::Exact(_)
            ),
            "{id}: known terminal tuple must be Exact"
        );
        let negative = match id {
            "78-C141-P-REUSE" => BTreeMap::from([
                ("ak".to_string(), LiteralValue::Number("1".to_string())),
                ("bk".to_string(), LiteralValue::Number("2".to_string())),
            ]),
            "78-C141-P-CHAIN" => {
                BTreeMap::from([("k".to_string(), LiteralValue::Number("3".to_string()))])
            }
            "78-C141-P-SHADOW" => {
                BTreeMap::from([("k".to_string(), LiteralValue::Number("100".to_string()))])
            }
            "78-C141-N-NULL-DUP" => BTreeMap::from([
                ("ak".to_string(), LiteralValue::Null),
                ("bk".to_string(), LiteralValue::Null),
            ]),
            other => panic!("unreviewed mandatory CTE fixture: {other}"),
        };
        assert!(
            matches!(
                physical_finite_cte_bag_plan(&bundle, target.id(), &snapshots, Some(&negative)),
                PhysicalCteProof::Impossible { .. }
            ),
            "{id}: frozen absent terminal tuple must be Impossible"
        );
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
fn unresolved_cte_order_by_column_cannot_receive_an_exact_proof() {
    let input = serde_json::json!({"s":[{"id":"s#0","k":1}]});
    for sql in [
        "WITH shared AS (SELECT k FROM s) SELECT k FROM shared ORDER BY missing",
        "WITH shared AS (SELECT k FROM s) SELECT k AS projected_k FROM shared ORDER BY missing",
    ] {
        for dialect in DIALECTS {
            let (bundle, snapshots) = case_bundle_with_dialect(sql, &input, dialect);
            let layer = bundle.layers().last().unwrap();
            assert!(
                matches!(
                    physical_finite_cte_bag_plan(&bundle, layer.id(), &snapshots, None),
                    PhysicalCteProof::Residual { .. }
                ),
                "{sql}/{dialect}: unresolved ORDER BY must not receive a physical proof"
            );
        }
    }
}

#[test]
fn partition_selected_cte_scan_is_not_a_complete_source_proof() {
    // MySQL partition selection is not equivalent to the attested full s bag.
    // Treating a TableFactor with partitions as a plain table is unsound.
    let sql = "WITH shared AS (SELECT k FROM s PARTITION (p0)) SELECT k FROM shared";
    let input = serde_json::json!({"s":[{"id":"s#0","k":1}]});
    let (bundle, snapshots) = case_bundle_with_dialect(sql, &input, "mysql");
    let layer = bundle.layers().last().unwrap();
    assert!(
        matches!(
            physical_finite_cte_bag_plan(&bundle, layer.id(), &snapshots, None),
            PhysicalCteProof::Residual { .. }
        ),
        "partition-restricted CTE cannot claim whole-source Exact"
    );
}

#[test]
fn source_column_alias_list_cannot_borrow_original_schema_names() {
    let sql = "WITH shared AS (SELECT k FROM s AS renamed(x)) SELECT k FROM shared";
    let input = serde_json::json!({"s":[{"id":"s#0","k":1}]});
    let (bundle, snapshots) = case_bundle(sql, &input);
    let layer = bundle.layers().last().unwrap();
    assert!(
        matches!(
            physical_finite_cte_bag_plan(&bundle, layer.id(), &snapshots, None),
            PhysicalCteProof::Residual { .. }
        ),
        "table alias column names must not be ignored by an Exact CTE proof"
    );
}

#[test]
fn quoted_cte_names_do_not_borrow_unquoted_source_evidence() {
    let sql = r#"WITH "shared" AS (SELECT k FROM s) SELECT k FROM "shared""#;
    let input = serde_json::json!({"s":[{"id":"s#0","k":1}]});
    let (bundle, snapshots) = case_bundle(sql, &input);
    let layer = bundle.layers().last().unwrap();
    assert!(
        matches!(
            physical_finite_cte_bag_plan(&bundle, layer.id(), &snapshots, None),
            PhysicalCteProof::Residual { .. }
        ),
        "quoted lexical bindings need attested dialect-specific name resolution"
    );
}

#[test]
fn quoted_output_alias_with_unquoted_sort_key_is_not_certified() {
    let sql = r#"WITH shared AS (SELECT k FROM s) SELECT k AS "k" FROM shared ORDER BY k"#;
    let input = serde_json::json!({"s":[{"id":"s#0","k":1}]});
    let (bundle, snapshots) = case_bundle(sql, &input);
    let layer = bundle.layers().last().unwrap();
    assert!(
        matches!(
            physical_finite_cte_bag_plan(&bundle, layer.id(), &snapshots, None),
            PhysicalCteProof::Residual { .. }
        ),
        "quoted output labels need dialect-specific case/binder proof"
    );
}

#[test]
fn differently_cased_cte_reference_never_borrows_a_physical_table() {
    let sql = "WITH shared AS (SELECT k FROM s) SELECT k FROM SHARED";
    let input = serde_json::json!({
        "s": [{"id":"s#0","k":1}],
        "SHARED": [{"id":"SHARED#0","k":99}]
    });
    let (bundle, snapshots) = case_bundle(sql, &input);
    let layer = bundle.layers().last().unwrap();
    match physical_finite_cte_bag_plan(&bundle, layer.id(), &snapshots, None) {
        PhysicalCteProof::Exact(result) => {
            assert_eq!(
                result.sources,
                vec!["s"],
                "unquoted local CTE reference must not resolve as physical SHARED"
            );
            assert_eq!(
                result.terminal[0].values["k"],
                LiteralValue::Number("1".to_string())
            );
        }
        PhysicalCteProof::Residual { .. } => {}
        other => panic!(
            "unresolved lexical case must be Exact with correct sources or Residual: {other:?}"
        ),
    }
}

#[test]
fn duplicate_unquoted_cte_names_cannot_receive_an_exact_proof() {
    use sql_semantic_protocol::analyze_inputs;
    let sql = "WITH shared AS (SELECT k FROM s), SHARED AS (SELECT k FROM s) SELECT k FROM shared";
    let dialect = dialect_from_name("duckdb").unwrap();
    // A parser-level duplicate binding rejection is also a safe boundary.
    if analyze_inputs(&[SqlInput::inline(sql)], "duckdb", dialect.as_ref()).is_ok() {
        let input = serde_json::json!({"s":[{"id":"s#0","k":1}]});
        let (bundle, snapshots) = case_bundle(sql, &input);
        let layer = bundle.layers().last().unwrap();
        assert!(
            matches!(
                physical_finite_cte_bag_plan(&bundle, layer.id(), &snapshots, None),
                PhysicalCteProof::Residual { .. }
            ),
            "duplicate WITH names have no canonical lexical identity"
        );
    }
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

#[test]
fn forward_cte_sibling_does_not_borrow_identically_named_physical_table() {
    let sql = "WITH early AS (SELECT k FROM later), later AS (SELECT k FROM s) \
               SELECT k FROM early";
    // Even when a physical table named 'later' exists, the engine's forward
    // binding law is conditional and cannot authorize an Exact CTE proof.
    let input = serde_json::json!({
        "later": [{"id": "later#0", "k": 42}],
        "s": [{"id": "s#0", "k": 1}]
    });
    let (bundle, snapshots) = case_bundle(sql, &input);
    let layer = bundle.layers().last().unwrap();
    assert!(matches!(
        physical_finite_cte_bag_plan(&bundle, layer.id(), &snapshots, None),
        PhysicalCteProof::Residual { .. }
    ));
}

#[test]
fn task_141_frozen_proof_matrix_maps_every_mandatory_and_conditional_cell() {
    let baseline: Value =
        serde_json::from_str(include_str!("../docs/task-141-acceptance-freeze.json")).unwrap();
    let matrix: Value =
        serde_json::from_str(include_str!("../docs/task-141-proof-matrix.json")).unwrap();
    assert_eq!(matrix["version"], 2);
    assert_eq!(matrix["owner"], "TASK-141");
    assert_eq!(matrix["parent"], "TASK-78");
    let mandatory = matrix["mandatory"].as_array().unwrap();
    assert_eq!(mandatory.len(), 4);
    for case in mandatory {
        let id = case["id"].as_str().unwrap();
        let original = baseline["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["id"] == id)
            .unwrap();
        assert_eq!(original["classification"], "mandatory_exact", "{id}");
        assert_eq!(case["expected_proof"], "Exact", "{id}");
        assert_eq!(
            case["expected_terminal_bag"], original["terminal_bag"],
            "{id}"
        );
        assert_eq!(
            case["expected_terminal_domains"], original["terminal_domains"],
            "{id}"
        );
        assert_eq!(
            case["expected_source_bags"], original["initial_bags"],
            "{id}"
        );
        assert_eq!(
            case["expected_producer_bags"], original["producer_bags"],
            "{id}"
        );
        let dialects = case["dialect_cells"].as_array().unwrap();
        assert_eq!(dialects.len(), DIALECTS.len(), "{id}: every parser name");
        for expected in DIALECTS {
            assert!(
                dialects
                    .iter()
                    .any(|cell| cell["dialect"] == *expected
                        && cell["canonical_physical"] == "Exact"),
                "{id}/{expected}: required physical proof"
            );
        }
        let adapters = case["adapter_cells"].as_array().unwrap();
        assert_eq!(adapters.len(), 3, "{id}: all evidence adapters");
        for expected in [
            "direct_sql",
            "dbt_compiled_manifest_catalog",
            "odcs_equivalent_schema",
        ] {
            assert!(
                adapters.iter().any(|cell| cell["adapter"] == expected),
                "{id}: absent adapter {expected}"
            );
        }
    }
    let conditional = matrix["conditional"].as_array().unwrap();
    assert_eq!(conditional.len(), 3);
    for case in conditional {
        let id = case["id"].as_str().unwrap();
        let original = baseline["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["id"] == id)
            .unwrap();
        assert_eq!(original["classification"], "conditional", "{id}");
        assert_eq!(case["owner"], "TASK-141", "{id}: release-review owner");
        assert!(case["release_disposition"]
            .as_str()
            .is_some_and(|text| !text.is_empty()));
        assert_eq!(
            case["dialect_cells"].as_array().unwrap().len(),
            DIALECTS.len(),
            "{id}"
        );
    }
    assert_eq!(
        matrix["verification"]["physical_dialect_cells"],
        4 * DIALECTS.len()
    );
    assert_eq!(matrix["verification"]["adapter_cells"], 12);
    assert_eq!(
        matrix["verification"]["conditional_cells"],
        3 * DIALECTS.len()
    );
    assert_eq!(matrix["verification"]["parent_done"], false);
}
