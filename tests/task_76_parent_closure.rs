//! TASK-76 parent closure: independently pin merged child evidence at the public API.
//! The child freezes own the individual SQL/data oracles. This parent guard
//! checks the complete union and re-executes a cross-child native/public path.

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_finite_composed_page_plan,
    physical_finite_page_bag_plan, ConfiguredSqlInput, LiteralValue, PhysicalComposedPageProof,
    PhysicalFinitePageProof, PhysicalWriteRow, PhysicalWriteState, RelationCatalog, RelationSchema,
    SchemaColumn, SqlInput,
};
use std::collections::{BTreeMap, BTreeSet};

const CHILDREN: [(&str, &str); 4] = [
    (
        "TASK-133",
        include_str!("../docs/task-133-acceptance-freeze.json"),
    ),
    (
        "TASK-134",
        include_str!("../docs/task-134-acceptance-freeze.json"),
    ),
    (
        "TASK-135",
        include_str!("../docs/task-135-acceptance-freeze.json"),
    ),
    (
        "TASK-136",
        include_str!("../docs/task-136-acceptance-freeze.json"),
    ),
];

fn parent() -> Value {
    serde_json::from_str(include_str!("../docs/task-76-acceptance-freeze.json"))
        .expect("versioned parent freeze")
}

fn ids(array: &Value) -> BTreeSet<String> {
    array
        .as_array()
        .expect("frozen ID array")
        .iter()
        .map(|entry| entry.as_str().expect("stable ID").to_owned())
        .collect()
}

fn cases(child: &Value) -> Vec<&Value> {
    ["cases", "additional_mandatory_cases", "conditional_cases"]
        .into_iter()
        .flat_map(|section| {
            child
                .get(section)
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
        })
        .collect()
}

fn has_array(case: &Value, keys: &[&str]) -> bool {
    keys.iter()
        .any(|key| case.get(*key).is_some_and(Value::is_array))
}

fn has_domain(case: &Value) -> bool {
    [
        "typed_output_domain",
        "terminal_domains",
        "terminal_domain",
        "domain",
    ]
    .iter()
    .any(|key| {
        case.get(*key)
            .is_some_and(|value| value.is_object() || value.is_array())
    })
}

#[test]
fn parent_inventory_reconciles_all_child_cases_and_every_original_criterion() {
    let parent = parent();
    assert_eq!(parent["task"], "TASK-76");
    assert_eq!(parent["version"], 2);
    assert_eq!(parent["release_approved"], false);
    assert!(parent["mandatory_unverified"]
        .as_array()
        .is_some_and(Vec::is_empty));

    let dialects = ids(&parent["required_dialects"]);
    let adapters = ids(&parent["required_adapters"]);
    assert_eq!(dialects.len(), 13);
    assert_eq!(
        adapters,
        ids(&serde_json::json!(["direct_sql", "dbt_compiled", "odcs"]))
    );
    let mut mandatory = BTreeSet::new();
    let mut conditional = BTreeSet::new();
    let mut fixtures = BTreeSet::new();

    for (name, content) in CHILDREN {
        let child: Value = serde_json::from_str(content).expect("versioned child freeze");
        assert_eq!(child["parent"], "TASK-76");
        assert_eq!(child.get("owner").unwrap_or(&child["task"]), name);
        let record = &parent["source_of_truth"][name];
        assert_eq!(record["version"], child["version"], "{name} version drift");
        assert_eq!(ids(&record["fixture_ids"]), ids(&child["fixture_ids"]));
        fixtures.extend(ids(&child["fixture_ids"]));

        let exact = ids(&record["mandatory_case_ids"]);
        let residual = ids(&record["conditional_case_ids"]);
        assert!(
            exact.is_disjoint(&residual),
            "{name}: double-classified case"
        );
        let mut observed = BTreeSet::new();
        for case in cases(&child) {
            let id = case["id"].as_str().expect("stable variant ID");
            assert!(observed.insert(id.to_owned()), "{name}: duplicate {id}");
            let advertised_exact = match name {
                "TASK-133" | "TASK-134" => case["proof"] == "Exact",
                "TASK-135" => case["classification"] == "mandatory",
                "TASK-136" => {
                    case["classification"] == "mandatory"
                        && case["expected"]
                            .as_str()
                            .is_some_and(|value| value.starts_with("Exact"))
                }
                _ => false,
            };
            assert_eq!(
                advertised_exact,
                exact.contains(id),
                "{id}: parent proof classification drift"
            );
            assert!(
                !case["sql"].as_str().unwrap_or("").is_empty(),
                "{id}: SQL absent"
            );
            assert!(
                has_array(case, &["source_bag", "source"]),
                "{id}: complete physical input not frozen"
            );
            if exact.contains(id) {
                assert!(
                    has_array(case, &["terminal_bag", "output"]),
                    "{id}: complete terminal oracle not frozen"
                );
                assert!(has_domain(case), "{id}: typed terminal domain not frozen");
                assert!(
                    has_array(
                        case,
                        &[
                            "selected_source_ids",
                            "selected_ids",
                            "left_selected",
                            "selected"
                        ]
                    ),
                    "{id}: positive source identity not frozen"
                );
                assert!(
                    has_array(
                        case,
                        &[
                            "rejected_source_ids",
                            "rejected_ids",
                            "left_page_rejected",
                            "rejected"
                        ]
                    ),
                    "{id}: negative source identity not frozen"
                );
                assert!(mandatory.insert(id.to_owned()), "duplicated exact {id}");
            } else {
                assert!(
                    residual.contains(id),
                    "{id}: unclassified parent obligation"
                );
                assert!(
                    !case["reason"].as_str().unwrap_or("").is_empty(),
                    "{id}: missing residual reason"
                );
                assert!(
                    !case["owner"].as_str().unwrap_or("").is_empty(),
                    "{id}: missing residual owner"
                );
                assert!(
                    case.get("disposition")
                        .or_else(|| case.get("release_disposition"))
                        .is_some(),
                    "{id}: unreviewed conditional disposition"
                );
                assert!(
                    conditional.insert(id.to_owned()),
                    "duplicated residual {id}"
                );
            }
            if let Some(cells) = case
                .get("parser_cells")
                .or_else(|| case.get("dialect_cells"))
            {
                let observed = if let Some(map) = cells.as_object() {
                    map.keys().cloned().collect::<BTreeSet<_>>()
                } else {
                    cells
                        .as_array()
                        .expect("typed parser matrix")
                        .iter()
                        .map(|cell| cell["dialect"].as_str().expect("dialect").to_owned())
                        .collect::<BTreeSet<_>>()
                };
                assert_eq!(observed, dialects, "{id}: incomplete parser matrix");
            } else {
                assert_eq!(name, "TASK-135", "{id}: missing parser evidence");
                if exact.contains(id) {
                    assert_eq!(case["dialect_status"], "13_parsed_and_exact", "{id}");
                } else {
                    assert_eq!(case["status"], "Residual", "{id}");
                    assert_eq!(ids(&child["dialect_matrix"]["dialects"]), dialects);
                }
            }
            if let Some(cells) = case.get("adapter_cells") {
                let observed = if let Some(map) = cells.as_object() {
                    map.keys().cloned().collect::<BTreeSet<_>>()
                } else {
                    cells
                        .as_array()
                        .expect("typed adapter matrix")
                        .iter()
                        .map(|cell| match cell["source"].as_str().expect("adapter") {
                            "compiled_dbt" => "dbt_compiled".to_owned(),
                            other => other.to_owned(),
                        })
                        .collect::<BTreeSet<_>>()
                };
                assert_eq!(observed, adapters, "{id}: incomplete adapter matrix");
            } else if exact.contains(id) {
                assert_eq!(name, "TASK-135", "{id}: missing adapter evidence");
                assert_eq!(
                    case["adapter_status"], "direct_sql_dbt_compiled_odcs_exact",
                    "{id}"
                );
            } else {
                assert_eq!(name, "TASK-135", "{id}: missing adapter evidence");
                assert_eq!(ids(&child["adapter_matrix"]["sources"]), adapters);
            }
        }
        assert_eq!(
            observed,
            exact.union(&residual).cloned().collect(),
            "{name}"
        );
    }

    assert_eq!(mandatory, ids(&parent["mandatory_case_ids"]));
    assert_eq!(conditional, ids(&parent["conditional_case_ids"]));
    assert_eq!(fixtures, ids(&parent["fixture_ids"]));
    assert!(mandatory.is_disjoint(&conditional));
    assert_eq!(mandatory.len(), 26);
    assert_eq!(conditional.len(), 16);
    assert_eq!(parent["total_mandatory_exact_cases"], 26);
    assert_eq!(parent["total_reviewed_conditional_cases"], 16);

    let mut covered_mandatory = BTreeSet::new();
    let mut covered_conditional = BTreeSet::new();
    let criteria = parent["original_acceptance_criteria"].as_array().unwrap();
    assert_eq!(criteria.len(), 6);
    for (offset, criterion) in criteria.iter().enumerate() {
        assert_eq!(criterion["id"], offset + 1);
        let exact_refs = ids(&criterion["mandatory_case_ids"]);
        let residual_refs = ids(&criterion["conditional_case_ids"]);
        if offset == 2 {
            // Sampling intentionally has only typed, engine-conditional Residuals.
            assert!(exact_refs.is_empty(), "stochastic sampling cannot be Exact");
            assert_eq!(
                residual_refs,
                ids(&serde_json::json!([
                    "76-C136-P-system",
                    "76-C136-N-seeded",
                    "76-C136-N-sample",
                    "76-C136-N-sample-seeded"
                ]))
            );
        } else {
            assert!(
                !exact_refs.is_empty(),
                "AC {} lacks executable evidence",
                offset + 1
            );
        }
        assert!(
            criterion["evidence"].as_array().is_some_and(|tests| !tests.is_empty()),
            "AC {} lacks independent executable test references",
            offset + 1
        );
        assert!(exact_refs.is_subset(&mandatory));
        assert!(residual_refs.is_subset(&conditional));
        covered_mandatory.extend(exact_refs);
        covered_conditional.extend(residual_refs);
    }
    for (criterion, required) in [
        (0, "76-C134-P-distinct-on"),
        (0, "76-C134-P-with-ties"),
        (0, "76-C133-P-top"),
        (1, "76-C134-P-with-ties"),
        (1, "76-C133-N-offset-past"),
        (3, "76-C135-P-cte-nested"),
        (3, "76-C135-P-set-branch"),
        (3, "76-C135-N-set-zero"),
    ] {
        assert!(
            ids(&criteria[criterion]["mandatory_case_ids"]).contains(required),
            "AC {} lacks {} evidence",
            criterion + 1,
            required
        );
    }
    assert_eq!(covered_mandatory, mandatory);
    assert_eq!(covered_conditional, conditional);
}

fn public_proof(
    sql: &str,
    keys: &[i64],
) -> (sql_semantic_protocol::AnalysisBundle, PhysicalWriteState) {
    let parser = dialect_from_name("duckdb").expect("supported DuckDB dialect");
    let sql_input = SqlInput::inline(sql);
    let configured = [ConfiguredSqlInput::new(
        "parent-page",
        &sql_input,
        "duckdb",
        parser.as_ref(),
    )];
    let catalog = RelationCatalog::from_schemas(&[RelationSchema::new(
        "s",
        vec![SchemaColumn::from_sql_type("k", "INTEGER", "postgresql").unwrap()],
    )
    .unwrap()])
    .unwrap();
    let bundle = analyze_configured_inputs_with_catalog(&configured, &catalog).unwrap();
    let state = PhysicalWriteState::new(
        "s",
        keys.iter()
            .map(|key| {
                PhysicalWriteRow::new(BTreeMap::from([(
                    "k".to_string(),
                    LiteralValue::Number(key.to_string()),
                )]))
                .unwrap()
            })
            .collect(),
    )
    .unwrap();
    (bundle, state)
}

fn native_bag(sql: &str, keys: &[i64]) -> Vec<i64> {
    let connection = Connection::open_in_memory().expect("DuckDB native execution");
    connection
        .execute_batch("CREATE TABLE s (k INTEGER)")
        .unwrap();
    for key in keys {
        connection
            .execute_batch(&format!("INSERT INTO s VALUES ({key})"))
            .unwrap();
    }
    let count: i64 = connection
        .query_row("SELECT COUNT(*) FROM s", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count as usize, keys.len(), "complete physical source");
    connection
        .prepare(sql)
        .unwrap()
        .query_map([], |row| row.get::<_, i64>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
}

#[test]
fn independent_direct_and_nested_page_bags_match_native_execution() {
    let keys = [1, 2, 3, 4, 5];
    let direct = "SELECT k FROM s ORDER BY k LIMIT 2 OFFSET 1";
    assert_eq!(native_bag(direct, &keys), vec![2, 3]);
    let (bundle, state) = public_proof(direct, &keys);
    let layer = bundle.layers().last().unwrap().id();
    let PhysicalFinitePageProof::Exact(exact) =
        physical_finite_page_bag_plan(&bundle, layer, &[state])
    else {
        panic!("TASK-133 parent direct page must be exact");
    };
    assert_eq!(exact.rows.len(), 5);
    assert_eq!(exact.terminal.len(), 2);
    assert_eq!(exact.rows.iter().filter(|row| !row.survives).count(), 3);
    assert_eq!(exact.terminal_domains["k"].admits_null(), Some(false));

    let nested = "WITH page AS (SELECT k FROM s ORDER BY k LIMIT 4) \
SELECT k FROM page ORDER BY k LIMIT 1 OFFSET 2";
    assert_eq!(native_bag(nested, &keys), vec![3]);
    let (bundle, state) = public_proof(nested, &keys);
    let layer = bundle.layers().last().unwrap().id();
    let PhysicalComposedPageProof::Exact(exact) =
        physical_finite_composed_page_plan(&bundle, layer, &[state])
    else {
        panic!("TASK-135 parent composed page must be exact");
    };
    assert_eq!(exact.source_row_ids.len(), 5);
    assert_eq!(exact.selected_source_ids, vec!["s#2"]);
    assert_eq!(exact.rejected_source_ids.len(), 4);
    assert_eq!(exact.terminal.len(), 1);
    assert_eq!(exact.terminal_domains["k"].admits_null(), Some(false));
}

#[test]
fn independent_distinct_and_tie_group_native_oracles_preserve_multiplicity() {
    let keys = [3, 2, 2, 1];
    // A cutoff ties with both physical rows having k=2.
    let tie_oracle = "SELECT k FROM s WHERE k >= \
(SELECT MIN(k) FROM (SELECT k FROM s ORDER BY k DESC LIMIT 2)) \
ORDER BY k DESC";
    assert_eq!(native_bag(tie_oracle, &keys), vec![3, 2, 2]);
    assert_eq!(
        native_bag("SELECT DISTINCT k FROM s ORDER BY k", &keys),
        vec![1, 2, 3]
    );
}
