//! TASK-155: finite preimplementation freeze, provenance and signoff invariants.

use std::collections::BTreeSet;
use serde_json::Value;

fn freeze() -> Value {
    serde_json::from_str(include_str!("../docs/task-155-acceptance-freeze.json")).unwrap()
}

#[test]
fn every_named_task_155_acceptance_cell_is_finite_and_owned() {
    let frozen = freeze();
    let manifest: Value =
        serde_json::from_str(include_str!("../docs/coverage-manifest.json")).unwrap();
    let record = &manifest["task_155_merge_conflict_acceptance"];
    assert_eq!(frozen["version"], 3);
    assert_eq!(frozen["owner"], "TASK-155");
    assert_eq!(frozen["parent"], "TASK-82");
    assert_eq!(frozen["fixture_ids"], serde_json::json!(["82-C155-P", "82-C155-N"]));
    assert_eq!(frozen["previous_snapshot"], "docs/task-155-acceptance-freeze-v2.json");
    assert_eq!(frozen["task_ac"], serde_json::json!([1, 2, 3, 4, 5]));
    assert_eq!(frozen["original_parent_ac"], serde_json::json!([1, 2, 3, 5, 6]));
    assert_eq!(frozen["remaining_mandatory_matrix"], serde_json::json!([]));
    assert_eq!(record["freeze"], "docs/task-155-acceptance-freeze.json");
    assert_eq!(record["freeze_version"], 2);
    assert_eq!(record["parent_signed_off"], false);
    assert_eq!(frozen["dialects"].as_array().unwrap().len(), 13);
    assert_eq!(frozen["dialect_syntax"].as_array().unwrap().len(), 4);
    assert_eq!(frozen["dialect_cells_required"], 52);
    assert_eq!(frozen["adapter_cases_required"], 12);
    assert_eq!(frozen["adapter_paths_required"], 3);
    assert_eq!(frozen["evidence_sources"].as_array().unwrap().len(), 3);
    assert_eq!(frozen["required_ci"].as_array().unwrap().len(), 8);
    for name in frozen["dialects"].as_array().unwrap() {
        assert_eq!(
            frozen["dialect_expected"][name.as_str().unwrap()]
                .as_array()
                .unwrap()
                .len(),
            4,
            "each dialect has an explicit parser verdict"
        );
    }
    assert_eq!(frozen["signoff"]["parent_done"], false);
    assert_eq!(frozen["signoff"]["release_approved"], false);
}

#[test]
fn all_cases_have_typed_domain_frequencies_and_closed_true_not_true_partitions() {
    let f = freeze();
    let cases = f["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 12);
    let mut ids = BTreeSet::new();
    let mut counts = (0, 0, 0);
    for case in cases {
        let id = case["id"].as_str().unwrap();
        assert!(ids.insert(id), "duplicate fixture id {id}");
        assert!(id.starts_with("82-C155-P-") || id.starts_with("82-C155-N-"));
        assert!(!case["variant"].as_str().unwrap().is_empty());
        assert!(!case["sql"].as_str().unwrap().is_empty());
        for (bag, domains, cols) in [
            (&case["target_before"], &case["before_domains"], &["id", "score"][..]),
            (&case["source"], &case["source_domains"], &["id", "score"][..]),
            (&case["parent"], &case["parent_domains"], &["id"][..]),
            (&case["target_after"], &case["after_domains"], &["id", "score"][..]),
        ] {
            let Some(rows) = bag.as_array() else {
                assert!(domains.is_null(), "{id}: absent bag has no invented domain");
                continue;
            };
            for column in cols {
                let expected = &domains[*column];
                assert_eq!(expected["type"], "INTEGER", "{id}/{column}");
                let nonnull = rows.iter().filter_map(|row| row[*column].as_i64()).collect::<Vec<_>>();
                assert_eq!(expected["lower"].as_i64(), nonnull.iter().min().copied(), "{id}");
                assert_eq!(expected["upper"].as_i64(), nonnull.iter().max().copied(), "{id}");
                assert_eq!(expected["nullable"], rows.iter().any(|row| row[*column].is_null()), "{id}");
                assert_eq!(expected["lower_inclusive"], !nonnull.is_empty(), "{id}");
                assert_eq!(expected["upper_inclusive"], !nonnull.is_empty(), "{id}");
                assert_eq!(
                    expected["frequencies"]
                        .as_object()
                        .unwrap()
                        .values()
                        .map(|count| count.as_u64().unwrap())
                        .sum::<u64>(),
                    rows.len() as u64,
                    "{id}: complete physical frequency bag"
                );
            }
        }
        if case["sql"].as_str().unwrap().starts_with("MERGE") {
            for (rows, yes, no) in [
                (&case["target_before"], &case["on_true_target_ordinals"], &case["on_not_true_target_ordinals"]),
                (&case["source"], &case["on_true_source_ordinals"], &case["on_not_true_source_ordinals"]),
            ] {
                let n = rows.as_array().unwrap().len();
                let truth = yes.as_array().unwrap();
                let nontruth = no.as_array().unwrap();
                let union = truth.iter().chain(nontruth).map(|x| x.as_u64().unwrap()).collect::<BTreeSet<_>>();
                assert_eq!(union, (0..n as u64).collect::<BTreeSet<_>>(), "{id}: exhaustive 3VL truth partition");
                assert_eq!(truth.len() + nontruth.len(), n, "{id}: no duplicated physical occurrence");
            }
        }
        match case["classification"].as_str().unwrap() {
            "Exact" => {
                counts.0 += 1;
                assert!(case["target_after"].is_array());
                assert!(case["affected"].is_object());
                assert_eq!(case["after_domains"].as_object().unwrap().len(), 2);
            }
            "Impossible" => {
                counts.1 += 1;
                assert!(case["target_after"].is_null(), "{id}: contradiction has no poststate");
            }
            "Residual" => {
                counts.2 += 1;
                assert!(case["target_after"].is_null(), "{id}: uncertainty cannot invent rows");
                assert!(case["gap"].is_string());
                assert_eq!(case["conditional_owner"], "TASK-155");
                assert!(case["release_disposition"].as_str().unwrap().len() > 15);
            }
            unexpected => panic!("{id}: unreviewed proof disposition {unexpected}"),
        }
    }
    assert_eq!(counts, (5, 2, 5));
}
