//! TASK-195 finite original SQL and 13-public-dialect acceptance boundary.
//! A C classification is canonical document equality, not typed semantic signoff.

use std::collections::BTreeSet;

use serde_json::Value;
use sql_semantic_protocol::{analyze_sql, dialect_from_name, to_json};

const FREEZE: &str = include_str!("../docs/task-195-acceptance-freeze.json");
const ORIGINAL: &str = include_str!("../docs/task-178-full-variant-inventory.json");

fn parse(source: &str) -> Value {
    serde_json::from_str(source).expect("versioned task-owned fixture")
}

fn without_dialect(mut document: Value) -> Value {
    for input in document["inputs"]
        .as_array_mut()
        .expect("public canonical input array")
    {
        input["dialect"] = Value::Null;
    }
    document
}

#[test]
fn original_36_cases_and_468_parser_cells_are_frozen_without_silent_exclusions() {
    let freeze = parse(FREEZE);
    let inventory = parse(ORIGINAL);
    assert_eq!(freeze["schema_version"], 1);
    assert_eq!(freeze["task_id"], "TASK-195");
    assert_eq!(freeze["case_count"], 36);
    assert_eq!(freeze["parser_cells"], 468);
    assert_eq!(freeze["dialects"], inventory["dialects"]);
    assert_eq!(freeze["positive_fixture_id"], "88-C195-P");
    assert_eq!(freeze["negative_fixture_id"], "88-C195-N");
    assert_eq!(freeze["independently_reviewed_cells"], 0);
    assert_eq!(freeze["task_complete"], false);
    assert_eq!(freeze["release_approved"], false);

    let originals = inventory["cases"]
        .as_array()
        .expect("original TASK-178 cases");
    let expected = originals
        .iter()
        .filter(|row| {
            row["feature_id"].as_str().is_some_and(|feature| {
                feature.starts_with("grouping.")
                    || feature.starts_with("windows.")
                    || feature.starts_with("ordering.")
            })
        })
        .map(|row| row["id"].as_str().expect("stable source ID"))
        .collect::<BTreeSet<_>>();
    assert_eq!(expected.len(), 36);

    let mut cases = BTreeSet::new();
    let mut cells = BTreeSet::new();
    let mut observed = [0_usize; 4]; // C, D, R, U
    for case in freeze["cases"].as_array().expect("36 fixed original cases") {
        let id = case["id"].as_str().expect("original SQL identity");
        assert!(cases.insert(id), "{id}: repeated case");
        let original = originals
            .iter()
            .find(|old| old["id"] == id)
            .expect("original source cannot be replaced");
        assert_eq!(case["sql"], original["sql"], "{id}: SQL source drift");
        assert_eq!(case["variant"], original["variant"], "{id}: variant drift");
        assert_eq!(
            case["feature_id"], original["feature_id"],
            "{id}: owner drift"
        );
        assert_eq!(
            case["original_observed_mask"], original["full_document_classification"],
            "{id}: observed parser/canonical status drift"
        );
        let native = &case["native_oracle"];
        let kind = native["kind"]
            .as_str()
            .expect("explicit physical evidence law");
        if matches!(
            kind,
            "native_parser_rejection" | "stochastic_conditional" | "unstable_tie_membership"
        ) {
            assert!(
                native["expected_terminal_bag"].is_null(),
                "{id}: conditional syntax/runtime cannot invent a deterministic bag"
            );
            assert!(native["boundary_owner"].is_string(), "{id}: boundary owner");
        } else {
            assert!(
                native["expected_terminal_bag"].is_array(),
                "{id}: independently specified literal terminal multiset"
            );
        }

        let dialects = case["cells"].as_array().expect("13 explicit dialect rows");
        assert_eq!(dialects.len(), 13, "{id}: no implicit dialect skip");
        for (index, cell) in dialects.iter().enumerate() {
            let dialect = cell["dialect"].as_str().expect("dialect identity");
            assert_eq!(cell["dialect"], freeze["dialects"][index], "{id}");
            let cell_id = format!("{id}/{dialect}");
            assert_eq!(cell["id"], cell_id);
            assert!(cells.insert(cell_id), "{id}: duplicate dialect cell");
            assert_eq!(
                cell["original_observed_status"],
                original["dialect_cells"][index]["observed_status"],
                "{id}/{dialect}: historical parser result must remain explicit"
            );
            assert_eq!(cell["external_vendor_execution"], "unattested");
            assert_eq!(cell["independent_semantic_review"], "pending");
            match cell["original_observed_status"]
                .as_str()
                .expect("classification")
            {
                "C" => observed[0] += 1,
                "D" => observed[1] += 1,
                "R" => observed[2] += 1,
                "U" => observed[3] += 1,
                status => panic!("{id}/{dialect}: unhandled status {status}"),
            }
        }
    }
    assert_eq!(cases, expected);
    assert_eq!(cells.len(), 468);
    assert_eq!(observed, [446, 21, 1, 0]);
}

#[test]
fn all_468_public_parser_cells_preserve_named_boundaries_and_supported_domains() {
    let fixture = parse(FREEZE);
    let mut observed = 0;
    for case in fixture["cases"].as_array().expect("finite original set") {
        let id = case["id"].as_str().expect("fixed case id");
        let sql = case["sql"].as_str().expect("original SQL");
        let mut baseline: Option<Value> = None;
        for cell in case["cells"].as_array().expect("all dialects") {
            let name = cell["dialect"].as_str().expect("named public dialect");
            let dialect = dialect_from_name(name).expect("public dialect registry");
            let analyzed = analyze_sql(sql, name, dialect.as_ref());
            let classification = cell["original_observed_status"].as_str().expect("observed");
            match classification {
                "R" => {
                    assert!(
                        analyzed.is_err(),
                        "{id}/{name}: historical parser rejection remains explicit"
                    );
                }
                "C" | "D" => {
                    let protocol = analyzed
                        .unwrap_or_else(|err| panic!("{id}/{name}: lost public analysis: {err}"));
                    let document = parse(&to_json(&protocol));
                    let columns = document["inputs"][0]["statements"][0]["output"]["columns"]
                        .as_array()
                        .expect("query output must have explicit columns");
                    assert!(!columns.is_empty(), "{id}/{name}: output omitted");
                    assert!(
                        columns.iter().all(|column| column["domain"].is_object()),
                        "{id}/{name}: uncertain domains must remain explicit"
                    );

                    if classification == "C" {
                        let normalized = without_dialect(document);
                        if let Some(first) = &baseline {
                            assert_eq!(
                                &normalized, first,
                                "{id}/{name}: portable canonical public document drift"
                            );
                        } else {
                            baseline = Some(normalized);
                        }
                    }
                    // D denotes independently owned canonical differences.
                    // It must not be silently promoted to C or native vendor equivalence.
                }
                other => panic!("{id}/{name}: unknown frozen classification {other}"),
            }
            observed += 1;
        }
    }
    assert_eq!(observed, 468);
}
