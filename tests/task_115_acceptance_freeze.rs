//! The TASK-115 planning matrix is immutable until its acceptance revision changes.
//! Mandatory status checks guard constructive signoff against relabeling unproved cells.
use serde_json::Value;
use std::collections::BTreeSet;

#[test]
fn frozen_mandatory_join_bags_have_complete_identity_and_truth_partitions() {
    let inventory: Value =
        serde_json::from_str(include_str!("../docs/task-115-acceptance-freeze.json"))
            .expect("frozen TASK-115 acceptance inventory");
    assert_eq!(inventory["owner"], "TASK-115");
    assert_eq!(inventory["version"], 1);
    assert_eq!(inventory["matrix_revision"], "2026-10-18-v3");
    assert_eq!(inventory["status"], "portable_constructive_bags_verified_with_explicit_native_boundaries");
    let cases = inventory["cases"].as_array().expect("cases");
    assert_eq!(cases.len(), 8);

    let dialects = inventory["dialects"].as_array().expect("dialects");
    assert_eq!(dialects.len(), 13);
    let mut ids = BTreeSet::new();
    let mut variants = BTreeSet::new();
    for case in cases {
        let id = case["id"].as_str().expect("case id");
        assert!(ids.insert(id), "repeated fixture case: {id}");
        variants.insert(case["variant"].as_str().expect("variant"));
        assert!(case["fixture"] == "71-C115-P" || case["fixture"] == "71-C115-N");
        let bags = &case["physical_source_bags"];
        let counts = ["l", "r", "t"].map(|source| {
            let rows = bags[source].as_array().expect("physical bag");
            let mut row_ids = BTreeSet::new();
            for row in rows {
                assert!(row_ids.insert(row["id"].as_str().expect("source identity")));
            }
            rows.len()
        });
        let positive = case["selected_physical_triplets"]
            .as_array()
            .expect("selected physical identities");
        let not_true = case["rejected_not_true_triplets"]
            .as_array()
            .expect("NOT TRUE physical identities");
        assert_eq!(
            positive.len() + not_true.len(),
            counts.iter().product::<usize>()
        );
        let all = positive.iter().chain(not_true).collect::<Vec<_>>();
        let triplets = all
            .iter()
            .map(|triple| {
                triple
                    .as_array()
                    .expect("triplet")
                    .iter()
                    .map(|identity| identity.as_str().expect("row id"))
                    .collect::<Vec<_>>()
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(
            triplets.len(),
            all.len(),
            "{id}: every source triple appears once"
        );

        // A complete identity partition requires exactly the Cartesian product,
        // with SQL NULL contributing to NOT TRUE rather than to equality.
        // Counting identities alone would accept a forged or missing triple.
        let positive_ids = positive
            .iter()
            .map(|triple| {
                triple
                    .as_array()
                    .expect("positive triple")
                    .iter()
                    .map(|part| part.as_str().expect("identity"))
                    .collect::<Vec<_>>()
            })
            .collect::<BTreeSet<_>>();
        let rejected_ids = not_true
            .iter()
            .map(|triple| {
                triple
                    .as_array()
                    .expect("NOT TRUE triple")
                    .iter()
                    .map(|part| part.as_str().expect("identity"))
                    .collect::<Vec<_>>()
            })
            .collect::<BTreeSet<_>>();
        let equality_is_true = |a: &Value, b: &Value| {
            a.as_i64()
                .zip(b.as_i64())
                .is_some_and(|(left, right)| left == right)
        };
        let composite = case["variant"] == "composite_nullable_keys";
        for l in bags["l"].as_array().expect("left bag") {
            for r in bags["r"].as_array().expect("middle bag") {
                for t in bags["t"].as_array().expect("right bag") {
                    let triple = vec![
                        l["id"].as_str().expect("left identity"),
                        r["id"].as_str().expect("middle identity"),
                        t["id"].as_str().expect("right identity"),
                    ];
                    let sql_true = l["a"].as_i64().is_some_and(|value| value > 0)
                        && equality_is_true(&l["k"], &r["k"])
                        && equality_is_true(&r["k"], &t["k"])
                        && (!composite
                            || (equality_is_true(&l["k2"], &r["k2"])
                                && equality_is_true(&r["k2"], &t["k2"])));
                    assert_eq!(
                        positive_ids.contains(&triple),
                        sql_true,
                        "{id}: frozen TRUE truth table for {triple:?}"
                    );
                    assert_eq!(
                        rejected_ids.contains(&triple),
                        !sql_true,
                        "{id}: frozen NOT TRUE truth table for {triple:?}"
                    );
                }
            }
        }

        // An attested column name without a stable physical instance is not
        // enough to correlate the canonical proof with a concrete source bag.
        for relation in ["l", "r", "t"] {
            let columns = case["source_column_types"][relation]
                .as_object()
                .expect("attested source columns");
            let instances = case["source_column_instance_ids"][relation]
                .as_object()
                .expect("source-column identity mapping");
            assert_eq!(instances.len(), columns.len(), "{id}: no omitted columns");
            for column in columns.keys() {
                assert_eq!(
                    instances[column].as_str(),
                    Some(format!("{relation}.{column}").as_str()),
                    "{id}: canonical physical column identity"
                );
                for row in bags[relation].as_array().expect("source bag") {
                    assert!(
                        row.get(column).is_some(),
                        "{id}: {relation}.{column} has explicit physical values including NULL"
                    );
                }
            }
        }

        let expected_adapters = inventory["cases"][0]["evidence_sources"]
            .as_array()
            .expect("three supported evidence sources")
            .iter()
            .map(|source| source.as_str().expect("source name"))
            .collect::<BTreeSet<_>>();
        let adapter_cells = case["adapter_matrix"]
            .as_array()
            .expect("adapter proof cells");
        assert_eq!(adapter_cells.len(), expected_adapters.len());
        assert_eq!(
            adapter_cells
                .iter()
                .map(|cell| cell["source"].as_str().expect("adapter source"))
                .collect::<BTreeSet<_>>(),
            expected_adapters,
            "{id}: adapter parity cannot silently omit a source"
        );
        for cell in adapter_cells {
            assert_eq!(cell["owner"], "TASK-115");
            assert_eq!(cell["source_schema_status"], "typed_columns_specified");
            assert_eq!(
                cell["canonical_population_status"],
                "verified_with_explicit_physical_attestation"
            );
            assert_eq!(cell["release_disposition"], "reviewed_conditional_native_not_certified");
        }

        let matrix = case["dialect_matrix"].as_array().expect("dialects");
        assert_eq!(matrix.len(), dialects.len());
        let covered = matrix
            .iter()
            .map(|cell| cell["dialect"].as_str().expect("dialect"))
            .collect::<BTreeSet<_>>();
        assert_eq!(
            covered,
            dialects
                .iter()
                .map(|d| d.as_str().expect("dialect"))
                .collect(),
            "{id}: every exposed dialect must appear exactly once"
        );
        assert_eq!(
            case["expected_terminal_bag"]
                .as_array()
                .expect("terminal rows")
                .len(),
            case["expected_terminal_row_count"].as_u64().expect("count") as usize
        );
        assert_eq!(case["acceptance_status"], "verified_exact_canonical_and_independent_duckdb_bags");
        for cell in matrix {
            assert_eq!(
                cell["canonical_status"],
                "verified_portable_canonical_source_bag",
                "{id}: no mandatory canonical dialect may remain unverified"
            );
        }
    }
    let conditional = inventory["conditional_boundaries"]
        .as_array()
        .expect("explicit conditional evidence boundaries");
    assert_eq!(conditional.len(), 3);
    let boundary_ids = conditional
        .iter()
        .map(|boundary| {
            assert_eq!(boundary["owner"], "TASK-115");
            assert_eq!(
                boundary["release_disposition"],
                "reviewed_conditional_native_not_certified"
            );
            assert_eq!(boundary["native_status"], "not_certified");
            assert!(
                boundary["missing_evidence"]
                    .as_str()
                    .is_some_and(|text| !text.is_empty()),
                "conditional boundary must name the missing evidence"
            );
            boundary["id"].as_str().expect("boundary identifier")
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(boundary_ids.len(), conditional.len());
    assert_eq!(variants.len(), 4);
}
