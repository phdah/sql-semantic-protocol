//! Exhaustive parser-equivalence gate for the portable TASK-117 tuple laws.

use serde_json::Value;
use sql_semantic_protocol::{
    analyze_sql, dialect_from_name, ProtocolStatement, SetWitnessDirection,
};

#[test]
fn six_two_column_set_laws_are_identical_across_thirteen_parser_dialects() {
    let freeze: Value =
        serde_json::from_str(include_str!("../docs/task-117-acceptance-freeze.json"))
            .expect("task-117 frozen matrix");
    assert_eq!(
        freeze["dialect_identities"]
            .as_array()
            .expect("canonical dialect identities")
            .len(),
        13
    );
    for case in freeze["cases"].as_array().expect("12 variant cases") {
        let id = case["id"].as_str().expect("case id");
        let sql = case["sql"].as_str().expect("SQL");
        for (index, dialect) in freeze["dialect_identities"]
            .as_array()
            .expect("thirteen parser dialects")
            .iter()
            .enumerate()
        {
            let dialect_name = dialect.as_str().expect("dialect");
            assert_eq!(
                case["dialects"][index]["name"], *dialect,
                "{id}: frozen dialect identity"
            );
            assert_eq!(
                case["dialects"][index]["classification"],
                "parser_equivalent",
                "{id}/{dialect_name}: frozen parser classification"
            );
            let parser = dialect_from_name(dialect_name).expect("supported dialect");
            let protocol = analyze_sql(sql, dialect_name, parser.as_ref())
                .unwrap_or_else(|error| panic!("{id}/{dialect_name}: parser boundary: {error}"));
            let query = protocol
                .statements()
                .iter()
                .find_map(|statement| match statement {
                    ProtocolStatement::Query(query) => Some(query),
                    _ => None,
                })
                .expect("query");
            let operation = query.set_operation().expect("canonical set tree");
            assert_eq!(
                operation
                    .multiplicity_rule()
                    .expect("positional set rule")
                    .as_str(),
                case["kind"].as_str().expect("frozen rule"),
                "{id}/{dialect_name}: identical typed semantics"
            );
            assert_eq!(operation.branches().len(), 2, "{id}/{dialect_name}");
            assert_eq!(
                operation.branches()[0].output().columns().len(),
                2,
                "{id}/{dialect_name}: complete positional row shape"
            );
            assert!(
                matches!(
                    operation.witness_directions(),
                    (SetWitnessDirection::Exact(_), SetWitnessDirection::Exact(_))
                ),
                "{id}/{dialect_name}: positive and absence laws must be explicit"
            );
        }
    }
}
