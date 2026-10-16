//! TASK-107: frozen expression variant, dialect and oracle acceptance inventory.
//!
//! Checks contract completeness, not semantic success. Semantic correctness is
//! established separately by public-API and independent DuckDB tests.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

const FREEZE: &str = include_str!("../docs/task-107-acceptance-freeze.json");
const COVERAGE: &str = include_str!("../docs/coverage-manifest.json");
const ORACLES: &str = include_str!("../docs/task-107-oracle-matrix.json");
const TASK: &str = include_str!(
    "../.backlog/tasks/task-107 - certify-expression-predicate-dialect-dbt-oracles.md"
);

fn parse(input: &str) -> Value {
    serde_json::from_str(input).expect("versioned JSON evidence must parse")
}

fn entries<'a>(value: &'a Value, name: &str) -> &'a [Value] {
    value[name]
        .as_array()
        .unwrap_or_else(|| panic!("{name}: required array"))
}

fn name<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key]
        .as_str()
        .unwrap_or_else(|| panic!("{key}: required nonempty string"))
}

#[test]
fn frozen_variant_dialect_inventory_matches_the_authoritative_task_66_scope() {
    let freeze = parse(FREEZE);
    let manifest = parse(COVERAGE);
    assert_eq!(freeze["schema_version"], 1);
    assert_eq!(freeze["task_id"], "TASK-107");
    assert_eq!(freeze["frozen_feature_variant_count"], 42);
    assert_eq!(freeze["frozen_dialect_cell_count"], 546);

    let expected_dialects: BTreeSet<_> = entries(&manifest, "dialects")
        .iter()
        .map(|dialect| name(dialect, "name"))
        .collect();
    let frozen_dialects: BTreeSet<_> = entries(&freeze, "frozen_dialects")
        .iter()
        .map(|dialect| dialect.as_str().expect("dialect name"))
        .collect();
    assert_eq!(expected_dialects.len(), 13);
    assert_eq!(frozen_dialects, expected_dialects);

    let expected_variants: BTreeSet<_> = entries(&manifest, "features")
        .iter()
        .filter(|feature| name(feature, "id").starts_with("predicates."))
        .flat_map(|feature| {
            entries(feature, "variants").iter().map(move |variant| {
                (
                    name(feature, "id").to_owned(),
                    variant.as_str().expect("variant string").to_owned(),
                )
            })
        })
        .collect();
    let frozen = entries(&freeze, "inventory_cells");
    assert_eq!(frozen.len(), 42);
    let mut ids = BTreeSet::new();
    let mut seen_variants = BTreeSet::new();
    let mut seen_dialects = BTreeSet::new();
    for cell in frozen {
        let id = name(cell, "id");
        assert!(ids.insert(id), "{id}: duplicate fixture ID");
        let pair = (
            name(cell, "feature_id").to_owned(),
            name(cell, "variant").to_owned(),
        );
        assert!(
            seen_variants.insert(pair),
            "{id}: duplicate feature variant"
        );
        assert!(
            !name(cell, "sql_shape").is_empty(),
            "{id}: missing frozen SQL shape"
        );
        assert!(
            !name(cell, "fixture_id").is_empty(),
            "{id}: missing fixture ID"
        );
        assert!(
            !name(cell, "exactness_floor").is_empty(),
            "{id}: missing exactness floor"
        );
        assert_eq!(entries(cell, "mandatory_oracle_fields").len(), 8, "{id}");

        match name(cell, "classification") {
            "exact_required" => {
                assert!(
                    cell.get("conditional_boundary").is_none(),
                    "{id}: mandatory case downgraded"
                );
            }
            "conditional" => {
                assert!(
                    !name(cell, "conditional_boundary").is_empty(),
                    "{id}: missing limitation"
                );
                assert!(
                    !name(cell, "defer_owner").is_empty(),
                    "{id}: no future owner"
                );
                assert_eq!(
                    name(cell, "release_disposition"),
                    "review_required_before_release",
                    "{id}: cannot preapprove deferral"
                );
            }
            classification => panic!("{id}: invalid proof classification {classification}"),
        }

        let cells = cell["dialect_cells"]
            .as_object()
            .expect("per-dialect cells");
        let keys: BTreeSet<_> = cells.keys().map(String::as_str).collect();
        assert_eq!(keys, expected_dialects, "{id}: missing dialect coverage");
        for (dialect, evidence) in cells {
            let fixture = name(evidence, "fixture_id");
            assert_eq!(fixture, format!("{id}-{dialect}"));
            assert!(!name(evidence, "semantic_expectation").is_empty());
            assert!(matches!(
                name(evidence, "parser"),
                "unverified" | "parsed" | "parse_rejected"
            ));
            assert!(matches!(
                name(evidence, "canonical"),
                "unverified" | "equivalent" | "assumption_recorded" | "not_applicable_parse"
            ));
            seen_dialects.insert(fixture);
        }
    }
    assert_eq!(
        seen_variants, expected_variants,
        "TASK-66 predicate feature inventory drift"
    );
    assert_eq!(
        seen_dialects.len(),
        546,
        "duplicate or missing dialect fixture IDs"
    );
}

#[test]
fn frozen_child_families_and_golden_cases_are_nonvacuous_and_closed_world() {
    let freeze = parse(FREEZE);
    let manifest = parse(COVERAGE);
    let expected: BTreeSet<_> = entries(&manifest["task_70_predicate_evidence"], "family_evidence")
        .iter()
        .map(|family| name(family, "family"))
        .collect();
    let actual: BTreeSet<_> = entries(&freeze, "frozen_family_ids")
        .iter()
        .map(|family| family.as_str().expect("family id"))
        .collect();
    assert_eq!(expected.len(), 14);
    assert_eq!(expected, actual);

    let oracles = parse(ORACLES);
    let mut named_evidence = BTreeSet::new();
    for fixture in entries(&oracles, "fixtures") {
        if let Some(family) = fixture["task_70_family"].as_str() {
            named_evidence.insert(family);
        }
    }
    assert_eq!(
        named_evidence, expected,
        "an exact TASK-70 family lost its owning test"
    );

    let mut golden_ids = BTreeSet::new();
    let goldens = entries(&freeze, "golden_cases");
    assert_eq!(goldens.len(), 9, "reviewed golden fixture set changed");
    for golden in goldens {
        let id = name(golden, "id");
        assert!(golden_ids.insert(id), "{id}: duplicate golden");
        assert_eq!(
            golden["source_closed_world"], true,
            "{id}: source bag must be complete"
        );
        assert!(!name(golden, "sql").is_empty());
        assert!(
            golden["expected_output_domain"].is_object(),
            "{id}: expected typed domain"
        );
        assert!(!name(golden, "positive_proof").is_empty());
        assert!(!name(golden, "not_true_proof").is_empty());
        assert_eq!(name(golden, "oracle"), "duckdb");

        let source: BTreeSet<_> = entries(golden, "source_rows")
            .iter()
            .map(|row| row[0].as_i64().expect("physical row ID"))
            .collect();
        let positive: BTreeSet<_> = entries(golden, "true_row_ids")
            .iter()
            .map(|row| row.as_i64().expect("positive row ID"))
            .collect();
        let rejected: BTreeSet<_> = entries(golden, "not_true_row_ids")
            .iter()
            .map(|row| row.as_i64().expect("rejected row ID"))
            .collect();
        assert_eq!(
            source.len(),
            entries(golden, "source_rows").len(),
            "{id}: duplicate source row ID"
        );
        assert!(
            positive.is_disjoint(&rejected),
            "{id}: TRUE/NOT TRUE overlap"
        );
        let partition: BTreeSet<_> = positive.union(&rejected).copied().collect();
        assert_eq!(
            partition, source,
            "{id}: incomplete TRUE/NOT TRUE partition"
        );
        assert!(matches!(
            name(golden, "status"),
            "contract_frozen_unverified" | "verified"
        ));
    }
    let mut compositions = BTreeMap::new();
    for case in entries(&freeze, "composition_cases") {
        assert!(
            compositions.insert(name(case, "id"), case).is_none(),
            "duplicate composition ID"
        );
        let file = name(case, "test_file");
        let test = name(case, "test_name");
        let source = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(file))
            .unwrap_or_else(|error| panic!("{}: missing test file {file}: {error}", case["id"]));
        assert!(
            source.contains(&format!("fn {test}(")),
            "{}: no executable regression {file}::{test}",
            case["id"]
        );
    }
    assert_eq!(compositions.len(), 9);
}

#[test]
fn unfinished_criteria_cannot_be_signed_off_by_a_green_partial_suite() {
    let freeze = parse(FREEZE);
    let status_done = TASK.lines().any(|line| line.trim() == "status: Done");
    let ac = TASK
        .split_once("<!-- AC:BEGIN -->")
        .expect("task acceptance markers")
        .1
        .split_once("<!-- AC:END -->")
        .expect("task acceptance end")
        .0;
    let criteria: Vec<_> = ac.lines().filter(|line| line.starts_with("- [")).collect();
    assert_eq!(criteria.len(), 5);
    for i in 1..=5 {
        let id = format!("AC{i}");
        assert!(!entries(&freeze["original_acceptance_criteria"][&id], "required").is_empty());
        assert!(!name(&freeze["original_acceptance_criteria"][&id], "pass").is_empty());
        assert!(
            criteria[i - 1].contains(&format!("#{i} ")),
            "missing original AC #{i}"
        );
    }

    if status_done
        || criteria
            .iter()
            .any(|criterion| criterion.starts_with("- [x]"))
    {
        assert!(
            status_done
                && criteria
                    .iter()
                    .all(|criterion| criterion.starts_with("- [x]"))
        );
        for cell in entries(&freeze, "inventory_cells") {
            for dialect in cell["dialect_cells"]
                .as_object()
                .expect("dialect object")
                .values()
            {
                assert_ne!(
                    name(dialect, "parser"),
                    "unverified",
                    "cannot close missing parser"
                );
                assert_ne!(
                    name(dialect, "canonical"),
                    "unverified",
                    "cannot close missing equivalence"
                );
            }
            if cell["classification"] == "conditional" {
                assert_ne!(
                    name(cell, "release_disposition"),
                    "review_required_before_release"
                );
            }
        }
        for golden in entries(&freeze, "golden_cases") {
            assert_eq!(name(golden, "status"), "verified");
        }
        for composition in entries(&freeze, "composition_cases") {
            assert_eq!(name(composition, "status"), "verified");
        }
        assert_eq!(
            freeze["final_ci_verified"], true,
            "CI needs verified final SHA evidence"
        );
    }
}
