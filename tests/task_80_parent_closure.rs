//! Independent TASK-80 parent acceptance inventory over four merged INSERT children.
//!
//! These assertions protect the crosswalk and prevent negative or vendor-only
//! boundaries from being promoted to exact semantics. Child-owned integration
//! tests remain responsible for executing the complete DuckDB and adapter oracles.

use serde_json::{json, Value};
use std::collections::BTreeSet;

const CHILDREN: [(&str, &str); 4] = [
    (
        "TASK-145",
        include_str!("../docs/task-145-acceptance-freeze.json"),
    ),
    (
        "TASK-146",
        include_str!("../docs/task-146-acceptance-freeze.json"),
    ),
    (
        "TASK-147",
        include_str!("../docs/task-147-acceptance-freeze.json"),
    ),
    (
        "TASK-148",
        include_str!("../docs/task-148-acceptance-freeze.json"),
    ),
];

fn parse(raw: &str) -> Value {
    serde_json::from_str(raw).expect("checked-in immutable acceptance contract")
}

fn unique_strings(value: &Value) -> BTreeSet<String> {
    let array = value.as_array().expect("an explicit inventory array");
    let unique = array
        .iter()
        .map(|v| v.as_str().expect("stable identifier").to_owned())
        .collect::<BTreeSet<_>>();
    assert_eq!(unique.len(), array.len(), "duplicate inventory entry");
    unique
}

fn nonempty(value: &Value) -> bool {
    value.as_str().is_some_and(|s| !s.trim().is_empty())
}

fn cases<'a>(task: &str, child: &'a Value) -> (Vec<&'a Value>, Vec<&'a Value>) {
    if task == "TASK-147" {
        return (
            child["mandatory_cases"]
                .as_array()
                .expect("mandatory conflict cases")
                .iter()
                .collect(),
            child["conditional_cases"]
                .as_array()
                .expect("conditional vendor boundaries")
                .iter()
                .collect(),
        );
    }
    let (mut mandatory, mut conditional) = (Vec::new(), Vec::new());
    for case in child["cases"].as_array().expect("frozen cases") {
        if case["classification"]
            .as_str()
            .is_some_and(|s| s.starts_with("conditional"))
        {
            conditional.push(case);
        } else {
            mandatory.push(case);
        }
    }
    if let Some(extra) = child["conditional_cases"].as_array() {
        conditional.extend(extra);
    }
    (mandatory, conditional)
}

fn has_typed_domain(case: &Value) -> bool {
    [
        "domain",
        "terminal_typed_domains",
        "typed_domains",
        "typed_domain",
    ]
    .iter()
    .any(|key| case[*key].is_object())
}

fn has_complete_target_bag(case: &Value) -> bool {
    ["expected_target_bag", "expected_poststate", "target_after"]
        .iter()
        .any(|key| case[*key].is_array())
}

#[test]
fn parent_cases_match_all_child_inventory_and_preserve_proof_boundaries() {
    let freeze = parse(include_str!(
        "../docs/task-80-parent-acceptance-freeze.json"
    ));
    assert_eq!(freeze["task"], "TASK-80");
    assert_eq!(freeze["version"], 2);
    assert_eq!(
        freeze["previous_snapshot"],
        "docs/task-80-parent-acceptance-freeze-v1.json"
    );
    assert_eq!(freeze["parent_signed_off"], true);
    assert_eq!(freeze["release_approved"], false);

    let expected_mandatory = unique_strings(&freeze["mandatory_case_ids"]);
    let expected_conditional = unique_strings(&freeze["conditional_case_ids"]);
    assert_eq!(expected_mandatory.len(), 32);
    assert_eq!(expected_conditional.len(), 18);
    assert!(expected_mandatory.is_disjoint(&expected_conditional));
    assert_eq!(freeze["case_index"].as_object().unwrap().len(), 50);

    let mut found_mandatory = BTreeSet::new();
    let mut found_conditional = BTreeSet::new();
    let mut found_fixtures = BTreeSet::new();

    for (task, raw) in CHILDREN {
        let child = parse(raw);
        let record = &freeze["merged_children"][task];
        assert_eq!(child["owner"], task);
        assert_eq!(child["parent"], "TASK-80");
        assert_eq!(record["version"], child["version"]);
        assert!(nonempty(&record["merged_pr"]));

        let fixtures = unique_strings(&record["fixture_ids"]);
        assert_eq!(fixtures, unique_strings(&child["fixture_ids"]));
        assert_eq!(fixtures.len(), 2);
        found_fixtures.extend(fixtures.iter().cloned());

        let (mandatory, conditional) = cases(task, &child);
        let mut local_mandatory = BTreeSet::new();
        let mut local_conditional = BTreeSet::new();

        for case in mandatory {
            let id = case["id"].as_str().expect("mandatory case ID");
            let index = &freeze["case_index"][id];
            assert_eq!(index["owner"], task, "{id}: wrong case owner");
            assert_eq!(index["classification"], "mandatory", "{id}");
            assert!(
                fixtures.contains(index["fixture_id"].as_str().unwrap()),
                "{id}"
            );
            assert!(
                has_complete_target_bag(case)
                    || case["proof"] == "Impossible"
                    || case["proof"] == "Residual",
                "{id}: missing complete poststate"
            );
            if case["proof"] == "Exact" {
                assert!(has_typed_domain(case), "{id}: missing typed exact output");
                assert!(has_complete_target_bag(case), "{id}: exact needs full bag");
            } else {
                assert!(
                    case["proof"] == "Impossible" || case["proof"] == "Residual",
                    "{id}: unspecified negative proof"
                );
            }
            assert!(local_mandatory.insert(id.to_owned()), "{id}: duplicate");
            assert!(found_mandatory.insert(id.to_owned()), "{id}: duplicate");
        }
        for case in conditional {
            let id = case["id"].as_str().expect("conditional case ID");
            let index = &freeze["case_index"][id];
            assert_eq!(index["owner"], task, "{id}");
            assert_eq!(index["classification"], "conditional", "{id}");
            assert!(
                fixtures.contains(index["fixture_id"].as_str().unwrap()),
                "{id}"
            );
            // TASK-146 declares one common conditional owner at the freeze root.
            // Other child contracts attach ownership to individual cases.
            if case["owner"].is_null() {
                assert_eq!(
                    child["conditional_ownership"]["owner"], task,
                    "{id}: missing frozen residual owner"
                );
            } else {
                assert_eq!(case["owner"], task, "{id}: wrong residual owner");
            }
            assert!(
                nonempty(&case["release_disposition"]) || nonempty(&case["disposition"]),
                "{id}: missing reviewed disposition"
            );
            assert!(
                case["proof"].is_null() || case["proof"] == "Residual",
                "{id}: conditional law cannot claim native exactness"
            );
            assert!(local_conditional.insert(id.to_owned()), "{id}: duplicate");
            assert!(found_conditional.insert(id.to_owned()), "{id}: duplicate");
        }
        assert_eq!(
            local_mandatory,
            unique_strings(&record["mandatory_case_ids"]),
            "{task}"
        );
        assert_eq!(
            local_conditional,
            unique_strings(&record["conditional_case_ids"]),
            "{task}"
        );

        let dialects = unique_strings(&child["dialects"]);
        assert_eq!(
            dialects,
            unique_strings(&freeze["dialect_identities"]),
            "{task}"
        );
        assert_eq!(unique_strings(&child["adapters"]).len(), 3, "{task}");
    }
    assert_eq!(found_mandatory, expected_mandatory);
    assert_eq!(found_conditional, expected_conditional);
    assert_eq!(found_fixtures, unique_strings(&freeze["fixture_ids"]));
}

#[test]
fn parent_ac_mapping_and_manifest_preserve_independent_signoff() {
    let freeze = parse(include_str!(
        "../docs/task-80-parent-acceptance-freeze.json"
    ));
    let manifest = parse(include_str!("../docs/coverage-manifest.json"));
    let original = freeze["original_acceptance_criteria"]
        .as_array()
        .expect("all six original AC");
    assert_eq!(original.len(), 6);
    let mandatory = unique_strings(&freeze["mandatory_case_ids"]);
    let conditional = unique_strings(&freeze["conditional_case_ids"]);
    for (index, ac) in original.iter().enumerate() {
        assert_eq!(ac["id"], json!(index + 1));
        assert!(nonempty(&ac["obligation"]));
        let m = unique_strings(&ac["mandatory_case_ids"]);
        assert!(!m.is_empty(), "AC {} needs mandatory evidence", index + 1);
        assert!(m.is_subset(&mandatory));
        assert!(unique_strings(&ac["conditional_case_ids"]).is_subset(&conditional));
    }

    let extra = freeze["parent_replay_oracles"]
        .as_array()
        .expect("two independent parent replay requirements");
    assert_eq!(extra.len(), 2);
    let ids = extra
        .iter()
        .map(|item| item["id"].as_str().expect("replay ID").to_owned())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        ids,
        BTreeSet::from([
            "80-P-REPLAY-CONFLICT".to_owned(),
            "80-N-REPLAY-PK".to_owned(),
        ])
    );
    assert_eq!(
        original[3]["supplementary_parent_oracle_ids"],
        json!(["80-P-REPLAY-CONFLICT", "80-N-REPLAY-PK"])
    );
    for case in extra {
        assert_eq!(case["owner"], "TASK-80");
        assert!(nonempty(&case["sql"]));
        assert!(nonempty(&case["native_oracle"]));
        assert!(nonempty(&case["proof_positive"]));
        assert!(nonempty(&case["proof_not_true"]));
        assert!(nonempty(&case["release_disposition"]));
        assert_eq!(case["executions"], 2);
        let child = case["child_case_id"].as_str().unwrap();
        assert!(
            mandatory.contains(child),
            "{child} is not a mandatory child witness"
        );
    }

    let entry = &manifest["task_80_parent_acceptance"];
    assert_eq!(entry["owner"], "TASK-80");
    assert_eq!(
        entry["freeze"],
        "docs/task-80-parent-acceptance-freeze.json"
    );
    assert_eq!(entry["original_ac"], json!([1, 2, 3, 4, 5, 6]));
    assert_eq!(entry["freeze_version"], 2);
    assert_eq!(entry["replay_oracles"], 2);
    assert_eq!(entry["replay_test"], "tests/task_80_parent_replay.rs");
    assert_eq!(entry["mandatory_cases"], json!(mandatory.len()));
    assert_eq!(entry["conditional_cases"], json!(conditional.len()));
    assert_eq!(entry["parent_signed_off"], true);
    assert_eq!(entry["release_approved"], false);
}
