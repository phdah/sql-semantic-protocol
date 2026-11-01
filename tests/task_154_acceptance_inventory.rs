//! TASK-154: executable completeness guard for the preimplementation contract.

use std::collections::BTreeSet;

use serde_json::Value;

fn matrix() -> Value {
    serde_json::from_str(include_str!("../docs/task-154-acceptance-freeze.json")).unwrap()
}

#[test]
fn frozen_unmatched_merge_acceptance_is_finite_and_linked() {
    let frozen = matrix();
    let manifest: Value =
        serde_json::from_str(include_str!("../docs/coverage-manifest.json")).unwrap();
    let record = &manifest["task_154_unmatched_merge_acceptance"];
    assert_eq!(frozen["version"], 2);
    assert_eq!(frozen["owner"], "TASK-154");
    assert_eq!(
        frozen["previous_snapshot"],
        "docs/task-154-acceptance-freeze-v1.json"
    );
    assert_eq!(frozen["parent"], "TASK-82");
    assert_eq!(
        frozen["fixture_ids"],
        serde_json::json!(["82-C154-P", "82-C154-N"])
    );
    assert_eq!(
        frozen["original_parent_ac"],
        serde_json::json!([1, 2, 3, 4, 5, 6])
    );
    assert_eq!(frozen["task_ac"], serde_json::json!([1, 2, 3, 4, 5]));
    assert_eq!(record["freeze"], "docs/task-154-acceptance-freeze.json");
    assert_eq!(record["freeze_version"], frozen["version"]);
    assert_eq!(record["owner"], "TASK-154");
    assert_eq!(record["parent"], "TASK-82");
    assert_eq!(record["parent_signed_off"], false);
    assert_eq!(frozen["dialects"].as_array().unwrap().len(), 13);
    assert_eq!(frozen["dialect_cells_required"], 65);
    assert_eq!(frozen["parser_syntax"].as_array().unwrap().len(), 5);
    for dialect in frozen["dialects"].as_array().unwrap() {
        let name = dialect.as_str().unwrap();
        assert_eq!(
            frozen["dialect_expected"][name].as_array().unwrap().len(),
            5,
            "{name}"
        );
    }
    assert_eq!(frozen["adapter_sources"].as_array().unwrap().len(), 3);
    assert_eq!(frozen["adapter_cases_required"], 3);
    assert_eq!(frozen["required_ci"].as_array().unwrap().len(), 7);
    assert_eq!(frozen["acceptance_map"].as_object().unwrap().len(), 5);
}

#[test]
fn frozen_cases_have_exact_typed_source_and_terminal_domains_and_on_partitions() {
    let frozen = matrix();
    let cases = frozen["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 12);
    let mut ids = BTreeSet::new();
    let mut exact = 0;
    let mut residual = 0;
    let mut impossible = 0;

    for case in cases {
        let id = case["id"].as_str().unwrap();
        assert!(ids.insert(id), "duplicate case id: {id}");
        assert!(id.starts_with("82-C154-P-") || id.starts_with("82-C154-N-"));
        assert!(!case["sql"].as_str().unwrap().is_empty());
        assert!(case["sql"].as_str().unwrap().contains("WHEN NOT MATCHED"));
        assert_eq!(
            case["evidence"]["comparison"],
            "signed INTEGER equality; SQL NULL never matches"
        );
        assert!(!case["variant"].as_str().unwrap().is_empty());
        assert!(case["target_before"].is_array());
        assert_eq!(case["before_domains"].as_object().unwrap().len(), 2);
        let proof = case["proof"].as_str().unwrap();
        match proof {
            "Exact" => {
                exact += 1;
                assert!(case["source"].is_array(), "{id}: exact requires source");
                assert!(
                    case["target_after"].is_array(),
                    "{id}: exact requires terminal"
                );
                assert_eq!(case["after_domains"].as_object().unwrap().len(), 2);
                assert_eq!(case["source_domains"].as_object().unwrap().len(), 2);
            }
            "Residual" => {
                residual += 1;
                assert!(case["gap"].is_string(), "{id}: typed residual gap required");
                assert!(
                    case["reason"].is_string(),
                    "{id}: residual owner rationale required"
                );
            }
            "Impossible" => {
                impossible += 1;
                assert!(
                    case["reason"].is_string(),
                    "{id}: infeasibility witness required"
                );
                assert_eq!(
                    case["pk"], true,
                    "{id}: enforced PK is the impossibility cause"
                );
            }
            other => panic!("{id}: unreviewed proof status {other}"),
        }

        for (bag, domain) in [
            (&case["target_before"], &case["before_domains"]),
            (&case["source"], &case["source_domains"]),
            (&case["target_after"], &case["after_domains"]),
        ] {
            if !bag.is_array() {
                assert!(
                    domain.is_null(),
                    "{id}: absent bag must not have guessed domain"
                );
                continue;
            }
            for column in ["id", "score"] {
                let values = bag.as_array().unwrap();
                let expected = &domain[column];
                assert_eq!(expected["type"], "INTEGER", "{id}/{column}");
                let nullable = values.iter().any(|row| row[column].is_null());
                assert_eq!(expected["nullable"], nullable, "{id}/{column}");
                let non_null = values
                    .iter()
                    .filter_map(|row| row[column].as_i64())
                    .collect::<Vec<_>>();
                assert_eq!(
                    expected["lower"].as_i64(),
                    non_null.iter().min().copied(),
                    "{id}/{column}"
                );
                assert_eq!(
                    expected["upper"].as_i64(),
                    non_null.iter().max().copied(),
                    "{id}/{column}"
                );
                assert_eq!(
                    expected["lower_inclusive"],
                    !non_null.is_empty(),
                    "{id}/{column}"
                );
                assert_eq!(
                    expected["upper_inclusive"],
                    !non_null.is_empty(),
                    "{id}/{column}"
                );
                let count = expected["frequencies"]
                    .as_object()
                    .unwrap()
                    .values()
                    .map(|value| value.as_u64().unwrap())
                    .sum::<u64>();
                assert_eq!(
                    count,
                    values.len() as u64,
                    "{id}/{column}: bag multiplicity"
                );
            }
        }

        for (rows, truth, not_true) in [
            (
                &case["target_before"],
                &case["on_true_target_ordinals"],
                &case["on_not_true_target_ordinals"],
            ),
            (
                &case["source"],
                &case["on_true_source_ordinals"],
                &case["on_not_true_source_ordinals"],
            ),
        ] {
            if !rows.is_array() || !case["source"].is_array() {
                assert!(
                    truth.is_null() && not_true.is_null(),
                    "{id}: missing source is unknown, not a negative witness"
                );
                continue;
            }
            let selected = truth.as_array().unwrap();
            let rejected = not_true.as_array().unwrap();
            let all = selected
                .iter()
                .chain(rejected)
                .map(|entry| entry.as_u64().unwrap())
                .collect::<BTreeSet<_>>();
            assert_eq!(
                all.len(),
                rows.as_array().unwrap().len(),
                "{id}: complete TRUE / NOT TRUE partition"
            );
            assert_eq!(
                selected.len() + rejected.len(),
                rows.as_array().unwrap().len(),
                "{id}: no identity duplicated"
            );
            assert_eq!(
                all,
                (0..rows.as_array().unwrap().len() as u64).collect::<BTreeSet<_>>()
            );
        }
    }
    assert_eq!((exact, residual, impossible), (9, 2, 1));
}
