//! TASK-197: second-pass review of every frozen adapter cell against
//! independently frozen owner reviews, never against the parser mask alone.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

const V1: &str = include_str!("../docs/task-197-acceptance-freeze.json");
const V2: &str = include_str!("../docs/task-197-acceptance-ledger-v2.json");

fn parse(value: &str) -> Value {
    serde_json::from_str(value).expect("immutable adapter evidence")
}

fn owner_review_cells() -> BTreeMap<String, String> {
    let mut cells = BTreeMap::new();
    let all = [
        parse(include_str!(
            "../docs/task-192-independent-semantic-review.json"
        )),
        parse(include_str!(
            "../docs/task-193-independent-semantic-review.json"
        )),
        parse(include_str!(
            "../docs/task-194-independent-semantic-review.json"
        )),
        parse(include_str!(
            "../docs/task-195-independent-semantic-review.json"
        )),
        parse(include_str!(
            "../docs/task-196-independent-semantic-review.json"
        )),
    ];
    for (index, review) in all.iter().enumerate() {
        let owner = index + 192;
        let source = if owner == 196 {
            review["original_SQL_cases"]
                .as_array()
                .expect("original SQL cases")
                .iter()
                .flat_map(|case| {
                    case["cells"]
                        .as_array()
                        .expect("independent DDL/DML cells")
                        .iter()
                })
                .collect::<Vec<_>>()
        } else if owner == 192 {
            Vec::new()
        } else {
            review["cells"]
                .as_array()
                .expect("reviewed dialect cells")
                .iter()
                .collect()
        };
        if owner == 192 {
            for case in review["cases"].as_array().expect("original scalar cases") {
                let id = case["case_id"]
                    .as_str()
                    .expect("frozen review case identity");
                let decision = case["decision"].as_str().expect("review decision");
                let source_case = parse(include_str!("../docs/task-192-acceptance-freeze.json"));
                let item = source_case["cases"]
                    .as_array()
                    .expect("source freeze")
                    .iter()
                    .find(|item| item["id"] == id)
                    .expect("frozen source review");
                for cell in item["dialect_cells"].as_array().expect("13 parser cells") {
                    let name = cell["dialect"].as_str().expect("public parser");
                    let key = format!("{id}/{name}");
                    assert!(
                        cells.insert(key, decision.to_string()).is_none(),
                        "no duplicate reviewed scalar cell"
                    );
                }
            }
            continue;
        }
        for cell in source {
            let id = if owner == 196 {
                cell["id"].as_str()
            } else {
                cell["cell_id"].as_str()
            }
            .expect("canonical reviewed cell identity");
            let decision = if owner == 196 {
                cell["final_acceptance"].as_str()
            } else {
                cell["review_disposition"]
                    .as_str()
                    .or_else(|| cell["independent_review_disposition"].as_str())
            }
            .expect("independent bounded outcome classification");
            assert!(
                cells.insert(id.to_string(), decision.to_string()).is_none(),
                "{id}: no duplicated independent cell review"
            );
        }
    }
    cells
}

#[test]
fn all_2119_cross_adapter_dispositions_follow_independent_review_and_not_parser_only() {
    let frozen = parse(V1);
    let reviewed = parse(V2);
    let decisions = owner_review_cells();
    assert_eq!(decisions.len(), 2119);
    assert_eq!(reviewed["schema_version"], 2);
    assert_eq!(
        reviewed["immutable_initial_freeze"],
        "docs/task-197-acceptance-freeze.json"
    );
    assert_eq!(reviewed["task197_signoff"], false);
    assert_eq!(reviewed["parent_task88_signoff"], false);
    assert_eq!(reviewed["release_gate"], false);
    assert_eq!(reviewed["case_count"], 163);
    assert_eq!(reviewed["cell_count"], 2119);

    let originals = frozen["cases"]
        .as_array()
        .expect("immutable 163-case freeze");
    let cases = reviewed["cases"]
        .as_array()
        .expect("reviewed 163-case ledger");
    assert_eq!(cases.len(), originals.len());
    let mut ids = BTreeSet::new();
    let mut certified = 0;
    let mut conditional = 0;
    let mut unrepresentable = 0;
    let mut cell_count = 0;
    for (case, prior) in cases.iter().zip(originals) {
        let id = case["id"].as_str().expect("stable original SQL identity");
        assert!(ids.insert(id), "{id}: duplicate review case");
        assert_eq!(case["id"], prior["id"]);
        assert_eq!(case["sql"], prior["sql"]);
        assert_eq!(case["owner"], prior["owner"]);
        assert_eq!(case["feature_id"], prior["feature_id"]);
        assert_eq!(
            case["source_relations_mentioned"],
            prior["source_relations_mentioned"]
        );
        let no_facts = case["source_relations_mentioned"]
            .as_array()
            .expect("explicit source applicability")
            .is_empty();
        assert!(case["evidence"]["independent_native_sql_oracle"]
            .as_str()
            .is_some_and(|path| path.starts_with("docs/")));
        assert!(case["evidence"]["original_semantic_review_case"]
            .as_str()
            .is_some_and(|value| value.contains(id)));

        let entries = case["cells"].as_array().expect("all parser cells");
        let prior_entries = prior["cells"].as_array().expect("original parser cells");
        assert_eq!(entries.len(), 13);
        for (cell, before) in entries.iter().zip(prior_entries) {
            assert_eq!(
                cell["dialect"], before["dialect"],
                "{id}: parser identity drift"
            );
            assert_eq!(cell["original_observation"], before["original_observation"]);
            let dialect = cell["dialect"].as_str().expect("dialect");
            let key = format!("{id}/{dialect}");
            let decision = decisions
                .get(&key)
                .expect("separate independent SQL review");
            assert_eq!(
                cell["reviewed_semantic_disposition"],
                decision.as_str(),
                "{key}: review classification was not independently sourced"
            );
            let status = cell["original_observation"]
                .as_str()
                .expect("parser result");
            let expected = if no_facts || status == "R" || status == "U" {
                "unrepresentable"
            } else if status == "D"
                || decision.contains("unapproved")
                || decision.contains("conditional")
            {
                "conditional"
            } else {
                "certified"
            };
            assert_eq!(cell["status"], expected, "{key}: accidental certification");
            assert_eq!(cell["external_native_execution"], "unattested");
            assert!(cell["certification_scope"]
                .as_str()
                .is_some_and(|scope| !scope.is_empty()));
            assert!(cell["independent_review_source"]
                .as_str()
                .is_some_and(|path| path.starts_with("docs/task-")));
            if status == "R" || status == "U" {
                assert_eq!(expected, "unrepresentable");
            }
            match expected {
                "certified" => certified += 1,
                "conditional" => conditional += 1,
                "unrepresentable" => unrepresentable += 1,
                _ => unreachable!("fixed classified statuses"),
            }
            cell_count += 1;
        }
    }
    assert_eq!(ids.len(), 163);
    assert_eq!(cell_count, 2119);
    assert_eq!((certified, conditional, unrepresentable), (1861, 152, 106));
    assert_eq!(reviewed["cell_status_counts"]["certified"], certified);
    assert_eq!(reviewed["cell_status_counts"]["conditional"], conditional);
    assert_eq!(
        reviewed["cell_status_counts"]["unrepresentable"],
        unrepresentable
    );
}
