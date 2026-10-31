//! TASK-81 independent original acceptance crosswalk over merged child contracts.
//!
//! Each child owns executable SQL/DuckDB and typed physical-state oracles.
//! This guard checks that parent closure cannot silently omit their cases,
//! conditional boundaries, dialects, adapter sources, or original criteria.

use serde_json::Value;
use std::collections::BTreeSet;

const CHILDREN: [(&str, &str); 4] = [
    ("TASK-149", include_str!("../docs/task-149-acceptance-freeze.json")),
    ("TASK-150", include_str!("../docs/task-150-acceptance-freeze.json")),
    ("TASK-151", include_str!("../docs/task-151-acceptance-freeze.json")),
    ("TASK-152", include_str!("../docs/task-152-acceptance-freeze.json")),
];

fn json(text: &str) -> Value {
    serde_json::from_str(text).expect("valid immutable acceptance JSON")
}

fn ids(value: &Value) -> BTreeSet<String> {
    let array = value.as_array().expect("stable ID array");
    let unique: BTreeSet<String> = array
        .iter()
        .map(|item| item.as_str().expect("string ID").to_owned())
        .collect();
    assert_eq!(array.len(), unique.len(), "repeated acceptance ID");
    unique
}

#[test]
fn task_81_parent_freeze_covers_every_child_case_and_explicit_boundary() {
    let parent = json(include_str!("../docs/task-81-parent-acceptance-freeze.json"));
    assert_eq!(parent["task"], "TASK-81");
    assert_eq!(parent["version"], 1);
    assert_eq!(parent["release_approved"], false);
    assert_eq!(parent["mandatory_unverified"], serde_json::json!([]));

    let mandatory = ids(&parent["mandatory_case_ids"]);
    let conditional = ids(&parent["conditional_case_ids"]);
    assert_eq!(mandatory.len(), 36);
    assert_eq!(conditional.len(), 10);
    assert!(mandatory.is_disjoint(&conditional));
    let case_index = parent["case_index"].as_object().expect("case index");
    assert_eq!(case_index.len(), mandatory.len() + conditional.len());
    let required_dialects = ids(&parent["required_dialects"]);
    let required_adapters = ids(&parent["required_adapters"]);
    assert_eq!(required_dialects.len(), 13);
    assert_eq!(
        required_adapters,
        ids(&serde_json::json!([
            "direct_sql",
            "dbt_compiled_catalog",
            "odcs_schema"
        ]))
    );

    let mut all_mandatory = BTreeSet::new();
    let mut all_conditional = BTreeSet::new();
    let mut all_fixtures = BTreeSet::new();
    for (task, raw) in CHILDREN {
        let child = json(raw);
        let record = &parent["source_of_truth"][task];
        assert_eq!(child["owner"], task);
        assert_eq!(child["parent"], "TASK-81");
        assert_eq!(record["version"], child["version"]);
        assert_eq!(record["fixture_ids"], child["fixture_ids"]);
        assert_eq!(
            ids(&child["dialects"]),
            required_dialects,
            "{task}: incomplete dialect inventory"
        );
        assert_eq!(
            ids(&child["adapters"]),
            required_adapters,
            "{task}: incomplete adapter inventory"
        );
        all_fixtures.extend(ids(&child["fixture_ids"]));

        let declared_mandatory = ids(&record["mandatory_case_ids"]);
        let declared_conditional = ids(&record["conditional_case_ids"]);
        for case in child["cases"].as_array().expect("child cases") {
            let id = case["id"].as_str().expect("stable case ID");
            let indexed = &parent["case_index"][id];
            assert_eq!(indexed["child"], task);
            assert_eq!(indexed["fixture_id"], case["fixture_id"]);
            assert!(
                ids(&child["fixture_ids"]).contains(
                    case["fixture_id"].as_str().expect("case fixture ID")
                ),
                "{id}: unknown fixture ID"
            );
            assert!(case["target_before"].is_array(), "{id}: no complete target prestate");
            let actual_proof = case
                .get("proof")
                .or_else(|| case.get("expected_failure"))
                .expect("typed physical outcome");
            assert_eq!(&indexed["expected_proof"], actual_proof, "{id}: proof changed");
            if declared_mandatory.contains(id) {
                assert_eq!(indexed["classification"], "mandatory");
                assert!(all_mandatory.insert(id.to_owned()), "{id}: duplicate");
                if matches!(
                    actual_proof.as_str(),
                    Some("exact" | "Exact" | "exact_with_attested_default" | "exact_not_true")
                ) {
                    assert!(
                        case["target_after"].is_array() || case["expected_final"].is_array(),
                        "{id}: mandatory Exact must have complete target poststate"
                    );
                }
            } else {
                assert!(declared_conditional.contains(id), "{id}: unclassified case");
                assert_eq!(indexed["classification"], "conditional");
                assert!(all_conditional.insert(id.to_owned()), "{id}: duplicate");
            }
        }

        // Some child contracts carry separately owned conditional evidence
        // rather than a case with a SQL prestate.
        for boundary in child
            .get("conditional")
            .or_else(|| child.get("conditional_boundaries"))
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let id = boundary["id"]
                .as_str()
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| {
                    format!(
                        "81-PARENT-C-{}",
                        boundary["family"].as_str().expect("conditional family")
                    )
                });
            assert!(declared_conditional.contains(&id), "{id}: untracked boundary");
            assert_eq!(parent["case_index"][&id]["owner"], task);
            assert!(
                parent["case_index"][&id]["release_disposition"]
                    .as_str()
                    .is_some_and(|text| !text.is_empty()),
                "{id}: missing conditional release disposition"
            );
            assert!(all_conditional.insert(id), "{task}: duplicate boundary");
        }
        assert_eq!(
            declared_mandatory,
            all_mandatory
                .iter()
                .filter(|id| id.starts_with(&format!("81-C{}", &task[5..])))
                .cloned()
                .collect(),
            "{task}: missing mandatory case"
        );
        assert_eq!(
            declared_conditional,
            all_conditional
                .iter()
                .filter(|id| {
                    id.starts_with(&format!("81-C{}", &task[5..]))
                        || (task == "TASK-152" && id.starts_with("81-PARENT-C-"))
                })
                .cloned()
                .collect(),
            "{task}: missing conditional boundary"
        );
    }
    assert_eq!(all_mandatory, mandatory);
    assert_eq!(all_conditional, conditional);
    assert_eq!(all_fixtures, ids(&parent["fixture_ids"]));

    let original = parent["original_acceptance_criteria"]
        .as_array()
        .expect("parent AC crosswalk");
    assert_eq!(original.len(), 6);
    for (index, ac) in original.iter().enumerate() {
        assert_eq!(ac["id"], index + 1);
        let required = ids(&ac["mandatory_case_ids"]);
        assert!(!required.is_empty(), "AC {} has no mandatory oracle", index + 1);
        assert!(required.is_subset(&mandatory), "AC {} has unknown case", index + 1);
        assert!(ids(&ac["conditional_case_ids"]).is_subset(&conditional));
    }

    // MERGE is already executable from the ordered write-state public API.
    // This is the independent full physical/native oracle for parent AC #4.
    assert_eq!(
        original[3]["additional_oracle"],
        "tests/physical_write_states.rs::task_106_composed_insert_update_merge_share_one_ordered_source_state"
    );
    assert!(include_str!("physical_write_states.rs").contains(
        "fn task_106_composed_insert_update_merge_share_one_ordered_source_state"
    ));
}

#[test]
fn task_81_manifest_keeps_parent_signoff_independent_of_release() {
    let manifest = json(include_str!("../docs/coverage-manifest.json"));
    let parent = json(include_str!("../docs/task-81-parent-acceptance-freeze.json"));
    let entry = &manifest["task_81_parent_acceptance"];
    assert_eq!(entry["owner"], "TASK-81");
    assert_eq!(entry["freeze"], "docs/task-81-parent-acceptance-freeze.json");
    assert_eq!(entry["mandatory_cases"], parent["mandatory_case_ids"].as_array().unwrap().len());
    assert_eq!(entry["conditional_cases"], parent["conditional_case_ids"].as_array().unwrap().len());
    assert_eq!(entry["original_ac"], serde_json::json!([1, 2, 3, 4, 5, 6]));
    assert_eq!(entry["release_approved"], false);
    assert_eq!(entry["parent_signed_off"], parent["parent_signed_off"]);
}
