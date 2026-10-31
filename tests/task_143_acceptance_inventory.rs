//! TASK-143: executable accounting for every reviewed frozen acceptance cell.
use serde_json::Value;
use std::collections::BTreeSet;

#[test]
fn acceptance_inventory_has_no_unowned_or_unclassified_mandatory_cells() {
    let freeze: Value =
        serde_json::from_str(include_str!("../docs/task-143-acceptance-freeze.json")).unwrap();
    let manifest: Value =
        serde_json::from_str(include_str!("../docs/coverage-manifest.json")).unwrap();
    assert_eq!(freeze["version"], 4);
    assert_eq!(freeze["owner"], "TASK-143");
    assert_eq!(
        manifest["task_143_cte_producer_acceptance"]["freeze_version"],
        4
    );
    assert_eq!(
        freeze["fixture_ids"],
        serde_json::json!(["78-C143-P", "78-C143-N"])
    );
    let expected: BTreeSet<&str> = [
        "78-C143-P-JOIN",
        "78-C143-N-NULL-DUP",
        "78-C143-P-COUNT",
        "78-C143-P-WRITE",
        "78-C143-N-WRITE",
        "78-C143-N-CHECK",
        "78-C143-N-OVERFLOW",
        "78-C143-N-OPAQUE",
        "78-C143-N-WIDTH",
    ]
    .into_iter()
    .collect();
    let cases = freeze["cases"].as_array().unwrap();
    let observed = cases
        .iter()
        .map(|case| case["id"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        observed, expected,
        "no implicit skipped mandatory or conditional fixture"
    );
    let expected_classifications = [
        ("mandatory_exact", 5),
        ("mandatory_impossible", 1),
        ("conditional_residual", 2),
        ("conditional_write_residual", 1),
    ];
    for (classification, expected_count) in expected_classifications {
        assert_eq!(
            cases
                .iter()
                .filter(|c| c["classification"] == classification)
                .count(),
            expected_count,
            "frozen class {classification}"
        );
    }
    for case in cases {
        assert_eq!(case["owner"], "TASK-143", "{}: owner", case["id"]);
        assert!(case["expected_physical"].as_str().is_some());
        assert!(case["sql"].as_str().is_some());
        assert!(case["initial_bags"].is_object());
        assert!(case["not_true_proof"].as_str().is_some());
        assert!(case["terminal_domains"].is_object());
        assert_eq!(case["adapter_cells"].as_array().unwrap().len(), 3);
        let adapters = case["adapter_cells"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["adapter"].as_str().unwrap())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            adapters,
            BTreeSet::from([
                "direct_sql",
                "dbt_compiled_manifest_catalog",
                "odcs_equivalent_schema"
            ])
        );
    }
    let dialects = freeze["dialects"].as_array().unwrap();
    assert_eq!(dialects.len(), 14);
    let families = freeze["parser_families"].as_array().unwrap();
    assert_eq!(families.len(), 5);
    let dialect_names = dialects
        .iter()
        .map(|d| d["name"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    for family in families {
        let cells = family["cells"].as_array().unwrap();
        assert_eq!(cells.len(), 14);
        let names = cells
            .iter()
            .map(|c| c["dialect"].as_str().unwrap())
            .collect::<BTreeSet<_>>();
        assert_eq!(names, dialect_names);
        for cell in cells {
            assert_eq!(cell["parser"], "parsed");
            assert_eq!(cell["owner"], "TASK-143");
        }
    }
    assert_eq!(
        manifest["task_143_cte_producer_acceptance"]["adapter_cells"],
        27
    );
    assert_eq!(
        manifest["task_143_cte_producer_acceptance"]["parser_cells"],
        70
    );
    assert_eq!(
        manifest["task_143_cte_producer_acceptance"]["child_signed_off"],
        false
    );
    assert_eq!(
        manifest["task_143_cte_producer_acceptance"]["parent_signed_off"],
        false
    );
}
