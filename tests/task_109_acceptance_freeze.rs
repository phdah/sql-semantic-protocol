//! Frozen TASK-109 child acceptance inventory. These checks guard coverage scope;
//! independent physical, DuckDB and adapter tests provide execution evidence.

use std::collections::BTreeSet;

use serde_json::Value;

const FREEZE: &str = include_str!("../docs/task-109-acceptance-freeze.json");
const MANIFEST: &str = include_str!("../docs/coverage-manifest.json");
const SCHEMA: &str = include_str!("../schema/protocol.schema.json");
const TASK: &str = include_str!(
    "../.backlog/tasks/task-109 - prove-two-input-inner-join-bags-and-composite-keys.md"
);

#[test]
fn task_109_has_all_mandatory_frozen_variants_and_exact_source_pair_partitions() {
    let frozen: Value = serde_json::from_str(FREEZE).expect("reviewed inventory JSON");
    let variants = frozen["variants"]
        .as_array()
        .expect("finite variant matrix");
    let expected = [
        ("71-C109-P-EQ", 4usize, 5usize),
        ("71-C109-P-COMPOSITE", 1, 8),
        ("71-C109-N-NULL", 0, 9),
        ("71-C109-N-COMPOSITE", 0, 9),
        ("71-C109-P-DUP", 6, 0),
    ];
    assert_eq!(variants.len(), expected.len());
    for (id, positive, not_true) in expected {
        let item = variants
            .iter()
            .find(|v| v["id"] == id)
            .expect("named frozen variant");
        assert_eq!(item["classification"], "exact_required");
        assert_eq!(item["matched_count"], positive);
        assert_eq!(item["matched"].as_array().expect("pairs").len(), positive);
        assert_eq!(
            item["not_true_pairs"].as_array().expect("not true").len(),
            not_true
        );
        let physical = item["source_bags"]["l"]
            .as_array()
            .expect("left complete bag")
            .len()
            * item["source_bags"]["r"]
                .as_array()
                .expect("right complete bag")
                .len();
        assert_eq!(
            physical,
            positive + not_true,
            "{id}: no sampled or omitted pair"
        );
        assert!(
            item["value_domain"].is_object(),
            "{id}: mandatory typed domain"
        );
        assert!(item["adapters"].as_array().expect("adapters").len() == 3);
        assert!(item["source_instances"]["left"].is_string());
        assert!(item["source_instances"]["right"].is_string());
    }
    let names = frozen["dialect_identities"]
        .as_array()
        .expect("exposed dialects")
        .iter()
        .map(|name| name.as_str().expect("name"))
        .collect::<BTreeSet<_>>();
    assert_eq!(names.len(), 13);
    assert_eq!(frozen["alias_spelling"], "postgres");
    for ac in 1..=5 {
        assert!(!frozen["acceptance_map"][ac.to_string()]
            .as_array()
            .expect("every AC has mandatory fixture coverage")
            .is_empty());
    }
}

#[test]
fn task_109_matrix_is_linked_and_protocol_schema_models_full_key_tuple() {
    let manifest: Value = serde_json::from_str(MANIFEST).expect("canonical coverage manifest");
    assert_eq!(
        manifest["task_109_join_acceptance"]["acceptance_freeze"],
        "docs/task-109-acceptance-freeze.json"
    );
    assert_eq!(
        manifest["task_109_join_acceptance"]["release_approved"],
        false
    );
    let schema: Value = serde_json::from_str(SCHEMA).expect("active protocol schema");
    let options = schema["$defs"]["constructiveObligation"]["oneOf"]
        .as_array()
        .expect("constructive variants");
    let join = options
        .iter()
        .find(|option| option["properties"]["kind"]["const"] == "join_population")
        .expect("canonical join population contract");
    assert_eq!(join["properties"]["additional_keys"]["minItems"], 1);
    assert_eq!(join["properties"]["additional_keys"]["maxItems"], 1);
    assert!(TASK.contains("docs/task-109-acceptance-freeze.json"));
    assert!(TASK.contains("status: In Progress"));
}
