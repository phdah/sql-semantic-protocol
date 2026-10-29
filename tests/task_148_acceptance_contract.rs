//! TASK-148: bounded acceptance contract must remain self-contained and executable.

fn freeze() -> serde_json::Value {
    serde_json::from_str(include_str!("../docs/task-148-acceptance-freeze.json")).unwrap()
}

#[test]
fn every_original_ac_has_a_frozen_case_and_no_mandatory_cell_is_unclassified() {
    let f = freeze();
    assert_eq!(f["version"], 2);
    assert_eq!(
        f["fixture_ids"],
        serde_json::json!(["80-C148-P", "80-C148-N"])
    );
    assert_eq!(
        f["original_parent_ac"],
        serde_json::json!([1, 2, 3, 4, 5, 6])
    );
    assert_eq!(f["task_ac"], serde_json::json!([1, 2, 3, 4, 5]));
    assert_eq!(
        f["freeze_history"],
        "docs/task-148-acceptance-freeze-v1.json"
    );
    let cases = f["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 6);
    assert_eq!(f["negative_contrasts"].as_array().unwrap().len(), 5);
    let mut ids = std::collections::BTreeSet::new();
    let mut cells = 0;
    let mut statement_cells = 0;
    for case in cases {
        let id = case["id"].as_str().unwrap();
        assert!(ids.insert(id), "duplicate fixture: {id}");
        let fixture = case["fixture_id"].as_str().unwrap();
        assert!(fixture == "80-C148-P" || fixture == "80-C148-N");
        assert!(!case["proof"].as_str().unwrap().is_empty());
        assert!(!case["comparison_assumptions"]
            .as_array()
            .unwrap()
            .is_empty());
        assert!(!case["adapter_evidence"].as_str().unwrap().is_empty());
        assert!(case["canonical_row_identity"]["target_before"].is_array());
        assert!(case["canonical_row_identity"]["target_after"].is_array());
        for (identity, population) in [
            ("source", "source"),
            ("target_before", "prestate"),
            ("target_after", "target_after"),
            ("report_after", "report_after"),
        ] {
            let identities = case["canonical_row_identity"][identity].as_array().unwrap();
            let expected_rows = case[population].as_array().unwrap();
            assert_eq!(identities.len(), expected_rows.len(), "{id}: {identity}");
            for (ordinal, (entry, expected)) in
                identities.iter().zip(expected_rows).enumerate()
            {
                let expected_id = format!("{identity}#{ordinal}");
                assert_eq!(entry["id"].as_str(), Some(expected_id.as_str()));
                assert_eq!(&entry["row"], expected, "{id}: {identity}#{ordinal}");
            }
        }
        let statements = case["statements"].as_array().unwrap();
        assert!(!statements.is_empty());
        for dialect in f["dialects"].as_array().unwrap() {
            let expected = case["parser_cells"][dialect.as_str().unwrap()]
                .as_array()
                .unwrap();
            assert_eq!(expected.len(), statements.len(), "{id}");
            for status in expected {
                assert!(matches!(
                    status.as_str().unwrap(),
                    "canonical" | "unsupported" | "parser_boundary"
                ));
                statement_cells += 1;
            }
            cells += 1;
        }
    }
    assert_eq!(f["dialects"].as_array().unwrap().len(), 13);
    assert_eq!(cells, 78);
    assert_eq!(statement_cells, 182);
    for number in 1..=6 {
        let mapping = &f["original_parent_ac_map"][number.to_string()];
        assert!(
            !mapping.as_array().unwrap().is_empty(),
            "missing parent AC #{number}"
        );
        for id in mapping.as_array().unwrap() {
            let known = cases.iter().any(|c| c["id"] == *id)
                || f["conditional_cases"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|c| c["id"] == *id);
            assert!(known, "unfrozen original parent AC #{number}: {id}");
        }
    }
    for case in f["negative_contrasts"].as_array().unwrap() {
        assert!(
            ids.insert(case["id"].as_str().unwrap()),
            "duplicate contrast"
        );
        assert!(case["oracle"]
            .as_str()
            .unwrap()
            .starts_with("tests/task_148_negative_matrix.rs:"));
    }
}

#[test]
fn conditional_cases_are_explicit_and_never_implicitly_release_approved() {
    let f = freeze();
    let conditional = f["conditional_cases"].as_array().unwrap();
    assert_eq!(conditional.len(), 3);
    for case in conditional {
        assert_eq!(case["owner"], "TASK-148");
        assert_eq!(case["classification"], "conditional_residual");
        assert!(!case["disposition"].as_str().unwrap().is_empty());
        assert!(!case["reviewed_boundary"].as_str().unwrap().is_empty());
        assert_eq!(case["release_approved"], false);
    }
    assert_eq!(f["signoff"]["parent_signed_off"], false);
}
