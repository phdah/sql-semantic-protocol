//! Independent TASK-72 parent acceptance guard.
//!
//! The physical behavior is exercised at the public seam by the child DuckDB,
//! SQL/dbt/ODCS and dialect tests. This guard prevents parent signoff when those
//! frozen obligations are dropped or relabeled as unsupported.

use std::collections::BTreeSet;

use serde_json::Value;

fn inventory(raw: &str) -> Value {
    serde_json::from_str(raw).expect("versioned acceptance inventory")
}

fn cases<'a>(inventory: &'a Value, name: &str) -> &'a [Value] {
    inventory[name].as_array().expect("owned case inventory")
}

fn nonempty_string(value: &Value) -> bool {
    value.as_str().is_some_and(|text| !text.trim().is_empty())
}

#[test]
fn task_72_parent_has_a_finite_original_ac_and_child_evidence_map() {
    let parent = inventory(include_str!("../docs/task-72-acceptance-freeze.json"));
    let child_117 = inventory(include_str!("../docs/task-117-acceptance-freeze.json"));
    let child_118 = inventory(include_str!("../docs/task-118-acceptance-freeze.json"));
    let child_119 = inventory(include_str!("../docs/task-119-acceptance-freeze.json"));
    let child_120 = inventory(include_str!("../docs/task-120-acceptance-freeze.json"));

    assert_eq!(parent["task"], "TASK-72");
    assert_eq!(parent["version"], 2);
    assert_eq!(cases(&child_117, "cases").len(), 12);
    assert_eq!(cases(&child_118, "cases").len(), 5);
    assert_eq!(cases(&child_119, "cases").len(), 11);

    let ac = cases(&parent, "original_acceptance_criteria");
    assert_eq!(ac.len(), 6);
    let ac_ids: BTreeSet<_> = ac
        .iter()
        .map(|criterion| criterion["id"].as_u64().expect("original AC number"))
        .collect();
    assert_eq!(ac_ids, BTreeSet::from([1, 2, 3, 4, 5, 6]));
    for criterion in ac {
        assert!(!criterion["mandatory"]
            .as_array()
            .expect("mandatory families")
            .is_empty());
        assert!(nonempty_string(&criterion["source"]));
    }

    let evidence_ids: BTreeSet<_> = [&child_117, &child_118, &child_119]
        .into_iter()
        .flat_map(|child| cases(child, "cases"))
        .map(|case| case["id"].as_str().expect("child fixture id"))
        .collect();
    for case in cases(&child_120, "mandatory_cases")
        .iter()
        .chain(cases(&child_120, "conditional_cases"))
    {
        let id = case["upstream_evidence"]
            .as_str()
            .expect("upstream child fixture");
        assert!(evidence_ids.contains(id), "unknown upstream fixture: {id}");
    }

    let fixtures: BTreeSet<_> = cases(&parent, "fixture_ids")
        .iter()
        .map(|id| id.as_str().expect("fixed fixture id"))
        .collect();
    assert_eq!(fixtures.len(), 8);
    for task in [117, 118, 119, 120] {
        for polarity in ["P", "N"] {
            let id = format!("72-C{task}-{polarity}");
            assert!(
                fixtures.contains(id.as_str()),
                "missing child fixture: {id}"
            );
        }
    }
}

#[test]
fn task_72_parent_mandatory_bags_keep_complete_exact_oracles() {
    let parent = inventory(include_str!("../docs/task-72-acceptance-freeze.json"));
    let child = inventory(include_str!("../docs/task-120-acceptance-freeze.json"));
    let mandatory = cases(&child, "mandatory_cases");
    assert_eq!(mandatory.len(), 21);
    assert_eq!(parent["portable_gate"]["mandatory_cases"], 21);
    assert_eq!(parent["portable_gate"]["mandatory_dialect_cells"], 273);
    assert_eq!(parent["portable_gate"]["mandatory_adapter_cells"], 63);

    let mut covered = BTreeSet::new();
    let mut operator_variants = BTreeSet::new();
    for case in mandatory {
        assert_eq!(case["classification"], "mandatory_exact");
        assert!(
            case["source_bags"].is_object(),
            "missing complete physical bag"
        );
        let declared: BTreeSet<_> = case["source_bags"]
            .as_object()
            .expect("complete source bags")
            .keys()
            .map(String::as_str)
            .collect();
        let sources = &case["canonical_source_instances"];
        let instances: BTreeSet<_> = if let Some(mapping) = sources.as_object() {
            mapping.keys().map(String::as_str).collect()
        } else {
            sources
                .as_array()
                .expect("canonical source identities")
                .iter()
                .map(|value| value.as_str().expect("source identity"))
                .collect()
        };
        assert_eq!(instances, declared, "physical source instance mismatch");
        assert!(case["expected_terminal_tuple_histogram"].is_array());
        assert!(case["typed_terminal_domains"].is_array());
        assert!(nonempty_string(&case["positive_proof"]));
        assert!(nonempty_string(&case["not_true_proof"]));
        assert!(nonempty_string(&case["oracle"]));

        let dialects = case["dialect_cells"].as_array().expect("dialect cells");
        assert_eq!(
            dialects.len(),
            13,
            "incomplete parser matrix: {}",
            case["id"]
        );
        let names: BTreeSet<_> = dialects
            .iter()
            .map(|cell| cell["name"].as_str().expect("dialect name"))
            .collect();
        assert_eq!(names.len(), 13, "duplicate parser classification");

        let adapters = case["adapter_cells"].as_object().expect("source adapters");
        assert_eq!(adapters.len(), 3);
        for adapter in ["sql", "dbt", "odcs"] {
            assert_eq!(
                adapters.get(adapter).and_then(Value::as_str),
                Some("mandatory")
            );
        }
        for ac in case["mapped_original_ac"]
            .as_array()
            .expect("original AC mapping")
        {
            covered.insert(ac.as_u64().expect("mapped AC"));
        }
        if case["variant"] == "six_operator_nullable_bag" {
            let id = case["id"].as_str().expect("fixture");
            if let Some(operator) = id.strip_prefix("72-C120-P-") {
                operator_variants.insert(operator);
            }
        }
    }
    assert_eq!(covered, BTreeSet::from([1, 2, 3, 4, 5, 6]));
    assert_eq!(
        operator_variants,
        BTreeSet::from([
            "sum",
            "union_distinct",
            "minimum",
            "intersect_distinct",
            "saturating_difference",
            "except_distinct",
        ]),
    );
}

#[test]
fn task_72_parent_conditionals_cannot_be_promoted_without_vendor_evidence() {
    let parent = inventory(include_str!("../docs/task-72-acceptance-freeze.json"));
    let child = inventory(include_str!("../docs/task-120-acceptance-freeze.json"));
    let conditional = cases(&child, "conditional_cases");
    assert_eq!(conditional.len(), 6);
    assert_eq!(parent["portable_gate"]["conditional_cases"], 6);
    let release_boundaries = cases(&parent, "conditional_boundaries");
    assert_eq!(release_boundaries.len(), 6);

    let ids: BTreeSet<_> = conditional
        .iter()
        .map(|case| case["id"].as_str().expect("conditional id"))
        .collect();
    let variants: BTreeSet<_> = release_boundaries
        .iter()
        .map(|boundary| boundary["variant"].as_str().expect("conditional variant"))
        .collect();
    assert_eq!(variants.len(), 6, "duplicate release disposition");

    for boundary in release_boundaries {
        let variant = boundary["variant"].as_str().expect("variant");
        assert!(
            ids.iter().any(|id| id.ends_with(variant)),
            "untracked conditional: {variant}"
        );
        assert_eq!(boundary["owner"], "TASK-120");
        assert!(nonempty_string(&boundary["reason"]));
        assert!(nonempty_string(&boundary["release_disposition"]));
    }
    for case in conditional {
        assert!(case["classification"]
            .as_str()
            .is_some_and(|c| c.starts_with("conditional")));
        assert_eq!(case["owner"], "TASK-120");
        assert!(nonempty_string(&case["boundary"]));
        assert!(nonempty_string(&case["release_disposition"]));
        assert_eq!(
            case["dialect_cells"]
                .as_array()
                .expect("dialect cells")
                .len(),
            13
        );
        assert!(case["expected_terminal_tuple_histogram"].is_null());
    }
}

#[test]
fn task_72_parent_each_original_ac_has_exact_and_conditional_case_coverage() {
    let parent = inventory(include_str!("../docs/task-72-acceptance-freeze.json"));
    let child = inventory(include_str!("../docs/task-120-acceptance-freeze.json"));

    for criterion in cases(&parent, "original_acceptance_criteria") {
        let ac = criterion["id"]
            .as_u64()
            .expect("original acceptance criterion");
        for family in cases(criterion, "mandatory") {
            let family = family.as_str().expect("mandatory semantic family");
            let covered = cases(&child, "mandatory_cases").iter().any(|case| {
                case["variant"] == family
                    && case["mapped_original_ac"]
                        .as_array()
                        .is_some_and(|ids| ids.iter().any(|id| id.as_u64() == Some(ac)))
            });
            assert!(covered, "AC #{ac} lacks an exact {family} case");
        }

        for variant in cases(criterion, "conditional") {
            let variant = variant.as_str().expect("conditional SQL variant");
            let child_covered = cases(&child, "conditional_cases").iter().any(|case| {
                case["id"].as_str().is_some_and(|id| id.ends_with(variant))
                    && case["mapped_original_ac"]
                        .as_array()
                        .is_some_and(|ids| ids.iter().any(|id| id.as_u64() == Some(ac)))
            });
            let parent_covered = cases(&parent, "parent_only_conditional")
                .iter()
                .any(|case| {
                    case["variant"] == variant
                        && case["mapped_original_ac"]
                            .as_array()
                            .is_some_and(|ids| ids.iter().any(|id| id.as_u64() == Some(ac)))
                        && case["classification"] == "conditional_residual"
                        && case["source_bags"].is_null()
                        && case["terminal_bag"].is_null()
                        && case["typed_terminal_domain"] == "unknown"
                        && nonempty_string(&case["owner"])
                        && nonempty_string(&case["release_disposition"])
                });
            assert!(
                child_covered || parent_covered,
                "AC #{ac} lacks a reviewed {variant} boundary"
            );
        }
    }
}
