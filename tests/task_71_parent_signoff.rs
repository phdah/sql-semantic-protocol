//! Guards TASK-71 parent closure against silently reopening or weakening frozen evidence.
//! Semantic truth is independently exercised by task_116_parent_join_oracles.

use std::collections::BTreeSet;

use serde_json::{json, Value};

#[test]
fn parent_closure_preserves_every_original_cell_and_reviewed_boundary() {
    let original: Value =
        serde_json::from_str(include_str!("../docs/task-71-acceptance-freeze.json")).unwrap();
    let evidence: Value =
        serde_json::from_str(include_str!("../docs/task-116-acceptance-freeze.json")).unwrap();
    let manifest: Value =
        serde_json::from_str(include_str!("../docs/coverage-manifest.json")).unwrap();

    assert_eq!(
        original["signoff"]["original_acceptance_criteria"],
        json!([1, 2, 3, 4, 5, 6])
    );
    assert_eq!(original["signoff"]["release_approved"], false);
    assert_eq!(
        manifest["task_71_join_acceptance"]["all_original_criteria_verified"],
        true
    );
    assert_eq!(
        manifest["task_71_join_acceptance"]["release_approved"],
        false
    );
    assert_eq!(
        manifest["task_116_join_acceptance"]["release_approved"],
        false
    );
    assert_eq!(original["mandatory"].as_array().unwrap().len(), 15);
    assert_eq!(original["conditional"].as_array().unwrap().len(), 12);

    for kind in ["mandatory", "conditional"] {
        let originals = original[kind].as_array().unwrap();
        let witnessed = evidence[kind].as_array().unwrap();
        let mut ids = BTreeSet::new();
        assert_eq!(originals.len(), witnessed.len());
        for case in originals {
            let id = case["id"].as_str().unwrap();
            assert!(ids.insert(id), "duplicate original cell {id}");
            let proof = witnessed
                .iter()
                .find(|item| item["id"] == id)
                .unwrap_or_else(|| panic!("missing independent evidence for {id}"));
            assert_eq!(case["sql"], proof["original_sql"], "{id}: frozen SQL");
            assert_eq!(case["acceptance_criteria"], proof["acceptance_criteria"]);
            if kind == "mandatory" {
                assert_eq!(case["classification"], "exact_required");
                assert_eq!(
                    case["verification"],
                    "verified_independent_duckdb_full_bag_domain_identity_and_child_adapters"
                );
                assert_eq!(
                    case["expected"]["terminal_count"],
                    proof["expected_terminal_count"]
                );
                let expected_source_counts = case["expected"]["physical_source_counts"]
                    .as_object()
                    .expect("frozen physical source population counts");
                let physical_sources = proof["physical_sources"]
                    .as_object()
                    .expect("independently seeded source bags");
                assert_eq!(
                    physical_sources.len(),
                    expected_source_counts.len(),
                    "{id}: physical source relation set"
                );
                for (relation, expected_count) in expected_source_counts {
                    assert_eq!(
                        physical_sources[relation].as_array().map(Vec::len),
                        expected_count.as_u64().map(|n| n as usize),
                        "{id}: complete physical bag size for {relation}"
                    );
                }
                assert_eq!(case["expected"]["positive_truth"], "exact");
                assert_eq!(
                    case["expected"]["not_true_truth"],
                    "exact_or_impossible_by_sql_three_valued_logic"
                );
                assert_eq!(case["dialects"].as_array().unwrap().len(), 14);
                assert!(!proof["output_domains"].as_array().unwrap().is_empty());
                assert!(proof["identity_sql"].is_string());
                assert_eq!(proof["dialects"].as_array().unwrap().len(), 13);
            } else {
                assert_eq!(case["owner"], "TASK-116");
                assert_eq!(
                    case["verification"],
                    "reviewed_parser_and_portable_proof_with_native_vendor_runtime_uncertified"
                );
                assert_eq!(
                    case["release_disposition"],
                    "reviewed_portable_only_native_laws_not_certified"
                );
                assert_eq!(
                    proof["native_vendor_execution"],
                    "not_certified_by_portable_DuckDB_or_parser"
                );
                let classifications = case["parser_classifications"].as_array().unwrap();
                assert_eq!(classifications.len(), 14, "{id}: 13 dialects plus postgres alias");
                let mut parent_dialects = BTreeSet::new();
                for classification in classifications {
                    let name = classification["dialect"].as_str().expect("named dialect");
                    assert!(parent_dialects.insert(name), "{id}: duplicate dialect {name}");
                }
                let alias = classifications
                    .iter()
                    .find(|entry| entry["dialect"] == "postgres")
                    .expect("postgres is a required spelling alias");
                assert_eq!(
                    alias["status"],
                    proof["dialect_classifications"]["postgresql"]["parser"],
                    "{id}: postgres alias must match postgresql"
                );
                for dialect in evidence["dialect_identities"].as_array().unwrap() {
                    let name = dialect.as_str().unwrap();
                    let parser = &proof["dialect_classifications"][name]["parser"];
                    let observed = case["parser_classifications"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|entry| entry["dialect"] == name)
                        .unwrap_or_else(|| panic!("{id}: missing parent dialect {name}"));
                    assert_eq!(&observed["status"], parser, "{id}: {name}");
                }
            }
        }
    }
}

#[test]
fn original_join_acceptance_snapshot_cannot_be_changed_with_its_evidence() {
    // Pin the pre-signoff parent inventory independently of the child freeze.
    // A coordinated change to both editable files must not silently pass.
    let golden: Value = serde_json::from_str(r#"{
  "mandatory": [
    [
      "J71-I2-EQ",
      "SELECT l.k FROM l INNER JOIN r ON l.k = r.k",
      2,
      {
        "l": 2,
        "r": 2
      }
    ],
    [
      "J71-I2-DUP",
      "SELECT l.k FROM l INNER JOIN r ON l.k = r.k",
      6,
      {
        "l": 2,
        "r": 3
      }
    ],
    [
      "J71-I3-CHAIN",
      "SELECT l.k FROM l JOIN r ON l.k = r.k JOIN t ON r.k = t.k",
      4,
      {
        "l": 1,
        "r": 2,
        "t": 2
      }
    ],
    [
      "J71-I3-WHERE",
      "SELECT l.k FROM l JOIN r ON l.k = r.k JOIN t ON r.k = t.k WHERE l.a > 0",
      4,
      {
        "l": 1,
        "r": 2,
        "t": 2
      }
    ],
    [
      "J71-LEFT-EMPTY",
      "SELECT l.k FROM l LEFT JOIN r ON l.k = r.k",
      3,
      {
        "l": 3,
        "r": 0
      }
    ],
    [
      "J71-RIGHT-EMPTY",
      "SELECT r.k FROM l RIGHT JOIN r ON l.k = r.k",
      3,
      {
        "l": 0,
        "r": 3
      }
    ],
    [
      "J71-FULL-UNMATCHED",
      "SELECT l.k, r.k FROM l FULL JOIN r ON l.k = r.k",
      3,
      {
        "l": 3,
        "r": 0
      }
    ],
    [
      "J71-SEMI-ABSENCE",
      "SELECT l.k FROM l LEFT SEMI JOIN r ON l.k = r.k",
      0,
      {
        "l": 2,
        "r": 0
      }
    ],
    [
      "J71-ANTI-ABSENCE",
      "SELECT l.k FROM l LEFT ANTI JOIN r ON l.k = r.k",
      2,
      {
        "l": 2,
        "r": 0
      }
    ],
    [
      "J71-SELF",
      "SELECT a.k FROM l a JOIN l b ON a.k = b.k",
      4,
      {
        "l": 2
      }
    ],
    [
      "J71-COMPOSITE",
      "SELECT l.k FROM l JOIN r ON l.k = r.k AND l.a = r.a",
      2,
      {
        "l": 2,
        "r": 2
      }
    ],
    [
      "J71-NULL-REJECT",
      "SELECT l.k FROM l JOIN r ON l.k = r.k",
      0,
      {
        "l": 1,
        "r": 1
      }
    ],
    [
      "J71-OUTER-REVERSED",
      "SELECT l.a FROM l LEFT JOIN r ON r.k = l.k AND l.a > 0",
      3,
      {
        "l": 3,
        "r": 0
      }
    ],
    [
      "J71-MATCH-REJECTED",
      "SELECT l.a FROM l JOIN r ON l.k = r.k",
      0,
      {
        "l": 1,
        "r": 1
      }
    ],
    [
      "J71-FK-KEY",
      "SELECT l.k FROM l JOIN r ON l.k = r.k",
      2,
      {
        "l": 2,
        "r": 2
      }
    ]
  ],
  "conditional": [
    [
      "J71-CROSS",
      "SELECT l.k FROM l CROSS JOIN r",
      "parser_equivalent"
    ],
    [
      "J71-USING",
      "SELECT l.k FROM l JOIN r USING(k)",
      "parser_equivalent"
    ],
    [
      "J71-NATURAL",
      "SELECT l.k FROM l NATURAL JOIN r",
      "parser_equivalent"
    ],
    [
      "J71-OR-ON",
      "SELECT l.k FROM l JOIN r ON l.k=r.k OR l.a=r.a",
      "parser_equivalent"
    ],
    [
      "J71-NONEQUI",
      "SELECT l.k FROM l JOIN r ON l.k < r.k",
      "parser_equivalent"
    ],
    [
      "J71-NULLSAFE",
      "SELECT l.k FROM l JOIN r ON l.k IS NOT DISTINCT FROM r.k",
      "parser_equivalent"
    ],
    [
      "J71-ASOF",
      "SELECT l.k FROM l ASOF JOIN r ON l.k >= r.k",
      "parser_boundary"
    ],
    [
      "J71-RANGE-TIME",
      "SELECT l.k FROM l JOIN r ON l.k BETWEEN r.a AND r.k",
      "parser_equivalent"
    ],
    [
      "J71-LATERAL",
      "SELECT l.k FROM l CROSS JOIN LATERAL (SELECT r.k FROM r WHERE r.k=l.k) AS x",
      "parsed_lateral_residual"
    ],
    [
      "J71-APPLY",
      "SELECT l.k FROM l CROSS APPLY (SELECT r.k FROM r WHERE r.k=l.k) AS x",
      "parsed_cross_apply_residual"
    ],
    [
      "J71-ALIAS-REUSE",
      "SELECT a.k FROM l a JOIN l b ON a.k=b.k JOIN l c ON b.k=c.k",
      "parser_equivalent"
    ],
    [
      "J71-OUTER-WHERE",
      "SELECT l.k FROM l LEFT JOIN r ON l.k=r.k WHERE r.k IS NULL",
      "expected_parser_equivalent"
    ]
  ]
}"#).unwrap();
    let original: Value =
        serde_json::from_str(include_str!("../docs/task-71-acceptance-freeze.json")).unwrap();
    let current = json!({
        "mandatory": original["mandatory"].as_array().unwrap().iter().map(|case| {
            json!([
                case["id"],
                case["sql"],
                case["expected"]["terminal_count"],
                case["expected"]["physical_source_counts"]
            ])
        }).collect::<Vec<_>>(),
        "conditional": original["conditional"].as_array().unwrap().iter().map(|case| {
            let classifications = case["parser_classifications"].as_array().unwrap();
            assert_eq!(classifications.len(), 14, "13 parser paths plus postgres alias");
            let expected = &classifications[0]["status"];
            assert!(
                classifications.iter().all(|item| &item["status"] == expected),
                "original per-dialect status unexpectedly diverged"
            );
            json!([case["id"], case["sql"], expected])
        }).collect::<Vec<_>>()
    });
    assert_eq!(current, golden, "original TASK-71 acceptance scope changed");
}
