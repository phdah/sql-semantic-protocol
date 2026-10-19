//! TASK-118: thirteen-dialect canonical parser laws for nested portable sets.

use serde_json::Value;
use sql_semantic_protocol::{
    analyze_sql, dialect_from_name, ProtocolStatement, SetMultiplicityRule, SetOperand,
};

#[test]
fn frozen_nested_and_shared_source_trees_retain_three_canonical_leaves_in_all_dialects() {
    let frozen: Value =
        serde_json::from_str(include_str!("../docs/task-118-acceptance-freeze.json"))
            .expect("frozen TASK-118 matrix");
    let dialects = frozen["dialect_identities"]
        .as_array()
        .expect("canonical dialects");
    assert_eq!(dialects.len(), 13);
    for case in frozen["cases"].as_array().expect("cases") {
        let id = case["id"].as_str().expect("fixture");
        for dialect in dialects {
            let name = dialect.as_str().expect("dialect");
            let parser = dialect_from_name(name).expect("supported parser");
            let protocol = analyze_sql(
                case["sql"].as_str().expect("frozen SQL"),
                name,
                parser.as_ref(),
            )
            .unwrap_or_else(|error| panic!("{id}/{name}: parser boundary: {error}"));
            let query = protocol
                .statements()
                .iter()
                .find_map(|statement| match statement {
                    ProtocolStatement::Query(query) => Some(query),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("{id}/{name}: no query"));
            let set = query
                .set_operation()
                .unwrap_or_else(|| panic!("{id}/{name}: canonical set tree missing"));
            assert_eq!(set.branches().len(), 3, "{id}/{name}: leaf identities");
            assert_eq!(
                set.branches()
                    .iter()
                    .map(|branch| branch.output().columns().len())
                    .collect::<Vec<_>>(),
                vec![2, 2, 2],
                "{id}/{name}: positional two-column domains",
            );
            assert!(
                set.set_level_safe(),
                "{id}/{name}: unexpected set-level shaping"
            );
            assert_eq!(
                set.multiplicity_rule(),
                Some(SetMultiplicityRule::SaturatingDifference),
                "{id}/{name}: EXCEPT ALL canonical count law",
            );
            let SetOperand::Operation(left) = set.left() else {
                panic!("{id}/{name}: UNION ALL branch is not a nested operation");
            };
            assert_eq!(
                left.multiplicity_rule(),
                Some(SetMultiplicityRule::Sum),
                "{id}/{name}: nested UNION ALL canonical law",
            );
            assert!(matches!(set.right(), SetOperand::Query), "{id}/{name}");
        }
    }
}
