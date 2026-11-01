//! TASK-156 executable acceptance inventory and complete physical occurrence contracts.

use std::collections::BTreeSet;

use serde_json::Value;

fn frozen() -> Value {
    serde_json::from_str(include_str!("../docs/task-156-acceptance-freeze.json")).unwrap()
}

#[test]
fn task_156_has_a_versioned_finite_matrix_and_all_original_parent_mappings() {
    let f = frozen();
    let manifest: Value =
        serde_json::from_str(include_str!("../docs/coverage-manifest.json")).unwrap();
    let entry = &manifest["task_156_computed_sequential_merge_acceptance"];
    assert_eq!(f["version"], 2);
    assert_eq!(f["owner"], "TASK-156");
    assert_eq!(f["parent"], "TASK-82");
    assert_eq!(
        f["previous_snapshot"],
        "docs/task-156-acceptance-freeze-v1.json"
    );
    assert_eq!(
        f["fixture_ids"],
        serde_json::json!(["82-C156-P", "82-C156-N"])
    );
    assert_eq!(f["task_ac"], serde_json::json!([1, 2, 3, 4, 5]));
    assert_eq!(
        f["original_parent_ac"],
        serde_json::json!([1, 2, 3, 4, 5, 6])
    );
    assert_eq!(entry["freeze"], "docs/task-156-acceptance-freeze.json");
    assert_eq!(entry["freeze_version"], 2);
    assert_eq!(entry["parent_signed_off"], false);
    assert_eq!(f["dialects"].as_array().unwrap().len(), 13);
    assert_eq!(f["dialect_syntax"].as_array().unwrap().len(), 5);
    assert_eq!(f["dialect_cells_required"], 65);
    assert_eq!(f["adapter_paths_required"], 3);
    assert_eq!(f["adapter_cases_required"], 6);
    assert_eq!(f["required_ci"].as_array().unwrap().len(), 8);
    assert_eq!(f["acceptance_mapping"].as_object().unwrap().len(), 5);
    assert_eq!(f["mandatory_oracle_case_ids"].as_array().unwrap().len(), 6);
    assert!(!f["reviewed_matrix_change"].as_str().unwrap().is_empty());
    assert_eq!(f["mandatory_signoff"]["parent_signoff"], false);
}

fn typed_domains(rows: &Value, columns: &Value, id: &str) {
    let rows = rows.as_array().unwrap();
    for column in ["id", "score"] {
        let values = rows.iter().map(|row| &row[column]).collect::<Vec<_>>();
        let nonnull = values.iter().filter_map(|v| v.as_i64()).collect::<Vec<_>>();
        let domain = &columns[column];
        assert_eq!(domain["type"], "INTEGER", "{id}/{column}");
        assert_eq!(
            domain["lower"].as_i64(),
            nonnull.iter().min().copied(),
            "{id}"
        );
        assert_eq!(
            domain["upper"].as_i64(),
            nonnull.iter().max().copied(),
            "{id}"
        );
        assert_eq!(
            domain["nullable"],
            values.iter().any(|v| v.is_null()),
            "{id}"
        );
        assert_eq!(domain["lower_inclusive"], !nonnull.is_empty(), "{id}");
        assert_eq!(domain["upper_inclusive"], !nonnull.is_empty(), "{id}");
        let mut frequencies = std::collections::BTreeMap::new();
        for value in &values {
            let key = value
                .as_i64()
                .map_or_else(|| "NULL".to_string(), |n| n.to_string());
            *frequencies.entry(key).or_insert(0_u64) += 1;
        }
        let frozen = domain["frequencies"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(key, count)| (key.clone(), count.as_u64().unwrap()))
            .collect::<std::collections::BTreeMap<_, _>>();
        assert_eq!(frequencies, frozen, "{id}: full physical frequency bag");
    }
}

fn partition(rows: &Value, yes: &Value, no: &Value, id: &str) {
    let actual = yes
        .as_array()
        .unwrap()
        .iter()
        .chain(no.as_array().unwrap())
        .map(|v| v.as_u64().unwrap())
        .collect::<Vec<_>>();
    let unique = actual.iter().copied().collect::<BTreeSet<_>>();
    assert_eq!(
        actual.len(),
        rows.as_array().unwrap().len(),
        "{id}: exact partition"
    );
    assert_eq!(unique.len(), actual.len(), "{id}: no repeated occurrence");
    assert_eq!(
        unique,
        (0..rows.as_array().unwrap().len() as u64).collect::<BTreeSet<_>>(),
        "{id}: every occurrence is TRUE or NOT TRUE"
    );
}

#[test]
fn every_case_has_complete_typed_bags_and_true_not_true_witnesses() {
    let matrix = frozen();
    let cases = matrix["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 6);
    let mut ids = BTreeSet::new();
    let mut counts = (0, 0, 0);
    for case in cases {
        let id = case["id"].as_str().unwrap();
        assert!(ids.insert(id), "duplicate fixture: {id}");
        assert!(id.starts_with("82-C156-P-") || id.starts_with("82-C156-N-"));
        assert!(!case["variant"].as_str().unwrap().is_empty());
        for (name, bag) in case["initial"].as_object().unwrap() {
            typed_domains(bag, &case["initial_domains"][name], id);
        }
        let steps = case["steps"].as_array().unwrap();
        assert!(
            !steps.is_empty(),
            "{id}: at least one complete source/target step"
        );
        for step in steps {
            assert!(
                step["sql"].as_str().unwrap().starts_with("MERGE INTO"),
                "{id}"
            );
            assert_ne!(
                step["target"], step["source"],
                "{id}: independent identities"
            );
            typed_domains(&step["before"], &step["before_domains"], id);
            typed_domains(&step["source_before"], &step["source_before_domains"], id);
            partition(
                &step["before"],
                &step["on_true_target_ordinals"],
                &step["on_not_true_target_ordinals"],
                id,
            );
            partition(
                &step["source_before"],
                &step["on_true_source_ordinals"],
                &step["on_not_true_source_ordinals"],
                id,
            );
            if step["after"].is_array() {
                typed_domains(&step["after"], &step["after_domains"], id);
                let affected = &step["affected"];
                assert!(affected["inserted"].is_u64());
                assert!(affected["updated"].is_u64());
                assert!(affected["deleted"].is_u64());
                assert!(step["affected_target_ordinals"].is_array());
            } else {
                assert!(
                    step["after_domains"].is_null(),
                    "{id}: no invented uncertain poststate"
                );
            }
        }
        match case["classification"].as_str().unwrap() {
            "Exact" => {
                counts.0 += 1;
                let final_bags = case["final"].as_object().unwrap();
                for (relation, bag) in final_bags {
                    typed_domains(bag, &case["final_domains"][relation], id);
                }
            }
            "Impossible" => {
                counts.1 += 1;
                assert!(case["final"].is_null());
                assert!(!case["impossible_reason"].as_str().unwrap().is_empty());
            }
            "Residual" => {
                counts.2 += 1;
                assert!(case["final"].is_null());
                assert_eq!(case["gap"], "UnprovedOperation");
                assert_eq!(case["conditional_owner"], "TASK-156");
                assert!(!case["release_disposition"].as_str().unwrap().is_empty());
            }
            other => panic!("{id}: unreviewed outcome class {other}"),
        }
    }
    assert_eq!(counts, (4, 1, 1));
    assert_eq!(
        ids,
        matrix["case_ids_required"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_owned())
            .collect::<BTreeSet<_>>()
    );
}
