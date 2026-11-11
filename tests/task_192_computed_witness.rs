//! TASK-192 computed-predicate native and typed operator-witness gates.
//! These source-local certificates do not upgrade the portable analyzer's
//! full-query residual to Exact or attest native non-DuckDB vendor behavior.

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_inputs, dialect_from_name, BooleanTruthCase,
    BooleanWitnessDirection, ComposedSemantics, ConfiguredSqlInput, RelationCatalog, SqlInput,
    ValueDomain,
};

const SOURCE: &str = include_str!("../docs/task-177-predicate-oracles.json");
const FREEZE: &str = include_str!("../docs/task-192-acceptance-freeze.json");
const COMPUTED_IDS: [&str; 6] = [
    "88-C177-S027",
    "88-C177-S028",
    "88-C177-S029",
    "88-C177-S030",
    "88-C177-S031",
    "88-C177-S032",
];

fn sorted_native_ids(connection: &Connection, sql: &str) -> Vec<i64> {
    let mut statement = connection.prepare(sql).expect("original native SQL");
    let rows = statement
        .query_map([], |row| row.get::<_, i64>(0))
        .expect("complete source identities");
    let mut identities = rows.map(|row| row.expect("source row")).collect::<Vec<_>>();
    identities.sort();
    identities
}

fn oracle<'a>(source: &'a Value, id: &str) -> &'a Value {
    source["cases"]
        .as_array()
        .expect("original independent source cases")
        .iter()
        .find(|item| item["id"] == id)
        .expect("original computed predicate")
}

fn expected_ids(case: &Value, key: &str) -> Vec<i64> {
    let mut result = serde_json::from_value::<Vec<i64>>(case[key].clone())
        .expect("independently frozen occurrence identities");
    result.sort();
    result
}

#[test]
fn original_computed_predicates_have_complete_native_three_valued_partitions() {
    let source: Value = serde_json::from_str(SOURCE).expect("native frozen oracles");
    let connection = Connection::open_in_memory().expect("DuckDB");
    connection
        .execute_batch(source["source_setup"].as_str().expect("full native source"))
        .expect("source population");

    for id in COMPUTED_IDS {
        let case = oracle(&source, id);
        let original_sql = case["sql"].as_str().expect("unaltered original SQL");
        let predicate = case["predicate"].as_str().expect("unaltered WHERE");
        assert_eq!(
            sorted_native_ids(&connection, original_sql),
            expected_ids(case, "expected_selected_ids"),
            "{id}: full selected terminal identities"
        );
        for (truth, expected) in [
            ("IS TRUE", "expected_selected_ids"),
            ("IS FALSE", "expected_false_ids"),
            ("IS UNKNOWN", "expected_unknown_ids"),
        ] {
            assert_eq!(
                sorted_native_ids(
                    &connection,
                    &format!("SELECT id FROM orders WHERE ({predicate}) {truth}")
                ),
                expected_ids(case, expected),
                "{id}: full original {truth} partition"
            );
        }
        let mut rejected = expected_ids(case, "expected_false_ids");
        rejected.extend(expected_ids(case, "expected_unknown_ids"));
        rejected.sort();
        assert_eq!(
            sorted_native_ids(
                &connection,
                &format!("SELECT id FROM orders WHERE ({predicate}) IS NOT TRUE")
            ),
            rejected,
            "{id}: full original NOT TRUE partition"
        );
    }
}

#[test]
fn signed_integer_computed_predicates_have_typed_true_and_not_true_witnesses() {
    let freeze: Value = serde_json::from_str(FREEZE).expect("task v1 freeze");
    let native_parser = dialect_from_name("duckdb").expect("native parser");
    let ddl = SqlInput::inline("CREATE TABLE orders (id INTEGER, amount INTEGER)");
    let schema = analyze_inputs(&[ddl], "duckdb", native_parser.as_ref())
        .expect("physical signed integer source");
    let catalog = RelationCatalog::from_schemas(schema.source_schemas()).expect("source datatypes");

    for id in COMPUTED_IDS {
        let sql = freeze["cases"]
            .as_array()
            .expect("43 SQL cases")
            .iter()
            .find(|case| case["id"] == id)
            .expect("frozen original case")["sql"]
            .as_str()
            .expect("original SQL");
        for dialect in freeze["dialects"].as_array().expect("13 parser dialects") {
            let name = dialect.as_str().expect("dialect name");
            let parser = dialect_from_name(name).expect("parser");
            let input = SqlInput::inline(sql);
            let configured = [ConfiguredSqlInput::new(
                "task192-original-computed",
                &input,
                name,
                parser.as_ref(),
            )];
            let result = analyze_configured_inputs_with_catalog(&configured, &catalog)
                .unwrap_or_else(|error| panic!("{id}/{name}: {error}"));
            let layer = result.layers().last().expect("query layer");
            let ComposedSemantics::Resolved(composed) = layer.composed_semantics() else {
                panic!("{id}/{name}: missing typed output");
            };
            assert!(
                matches!(
                    composed.output().columns()[0].domain(),
                    ValueDomain::Unbounded | ValueDomain::Unknown(_)
                ),
                "{id}/{name}: filtering amount cannot constrain projected id"
            );
            let exact = composed.boolean_witnesses().iter().any(|entry| {
                matches!(
                    entry.witness().qualifying(),
                    BooleanWitnessDirection::Exact(BooleanTruthCase::True)
                ) && matches!(
                    entry.witness().rejected(),
                    BooleanWitnessDirection::Exact(BooleanTruthCase::NotTrue)
                )
            });
            let expected_exact = matches!(
                id,
                "88-C177-S028" | "88-C177-S029" | "88-C177-S030" | "88-C177-S031"
            );
            assert_eq!(
                exact, expected_exact,
                "{id}/{name}: signed-integer proof must not invent arithmetic overflow laws; witnesses={:?}",
                composed.boolean_witnesses()
            );
        }
    }
}
